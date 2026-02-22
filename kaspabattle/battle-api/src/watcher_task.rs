use crate::db::Database;
use battle_core::match_state::{transition, MatchAction, MatchState};
use battle_core::models::match_::{BattleMatch, MatchStatus};
use battle_kaspa::payout::PayoutService;
use battle_kaspa::watcher::BlockchainWatcher;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Starts the blockchain watcher background task.
///
/// The task runs two sub-cycles on every iteration:
///   1. `watch_cycle`:   polls active escrow addresses for new deposits,
///                        transitions matches to Locked when both players deposited.
///   2. `timeout_cycle`: F-006 — detects matches with expired deposit windows,
///                        issues refunds and cancels them.
pub fn start_watcher_task(
    db: Arc<Database>,
    watcher: Arc<BlockchainWatcher>,
    payout: Arc<PayoutService>,
    poll_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        log::info!(
            "🔍 Blockchain Watcher gestartet (Intervall: {:?})",
            poll_interval
        );
        loop {
            // --- Deposit detection ---
            match watch_cycle(&db, &watcher).await {
                Ok(checked) => {
                    if checked > 0 {
                        log::debug!("Checked {} active escrows", checked);
                    }
                }
                Err(e) => {
                    log::error!("Watcher cycle error: {}", e);
                }
            }

            // --- F-006: Timeout / refund detection ---
            match timeout_cycle(&db, &payout).await {
                Ok(0) => {}
                Ok(n) => log::info!("⏰ Processed {} timed-out matches", n),
                Err(e) => log::error!("Timeout cycle error: {}", e),
            }

            tokio::time::sleep(poll_interval).await;
        }
    })
}

/// F-006: Check for matches that have been in WaitingForDeposits state
/// past their configured timeout, issue refunds, and cancel them.
///
/// For each timed-out match:
///   1. Retrieve the deposited amounts from the chain.
///   2. If any funds are present, trigger a refund via PayoutService.
///   3. Transition the match state to Cancelled { reason: "deposit_timeout" }.
///
/// Errors per match are logged and not propagated — the loop continues
/// with remaining matches.
async fn timeout_cycle(
    db: &Database,
    payout: &PayoutService,
) -> Result<usize, Box<dyn std::error::Error>> {
    let now = Utc::now().naive_utc().to_string();

    // Find all matches waiting for deposits where timeout_at has passed
    let timed_out = db.get_timed_out_escrows(&now).await?;
    let count = timed_out.len();

    for escrow in &timed_out {
        log::warn!(
            "⏰ Match {} deposit timeout expired (timeout_at: {:?}). Processing refund.",
            escrow.match_id,
            escrow.timeout_at
        );

        // Build a minimal BattleMatch for the refund service.
        // F-009: Use Cancelled status (semantically correct for timeout).
        // Note: execute_refund() does NOT check allows_payout(), so this
        // status is for documentation only — the refund runs regardless.
        let battle_match = BattleMatch {
            id: Uuid::parse_str(&escrow.match_id).unwrap_or_default(),
            player_a_kas_address: escrow.player_a_addr.clone().unwrap_or_default(),
            player_b_kas_address: escrow.player_b_addr.clone().unwrap_or_default(),
            player_a_faceit_id: escrow.player_a_id.clone(),
            player_b_faceit_id: escrow.player_b_id.clone().unwrap_or_default(),
            faceit_match_id: None,
            wager_amount_sompi: escrow.wager_sompi as u64,
            escrow_address: escrow.escrow_address.clone(),
            status: MatchStatus::Cancelled,
            winner_kas_address: None,
            payout_tx_hash: None,
            oracle_result_signature: None,
            created_at: Utc::now(),
            locked_at: None,
            resolved_at: None,
            timeout_at: Utc::now(),
        };

        // Attempt refund — this sends any deposited KAS back to the players
        match payout.execute_refund(&battle_match).await {
            Ok((tx_id_a, tx_id_b)) => {
                log::info!(
                    "✅ Refund for timed-out match {}: tx_a={}, tx_b={}",
                    escrow.match_id,
                    tx_id_a,
                    tx_id_b
                );
                // Record refund in the escrows table
                if let Err(e) = db
                    .update_escrow_refund(
                        &escrow.match_id,
                        Some(&tx_id_a),
                        Some(&tx_id_b),
                    )
                    .await
                {
                    log::error!(
                        "Failed to record refund for match {}: {}",
                        escrow.match_id,
                        e
                    );
                }
            }
            Err(e) => {
                log::error!(
                    "Refund failed for timed-out match {} (will retry next cycle): {}",
                    escrow.match_id,
                    e
                );
                // Do NOT cancel the match yet — retry on next cycle
                continue;
            }
        }

        // Transition match state to Cancelled
        let cancel_state = MatchState::Cancelled {
            reason: "deposit_timeout".to_string(),
        };
        if let Err(e) = db.update_match_state(&escrow.match_id, &cancel_state).await {
            log::error!(
                "Failed to cancel timed-out match {}: {}",
                escrow.match_id,
                e
            );
        } else {
            log::info!("Match {} cancelled due to deposit timeout.", escrow.match_id);
        }
    }

    Ok(count)
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

        // F-008: Use per-address deposit evaluation
        let player_a_addr = escrow.player_a_addr.as_deref().unwrap_or("");
        let player_b_addr = escrow.player_b_addr.as_deref().unwrap_or("");
        let eval = watcher.evaluate_deposits(&status, wager, player_a_addr, player_b_addr);

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

            let intermediate_state = MatchState::WaitingForDeposits {
                player_a_deposited: true,
                player_b_deposited: false,
            };

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
