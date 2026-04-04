//! # Payout Worker (Phase 4a)
//!
//! Background Tokio task that polls `FINISHED_FACEIT` matches and:
//! 1. Loads the winner's Kaspa address from the `users` table.
//! 2. Calls `MultisigEscrowService::create_pskt()`.
//! 3. Persists the PSKT hex to `matches.payout_pskt_hex`.
//! 4. Transitions the match to `READY_FOR_PAYOUT`.
//!
//! ## Configuration
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `PAYOUT_WORKER_POLL_INTERVAL_SECS` | `15` | Interval between poll cycles |

use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Starts the payout worker loop. Never returns.
///
/// Spawn via `tokio::spawn(run_payout_worker(...))`.
pub async fn run_payout_worker(
    pool: Arc<PgPool>,
    multisig_service: Arc<battle_kaspa::multisig::service::MultisigEscrowService>,
) {
    let poll_secs = std::env::var("PAYOUT_WORKER_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15);

    info!("💳 Payout Worker started (poll_interval={}s)", poll_secs);

    loop {
        if let Err(e) = poll_finished_matches(&pool, &multisig_service).await {
            error!(error = %e, "Payout Worker: error during poll cycle");
        }
        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
    }
}

/// Loads all matches in `FINISHED_FACEIT` status (= winner known, PSKT pending).
async fn poll_finished_matches(
    pool: &PgPool,
    multisig_service: &battle_kaspa::multisig::service::MultisigEscrowService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rows = sqlx::query(
        "SELECT m.id, m.winner_user_id, m.wager_sompi, m.escrow_address, \
         u.kaspa_address AS winner_kaspa_address, \
         me.pubkey_a_hex, me.pubkey_b_hex, me.pubkey_oracle_hex, \
         me.redeem_script_hex, me.p2sh_address AS escrow_p2sh, \
         me.wager_per_player_sompi, me.timelock_timestamp, m.faceit_finished_at \
         FROM matches m \
         JOIN users u ON u.id = m.winner_user_id \
         LEFT JOIN multisig_escrows me ON me.match_id = m.id \
         WHERE m.status = 'FINISHED_FACEIT' \
         AND m.winner_user_id IS NOT NULL \
         AND m.payout_pskt_hex IS NULL \
         ORDER BY m.created_at ASC \
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    if !rows.is_empty() {
        info!(count = rows.len(), "Payout Worker: processing FINISHED_FACEIT matches");
    }

    for row in rows {
        let match_id: Uuid = row.try_get("id")?;
        let winner_kaspa_address: Option<String> = row.try_get("winner_kaspa_address").ok().flatten();
        let pubkey_a_hex: Option<String> = row.try_get("pubkey_a_hex").ok().flatten();
        let pubkey_b_hex: Option<String> = row.try_get("pubkey_b_hex").ok().flatten();
        let pubkey_oracle_hex: Option<String> = row.try_get("pubkey_oracle_hex").ok().flatten();
        let redeem_script_hex: Option<String> = row.try_get("redeem_script_hex").ok().flatten();
        let escrow_p2sh: Option<String> = row.try_get("escrow_p2sh").ok().flatten();
        let wager_per_player_sompi: Option<i64> = row.try_get("wager_per_player_sompi").ok().flatten();
        let timelock_timestamp: Option<i64> = row.try_get("timelock_timestamp").ok().flatten();
        let faceit_finished_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("faceit_finished_at").ok().flatten();

        // Check if stuck in FINISHED_FACEIT for > 24 hours
        if let Some(finished_at) = faceit_finished_at {
            if (chrono::Utc::now() - finished_at).num_hours() > 24 {
                warn!(
                    match_id = %match_id,
                    "Payout Worker: match stuck in FINISHED_FACEIT > 24h. Moving to DISPUTED"
                );
                let _ = sqlx::query(
                    "UPDATE matches SET status = 'DISPUTED', payout_status = 'pending_dispute_resolution' WHERE id = $1 AND status = 'FINISHED_FACEIT'",
                )
                .bind(match_id)
                .execute(pool)
                .await;
                continue;
            }
        }

        // Validate winner address
        let winner_address = match winner_kaspa_address {
            Some(addr) if !addr.is_empty() => addr,
            _ => {
                warn!(
                    match_id = %match_id,
                    "Payout Worker: winner has no Kaspa address — skipping (will retry)"
                );
                continue;
            }
        };

        // Ensure escrow is loaded in-memory — restore from DB if missing
        if let (Some(pk_a), Some(pk_b), Some(pk_oracle), Some(rs_hex), Some(p2sh), Some(wager)) = (
            pubkey_a_hex,
            pubkey_b_hex,
            pubkey_oracle_hex,
            redeem_script_hex,
            escrow_p2sh,
            wager_per_player_sompi,
        ) {
            if multisig_service.get_escrow(&match_id).await.is_none() {
                multisig_service
                    .restore_escrow_from_row(
                        match_id,
                        pk_a,
                        pk_b,
                        pk_oracle,
                        rs_hex,
                        p2sh,
                        wager as u64,
                        timelock_timestamp.map(|v| v as u64),
                    )
                    .await;
                info!(match_id = %match_id, "Payout Worker: escrow restored from DB");
            }
        } else {
            warn!(
                match_id = %match_id,
                "Payout Worker: no multisig_escrows row found — using legacy EscrowService path"
            );
            // Legacy matches without multisig escrow → escalate to DISPUTED
            if let Err(e) = escalate_legacy_to_disputed(pool, match_id).await {
                error!(match_id = %match_id, error = %e, "Payout Worker: legacy escalation to DISPUTED failed");
            }
            continue;
        }

        // Create PSKT
        match multisig_service.create_pskt(&match_id, &winner_address).await {
            Ok(pskt) => {
                info!(
                    match_id = %match_id,
                    winner_address = %winner_address,
                    winner_amount_sompi = pskt.winner_amount_sompi,
                    "✅ Payout Worker: PSKT created"
                );

                // Persist PSKT and transition to READY_FOR_PAYOUT (atomic)
                let rows_affected = sqlx::query(
                    "UPDATE matches \
                     SET payout_pskt_hex = $1, \
                         payout_status = 'pending_winner_sig', \
                         status = 'READY_FOR_PAYOUT' \
                     WHERE id = $2 AND status = 'FINISHED_FACEIT'",
                )
                .bind(&pskt.pskt_hex)
                .bind(match_id)
                .execute(pool)
                .await?
                .rows_affected();

                if rows_affected > 0 {
                    info!(match_id = %match_id, "✅ Payout Worker: match → READY_FOR_PAYOUT");
                } else {
                    warn!(
                        match_id = %match_id,
                        "Payout Worker: match was not in FINISHED_FACEIT when writing PSKT (concurrent update?)"
                    );
                }
            }
            Err(e) => {
                error!(
                    match_id = %match_id,
                    winner_address = %winner_address,
                    error = %e,
                    "Payout Worker: create_pskt failed — will retry next cycle"
                );
                // Non-fatal: log and continue. The match stays in FINISHED_FACEIT
                // so we'll retry on the next poll cycle.
            }
        }
    }
    Ok(())
}

/// Legacy escalation: moves non-multisig matches to DISPUTED instead of
/// creating fake PSKTs. These matches need manual admin resolution.
async fn escalate_legacy_to_disputed(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sqlx::query(
        "UPDATE matches SET status = 'DISPUTED', payout_status = 'legacy_unsupported' \
         WHERE id = $1 AND status = 'FINISHED_FACEIT'",
    )
    .bind(match_id)
    .execute(pool)
    .await?;

    warn!(
        match_id = %match_id,
        "Payout Worker: legacy match without multisig escrow → DISPUTED (requires admin resolution)"
    );
    Ok(())
}
