//! BattleHandler — bridges on-chain Episode events to the PostgreSQL database
//! and WebSocket broadcast channel.
//!
//! Each `on_command` call maps to a SQL UPSERT/UPDATE that keeps the `matches`
//! and `payments` tables in sync with the on-chain Episode state. A JSON
//! notification is broadcast via the WebSocket channel so the frontend can
//! react in real-time.

use crate::commands::{BattleCommand, MatchPhase};
use crate::episode::BattleEpisode;
use crate::kdapp_episode::{EpisodeEventHandler, EpisodeId, PayloadMetadata};
use crate::kdapp_pki::PubKey;

use sqlx::PgPool;
use tokio::sync::broadcast;

/// Event handler bridging on-chain Episode events to off-chain systems.
///
/// Holds a database connection pool and a WebSocket broadcast sender.
/// The handler is called synchronously by the Engine (on a blocking thread),
/// so all async DB operations are dispatched via `tokio::runtime::Handle`.
#[derive(Clone)]
pub struct BattleHandler {
    pool: PgPool,
    ws_tx: broadcast::Sender<String>,
    /// Tokio runtime handle for spawning async DB work from sync context
    rt_handle: tokio::runtime::Handle,
}

impl BattleHandler {
    pub fn new(pool: PgPool, ws_tx: broadcast::Sender<String>) -> Self {
        Self {
            pool,
            ws_tx,
            rt_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Broadcast a JSON event to all connected WebSocket clients.
    fn broadcast(&self, event_type: &str, episode_id: EpisodeId, data: serde_json::Value) {
        let msg = serde_json::json!({
            "type": event_type,
            "episode_id": episode_id,
            "data": data,
        });
        if let Err(e) = self.ws_tx.send(msg.to_string()) {
            log::debug!("WS broadcast failed (no receivers): {}", e);
        }
    }

    /// Execute an async block on the tokio runtime from sync context.
    /// Used because the Engine calls handlers from a blocking thread.
    fn block_on<F: std::future::Future<Output = T>, T>(&self, f: F) -> T {
        self.rt_handle.block_on(f)
    }

    // ── DB operations per command ─────────────────────────────────────────

    fn handle_create_match(
        &self,
        episode_id: EpisodeId,
        episode: &BattleEpisode,
    ) {
        let pool = self.pool.clone();
        let wager = episode.wager_sompi;
        let game_type = format!("{:?}", episode.game_type);
        let game_type_ws = game_type.clone();

        self.block_on(async move {
            let result = sqlx::query(
                "INSERT INTO matches (onchain_match_id, stake_kas, game_id, status, created_at) \
                 VALUES ($1, $2, $3, 'OPEN', NOW()) \
                 ON CONFLICT (onchain_match_id) DO UPDATE SET stake_kas = $2"
            )
            .bind(episode_id as i64)
            .bind(wager as i64)
            .bind(&game_type)
            .execute(&pool)
            .await;

            match result {
                Ok(_) => log::info!("DB: Match {} created (wager={} sompi)", episode_id, wager),
                Err(e) => log::error!("DB: Failed to create match {}: {}", episode_id, e),
            }
        });

        self.broadcast("match_created", episode_id, serde_json::json!({
            "wager_sompi": episode.wager_sompi,
            "game_type": game_type_ws,
        }));
    }

    fn handle_join_match(
        &self,
        episode_id: EpisodeId,
        _episode: &BattleEpisode,
    ) {
        let pool = self.pool.clone();

        self.block_on(async move {
            let result = sqlx::query(
                "UPDATE matches SET status = 'AWAITING_FUNDING' \
                 WHERE onchain_match_id = $1"
            )
            .bind(episode_id as i64)
            .execute(&pool)
            .await;

            match result {
                Ok(r) => log::info!("DB: Match {} joined ({} rows)", episode_id, r.rows_affected()),
                Err(e) => log::error!("DB: Failed to update join for match {}: {}", episode_id, e),
            }
        });

        self.broadcast("opponent_joined", episode_id, serde_json::json!({}));
    }

    fn handle_confirm_deposit(
        &self,
        episode_id: EpisodeId,
        episode: &BattleEpisode,
        amount_sompi: u64,
        authorization: Option<PubKey>,
    ) {
        let pool = self.pool.clone();
        let phase = episode.phase.clone();
        let is_locked = matches!(phase, MatchPhase::Locked);
        let auth_str = authorization.map(|pk| format!("{}", pk)).unwrap_or_default();

        self.block_on(async move {
            // Upsert payment record
            let result = sqlx::query(
                "INSERT INTO payments (match_id, player_role, amount_sompi, status, created_at) \
                 SELECT m.id, $2, $3, 'CONFIRMED', NOW() \
                 FROM matches m WHERE m.onchain_match_id = $1 \
                 ON CONFLICT (match_id, player_role) DO UPDATE SET \
                 amount_sompi = payments.amount_sompi + $3, status = 'CONFIRMED'"
            )
            .bind(episode_id as i64)
            .bind(&auth_str)
            .bind(amount_sompi as i64)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                log::error!("DB: Failed to upsert payment for match {}: {}", episode_id, e);
            }

            // If both deposits confirmed → update match status to LOCKED
            if is_locked {
                let _ = sqlx::query(
                    "UPDATE matches SET status = 'LOCKED' WHERE onchain_match_id = $1"
                )
                .bind(episode_id as i64)
                .execute(&pool)
                .await;
                log::info!("DB: Match {} locked (both deposits confirmed)", episode_id);
            }
        });

        self.broadcast("deposit_confirmed", episode_id, serde_json::json!({
            "amount_sompi": amount_sompi,
            "phase": format!("{:?}", phase),
        }));
    }

    fn handle_report_result(
        &self,
        episode_id: EpisodeId,
        episode: &BattleEpisode,
        score_a: u8,
        score_b: u8,
    ) {
        let pool = self.pool.clone();
        let winner_idx = episode.winner_idx;

        self.block_on(async move {
            let result = sqlx::query(
                "UPDATE matches SET status = 'RESOLVED' WHERE onchain_match_id = $1"
            )
            .bind(episode_id as i64)
            .execute(&pool)
            .await;

            match result {
                Ok(_) => log::info!("DB: Match {} resolved ({}:{})", episode_id, score_a, score_b),
                Err(e) => log::error!("DB: Failed to resolve match {}: {}", episode_id, e),
            }
        });

        self.broadcast("result_reported", episode_id, serde_json::json!({
            "score_a": score_a,
            "score_b": score_b,
            "winner_idx": winner_idx,
        }));
    }

    fn handle_payout(
        &self,
        episode_id: EpisodeId,
    ) {
        let pool = self.pool.clone();

        self.block_on(async move {
            let result = sqlx::query(
                "UPDATE matches SET status = 'PAID_OUT' WHERE onchain_match_id = $1"
            )
            .bind(episode_id as i64)
            .execute(&pool)
            .await;

            match result {
                Ok(_) => log::info!("DB: Match {} paid out", episode_id),
                Err(e) => log::error!("DB: Failed to mark payout for match {}: {}", episode_id, e),
            }
        });

        self.broadcast("payout_completed", episode_id, serde_json::json!({}));
    }

    fn handle_dispute(
        &self,
        episode_id: EpisodeId,
        reason_code: u8,
    ) {
        let pool = self.pool.clone();

        self.block_on(async move {
            let result = sqlx::query(
                "UPDATE matches SET status = 'DISPUTED' WHERE onchain_match_id = $1"
            )
            .bind(episode_id as i64)
            .execute(&pool)
            .await;

            match result {
                Ok(_) => log::info!("DB: Match {} disputed (reason={})", episode_id, reason_code),
                Err(e) => log::error!("DB: Failed to dispute match {}: {}", episode_id, e),
            }
        });

        self.broadcast("match_disputed", episode_id, serde_json::json!({
            "reason_code": reason_code,
        }));
    }

    fn handle_cancel(
        &self,
        episode_id: EpisodeId,
        reason_code: u8,
    ) {
        let pool = self.pool.clone();

        self.block_on(async move {
            let result = sqlx::query(
                "UPDATE matches SET status = 'CANCELLED' WHERE onchain_match_id = $1"
            )
            .bind(episode_id as i64)
            .execute(&pool)
            .await;

            match result {
                Ok(_) => log::info!("DB: Match {} cancelled (reason={})", episode_id, reason_code),
                Err(e) => log::error!("DB: Failed to cancel match {}: {}", episode_id, e),
            }
        });

        self.broadcast("match_cancelled", episode_id, serde_json::json!({
            "reason_code": reason_code,
        }));
    }
}

impl EpisodeEventHandler<BattleEpisode> for BattleHandler {
    fn on_initialize(&self, episode_id: EpisodeId, episode: &BattleEpisode) {
        log::info!(
            "Episode {} initialized: phase={:?}, participants={}",
            episode_id, episode.phase, episode.participants.len(),
        );
    }

    fn on_command(
        &self,
        episode_id: EpisodeId,
        episode: &BattleEpisode,
        cmd: &BattleCommand,
        authorization: Option<PubKey>,
        _metadata: &PayloadMetadata,
    ) {
        log::info!(
            "Episode {} command: {:?}, phase={:?}",
            episode_id, cmd, episode.phase,
        );

        match cmd {
            BattleCommand::CreateMatch { .. } => {
                self.handle_create_match(episode_id, episode);
            }
            BattleCommand::JoinMatch => {
                self.handle_join_match(episode_id, episode);
            }
            BattleCommand::ConfirmDeposit { amount_sompi, .. } => {
                self.handle_confirm_deposit(episode_id, episode, *amount_sompi, authorization);
            }
            BattleCommand::ReportResult { score_a, score_b, .. } => {
                self.handle_report_result(episode_id, episode, *score_a, *score_b);
            }
            BattleCommand::InitiatePayout => {
                self.handle_payout(episode_id);
            }
            BattleCommand::Dispute { reason_code } => {
                self.handle_dispute(episode_id, *reason_code);
            }
            BattleCommand::CancelMatch { reason_code } => {
                self.handle_cancel(episode_id, *reason_code);
            }
        }
    }

    fn on_rollback(&self, episode_id: EpisodeId, episode: &BattleEpisode) {
        log::warn!(
            "Episode {} rolled back (DAG re-org): phase={:?}",
            episode_id, episode.phase,
        );

        // Revert DB status to match the rolled-back phase
        let pool = self.pool.clone();
        let new_status = match &episode.phase {
            MatchPhase::WaitingForOpponent => "OPEN",
            MatchPhase::WaitingForDeposits { .. } => "AWAITING_FUNDING",
            MatchPhase::Locked => "LOCKED",
            MatchPhase::Resolved { .. } => "RESOLVED",
            MatchPhase::Disputed { .. } => "DISPUTED",
            MatchPhase::Cancelled { .. } => "CANCELLED",
            MatchPhase::Completed => "PAID_OUT",
        };

        self.block_on(async move {
            let _ = sqlx::query(
                "UPDATE matches SET status = $1 WHERE onchain_match_id = $2"
            )
            .bind(new_status)
            .bind(episode_id as i64)
            .execute(&pool)
            .await;
        });

        self.broadcast("episode_rollback", episode_id, serde_json::json!({
            "reverted_to": new_status,
        }));
    }
}
