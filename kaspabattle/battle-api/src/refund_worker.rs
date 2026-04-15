//! # Refund Worker
//!
//! Background Tokio task that polls `CANCELLED` and `DISPUTED` matches and:
//! 1. Restores the multisig escrow from DB if not in memory.
//! 2. Calls `MultisigEscrowService::execute_refund()`.
//! 3. Persists the refund TX hash and status to the `matches` table.
//! 4. Transitions the match to `REFUNDED`.
//!
//! ## Configuration
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `REFUND_WORKER_POLL_INTERVAL_SECS` | `30` | Interval between poll cycles |

use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Starts the refund worker loop. Never returns.
///
/// Spawn via `tokio::spawn(run_refund_worker(...))`.
pub async fn run_refund_worker(
    pool: Arc<PgPool>,
    multisig_service: Arc<battle_kaspa::multisig::service::MultisigEscrowService>,
) {
    let poll_secs = std::env::var("REFUND_WORKER_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    info!("💸 Refund Worker started (poll_interval={}s)", poll_secs);

    loop {
        if let Err(e) = poll_refundable_matches(&pool, &multisig_service).await {
            error!(error = %e, "Refund Worker: error during poll cycle");
        }
        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
    }
}

/// Loads all matches that are eligible for automatic refund.
///
/// Selection criteria:
/// - Status is CANCELLED or DISPUTED
/// - refund_status is 'none' or 'failed' (not yet refunded / previous failure)
/// - No payout has been executed (payout_tx_hash IS NULL)
async fn poll_refundable_matches(
    pool: &PgPool,
    multisig_service: &battle_kaspa::multisig::service::MultisigEscrowService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rows = sqlx::query(
        "SELECT m.id, m.status, m.creator_user_id, m.opponent_user_id, \
         ua.kaspa_address AS player_a_kas, \
         ub.kaspa_address AS player_b_kas, \
         me.pubkey_a_hex, me.pubkey_b_hex, me.pubkey_oracle_hex, \
         me.redeem_script_hex, me.p2sh_address, \
         me.wager_per_player_sompi, me.timelock_timestamp \
         FROM matches m \
         JOIN users ua ON ua.id = m.creator_user_id \
         LEFT JOIN users ub ON ub.id = m.opponent_user_id \
         LEFT JOIN multisig_escrows me ON me.match_id = m.id \
         WHERE m.status IN ('CANCELLED', 'DISPUTED') \
         AND COALESCE(m.refund_status, 'none') IN ('none', 'failed') \
         AND m.payout_pskt_hex IS NULL \
         AND m.payout_tx_hash IS NULL \
         ORDER BY m.cancelled_at ASC NULLS LAST \
         LIMIT 50",
    )
    .fetch_all(pool)
    .await?;

    if !rows.is_empty() {
        info!(count = rows.len(), "Refund Worker: processing refundable matches");
    }

    for row in rows {
        let match_id: Uuid = row.try_get("id")?;
        let player_a_kas: Option<String> = row.try_get("player_a_kas").ok().flatten();
        let player_b_kas: Option<String> = row.try_get("player_b_kas").ok().flatten();
        let pubkey_a_hex: Option<String> = row.try_get("pubkey_a_hex").ok().flatten();
        let pubkey_b_hex: Option<String> = row.try_get("pubkey_b_hex").ok().flatten();
        let pubkey_oracle_hex: Option<String> = row.try_get("pubkey_oracle_hex").ok().flatten();
        let redeem_script_hex: Option<String> = row.try_get("redeem_script_hex").ok().flatten();
        let p2sh_address: Option<String> = row.try_get("p2sh_address").ok().flatten();
        let wager_per_player_sompi: Option<i64> = row.try_get("wager_per_player_sompi").ok().flatten();
        let timelock_timestamp: Option<i64> = row.try_get("timelock_timestamp").ok().flatten();

        // Validate player addresses
        let player_a_address = match player_a_kas {
            Some(addr) if !addr.is_empty() => addr,
            _ => {
                warn!(
                    match_id = %match_id,
                    "Refund Worker: player A has no Kaspa address — skipping (will retry)"
                );
                continue;
            }
        };

        // Player B might be None if match was cancelled before anyone joined
        let player_b_address = match player_b_kas {
            Some(addr) if !addr.is_empty() => addr,
            _ => {
                // Single-player refund: use player A's address for both outputs
                // (the refund TX builder will send the full balance to player A)
                player_a_address.clone()
            }
        };

        // Check if escrow data exists in DB
        if let (Some(pk_a), Some(pk_b), Some(pk_oracle), Some(rs_hex), Some(p2sh), Some(wager)) = (
            pubkey_a_hex,
            pubkey_b_hex,
            pubkey_oracle_hex,
            redeem_script_hex,
            p2sh_address,
            wager_per_player_sompi,
        ) {
            // Restore escrow in-memory if needed
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
                info!(match_id = %match_id, "Refund Worker: escrow restored from DB");
            }
        } else {
            // No multisig escrow row → nothing to refund on-chain
            // Mark as refunded so we don't keep polling
            info!(
                match_id = %match_id,
                "Refund Worker: no multisig_escrows row — marking refund_status='success' (no on-chain funds)"
            );
            let _ = sqlx::query(
                "UPDATE matches SET refund_status = 'success' WHERE id = $1",
            )
            .bind(match_id)
            .execute(pool)
            .await;
            continue;
        }

        // Execute refund via MultisigEscrowService
        match multisig_service
            .execute_refund(&match_id, &player_a_address, &player_b_address)
            .await
        {
            Ok(result) => {
                let tx_hash = if result.tx_id.is_empty() {
                    None
                } else {
                    Some(result.tx_id.clone())
                };

                info!(
                    match_id = %match_id,
                    tx_hash = ?tx_hash,
                    refund_amount_sompi = result.winner_amount_sompi,
                    network_fee = result.network_fee_sompi,
                    "✅ Refund Worker: refund executed"
                );

                // Persist refund result and transition to REFUNDED
                let rows_affected = sqlx::query(
                    "UPDATE matches \
                     SET refund_tx_hash = $1, \
                         refund_status = 'success', \
                         status = 'REFUNDED' \
                     WHERE id = $2 AND status IN ('CANCELLED', 'DISPUTED')",
                )
                .bind(&tx_hash)
                .bind(match_id)
                .execute(pool)
                .await?
                .rows_affected();

                if rows_affected > 0 {
                    info!(match_id = %match_id, "✅ Refund Worker: match → REFUNDED");
                } else {
                    warn!(
                        match_id = %match_id,
                        "Refund Worker: match status changed between poll and update (concurrent?)"
                    );
                }
            }
            Err(e) => {
                let err_msg = e.to_string();

                // If node is not synced, don't mark as failed — just log and retry
                if err_msg.contains("not synced") {
                    warn!(
                        match_id = %match_id,
                        "Refund Worker: node not synced — will retry next cycle"
                    );
                    continue;
                }

                error!(
                    match_id = %match_id,
                    error = %e,
                    "Refund Worker: execute_refund failed — marking refund_status='failed'"
                );

                // Mark as failed so we retry on next poll
                let _ = sqlx::query(
                    "UPDATE matches SET refund_status = 'failed' WHERE id = $1",
                )
                .bind(match_id)
                .execute(pool)
                .await;
            }
        }
    }

    Ok(())
}
