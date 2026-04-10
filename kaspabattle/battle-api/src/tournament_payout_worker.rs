use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

use battle_kaspa::watcher::BlockchainWatcher;
use battle_kaspa::payout::{PayoutService, TournamentPayoutParams};

// EscrowService reserved for Phase 5 key registration flow
#[allow(dead_code)]
use battle_kaspa::escrow::EscrowService;

// ─── Config ───────────────────────────────────────────────────────────────────

const DEPOSIT_POLL_INTERVAL_SECS: u64 = 30;
const PAYOUT_POLL_INTERVAL_SECS: u64 = 60;
const REFUND_POLL_INTERVAL_SECS: u64 = 120;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

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
        match poll_tournament_deposits(&pool, &watcher).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!("💰 Tournament deposit watcher: {} deposit(s) processed", n);
                }
                consecutive_errors = 0;
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::error!(error = %e, consecutive_errors, "Tournament deposit watcher error");
            }
        }
        let sleep = match consecutive_errors {
            0 => DEPOSIT_POLL_INTERVAL_SECS,
            1 => 60,
            2 => 120,
            _ => 300,
        };
        tokio::time::sleep(std::time::Duration::from_secs(sleep)).await;
    }
}

async fn poll_tournament_deposits(pool: &PgPool, watcher: &BlockchainWatcher) -> Result<usize, BoxError> {
    // C-02: Only process tournaments that are still accepting deposits.
    // Explicitly exclude CANCELLED/COMPLETED to prevent crediting late-arriving UTXOs.
    let rows = sqlx::query(
        "SELECT id, escrow_address, buy_in_sompi \
         FROM tournaments \
         WHERE status IN ('REGISTRATION', 'FUNDED') \
           AND status NOT IN ('CANCELLED', 'COMPLETED', 'DISPUTED') \
           AND escrow_address IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut total = 0usize;

    for row in &rows {
        use sqlx::Row;
        let tournament_id: Uuid = row.try_get("id")?;
        let escrow_address: String = row.try_get("escrow_address")?;
        let buy_in_sompi: i64 = row.try_get("buy_in_sompi")?;

        let escrow_status = match watcher.check_escrow_balance(&escrow_address).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(tournament_id = %tournament_id, error = %e, "Deposit watcher: RPC check failed");
                continue;
            }
        };

        for utxo in &escrow_status.utxos {
            if utxo.block_daa_score == 0 || utxo.is_coinbase {
                continue;
            }
            let tx_id = utxo.tx_id.clone();
            let amount_sompi = utxo.amount as i64;

            // C-02: Re-check tournament status before processing each UTXO
            // to guard against mid-cycle cancellation.
            let current_status: Option<String> = sqlx::query_scalar(
                "SELECT status::text FROM tournaments WHERE id = $1",
            )
            .bind(tournament_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if !matches!(current_status.as_deref(), Some("REGISTRATION") | Some("FUNDED")) {
                tracing::info!(tournament_id = %tournament_id, status = ?current_status, "Deposit watcher: tournament no longer accepting deposits — skipping");
                break;
            }

            let already: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM tournament_payments WHERE tx_id = $1 AND tournament_id = $2)",
            )
            .bind(&tx_id)
            .bind(tournament_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if already {
                continue;
            }

            // C-01: Atomic deposit attribution using UPDATE...RETURNING with
            // FOR UPDATE SKIP LOCKED to prevent two UTXOs from crediting the same team.
            let team_to_credit: Option<Uuid> = if amount_sompi >= buy_in_sompi {
                sqlx::query_scalar(
                    "UPDATE tournament_teams \
                     SET deposit_status = 'CONFIRMED', deposit_tx_hash = $2, \
                         deposit_confirmed_at = NOW(), updated_at = NOW() \
                     WHERE id = ( \
                         SELECT id FROM tournament_teams \
                         WHERE tournament_id = $1 AND deposit_status = 'PENDING' \
                         ORDER BY seed ASC NULLS LAST, created_at ASC \
                         LIMIT 1 \
                         FOR UPDATE SKIP LOCKED \
                     ) \
                     RETURNING id",
                )
                .bind(tournament_id)
                .bind(&tx_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None)
            } else {
                None
            };

            sqlx::query(
                "INSERT INTO tournament_payments (id, tournament_id, team_id, tx_id, amount_sompi, confirmations) \
                 VALUES (gen_random_uuid(), $1, $2, $3, $4, 1) \
                 ON CONFLICT (tx_id, tournament_id) DO NOTHING",
            )
            .bind(tournament_id)
            .bind(team_to_credit)
            .bind(&tx_id)
            .bind(amount_sompi)
            .execute(pool)
            .await?;

            if let Some(team_id) = team_to_credit {
                tracing::info!(tournament_id = %tournament_id, team_id = %team_id, "✅ Team deposit confirmed");
            }

            sqlx::query(
                "UPDATE tournaments SET total_prize_pool_sompi = total_prize_pool_sompi + $1, updated_at = NOW() \
                 WHERE id = $2",
            )
            .bind(amount_sompi)
            .bind(tournament_id)
            .execute(pool)
            .await?;

            total += 1;
        }

        // Auto-transition to FUNDED
        let total_teams: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tournament_teams WHERE tournament_id = $1")
            .bind(tournament_id).fetch_one(pool).await.unwrap_or(0);
        let confirmed_teams: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tournament_teams WHERE tournament_id = $1 AND deposit_status = 'CONFIRMED'",
        )
        .bind(tournament_id).fetch_one(pool).await.unwrap_or(0);

        if total_teams >= 2 && confirmed_teams == total_teams {
            let rows_affected = sqlx::query(
                "UPDATE tournaments SET status = 'FUNDED', updated_at = NOW() \
                 WHERE id = $1 AND status = 'REGISTRATION'",
            )
            .bind(tournament_id)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);

            if rows_affected > 0 {
                tracing::info!(tournament_id = %tournament_id, teams = total_teams, "🎉 Tournament FUNDED");
            }
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

async fn execute_pending_payouts(pool: &PgPool, payout_service: &PayoutService) -> Result<usize, BoxError> {
    let treasury_address = std::env::var("TREASURY_ADDRESS").unwrap_or_default();

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
        let fee_pct: i64 = row.try_get::<i16, _>("platform_fee_pct").unwrap_or(10) as i64;
        let winner_addr: Option<String> = row.try_get("winner_kaspa_address").unwrap_or(None);
        let runner_up_addr: Option<String> = row.try_get("runner_up_kaspa_address").unwrap_or(None);

        let (winner_kaspa, runner_up_kaspa) = match (winner_addr, runner_up_addr) {
            (Some(w), Some(r)) => (w, r),
            _ => {
                tracing::warn!(tournament_id = %tournament_id, "⚠️ Payout skipped: missing Kaspa addresses");
                continue;
            }
        };

        if pool_sompi <= 0 {
            tracing::warn!(tournament_id = %tournament_id, "⚠️ Payout skipped: empty prize pool");
            continue;
        }

        let winner_sompi = (pool_sompi * winner_pct / 100) as u64;
        let runner_up_sompi = (pool_sompi * runner_up_pct / 100) as u64;
        let fee_sompi_amt = (pool_sompi * fee_pct / 100) as u64;

        let mut outputs = vec![
            (winner_kaspa.clone(), winner_sompi),
            (runner_up_kaspa.clone(), runner_up_sompi),
        ];
        if fee_sompi_amt > 0 && !treasury_address.is_empty() {
            outputs.push((treasury_address.clone(), fee_sompi_amt));
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
                     WHERE id = $2",
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

async fn execute_pending_refunds(pool: &PgPool, payout_service: &PayoutService) -> Result<usize, BoxError> {
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
                "UPDATE tournaments SET payout_tx_hash = 'no-refund-needed', \
                 payout_executed_at = NOW(), updated_at = NOW() WHERE id = $1",
            )
            .bind(tournament_id)
            .execute(pool)
            .await?;
            continue;
        }

        let refund_outputs: Vec<(String, u64)> = teams.iter().filter_map(|r| {
            use sqlx::Row;
            let addr: Option<String> = r.try_get("kaspa_address").unwrap_or(None);
            let amount: i64 = r.try_get("amount_sompi").unwrap_or(0);
            addr.map(|a| (a, amount as u64))
        }).collect();

        if refund_outputs.is_empty() {
            continue;
        }

        tracing::info!(
            tournament_id = %tournament_id,
            recipients = refund_outputs.len(),
            "💸 Executing tournament refund"
        );

        match payout_service.execute_tournament_refund(&escrow_address, refund_outputs).await {
            Ok(result) => {
                sqlx::query(
                    "UPDATE tournaments \
                     SET payout_tx_hash = $1, payout_executed_at = NOW(), updated_at = NOW() \
                     WHERE id = $2",
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
                .bind(serde_json::json!({ "tx_id": result.tx_id, "total_sompi": result.total_sompi }))
                .execute(pool)
                .await;

                tracing::info!(tournament_id = %tournament_id, tx_id = %result.tx_id, "✅ Refund TX submitted");
                count += 1;
            }
            Err(e) => {
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
