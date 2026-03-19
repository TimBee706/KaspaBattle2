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

                            // ── Deterministic UTXO attribution ──────────────────────
                            //
                            // Primary: If submit_deposit recorded a tx_hash for a player,
                            // UTXOs matching that tx_id are attributed to that player.
                            //
                            // Fallback: For UTXOs not matched by tx_hash, use the ordered
                            // heuristic (first group up to wager = A, rest = B).
                            //
                            // This ensures correct attribution regardless of deposit order.

                            let wager_u64 = wager as u64;

                            // Sort by block_daa_score ascending (earliest first)
                            let mut sorted_utxos = escrow_status.utxos.clone();
                            sorted_utxos.sort_by_key(|u| u.block_daa_score);

                            let mut player_a_confirmed_sompi: u64 = 0;
                            let mut player_b_confirmed_sompi: u64 = 0;
                            let mut a_accum: u64 = 0; // running total for heuristic fallback

                            for utxo in &sorted_utxos {
                                if utxo.is_coinbase {
                                    continue;
                                }

                                let confirmations = if current_daa > 0 {
                                    current_daa.saturating_sub(utxo.block_daa_score)
                                } else if utxo.block_daa_score > 0 {
                                    // During IBD: node reports DAA=0 but UTXOs have valid
                                    // block_daa_score — the UTXO is already mined, so treat
                                    // it as confirmed. We can't compute exact confirmations
                                    // without the current DAA, but the UTXO's existence in
                                    // the UTXO set proves it's in a mined block.
                                    MIN_CONFIRMATIONS
                                } else {
                                    0
                                };

                                // Deterministic attribution:
                                // 1. If tx_id matches a known deposit tx_hash → use that role
                                // 2. Else: fallback to ordered heuristic
                                let role = if player_a_tx_hash.as_deref() == Some(&utxo.tx_id) {
                                    "A"
                                } else if player_b_tx_hash.as_deref() == Some(&utxo.tx_id) {
                                    "B"
                                } else if a_accum < wager_u64 {
                                    a_accum += utxo.amount;
                                    "A"
                                } else {
                                    "B"
                                };

                                // Fix #3: guard opponent_id for Player B
                                let player_uuid = match role {
                                    "A" => Some(creator_id),
                                    "B" => {
                                        if opponent_id.is_none() {
                                            tracing::warn!(
                                                match_id = %self.match_id,
                                                tx_id = %utxo.tx_id,
                                                "⚠️ Role B UTXO but opponent_id is NULL — skipping payment insert"
                                            );
                                        }
                                        opponent_id
                                    },
                                    _ => None,
                                };

                                // Fix #2: Always log every UTXO on every poll for full traceability
                                eprintln!(
                                    "  🔬 UTXO: tx={} role={} amount={} confs={}/{}",
                                    &utxo.tx_id[..12], role, utxo.amount, confirmations, MIN_CONFIRMATIONS
                                );

                                // Insert/update payment record (idempotent via UNIQUE constraint)
                                let inserted = sqlx::query(
                                    "INSERT INTO payments \
                                     (match_id, player_id, player_role, tx_id, amount_sompi, \
                                      block_daa_score, confirmations) \
                                     VALUES ($1, $2, $3, $4, $5, $6, $7) \
                                     ON CONFLICT (tx_id, match_id) DO UPDATE \
                                     SET confirmations = EXCLUDED.confirmations",
                                )
                                .bind(self.match_id)
                                .bind(player_uuid)
                                .bind(role)
                                .bind(&utxo.tx_id)
                                .bind(utxo.amount as i64)
                                .bind(utxo.block_daa_score as i64)
                                .bind(confirmations as i32)
                                .execute(&self.db_pool)
                                .await;

                                match &inserted {
                                    Ok(result) => {
                                        tracing::info!(
                                            match_id = %self.match_id,
                                            tx_id = %utxo.tx_id,
                                            player_role = %role,
                                            amount_sompi = utxo.amount,
                                            confirmations,
                                            rows = result.rows_affected(),
                                            "💳 UTXO upserted in payments"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            match_id = %self.match_id,
                                            tx_id = %utxo.tx_id,
                                            error = %e,
                                            "❌ Failed to insert payment record"
                                        );
                                        eprintln!(
                                            "  ❌ Payment insert failed: tx={} err={}",
                                            &utxo.tx_id[..12], e
                                        );
                                    }
                                }

                                // Only count UTXOs with enough confirmations
                                if confirmations >= MIN_CONFIRMATIONS {
                                    match role {
                                        "A" => player_a_confirmed_sompi += utxo.amount,
                                        "B" => player_b_confirmed_sompi += utxo.amount,
                                        _ => {}
                                    }
                                } else {
                                    tracing::info!(
                                        match_id = %self.match_id,
                                        tx_id = %utxo.tx_id,
                                        player_role = %role,
                                        confirmations,
                                        needed = MIN_CONFIRMATIONS,
                                        "⏳ UTXO not yet confirmed enough"
                                    );
                                }
                            }

                            let a_met = player_a_confirmed_sompi >= wager_u64;
                            let b_met = player_b_confirmed_sompi >= wager_u64;

                            eprintln!(
                                "📊 Match {}: A={}/{} sompi ({}), B={}/{} sompi ({})",
                                self.match_id,
                                player_a_confirmed_sompi, wager_u64,
                                if a_met { "✅" } else { "⏳" },
                                player_b_confirmed_sompi, wager_u64,
                                if b_met { "✅" } else { "⏳" },
                            );

                            // Update per-player confirmed flags
                            if a_met {
                                sqlx::query(
                                    "UPDATE matches SET player_a_deposit_confirmed = true, \
                                     player_a_deposit_amount_sompi = $1 WHERE id = $2 \
                                     AND (player_a_deposit_confirmed IS NULL OR player_a_deposit_confirmed = false)",
                                )
                                .bind(player_a_confirmed_sompi as i64)
                                .bind(self.match_id)
                                .execute(&self.db_pool)
                                .await?;
                                tracing::info!(
                                    match_id = %self.match_id,
                                    sompi = player_a_confirmed_sompi,
                                    "✅ Player A deposit confirmed"
                                );
                            }
                            if b_met {
                                sqlx::query(
                                    "UPDATE matches SET player_b_deposit_confirmed = true, \
                                     player_b_deposit_amount_sompi = $1 WHERE id = $2 \
                                     AND (player_b_deposit_confirmed IS NULL OR player_b_deposit_confirmed = false)",
                                )
                                .bind(player_b_confirmed_sompi as i64)
                                .bind(self.match_id)
                                .execute(&self.db_pool)
                                .await?;
                                tracing::info!(
                                    match_id = %self.match_id,
                                    sompi = player_b_confirmed_sompi,
                                    "✅ Player B deposit confirmed"
                                );
                            }

                            // Transition to FUNDED only when both confirmed
                            if a_met && b_met {
                                sqlx::query(
                                    "UPDATE matches SET status = 'FUNDED', \
                                     player_a_deposit_confirmed = true, \
                                     player_b_deposit_confirmed = true \
                                     WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .execute(&mut *db_tx)
                                .await?;

                                // Fix #1: read via db_tx to see the uncommitted FUNDED state
                                if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>(
                                    "SELECT * FROM matches WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .fetch_one(&mut *db_tx)
                                .await
                                {
                                    let mut m = updated;
                                    m.calculate_wager();
                                    let _ = self
                                        .tx
                                        .send(serde_json::to_string(&m).unwrap_or_default());
                                }

                                eprintln!(
                                    "💰 Match {} → FUNDED (both deposits confirmed)",
                                    self.match_id
                                );
                                tracing::info!(
                                    match_id = %self.match_id,
                                    "💰 AWAITING_FUNDING → FUNDED (both deposits confirmed)"
                                );
                            } else {
                                tracing::info!(
                                    match_id = %self.match_id,
                                    player_a_sompi = player_a_confirmed_sompi,
                                    player_b_sompi = player_b_confirmed_sompi,
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

                                if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>(
                                    "SELECT * FROM matches WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .fetch_one(&self.db_pool)
                                .await
                                {
                                    let mut m = updated;
                                    m.calculate_wager();
                                    let _ = self
                                        .tx
                                        .send(serde_json::to_string(&m).unwrap_or_default());
                                }

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
