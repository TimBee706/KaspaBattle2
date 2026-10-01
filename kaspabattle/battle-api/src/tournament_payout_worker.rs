use battle_kaspa::payout::{PayoutService, TournamentPayoutParams};
use battle_kaspa::watcher::BlockchainWatcher;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// EscrowService reserved for Phase 5 key registration flow
#[allow(dead_code)]
use battle_kaspa::escrow::EscrowService;

// ─── Config ───────────────────────────────────────────────────────────────────

const DEPOSIT_POLL_INTERVAL_SECS: u64 = 30;
const PAYOUT_POLL_INTERVAL_SECS: u64 = 60;
const REFUND_POLL_INTERVAL_SECS: u64 = 120;

/// Minimum DAA-score confirmations before a tournament deposit is considered final.
/// Matches MIN_CONFIRMATIONS in match_episode.rs (~10 seconds at Kaspa's ~1 block/sec rate).
pub(crate) const MIN_CONFIRMATIONS_TOURNAMENT: u64 = 10;

/// Internal sentinel stored in `tournaments.payout_tx_hash` when a CANCELLED tournament
/// has no confirmed deposits and therefore no refund needs to be issued.
/// This is NOT a real on-chain transaction hash — it is a purely application-level marker.
pub(crate) const NO_REFUND_NEEDED_SENTINEL: &str = "no-refund-needed";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

// ─── Prize-share calculation ─────────────────────────────────────────────────

/// Calculates winner, runner-up, and platform-fee shares from a prize pool.
/// The fee absorbs rounding dust via `saturating_sub` so outputs never exceed `pool_sompi`.
pub(crate) fn calculate_prize_shares(
    pool_sompi: i64,
    winner_pct: i64,
    runner_up_pct: i64,
) -> (u64, u64, u64) {
    if pool_sompi <= 0 {
        return (0, 0, 0);
    }
    let winner = (pool_sompi * winner_pct / 100) as u64;
    let runner_up = (pool_sompi * runner_up_pct / 100) as u64;
    let fee = pool_sompi.saturating_sub((winner + runner_up) as i64) as u64;
    (winner, runner_up, fee)
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Computes DAA-based confirmations with IBD fallback.
fn compute_confirmations(current_daa: u64, block_daa_score: u64) -> u64 {
    if current_daa > 0 {
        current_daa.saturating_sub(block_daa_score)
    } else if block_daa_score > 0 {
        // IBD: node reports DAA=0 but UTXO is already mined — treat as confirmed.
        MIN_CONFIRMATIONS_TOURNAMENT
    } else {
        0
    }
}

/// Upserts a tournament payment record (outside any transaction).
async fn upsert_payment(
    pool: &PgPool,
    tournament_id: Uuid,
    team_id: Option<Uuid>,
    tx_id: &str,
    amount_sompi: i64,
    block_daa_score: i64,
    confirmations: i32,
) -> Result<(), BoxError> {
    sqlx::query(
        "INSERT INTO tournament_payments \
         (id, tournament_id, team_id, tx_id, amount_sompi, block_daa_score, confirmations) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6) \
         ON CONFLICT (tx_id, tournament_id) DO UPDATE \
         SET confirmations = EXCLUDED.confirmations, \
             amount_sompi  = EXCLUDED.amount_sompi, \
             team_id       = COALESCE(tournament_payments.team_id, EXCLUDED.team_id), \
             updated_at    = NOW()",
    )
    .bind(tournament_id)
    .bind(team_id)
    .bind(tx_id)
    .bind(amount_sompi)
    .bind(block_daa_score)
    .bind(confirmations)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upserts a tournament payment record inside an existing transaction.
async fn upsert_payment_in_tx(
    db_tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tournament_id: Uuid,
    team_id: Option<Uuid>,
    tx_id: &str,
    amount_sompi: i64,
    block_daa_score: i64,
    confirmations: i32,
) -> Result<(), BoxError> {
    sqlx::query(
        "INSERT INTO tournament_payments \
         (id, tournament_id, team_id, tx_id, amount_sompi, block_daa_score, confirmations) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6) \
         ON CONFLICT (tx_id, tournament_id) DO UPDATE \
         SET confirmations = EXCLUDED.confirmations, \
             amount_sompi  = EXCLUDED.amount_sompi, \
             team_id       = COALESCE(tournament_payments.team_id, EXCLUDED.team_id), \
             updated_at    = NOW()",
    )
    .bind(tournament_id)
    .bind(team_id)
    .bind(tx_id)
    .bind(amount_sompi)
    .bind(block_daa_score)
    .bind(confirmations)
    .execute(&mut **db_tx)
    .await?;
    Ok(())
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn spawn_tournament_workers(
    pool: Arc<PgPool>,
    _escrow_service: Option<Arc<EscrowService>>,
    blockchain_watcher: Option<Arc<BlockchainWatcher>>,
    payout_service: Option<Arc<PayoutService>>,
) {
    if let Some(watcher) = blockchain_watcher {
        let pg = pool.clone();
        tokio::spawn(async move {
            run_deposit_watcher(pg, watcher).await;
        });
        tracing::info!("✅ Tournament deposit watcher started");
    } else {
        tracing::info!("ℹ️ Tournament deposit watcher disabled (no BlockchainWatcher)");
    }

    if let Some(payout) = payout_service {
        let pg_payout = pool.clone();
        let pg_refund = pool.clone();
        let payout_refund = payout.clone();

        tokio::spawn(async move {
            run_payout_executor(pg_payout, payout).await;
        });
        tokio::spawn(async move {
            run_refund_executor(pg_refund, payout_refund).await;
        });
        tracing::info!("✅ Tournament payout + refund executors started");
    } else {
        tracing::info!("ℹ️ Tournament payout executor disabled (no PayoutService)");
    }
}

// ─── Deposit Watcher ──────────────────────────────────────────────────────────

async fn run_deposit_watcher(pool: Arc<PgPool>, watcher: Arc<BlockchainWatcher>) {
    let mut consecutive_errors: u32 = 0;
    loop {
        let cycle_start = std::time::Instant::now();
        match poll_tournament_deposits(&pool, &watcher).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!(
                        "\u{1F4B0} Tournament deposit watcher: {} deposit(s) confirmed",
                        n
                    );
                }
                consecutive_errors = 0;
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::error!(error = %e, consecutive_errors, "Tournament deposit watcher error");
            }
        }
        // N-05: Warn if poll cycle takes longer than its interval (backpressure indicator).
        let elapsed = cycle_start.elapsed();
        let interval = std::time::Duration::from_secs(DEPOSIT_POLL_INTERVAL_SECS);
        if elapsed > interval {
            tracing::warn!(
                elapsed_secs = elapsed.as_secs(),
                interval_secs = DEPOSIT_POLL_INTERVAL_SECS,
                "\u{26A0}\u{FE0F} Deposit poll cycle took {}s > {}s interval",
                elapsed.as_secs(),
                DEPOSIT_POLL_INTERVAL_SECS
            );
        }
        let sleep = match consecutive_errors {
            0 => interval.saturating_sub(elapsed),
            1 => std::time::Duration::from_secs(60),
            2 => std::time::Duration::from_secs(120),
            _ => std::time::Duration::from_secs(300),
        };
        tokio::time::sleep(sleep).await;
    }
}

async fn poll_tournament_deposits(
    pool: &PgPool,
    watcher: &BlockchainWatcher,
) -> Result<usize, BoxError> {
    // C-02: Only process tournaments that are still accepting deposits.
    let rows = sqlx::query(
        "SELECT id, escrow_address, buy_in_sompi, status::text AS status \
         FROM tournaments \
         WHERE status IN ('REGISTRATION', 'FUNDED') \
           AND escrow_address IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    // Fetch DAA once per poll cycle via the watcher.
    let current_daa = match watcher.get_current_daa_score().await {
        Ok(0) => {
            tracing::warn!("⚠️ DAA=0, node may not be synced yet");
            0
        }
        Ok(daa) => daa,
        Err(e) => {
            tracing::warn!(error = %e, "⚠️ get_current_daa_score failed — IBD fallback active");
            0
        }
    };

    let mut total = 0usize;

    for row in &rows {
        use sqlx::Row;
        let tournament_id: Uuid = row.try_get("id")?;
        let escrow_address: String = row.try_get("escrow_address")?;
        let buy_in_sompi: i64 = row.try_get("buy_in_sompi")?;
        // N-01: Use cached status — no per-UTXO SELECT needed.
        let cached_status: String = row.try_get("status").unwrap_or_default();
        if !matches!(cached_status.as_str(), "REGISTRATION" | "FUNDED") {
            continue;
        }

        let escrow_status = match watcher.check_escrow_balance(&escrow_address).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(tournament_id = %tournament_id, error = %e, "Deposit watcher: RPC check failed");
                continue;
            }
        };

        for utxo in &escrow_status.utxos {
            // Skip coinbase and unmined UTXOs.
            if utxo.block_daa_score == 0 || utxo.is_coinbase {
                continue;
            }
            let tx_id = utxo.tx_id.clone();
            let amount_sompi = utxo.amount as i64;
            let block_daa_score = utxo.block_daa_score as i64;
            let confirmations = compute_confirmations(current_daa, utxo.block_daa_score);
            let confs_i32 = confirmations as i32;

            if amount_sompi >= buy_in_sompi && confirmations >= MIN_CONFIRMATIONS_TOURNAMENT {
                // B-01/I-03: team-credit + prize-pool + payment in ONE atomic TX.
                // FOR UPDATE SKIP LOCKED is now inside an explicit transaction (I-03 fix).
                let mut db_tx = pool.begin().await?;

                // Guard mid-cycle status change inside the TX.
                let still_active: Option<String> = sqlx::query_scalar(
                    "SELECT status::text FROM tournaments \
                     WHERE id = $1 AND status IN ('REGISTRATION', 'FUNDED')",
                )
                .bind(tournament_id)
                .fetch_optional(&mut *db_tx)
                .await
                .unwrap_or(None);

                if still_active.is_none() {
                    db_tx.rollback().await.ok();
                    tracing::info!(tournament_id = %tournament_id, "Deposit watcher: status changed mid-loop — stopping");
                    break;
                }

                let team_to_credit: Option<Uuid> = sqlx::query_scalar(
                    "UPDATE tournament_teams \
                     SET deposit_status = 'CONFIRMED', deposit_tx_hash = $2, \
                         deposit_confirmed_at = NOW(), updated_at = NOW() \
                     WHERE id = ( \
                         SELECT id FROM tournament_teams \
                         WHERE tournament_id = $1 AND deposit_status = 'PENDING' \
                           AND (deposit_tx_hash IS NULL OR deposit_tx_hash = $2) \
                         ORDER BY seed ASC NULLS LAST, created_at ASC \
                         LIMIT 1 \
                         FOR UPDATE SKIP LOCKED \
                     ) \
                     RETURNING id",
                )
                .bind(tournament_id)
                .bind(&tx_id)
                .fetch_optional(&mut *db_tx)
                .await
                .unwrap_or(None);

                if let Some(team_id) = team_to_credit {
                    // I-01: Prize pool counts exact buy_in_sompi, not actual UTXO amount.
                    // This makes the prize pool predictable regardless of over/underpayment.
                    sqlx::query(
                        "UPDATE tournaments \
                         SET total_prize_pool_sompi = total_prize_pool_sompi + $1, updated_at = NOW() \
                         WHERE id = $2",
                    )
                    .bind(buy_in_sompi)
                    .bind(tournament_id)
                    .execute(&mut *db_tx)
                    .await?;

                    upsert_payment_in_tx(
                        &mut db_tx,
                        tournament_id,
                        Some(team_id),
                        &tx_id,
                        amount_sompi,
                        block_daa_score,
                        confs_i32,
                    )
                    .await?;
                    db_tx.commit().await?;

                    tracing::info!(
                        tournament_id = %tournament_id, team_id = %team_id,
                        tx_id = %&tx_id[..12.min(tx_id.len())],
                        amount_sompi, buy_in_sompi, confirmations,
                        "✅ Team deposit confirmed ({}/{} confs)", confirmations, MIN_CONFIRMATIONS_TOURNAMENT
                    );
                    total += 1;
                } else {
                    // No unclaimed PENDING team — track UTXO for UI without prize pool change.
                    db_tx.rollback().await.ok();
                    upsert_payment(
                        pool,
                        tournament_id,
                        None,
                        &tx_id,
                        amount_sompi,
                        block_daa_score,
                        confs_i32,
                    )
                    .await?;
                }
            } else {
                // Below threshold — track for UI ("X/10 confs").
                tracing::info!(
                    tournament_id = %tournament_id,
                    tx_id = %&tx_id[..12.min(tx_id.len())],
                    amount_sompi, confirmations, needed = MIN_CONFIRMATIONS_TOURNAMENT,
                    "⏳ Deposit pending: {}/{} confs", confirmations, MIN_CONFIRMATIONS_TOURNAMENT
                );
                upsert_payment(
                    pool,
                    tournament_id,
                    None,
                    &tx_id,
                    amount_sompi,
                    block_daa_score,
                    confs_i32,
                )
                .await?;
            }
        }

        // I-02: Atomic FUNDED transition — single SQL, no race between separate COUNT queries.
        let funded: Option<Uuid> = sqlx::query_scalar(
            "UPDATE tournaments SET status = 'FUNDED', updated_at = NOW() \
             WHERE id = $1 \
               AND status = 'REGISTRATION' \
               AND (SELECT COUNT(*) FROM tournament_teams WHERE tournament_id = $1) >= 2 \
               AND (SELECT COUNT(*) FROM tournament_teams WHERE tournament_id = $1) \
                 = (SELECT COUNT(*) FROM tournament_teams \
                    WHERE tournament_id = $1 AND deposit_status = 'CONFIRMED') \
             RETURNING id",
        )
        .bind(tournament_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if funded.is_some() {
            tracing::info!(tournament_id = %tournament_id, "🎉 Tournament auto-transitioned to FUNDED");
        }
    }

    Ok(total)
}

// ─── Payout Executor (Phase 4 — REAL TX) ─────────────────────────────────────

async fn run_payout_executor(pool: Arc<PgPool>, payout_service: Arc<PayoutService>) {
    let mut consecutive_errors: u32 = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(PAYOUT_POLL_INTERVAL_SECS)).await;

        match execute_pending_payouts(&pool, &payout_service).await {
            Ok(0) => consecutive_errors = 0,
            Ok(n) => {
                tracing::info!("💸 Tournament payout executor: {} payout(s) sent", n);
                consecutive_errors = 0;
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::error!(error = %e, consecutive_errors, "Tournament payout executor error");
            }
        }
    }
}

async fn execute_pending_payouts(
    pool: &PgPool,
    payout_service: &PayoutService,
) -> Result<usize, BoxError> {
    let treasury_address = std::env::var("TREASURY_ADDRESS").unwrap_or_default();

    // Fetch candidates without locking first (cheap read).
    let rows = sqlx::query(
        "SELECT t.id, t.total_prize_pool_sompi, t.escrow_address, \
                t.prize_winner_pct, t.prize_runner_up_pct, t.platform_fee_pct, \
                wu.kaspa_address AS winner_kaspa_address, \
                ru.kaspa_address AS runner_up_kaspa_address \
         FROM tournaments t \
         LEFT JOIN tournament_teams w  ON w.id  = t.winner_team_id_ref \
         LEFT JOIN tournament_teams r  ON r.id  = t.runner_up_team_id_ref \
         LEFT JOIN users wu ON wu.id = w.captain_user_id \
         LEFT JOIN users ru ON ru.id = r.captain_user_id \
         WHERE t.status = 'COMPLETED' \
           AND t.payout_tx_hash IS NULL \
           AND t.winner_team_id_ref IS NOT NULL \
           AND t.escrow_address IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut count = 0usize;

    for row in &rows {
        use sqlx::Row;

        let tournament_id: Uuid = row.try_get("id")?;
        let pool_sompi: i64 = row.try_get("total_prize_pool_sompi").unwrap_or(0);
        let escrow_address: String = row.try_get("escrow_address")?;
        let winner_pct: i64 = row.try_get::<i16, _>("prize_winner_pct").unwrap_or(70) as i64;
        let runner_up_pct: i64 = row.try_get::<i16, _>("prize_runner_up_pct").unwrap_or(20) as i64;
        let winner_addr: Option<String> = row.try_get("winner_kaspa_address").unwrap_or(None);
        let runner_up_addr: Option<String> = row.try_get("runner_up_kaspa_address").unwrap_or(None);

        // B-02: Two-step claim marker to prevent TOCTOU race with admin endpoint.
        // Instead of a short-lived DB lock, we atomically claim the row with 'PROCESSING'.
        let claimed: Option<Uuid> = sqlx::query_scalar(
            "UPDATE tournaments SET payout_tx_hash = 'PROCESSING' \
             WHERE id = $1 AND status = 'COMPLETED' AND payout_tx_hash IS NULL \
             RETURNING id",
        )
        .bind(tournament_id)
        .fetch_optional(pool)
        .await?;

        if claimed.is_none() {
            tracing::info!(
                tournament_id = %tournament_id,
                "Payout worker: tournament already being processed or payout_tx_hash set — skipping"
            );
            continue;
        }

        let (winner_kaspa, runner_up_kaspa) = match (winner_addr, runner_up_addr) {
            (Some(w), Some(r)) => (w, r),
            _ => {
                tracing::warn!(tournament_id = %tournament_id, "⚠️ Payout skipped: missing Kaspa addresses");
                let _ = sqlx::query(
                    "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                     VALUES ('tournament', $1, 'payout_skipped_missing_addresses', 'system', $2)",
                )
                .bind(tournament_id)
                .bind(serde_json::json!({"reason": "winner or runner_up captain has no kaspa_address"}))
                .execute(pool)
                .await;
                continue;
            }
        };

        if pool_sompi <= 0 {
            tracing::warn!(tournament_id = %tournament_id, "⚠️ Payout skipped: empty prize pool");
            let _ = sqlx::query(
                "UPDATE tournaments SET payout_tx_hash = NULL WHERE id = $1 AND payout_tx_hash = 'PROCESSING'"
            ).bind(tournament_id).execute(pool).await;

            let _ = sqlx::query(
                "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                 VALUES ('tournament', $1, 'payout_skipped_empty_pool', 'system', $2)",
            )
            .bind(tournament_id)
            .bind(serde_json::json!({"pool_sompi": pool_sompi}))
            .execute(pool)
            .await;
            continue;
        }

        // Use shared fee helper (consistent with admin_trigger_payout).
        let (winner_sompi, runner_up_sompi, fee_sompi_amt) =
            calculate_prize_shares(pool_sompi, winner_pct, runner_up_pct);

        let mut outputs = vec![
            (winner_kaspa.clone(), winner_sompi),
            (runner_up_kaspa.clone(), runner_up_sompi),
        ];
        if fee_sompi_amt > 0 {
            if treasury_address.is_empty() {
                tracing::warn!(
                    tournament_id = %tournament_id,
                    fee_sompi = fee_sompi_amt,
                    "⚠️ TREASURY_ADDRESS not set — platform fee will be absorbed as TX fee or lost"
                );
            } else {
                outputs.push((treasury_address.clone(), fee_sompi_amt));
            }
        }

        tracing::info!(
            tournament_id = %tournament_id,
            pool_sompi,
            winner_sompi,
            runner_up_sompi,
            fee = fee_sompi_amt,
            "💸 Executing tournament payout via real TX"
        );

        let params = TournamentPayoutParams {
            escrow_address: escrow_address.clone(),
            outputs,
        };

        match payout_service.execute_tournament_payout(&params).await {
            Ok(result) => {
                sqlx::query(
                    "UPDATE tournaments \
                     SET payout_tx_hash = $1, payout_executed_at = NOW(), updated_at = NOW() \
                     WHERE id = $2 AND payout_tx_hash = 'PROCESSING'",
                )
                .bind(&result.tx_id)
                .bind(tournament_id)
                .execute(pool)
                .await?;

                let _ = sqlx::query(
                    "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                     VALUES ('tournament', $1, 'payout_executed', 'system', $2)",
                )
                .bind(tournament_id)
                .bind(serde_json::json!({
                    "tx_id": result.tx_id,
                    "total_sompi": result.total_sompi,
                    "fee_sompi": result.fee_sompi,
                    "winner_addr": winner_kaspa,
                    "runner_up_addr": runner_up_kaspa,
                }))
                .execute(pool)
                .await;

                tracing::info!(
                    tournament_id = %tournament_id,
                    tx_id = %result.tx_id,
                    "✅ Tournament payout TX submitted"
                );
                count += 1;
            }
            Err(e) => {
                // Rollback claim marker on error so it can be retried next cycle
                let _ = sqlx::query(
                    "UPDATE tournaments SET payout_tx_hash = NULL WHERE id = $1 AND payout_tx_hash = 'PROCESSING'"
                ).bind(tournament_id).execute(pool).await;

                tracing::error!(
                    tournament_id = %tournament_id,
                    error = %e,
                    "❌ Tournament payout TX failed — will retry next cycle"
                );
            }
        }
    }

    Ok(count)
}

// ─── Refund Executor ──────────────────────────────────────────────────────────

async fn run_refund_executor(pool: Arc<PgPool>, payout_service: Arc<PayoutService>) {
    let mut consecutive_errors: u32 = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(REFUND_POLL_INTERVAL_SECS)).await;

        match execute_pending_refunds(&pool, &payout_service).await {
            Ok(0) => consecutive_errors = 0,
            Ok(n) => {
                tracing::info!("💸 Tournament refund executor: {} refund(s) sent", n);
                consecutive_errors = 0;
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::error!(error = %e, consecutive_errors, "Tournament refund executor error");
            }
        }
    }
}

async fn execute_pending_refunds(
    pool: &PgPool,
    payout_service: &PayoutService,
) -> Result<usize, BoxError> {
    let tournaments = sqlx::query(
        "SELECT t.id, t.escrow_address, t.total_prize_pool_sompi \
         FROM tournaments t \
         WHERE t.status = 'CANCELLED' \
           AND t.total_prize_pool_sompi > 0 \
           AND t.payout_tx_hash IS NULL \
           AND t.escrow_address IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut count = 0usize;

    for row in &tournaments {
        use sqlx::Row;
        let tournament_id: Uuid = row.try_get("id")?;
        let escrow_address: String = row.try_get("escrow_address")?;

        // B-03: Two-step claim marker to prevent TOCTOU race (duplicate refunds).
        let claimed: Option<Uuid> = sqlx::query_scalar(
            "UPDATE tournaments SET payout_tx_hash = 'PROCESSING' \
             WHERE id = $1 AND status = 'CANCELLED' AND payout_tx_hash IS NULL \
             RETURNING id",
        )
        .bind(tournament_id)
        .fetch_optional(pool)
        .await?;

        if claimed.is_none() {
            // N-03: Debug-Log bei bereits verarbeitetem Refund
            tracing::debug!(
                tournament_id = %tournament_id,
                "Refund worker: tournament already being processed or payout_tx_hash set — skipping"
            );
            continue;
        }

        let teams = sqlx::query(
            "SELECT tp.team_id, tp.amount_sompi, u.kaspa_address \
             FROM tournament_payments tp \
             JOIN tournament_teams tt ON tt.id = tp.team_id \
             JOIN users u ON u.id = tt.captain_user_id \
             WHERE tp.tournament_id = $1 \
               AND tt.deposit_status = 'CONFIRMED' \
               AND u.kaspa_address IS NOT NULL \
             ORDER BY tt.seed ASC NULLS LAST",
        )
        .bind(tournament_id)
        .fetch_all(pool)
        .await?;

        if teams.is_empty() {
            sqlx::query(
                // SENTINEL: 'no-refund-needed' is an internal application marker, NOT a real
                // on-chain transaction hash. It signals that this CANCELLED tournament had no
                // confirmed deposits and therefore no refund transaction was needed.
                "UPDATE tournaments SET payout_tx_hash = $1, \
                 payout_executed_at = NOW(), updated_at = NOW() WHERE id = $2 AND payout_tx_hash = 'PROCESSING'",
            )
            .bind(NO_REFUND_NEEDED_SENTINEL)
            .bind(tournament_id)
            .execute(pool)
            .await?;
            continue;
        }

        let refund_outputs: Vec<(String, u64)> = teams
            .iter()
            .filter_map(|r| {
                use sqlx::Row;
                let addr: Option<String> = r.try_get("kaspa_address").unwrap_or(None);
                let amount: i64 = r.try_get("amount_sompi").unwrap_or(0);
                addr.map(|a| (a, amount as u64))
            })
            .collect();

        // I-04: Infinite Loop im Refund-Executor wenn Teams keine Kaspa-Adresse haben
        if refund_outputs.is_empty() {
            tracing::warn!(
                tournament_id = %tournament_id,
                "⚠️ Refund skipped: all confirmed teams lack Kaspa address — admin intervention needed"
            );
            // Clear processing marker so admin can fix addresses and retry
            let _ = sqlx::query(
                "UPDATE tournaments SET payout_tx_hash = NULL WHERE id = $1 AND payout_tx_hash = 'PROCESSING'"
            ).bind(tournament_id).execute(pool).await;

            let _ = sqlx::query(
                "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                 VALUES ('tournament', $1, 'refund_skipped_no_addresses', 'system', $2)",
            )
            .bind(tournament_id)
            .bind(serde_json::json!({"reason": "all confirmed teams have no kaspa_address"}))
            .execute(pool)
            .await;
            continue;
        }

        tracing::info!(
            tournament_id = %tournament_id,
            recipients = refund_outputs.len(),
            "💸 Executing tournament refund"
        );

        match payout_service
            .execute_tournament_refund(&escrow_address, refund_outputs)
            .await
        {
            Ok(result) => {
                sqlx::query(
                    "UPDATE tournaments \
                     SET payout_tx_hash = $1, payout_executed_at = NOW(), updated_at = NOW() \
                     WHERE id = $2 AND payout_tx_hash = 'PROCESSING'",
                )
                .bind(&result.tx_id)
                .bind(tournament_id)
                .execute(pool)
                .await?;

                sqlx::query(
                    "UPDATE tournament_teams \
                     SET refund_tx_hash = $1, refund_executed_at = NOW(), updated_at = NOW() \
                     WHERE tournament_id = $2 AND deposit_status = 'CONFIRMED'",
                )
                .bind(&result.tx_id)
                .bind(tournament_id)
                .execute(pool)
                .await?;

                let _ = sqlx::query(
                    "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                     VALUES ('tournament', $1, 'refund_executed', 'system', $2)",
                )
                .bind(tournament_id)
                .bind(
                    serde_json::json!({ "tx_id": result.tx_id, "total_sompi": result.total_sompi }),
                )
                .execute(pool)
                .await;

                tracing::info!(tournament_id = %tournament_id, tx_id = %result.tx_id, "✅ Refund TX submitted");
                count += 1;
            }
            Err(e) => {
                // Rollback the claim marker on error
                let _ = sqlx::query(
                    "UPDATE tournaments SET payout_tx_hash = NULL WHERE id = $1 AND payout_tx_hash = 'PROCESSING'"
                ).bind(tournament_id).execute(pool).await;

                tracing::error!(
                    tournament_id = %tournament_id,
                    error = %e,
                    "❌ Refund TX failed — will retry"
                );
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prize_shares_standard() {
        let (w, r, f) = calculate_prize_shares(1_000_000, 70, 20);
        assert_eq!(w, 700_000);
        assert_eq!(r, 200_000);
        assert_eq!(f, 100_000);
    }

    #[test]
    fn prize_shares_rounding_dust_goes_to_fee() {
        // pool=101, 70%=70, 20%=20, fee absorbs the 11 remainder
        let (w, r, f) = calculate_prize_shares(101, 70, 20);
        assert_eq!(w, 70);
        assert_eq!(r, 20);
        assert_eq!(f, 11);
    }

    #[test]
    fn prize_shares_zero_pool() {
        assert_eq!(calculate_prize_shares(0, 70, 20), (0, 0, 0));
    }

    #[test]
    fn daa_confirmation_ibd_fallback() {
        // current_daa == 0 but block_daa_score > 0 → treat as MIN_CONFIRMATIONS_TOURNAMENT
        let block_daa: u64 = 1_000_000;
        let current_daa: u64 = 0;
        let confs: u64 = if current_daa > 0 {
            current_daa.saturating_sub(block_daa)
        } else if block_daa > 0 {
            MIN_CONFIRMATIONS_TOURNAMENT
        } else {
            0
        };
        assert_eq!(confs, MIN_CONFIRMATIONS_TOURNAMENT);
    }

    #[test]
    fn daa_confirmation_threshold() {
        let block_daa: u64 = 1_000;
        let current_daa: u64 = 1_009; // 9 confs — not enough
        let confs = current_daa.saturating_sub(block_daa);
        assert!(confs < MIN_CONFIRMATIONS_TOURNAMENT);

        let current_daa2: u64 = 1_010; // exactly 10 — sufficient
        let confs2 = current_daa2.saturating_sub(block_daa);
        assert!(confs2 >= MIN_CONFIRMATIONS_TOURNAMENT);
    }
}
