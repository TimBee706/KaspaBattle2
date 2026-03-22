use super::{DepositCheckResult, EpisodeTrait, PlayerRole};
use crate::models::MatchStatus;
use async_trait::async_trait;
use battle_kaspa::escrow::EscrowService;
use battle_kaspa::rpc::KaspaRpc;
use battle_kaspa::watcher::BlockchainWatcher;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use uuid::Uuid;

/// Minimum number of DAA-score confirmations before a deposit is considered final.
/// Kaspa's ~1-block-per-second rate means 10 confirmations ≈ 10 seconds.
const MIN_CONFIRMATIONS: u64 = 10;

/// Encapsulates one match's lifecycle as a kdapp-style Episode.
///
/// State transitions driven by this episode:
///   Open → AwaitingFunding (on join)
///   AwaitingFunding → Funded (both deposits on-chain confirmed with MIN_CONFIRMATIONS)
///   Funded → Locked (escrow locked, FACEIT match started)
///   Resolving → Resolved (oracle result received)
///   Resolved → PaidOut (PayoutService executed)
pub struct MatchEpisode {
    pub match_id: Uuid,
    pub db_pool: PgPool,
    pub escrow_service: Option<Arc<EscrowService>>,
    pub tx: Sender<String>,
    /// BlockchainWatcher for per-player UTXO attribution.
    pub blockchain_watcher: Option<Arc<BlockchainWatcher>>,
    /// KaspaRpc for DAA score queries.
    pub kaspa_rpc: Option<Arc<dyn KaspaRpc>>,
}

#[async_trait]
impl EpisodeTrait for MatchEpisode {
    /// Context: (pool, escrow_service, ws_tx, blockchain_watcher, kaspa_rpc)
    type Context = (
        PgPool,
        Option<Arc<EscrowService>>,
        Sender<String>,
        Option<Arc<BlockchainWatcher>>,
        Option<Arc<dyn KaspaRpc>>,
    );
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn initialize(ctx: &Self::Context, match_id: Uuid) -> Result<Self, Self::Error> {
        Ok(Self {
            match_id,
            db_pool: ctx.0.clone(),
            escrow_service: ctx.1.clone(),
            tx: ctx.2.clone(),
            blockchain_watcher: ctx.3.clone(),
            kaspa_rpc: ctx.4.clone(),
        })
    }

    async fn execute(&mut self) -> Result<(), Self::Error> {
        let mut db_tx = self.db_pool.begin().await?;

        let row = sqlx::query(
            "SELECT m.status, m.creator_user_id, m.opponent_user_id, \
             m.escrow_address, m.stake_kas, m.created_at, \
             m.player_a_deposit_tx_hash, m.player_b_deposit_tx_hash, \
             u_a.kaspa_address AS player_a_addr, \
             u_b.kaspa_address AS player_b_addr \
             FROM matches m \
             JOIN users u_a ON m.creator_user_id = u_a.id \
             LEFT JOIN users u_b ON m.opponent_user_id = u_b.id \
             WHERE m.id = $1 FOR UPDATE OF m",
        )
        .bind(self.match_id)
        .fetch_one(&mut *db_tx)
        .await?;

        let status: MatchStatus = row.try_get("status")?;

        match status {
            // ─── Open | AwaitingFunding: check on-chain deposits per player ─────
            // OPEN = match created but opponent not yet joined. Still valid to
            // detect and record UTXOs so we're ready when the opponent joins.
            MatchStatus::Open | MatchStatus::AwaitingFunding => {
                let created_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("created_at")?;

                // Timeout: cancel match if waiting >60 min (based on created_at)
                // NOTE: ideally we'd use an `awaiting_funding_since` field,
                // but created_at is a reasonable approximation for now.
                if let Some(created) = created_at {
                    let age = chrono::Utc::now() - created;
                    if age > chrono::Duration::minutes(60) {
                        tracing::warn!(
                            match_id = %self.match_id,
                            age_min = age.num_minutes(),
                            "AWAITING_FUNDING timeout reached (>60 min) — cancelling match"
                        );
                        eprintln!(
                            "⏰ Match {} cancelled (AWAITING_FUNDING timeout: {} min)",
                            self.match_id, age.num_minutes()
                        );
                        sqlx::query("UPDATE matches SET status = 'CANCELLED' WHERE id = $1")
                            .bind(self.match_id)
                            .execute(&mut *db_tx)
                            .await?;
                        db_tx.commit().await?;
                        let _ = self.poll().await;
                        return Ok(());
                    } else if age > chrono::Duration::minutes(50) {
                        tracing::warn!(
                            match_id = %self.match_id,
                            age_min = age.num_minutes(),
                            "⚠️ AWAITING_FUNDING will timeout in {} min",
                            60 - age.num_minutes()
                        );
                    }
                }

                let addr: String = row.try_get("escrow_address").unwrap_or_default();
                if addr.is_empty() {
                    tracing::warn!(match_id = %self.match_id, "No escrow address set — skipping deposit check");
                    db_tx.commit().await?;
                    return Ok(());
                }

                let wager: i64 = row.try_get("stake_kas")?;
                let player_a_addr: Option<String> = row.try_get("player_a_addr").ok();
                let player_b_addr: Option<String> = row.try_get("player_b_addr").ok();
                let creator_id: Uuid = row.try_get("creator_user_id")?;
                let opponent_id: Option<Uuid> = row.try_get("opponent_user_id")?;

                // TX hashes recorded by submit_deposit API — used for deterministic attribution
                let player_a_tx_hash: Option<String> = row.try_get("player_a_deposit_tx_hash").ok().flatten();
                let player_b_tx_hash: Option<String> = row.try_get("player_b_deposit_tx_hash").ok().flatten();

                // Fetch current DAA score for confirmation calculation
                let current_daa = if let Some(ref rpc) = self.kaspa_rpc {
                    match rpc.get_current_daa_score().await {
                        Ok(daa) if daa == 0 => {
                            tracing::warn!(
                                match_id = %self.match_id,
                                "⚠️ DAA score is 0 — node may not be synced yet"
                            );
                            0
                        }
                        Ok(daa) => daa,
                        Err(e) => {
                            tracing::error!(
                                match_id = %self.match_id,
                                error = %e,
                                "❌ get_current_daa_score failed — RPC connection may be stale"
                            );
                            0
                        }
                    }
                } else {
                    tracing::warn!(
                        match_id = %self.match_id,
                        "⚠️ kaspa_rpc is None — cannot query DAA score"
                    );
                    0
                };

                if let Some(ref watcher) = self.blockchain_watcher {
                    match watcher.check_escrow_balance(&addr).await {
                        Ok(escrow_status) => {
                            tracing::info!(
                                match_id = %self.match_id,
                                escrow_address = %addr,
                                utxo_count = escrow_status.utxos.len(),
                                balance_sompi = escrow_status.total_balance,
                                current_daa,
                                "Escrow balance checked"
                            );
                            eprintln!(
                                "🔍 Match {}: {} UTXOs, balance={} sompi, current_daa={}",
                                self.match_id,
                                escrow_status.utxos.len(),
                                escrow_status.total_balance,
                                current_daa
                            );

                            // ── v0.6: Aggregate UTXOs per player role ────────────────
                            //
                            // Design: instead of one DB row per UTXO, we maintain ONE row
                            // per (match_id, player_role) with the total confirmed amount.
                            // This eliminates all duplicate-key errors from multi-UTXO deposits.
                            //
                            // Attribution priority:
                            //  1. If UTXO tx_id matches player_X_deposit_tx_hash → that role
                            //  2. Fallback ordered heuristic: accumulate to wager → A, rest → B
                            //     (separate accumulator so tx_hash matches don't corrupt it)

                            let wager_u64 = wager as u64;

                            // Sort by block_daa_score ascending (earliest first)
                            let mut sorted_utxos = escrow_status.utxos.clone();
                            sorted_utxos.sort_by_key(|u| u.block_daa_score);

                            // Aggregated totals per role
                            let mut a_total_sompi: u64 = 0;
                            let mut b_total_sompi: u64 = 0;
                            // Track the minimum confirmations across all UTXOs per role
                            // (deposit is only "confirmed" when ALL its UTXOs meet MIN_CONFIRMATIONS)
                            let mut a_min_confs: u64 = u64::MAX;
                            let mut b_min_confs: u64 = u64::MAX;
                            // First tx_id seen for each role (for reference / logging)
                            let mut a_first_tx = String::new();
                            let mut b_first_tx = String::new();
                            // Separate heuristic accumulator (doesn't mix with deterministic)
                            let mut heuristic_accum: u64 = 0;

                            for utxo in &sorted_utxos {
                                if utxo.is_coinbase {
                                    continue;
                                }

                                let confirmations = if current_daa > 0 {
                                    current_daa.saturating_sub(utxo.block_daa_score)
                                } else if utxo.block_daa_score > 0 {
                                    // IBD: node reports DAA=0 but UTXO is mined → treat as confirmed
                                    MIN_CONFIRMATIONS
                                } else {
                                    0
                                };

                                // Attribution: deterministic first, then heuristic fallback
                                let role = if player_a_tx_hash.as_deref() == Some(&utxo.tx_id) {
                                    "A" // matched by recorded tx hash
                                } else if player_b_tx_hash.as_deref() == Some(&utxo.tx_id) {
                                    "B"
                                } else if heuristic_accum < wager_u64 {
                                    heuristic_accum += utxo.amount;
                                    "A"
                                } else {
                                    "B"
                                };

                                eprintln!(
                                    "  🔬 UTXO: tx={} role={} amount={} confs={}/{}",
                                    &utxo.tx_id[..12.min(utxo.tx_id.len())], role,
                                    utxo.amount, confirmations, MIN_CONFIRMATIONS
                                );

                                match role {
                                    "A" => {
                                        a_total_sompi += utxo.amount;
                                        a_min_confs = a_min_confs.min(confirmations);
                                        if a_first_tx.is_empty() { a_first_tx = utxo.tx_id.clone(); }
                                    }
                                    "B" => {
                                        b_total_sompi += utxo.amount;
                                        b_min_confs = b_min_confs.min(confirmations);
                                        if b_first_tx.is_empty() { b_first_tx = utxo.tx_id.clone(); }
                                    }
                                    _ => {}
                                }
                            }

                            // Normalize: if no UTXOs seen for a role, set confs to 0
                            if a_total_sompi == 0 { a_min_confs = 0; }
                            if b_total_sompi == 0 { b_min_confs = 0; }

                            let a_confirmed = a_total_sompi >= wager_u64 && a_min_confs >= MIN_CONFIRMATIONS;
                            let b_confirmed = b_total_sompi >= wager_u64 && b_min_confs >= MIN_CONFIRMATIONS;

                            eprintln!(
                                "📊 Match {}: A={}/{} sompi ({} confs, {}), B={}/{} sompi ({} confs, {})",
                                self.match_id,
                                a_total_sompi, wager_u64, a_min_confs,
                                if a_confirmed { "✅" } else { "⏳" },
                                b_total_sompi, wager_u64, b_min_confs,
                                if b_confirmed { "✅" } else { "⏳" },
                            );

                            // ── Upsert payments: one row per role, ALL via db_tx ──────
                            //
                            // ON CONFLICT (match_id, player_role) → idempotent update.
                            // No more duplicate-key errors, no mixed pool/transaction writes.
                            if a_total_sompi > 0 && !a_first_tx.is_empty() {
                                sqlx::query(
                                    "INSERT INTO payments \
                                     (match_id, player_id, player_role, tx_id, amount_sompi, \
                                      block_daa_score, confirmations) \
                                     VALUES ($1, $2, 'A', $3, $4, $5, $6) \
                                     ON CONFLICT (match_id, player_role) DO UPDATE \
                                     SET amount_sompi = EXCLUDED.amount_sompi, \
                                         confirmations = EXCLUDED.confirmations, \
                                         tx_id = EXCLUDED.tx_id",
                                )
                                .bind(self.match_id)
                                .bind(creator_id)
                                .bind(&a_first_tx)
                                .bind(a_total_sompi as i64)
                                .bind(0i64) // block_daa_score: not used for aggregate
                                .bind(a_min_confs as i32)
                                .execute(&mut *db_tx)
                                .await?;

                                tracing::info!(
                                    match_id = %self.match_id,
                                    tx_id = %a_first_tx,
                                    amount_sompi = a_total_sompi,
                                    confirmations = a_min_confs,
                                    confirmed = a_confirmed,
                                    "💳 Player A payments upserted"
                                );
                            }

                            if b_total_sompi > 0 && !b_first_tx.is_empty() {
                                if opponent_id.is_none() {
                                    tracing::warn!(
                                        match_id = %self.match_id,
                                        "⚠️ Player B UTXO detected but opponent_id is NULL — recording without player_id"
                                    );
                                }
                                sqlx::query(
                                    "INSERT INTO payments \
                                     (match_id, player_id, player_role, tx_id, amount_sompi, \
                                      block_daa_score, confirmations) \
                                     VALUES ($1, $2, 'B', $3, $4, $5, $6) \
                                     ON CONFLICT (match_id, player_role) DO UPDATE \
                                     SET amount_sompi = EXCLUDED.amount_sompi, \
                                         confirmations = EXCLUDED.confirmations, \
                                         tx_id = EXCLUDED.tx_id",
                                )
                                .bind(self.match_id)
                                .bind(opponent_id)
                                .bind(&b_first_tx)
                                .bind(b_total_sompi as i64)
                                .bind(0i64)
                                .bind(b_min_confs as i32)
                                .execute(&mut *db_tx)
                                .await?;

                                tracing::info!(
                                    match_id = %self.match_id,
                                    tx_id = %b_first_tx,
                                    amount_sompi = b_total_sompi,
                                    confirmations = b_min_confs,
                                    confirmed = b_confirmed,
                                    "💳 Player B payments upserted"
                                );
                            }

                            // ── Match status update — all in db_tx ───────────────────
                            if a_confirmed && b_confirmed {
                                // Both deposits confirmed → FUNDED
                                sqlx::query(
                                    "UPDATE matches SET \
                                     status = 'FUNDED', \
                                     player_a_deposit_confirmed = true, \
                                     player_b_deposit_confirmed = true, \
                                     player_a_deposit_amount_sompi = $2, \
                                     player_b_deposit_amount_sompi = $3 \
                                     WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .bind(a_total_sompi as i64)
                                .bind(b_total_sompi as i64)
                                .execute(&mut *db_tx)
                                .await?;

                                eprintln!(
                                    "💰 Match {} → FUNDED (A={} confs, B={} confs)",
                                    self.match_id, a_min_confs, b_min_confs
                                );
                                tracing::info!(
                                    match_id = %self.match_id,
                                    "💰 AWAITING_FUNDING → FUNDED (both deposits confirmed)"
                                );
                            } else {
                                // Update partial deposit state (for frontend display)
                                sqlx::query(
                                    "UPDATE matches SET \
                                     player_a_deposit_confirmed = $2, \
                                     player_b_deposit_confirmed = $3, \
                                     player_a_deposit_amount_sompi = $4, \
                                     player_b_deposit_amount_sompi = $5 \
                                     WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .bind(a_confirmed)
                                .bind(b_confirmed)
                                .bind(a_total_sompi as i64)
                                .bind(b_total_sompi as i64)
                                .execute(&mut *db_tx)
                                .await?;

                                tracing::info!(
                                    match_id = %self.match_id,
                                    player_a_sompi = a_total_sompi,
                                    player_b_sompi = b_total_sompi,
                                    a_confs = a_min_confs,
                                    b_confs = b_min_confs,
                                    wager = wager_u64,
                                    "⏳ Deposits still pending"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                match_id = %self.match_id,
                                escrow_address = %addr,
                                error = %e,
                                "⚠️ Deposit check failed — will retry next cycle"
                            );
                        }
                    }
                } else {
                    // Fallback: use EscrowService simple balance check when no watcher
                    if let Some(ref svc) = self.escrow_service {
                        match svc.check_deposits(&addr, wager as u64).await {
                            Ok(deposit_status) if deposit_status.status
                                == battle_kaspa::escrow::DepositState::Complete =>
                            {
                                sqlx::query(
                                    "UPDATE matches SET status = 'FUNDED', \
                                     player_a_deposit_confirmed = true, \
                                     player_b_deposit_confirmed = true \
                                     WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .execute(&mut *db_tx)
                                .await?;

                                tracing::info!(
                                    match_id = %self.match_id,
                                    "💰 AWAITING_FUNDING → FUNDED (fallback balance check)"
                                );
                            }
                            Ok(_) => {
                                tracing::info!(match_id = %self.match_id, "⏳ Deposits still pending (fallback)");
                            }
                            Err(e) => {
                                tracing::error!(match_id = %self.match_id, error = %e, "⚠️ Fallback deposit check failed");
                            }
                        }
                    }
                }
            }

            // ─── Funded: lock escrow, start FACEIT match ────────────────────────
            MatchStatus::Funded => {
                let creator_id: Uuid = row.try_get("creator_user_id")?;
                let opponent_id: Option<Uuid> = row.try_get("opponent_user_id")?;

                if let Some(opp_id) = opponent_id {
                    let external_id = crate::services::faceit::create_faceit_match(
                        &creator_id.to_string(),
                        &opp_id.to_string(),
                    )
                    .await
                    .unwrap_or_else(|_| format!("local-{}", self.match_id));

                    sqlx::query(
                        "UPDATE matches SET status = 'LOCKED', \
                         external_match_id = $1 WHERE id = $2",
                    )
                    .bind(&external_id)
                    .bind(self.match_id)
                    .execute(&mut *db_tx)
                    .await?;

                    if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>(
                        "SELECT * FROM matches WHERE id = $1",
                    )
                    .bind(self.match_id)
                    .fetch_one(&self.db_pool)
                    .await
                    {
                        let mut m = updated;
                        m.calculate_wager();
                        let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
                    }

                    tracing::info!(
                        match_id = %self.match_id,
                        faceit_id = %external_id,
                        "🔒 FUNDED → LOCKED"
                    );
                }
            }

            _ => {
                // Terminal or not-yet-active states: no-op
            }
        }

        db_tx.commit().await?;

        // ── Post-commit: broadcast final committed state via WebSocket ──
        // Reading AFTER commit ensures clients see the real DB state.
        if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>(
            "SELECT * FROM matches WHERE id = $1",
        )
        .bind(self.match_id)
        .fetch_one(&self.db_pool)
        .await
        {
            let mut m = updated;
            m.calculate_wager();
            let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
        }

        Ok(())

    }

    async fn rollback(&mut self) -> Result<(), Self::Error> {
        sqlx::query(
            "UPDATE matches SET status = 'CANCELLED' WHERE id = $1 \
             AND status IN ('OPEN', 'AWAITING_FUNDING', 'FUNDED')",
        )
        .bind(self.match_id)
        .execute(&self.db_pool)
        .await?;

        if let Ok(updated) =
            sqlx::query_as::<_, crate::models::Match>("SELECT * FROM matches WHERE id = $1")
                .bind(self.match_id)
                .fetch_one(&self.db_pool)
                .await
        {
            let mut m = updated;
            m.calculate_wager();
            let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
        }

        tracing::info!(match_id = %self.match_id, "❌ Episode rolled back → CANCELLED");
        Ok(())
    }

    async fn poll(&mut self) -> Result<bool, Self::Error> {
        let row = sqlx::query("SELECT status FROM matches WHERE id = $1")
            .bind(self.match_id)
            .fetch_one(&self.db_pool)
            .await?;
        let status: MatchStatus = row.try_get("status")?;
        Ok(matches!(
            status,
            MatchStatus::Resolved | MatchStatus::PaidOut | MatchStatus::Cancelled
        ))
    }

    // ── v0.2: record_deposit ──────────────────────────────────────────────

    async fn record_deposit(
        &mut self,
        player: PlayerRole,
        tx_hash: &str,
    ) -> Result<(), Self::Error> {
        let col = match player {
            PlayerRole::A => "player_a_deposit_tx_hash",
            PlayerRole::B => "player_b_deposit_tx_hash",
        };
        sqlx::query(&format!("UPDATE matches SET {} = $1 WHERE id = $2", col))
            .bind(tx_hash)
            .bind(self.match_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    // ── v0.2: confirm_deposits ────────────────────────────────────────────

    async fn confirm_deposits(&mut self) -> Result<DepositCheckResult, Self::Error> {
        let row =
            sqlx::query("SELECT escrow_address, stake_kas FROM matches WHERE id = $1")
                .bind(self.match_id)
                .fetch_one(&self.db_pool)
                .await?;

        let addr: String = row.try_get("escrow_address").unwrap_or_default();
        let wager: i64 = row.try_get("stake_kas")?;

        if let Some(ref svc) = self.escrow_service {
            let status = svc.check_deposits(&addr, wager as u64).await?;
            match status.status {
                battle_kaspa::escrow::DepositState::Complete => {
                    sqlx::query(
                        "UPDATE matches SET status = 'FUNDED', \
                         player_a_deposit_confirmed = true, \
                         player_b_deposit_confirmed = true \
                         WHERE id = $1",
                    )
                    .bind(self.match_id)
                    .execute(&self.db_pool)
                    .await?;
                    Ok(DepositCheckResult::Complete)
                }
                battle_kaspa::escrow::DepositState::Partial => Ok(DepositCheckResult::Partial {
                    balance_sompi: status.deposited_sompi,
                }),
                battle_kaspa::escrow::DepositState::None => Ok(DepositCheckResult::None),
            }
        } else {
            Ok(DepositCheckResult::None)
        }
    }

    // ── v0.2: submit_faceid ───────────────────────────────────────────────

    async fn submit_faceid(&mut self, player: PlayerRole, hash: &str) -> Result<(), Self::Error> {
        let col = match player {
            PlayerRole::A => "player_a_faceid_hash",
            PlayerRole::B => "player_b_faceid_hash",
        };
        sqlx::query(&format!("UPDATE matches SET {} = $1 WHERE id = $2", col))
            .bind(hash)
            .bind(self.match_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }
}

// NOTE: The old `determine_player_role()` function was removed because it tried
// to match player *addresses* against UTXO *output script_public_key* hex.
// Since all UTXOs at the escrow address share the escrow's own P2PK script as
// their output script, this comparison always returned None.
//
// Player attribution is now handled by the ordered-heuristic in `execute()`:
// UTXOs sorted by block_daa_score — first group summing to ≥ wager = Player A,
// remaining = Player B.
//
// For production, implement sender attribution via `get_transaction` RPC to
// resolve actual input addresses.
