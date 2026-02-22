use crate::db::Database;
use battle_core::match_state::{transition, MatchAction, MatchState};
use battle_kaspa::watcher::BlockchainWatcher;
use std::sync::Arc;
use std::time::Duration;

/// Starts the blockchain watcher background task.
/// This task polls all active escrow addresses every `poll_interval` seconds,
/// detects deposits, and transitions matches to Locked when both players deposit.
pub fn start_watcher_task(
    db: Arc<Database>,
    watcher: Arc<BlockchainWatcher>,
    poll_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        log::info!(
            "🔍 Blockchain Watcher gestartet (Intervall: {:?})",
            poll_interval
        );
        loop {
            match watch_cycle(&db, &watcher).await {
                Ok(checked) => {
                    if checked > 0 {
                        log::debug!("Checked {} active escrows", checked);
                    }
                }
                Err(e) => {
                    log::error!("Watcher error: {}", e);
                }
            }
            tokio::time::sleep(poll_interval).await;
        }
    })
}

async fn watch_cycle(
    db: &Database,
    watcher: &BlockchainWatcher,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Get all matches waiting for deposits
    let active_escrows = db.get_active_escrow_addresses().await?;
    let count = active_escrows.len();

    for escrow in &active_escrows {
        // Check on-chain balance
        let status = match watcher.check_escrow_balance(&escrow.escrow_address).await {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "Failed to check escrow {} for match {}: {}",
                    escrow.escrow_address,
                    escrow.match_id,
                    e
                );
                continue;
            }
        };

        let wager = escrow.wager_sompi as u64;
        let eval = watcher.evaluate_deposits(&status, wager);

        // Check if deposit status changed
        let a_changed = eval.player_a_deposited && !escrow.player_a_deposited;
        let b_changed = eval.player_b_deposited && !escrow.player_b_deposited;

        if a_changed || b_changed {
            // Update deposit status in DB
            if let Err(e) = db
                .update_deposit_status(
                    &escrow.match_id,
                    eval.player_a_deposited,
                    eval.player_b_deposited,
                )
                .await
            {
                log::error!(
                    "Failed to update deposit status for {}: {}",
                    escrow.match_id,
                    e
                );
                continue;
            }

            if a_changed {
                log::info!(
                    "💰 Player A ({}) deposit detected for match {} ({} sompi)",
                    escrow.player_a_id,
                    escrow.match_id,
                    wager
                );
                // Record the deposit
                if let Err(e) = db
                    .record_deposit(&escrow.match_id, &escrow.player_a_id, None, wager as i64)
                    .await
                {
                    log::error!("Failed to record deposit: {}", e);
                }
            }

            if b_changed {
                if let Some(ref player_b_id) = escrow.player_b_id {
                    log::info!(
                        "💰 Player B ({}) deposit detected for match {} ({} sompi)",
                        player_b_id,
                        escrow.match_id,
                        wager
                    );
                    if let Err(e) = db
                        .record_deposit(&escrow.match_id, player_b_id, None, wager as i64)
                        .await
                    {
                        log::error!("Failed to record deposit: {}", e);
                    }
                }
            }
        }

        // If both deposited, transition to Locked
        if eval.both_deposited && (!escrow.player_a_deposited || !escrow.player_b_deposited) {
            log::info!(
                "🔒 Both players deposited for match {} – locking!",
                escrow.match_id
            );

            // Deserialize current state
            let _current_state: MatchState = match serde_json::from_str(&escrow.state_json) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to deserialize state for {}: {}", escrow.match_id, e);
                    continue;
                }
            };

            // Create a DepositConfirmed action for the final confirming player
            // Since both are now deposited, we can use player_a_id as the confirming player
            // if they weren't already deposited, or player_b_id
            let confirming_player = if !escrow.player_a_deposited {
                escrow.player_a_id.clone()
            } else if let Some(ref pb) = escrow.player_b_id {
                pb.clone()
            } else {
                continue;
            };

            let _action = MatchAction::DepositConfirmed {
                player_id: confirming_player,
                tx_hash: "watcher-detected".to_string(),
                amount: wager,
            };

            // We need to handle sequential deposit confirmations if needed
            // First ensure both deposit flags are set in state
            let intermediate_state = MatchState::WaitingForDeposits {
                player_a_deposited: true,
                player_b_deposited: false,
            };

            // Apply the second deposit to transition to Locked
            let action_b = MatchAction::DepositConfirmed {
                player_id: escrow.player_b_id.clone().unwrap_or_default(),
                tx_hash: "watcher-detected".to_string(),
                amount: wager,
            };

            let new_state = match transition(
                &intermediate_state,
                &action_b,
                &escrow.player_a_id,
                escrow.player_b_id.as_deref(),
            ) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("State transition failed for {}: {}", escrow.match_id, e);
                    continue;
                }
            };

            if let Err(e) = db.update_match_state(&escrow.match_id, &new_state).await {
                log::error!(
                    "Failed to update match state for {}: {}",
                    escrow.match_id,
                    e
                );
                continue;
            }

            log::info!(
                "✅ Match {} is now LOCKED – both players deposited! (total: {} sompi)",
                escrow.match_id,
                status.total_balance
            );
        }
    }

    Ok(count)
}
