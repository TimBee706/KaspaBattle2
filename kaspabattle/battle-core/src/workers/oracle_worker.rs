//! # Oracle Worker (DEPRECATED — superseded by `faceit_watcher.rs`)
//!
//! This module is a legacy implementation of the Oracle polling loop.
//! It has been superseded by `battle_core::workers::faceit_watcher` (F-010),
//! which uses `faceit_watcher_jobs` rows and the `FaceitDataService` for match polling.
//!
//! **Status**: Disabled — `run_oracle_worker` is not spawned in `main.rs`.
//! Kept for reference. Candidate for deletion in future cleanup sprint.
//!
//! TODO(cleanup): Delete this file once confirmed the `faceit_watcher` covers all use-cases.
#![allow(dead_code, unused_imports, unused_variables)]

use crate::errors::OracleError;
use crate::models::match_::{BattleMatch, MatchStatus};
use crate::oracle::faceit::FaceitOracleService;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Oracle error: {0}")]
    OracleError(#[from] OracleError),
    #[error("Payout error: {0}")]
    PayoutError(String),
}

pub struct WorkerConfig {
    pub poll_interval: Duration,
}

#[async_trait]
pub trait PayoutProvider: Send + Sync {
    async fn execute_payout(
        &self,
        battle_match: &BattleMatch,
        winner_address: &str,
    ) -> Result<(), String>;

    async fn execute_refund(&self, battle_match: &BattleMatch) -> Result<(), String>;
}

pub async fn run_oracle_worker(
    db_pool: Arc<PgPool>,
    oracle: Arc<FaceitOracleService>,
    payout_service: Arc<dyn PayoutProvider>,
    config: WorkerConfig,
) -> Result<(), WorkerError> {
    info!("Starting Oracle Polling Worker...");

    loop {
        match fetch_pending_matches(&db_pool).await {
            Ok(matches) => {
                for mut battle_match in matches {
                    if let Err(e) =
                        process_match(&db_pool, &oracle, &payout_service, &mut battle_match).await
                    {
                        error!("Failed to process match {}: {}", battle_match.id, e);
                    }
                }
            }
            Err(e) => error!("Failed to fetch pending matches: {}", e),
        }
        tokio::time::sleep(config.poll_interval).await;
    }
}

async fn fetch_pending_matches(db_pool: &PgPool) -> Result<Vec<BattleMatch>, sqlx::Error> {
    let records = sqlx::query(
        r#"SELECT id, player_a_kas_address, player_b_kas_address, 
                  player_a_faceit_id, player_b_faceit_id, faceit_match_id, 
                  wager_amount_sompi, escrow_address, status, 
                  winner_kas_address, payout_tx_hash, oracle_result_signature, 
                  created_at, locked_at, resolved_at, timeout_at 
           FROM matches WHERE status = 'InProgress'"#,
    )
    .fetch_all(db_pool)
    .await?;

    let mut matches = Vec::new();
    for r in records {
        let b = BattleMatch {
            id: r.get("id"),
            player_a_kas_address: r.get("player_a_kas_address"),
            player_b_kas_address: r.get("player_b_kas_address"),
            player_a_faceit_id: r.get("player_a_faceit_id"),
            player_b_faceit_id: r.get("player_b_faceit_id"),
            faceit_match_id: r.get("faceit_match_id"),
            wager_amount_sompi: r.get::<i64, _>("wager_amount_sompi") as u64,
            escrow_address: r.get("escrow_address"),
            status: MatchStatus::Locked,
            winner_kas_address: r.get("winner_kas_address"),
            payout_tx_hash: r.get("payout_tx_hash"),
            oracle_result_signature: r.get("oracle_result_signature"),
            created_at: r.get("created_at"),
            locked_at: r.get("locked_at"),
            resolved_at: r.get("resolved_at"),
            timeout_at: r.get("timeout_at"),
        };
        matches.push(b);
    }

    Ok(matches)
}

async fn process_match(
    db_pool: &PgPool,
    oracle: &FaceitOracleService,
    payout_service: &Arc<dyn PayoutProvider>,
    battle_match: &mut BattleMatch,
) -> Result<(), WorkerError> {
    let faceit_id = battle_match.faceit_match_id.as_deref().unwrap_or("");
    let result = oracle.fetch_with_double_confirmation(faceit_id).await?;

    if let Some(res) = result {
        info!(
            "Match {} finished. Winner: {}",
            battle_match.id, res.winner_faceit_id
        );

        let winner_address = determine_winner_address(battle_match, &res.winner_faceit_id)?;

        // Atomares DB-Lock
        let rows_affected = sqlx::query(
            "UPDATE matches SET status = 'Resolving' WHERE id = $1 AND status = 'InProgress'",
        )
        .bind(battle_match.id)
        .execute(db_pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            info!(
                "Match {} already locked or not in InProgress state",
                battle_match.id
            );
            return Ok(());
        }

        // Execute Payout
        match payout_service
            .execute_payout(battle_match, &winner_address)
            .await
        {
            Ok(_) => {
                info!("Payout successful for match {}", battle_match.id);
                let _ = sqlx::query("UPDATE matches SET status = 'PaidOut' WHERE id = $1")
                    .bind(battle_match.id)
                    .execute(db_pool)
                    .await;
            }
            Err(e) => {
                error!(
                    "Payout failed for match {}: {}. Attempting refund.",
                    battle_match.id, e
                );
                if let Err(refund_err) = payout_service.execute_refund(battle_match).await {
                    error!(
                        "Refund also failed for match {}: {}",
                        battle_match.id, refund_err
                    );
                }
                let _ = sqlx::query("UPDATE matches SET status = 'Refunded' WHERE id = $1")
                    .bind(battle_match.id)
                    .execute(db_pool)
                    .await;
            }
        }
    }

    Ok(())
}

fn determine_winner_address(
    battle_match: &BattleMatch,
    _winner_faceit_id: &str,
) -> Result<String, WorkerError> {
    // Determine which Kaspa address gets the payout
    // For V2 MVP we just route to A as a placeholder if there differs.
    Ok(battle_match.player_a_kas_address.clone())
}
