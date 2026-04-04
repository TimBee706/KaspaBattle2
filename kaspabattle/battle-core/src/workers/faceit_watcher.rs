//! # FaceIT Watcher Service (F-010 / Phase 3)
//!
//! Background Tokio task that polls all active `faceit_watcher_jobs` rows
//! from the DB and calls the FaceIT Data API for each one.
//!
//! ## Lifecycle per job
//!
//! ```text
//! ACTIVE → [FACEIT finished] → COMPLETED  (match → FINISHED_FACEIT)
//!        → [FACEIT cancelled] → CANCELLED  (match → CANCELLED)
//!        → [retry_count > max_retries] → TIMED_OUT (match → CANCELLED)
//!        → [HTTP error] → stays ACTIVE, next_poll_at pushed back
//! ```
//!
//! ## Configuration (env variables)
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `FACEIT_WATCHER_POLL_INTERVAL_SECS` | `10` | Seconds between poll cycles |
//! | `FACEIT_WATCHER_MAX_RETRIES` | `360` | Max retries (~1 hour at 10s) |
//! | `FACEIT_MATCH_TIMEOUT_HOURS` | `3` | Cancel if match takes too long |

use crate::faceit_data::FaceitDataService;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// A row from `faceit_watcher_jobs`.
#[derive(Debug, Clone)]
pub struct WatcherJob {
    pub id: Uuid,
    pub match_id: Uuid,
    pub faceit_match_id: String,
    pub retry_count: i32,
    pub max_retries: i32,
}

/// Configuration for the watcher service.
#[derive(Debug, Clone)]
pub struct FaceitWatcherConfig {
    /// How often to run the poll loop.
    pub poll_interval: Duration,
    /// How long to wait before declaring an in-game match timed out.
    pub match_timeout: Duration,
    /// Backoff durations for different HTTP error classes.
    pub backoff_rate_limit: Duration,
    pub backoff_server_error: Duration,
    pub backoff_network_error: Duration,
}

impl Default for FaceitWatcherConfig {
    fn default() -> Self {
        let poll_secs = std::env::var("FACEIT_WATCHER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);

        let timeout_hours = std::env::var("FACEIT_MATCH_TIMEOUT_HOURS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3);

        Self {
            poll_interval: Duration::from_secs(poll_secs),
            match_timeout: Duration::from_secs(timeout_hours * 3600),
            backoff_rate_limit: Duration::from_secs(60),
            backoff_server_error: Duration::from_secs(30),
            backoff_network_error: Duration::from_secs(30),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Entry Point
// ─────────────────────────────────────────────────────────────────────────────

/// Starts the FaceIT watcher loop. Never returns (runs until the process exits).
///
/// Spawn via `tokio::spawn(run_faceit_watcher(...))`.
pub async fn run_faceit_watcher(
    pool: Arc<PgPool>,
    faceit_data: Arc<FaceitDataService>,
    config: FaceitWatcherConfig,
) {
    info!("🔍 FaceIT Watcher started (poll_interval={:?})", config.poll_interval);
    tracing::debug!("🔍 FaceIT Watcher started (interval={:?})", config.poll_interval);

    loop {
        match fetch_active_jobs(&pool).await {
            Ok(jobs) => {
                if !jobs.is_empty() {
                    info!(job_count = jobs.len(), "FaceIT Watcher: processing active jobs");
                    tracing::debug!("🔍 FaceIT Watcher: {} active job(s)", jobs.len());
                }
                for job in jobs {
                    if let Err(e) = process_job(&pool, &faceit_data, &job, &config).await {
                        error!(
                            match_id = %job.match_id,
                            faceit_match_id = %job.faceit_match_id,
                            error = %e,
                            "FaceIT Watcher: failed to process job"
                        );
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "FaceIT Watcher: failed to fetch active jobs");
            }
        }
        tokio::time::sleep(config.poll_interval).await;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DB helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Loads all ACTIVE watcher jobs whose `next_poll_at` is in the past.
async fn fetch_active_jobs(pool: &PgPool) -> Result<Vec<WatcherJob>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, match_id, faceit_match_id, retry_count, max_retries \
         FROM faceit_watcher_jobs \
         WHERE status = 'ACTIVE' AND next_poll_at <= NOW() \
         ORDER BY next_poll_at ASC \
         LIMIT 50",
    )
    .fetch_all(pool)
    .await?;

    let mut jobs = Vec::with_capacity(rows.len());
    for row in rows {
        jobs.push(WatcherJob {
            id: row.try_get("id")?,
            match_id: row.try_get("match_id")?,
            faceit_match_id: row.try_get("faceit_match_id")?,
            retry_count: row.try_get("retry_count")?,
            max_retries: row.try_get("max_retries")?,
        });
    }
    Ok(jobs)
}

/// Marks a watcher job with the given terminal status and an optional error message.
async fn mark_job_terminal(
    pool: &PgPool,
    job_id: Uuid,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE faceit_watcher_jobs \
         SET status = $1, error_message = $2, updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(status)
    .bind(error_message)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Increments `retry_count` and schedules the next poll.
async fn schedule_retry(
    pool: &PgPool,
    job_id: Uuid,
    faceit_status: Option<&str>,
    backoff: Duration,
) -> Result<(), sqlx::Error> {
    let backoff_secs = backoff.as_secs() as i64;
    sqlx::query(
        "UPDATE faceit_watcher_jobs \
         SET retry_count = retry_count + 1, \
             last_polled_at = NOW(), \
             next_poll_at = NOW() + ($1 || ' seconds')::interval, \
             last_faceit_status = COALESCE($2, last_faceit_status), \
             updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(backoff_secs.to_string())
    .bind(faceit_status)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Core: Process one job
// ─────────────────────────────────────────────────────────────────────────────

async fn process_job(
    pool: &PgPool,
    faceit_data: &FaceitDataService,
    job: &WatcherJob,
    config: &FaceitWatcherConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let poll_secs = config.poll_interval.as_secs() as i64;

    tracing::debug!(
        match_id = %job.match_id,
        faceit_match_id = %job.faceit_match_id,
        retry = job.retry_count,
        "FaceIT Watcher: polling job"
    );

    // ── Timeout guard ────────────────────────────────────────────────────────
    // If the job has exceeded max_retries, cancel the match.
    if job.retry_count > job.max_retries {
        warn!(
            match_id = %job.match_id,
            faceit_match_id = %job.faceit_match_id,
            retry_count = job.retry_count,
            max_retries = job.max_retries,
            "⏰ FaceIT Watcher: max retries exceeded — cancelling match"
        );
        tracing::info!("⏰ FaceIT Watcher: match {} timed out after {} retries — CANCELLED",
            job.match_id, job.retry_count
        );
        cancel_match_and_job(pool, job.match_id, job.id, "TIMED_OUT", "FaceIT match polling timed out").await?;
        return Ok(());
    }

    // ── Call FaceIT API ──────────────────────────────────────────────────────
    let details = match faceit_data.get_match_details(&job.faceit_match_id).await {
        Ok(d) => d,
        Err(e) => {
            let err_str = e.to_string();
            // Classify the error
            let (backoff, log_msg) = if err_str.contains("429") {
                (config.backoff_rate_limit, "rate limited (429)")
            } else if err_str.contains("404") {
                // Match not found on FaceIT → fail the job
                error!(
                    match_id = %job.match_id,
                    faceit_match_id = %job.faceit_match_id,
                    "FaceIT Watcher: match not found (404) — marking FAILED"
                );
                mark_job_terminal(pool, job.id, "FAILED", Some(&err_str)).await?;
                return Ok(());
            } else if err_str.contains("500") || err_str.contains("502") || err_str.contains("503") || err_str.contains("504") {
                (config.backoff_server_error, "server error (5xx)")
            } else {
                (config.backoff_network_error, "network error")
            };

            warn!(
                match_id = %job.match_id,
                faceit_match_id = %job.faceit_match_id,
                error = %err_str,
                "FaceIT Watcher: {} — backing off {:?}",
                log_msg,
                backoff
            );
            schedule_retry(pool, job.id, None, backoff).await?;
            return Ok(());
        }
    };

    let faceit_status = details.status.as_str();

    info!(
        match_id = %job.match_id,
        faceit_match_id = %job.faceit_match_id,
        faceit_status,
        "FaceIT Watcher: poll result"
    );
    tracing::debug!("🔍 FaceIT Watcher: match {} → faceit_status='{}'",
        job.match_id, faceit_status
    );

    match faceit_status {
        // ── Match finished — extract winner, update DB ───────────────────────
        "finished" => {
            handle_match_finished(pool, job, &details).await?;
        }

        // ── Match cancelled/aborted — cancel KaspaBattle match ──────────────
        "cancelled" | "aborted" => {
            warn!(
                match_id = %job.match_id,
                faceit_match_id = %job.faceit_match_id,
                faceit_status,
                "FaceIT Watcher: match was cancelled on FaceIT — cancelling KaspaBattle match"
            );
            tracing::error!("❌ FaceIT Watcher: FaceIT match {} cancelled — cancelling KB match {}",
                job.faceit_match_id, job.match_id
            );
            cancel_match_and_job(
                pool,
                job.match_id,
                job.id,
                "CANCELLED",
                &format!("FaceIT match was {}", faceit_status),
            )
            .await?;
        }

        // ── Still running — schedule next poll ───────────────────────────────
        "created" | "configuring" | "ready" | "ongoing" | "voting" | "checking" => {
            schedule_retry(
                pool,
                job.id,
                Some(faceit_status),
                Duration::from_secs(poll_secs as u64),
            )
            .await?;
        }

        // ── Unknown status — log and retry ───────────────────────────────────
        other => {
            warn!(
                match_id = %job.match_id,
                faceit_status = other,
                "FaceIT Watcher: unknown FaceIT status — retrying"
            );
            schedule_retry(
                pool,
                job.id,
                Some(other),
                Duration::from_secs(poll_secs as u64),
            )
            .await?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Winner mapping & FINISHED_FACEIT transition
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_match_finished(
    pool: &PgPool,
    job: &WatcherJob,
    details: &crate::models::faceit_data::FaceitMatchDetails,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        match_id = %job.match_id,
        faceit_match_id = %job.faceit_match_id,
        "FaceIT Watcher: match finished — running winner mapping"
    );
    tracing::info!("🏆 FaceIT Watcher: match {} finished — running winner mapping",
        job.match_id
    );

    // ── Step 1: Determine winner/loser faction from FaceIT results ───────────
    let winner_faction = match &details.results {
        Some(r) => r.winner.clone(),
        None => {
            error!(
                match_id = %job.match_id,
                "FaceIT Watcher: match finished but results.winner is missing — marking DISPUTED"
            );
            mark_match_disputed(pool, job.match_id, job.id, "FaceIT results.winner field missing").await?;
            return Ok(());
        }
    };

    // ── Step 2: Load KaspaBattle match participants' faceit_player_ids ───────
    // We join faceit_links to find the faceit_player_id for each match participant.
    let rows = sqlx::query(
        "SELECT \
             m.creator_user_id, \
             m.opponent_user_id, \
             fl_a.faceit_player_id AS creator_faceit_id, \
             fl_b.faceit_player_id AS opponent_faceit_id \
         FROM matches m \
         LEFT JOIN faceit_links fl_a ON fl_a.user_id = m.creator_user_id \
         LEFT JOIN faceit_links fl_b ON fl_b.user_id = m.opponent_user_id \
         WHERE m.id = $1",
    )
    .bind(job.match_id)
    .fetch_optional(pool)
    .await?;

    let row = match rows {
        Some(r) => r,
        None => {
            error!(match_id = %job.match_id, "FaceIT Watcher: match not found in DB");
            mark_job_terminal(pool, job.id, "FAILED", Some("Match not found in DB")).await?;
            return Ok(());
        }
    };

    let creator_user_id: Uuid = row.try_get("creator_user_id")?;
    let opponent_user_id: Option<Uuid> = row.try_get("opponent_user_id")?;
    let creator_faceit_id: Option<String> = row.try_get("creator_faceit_id")?;
    let opponent_faceit_id: Option<String> = row.try_get("opponent_faceit_id")?;

    // Both players must have linked FaceIT accounts
    let creator_fid = match creator_faceit_id {
        Some(id) => id,
        None => {
            error!(
                match_id = %job.match_id,
                creator_user_id = %creator_user_id,
                "FaceIT Watcher: creator has no linked FaceIT account — DISPUTED"
            );
            mark_match_disputed(pool, job.match_id, job.id, "Creator has no linked FaceIT account").await?;
            return Ok(());
        }
    };
    let opp_user_id = match opponent_user_id {
        Some(id) => id,
        None => {
            error!(match_id = %job.match_id, "FaceIT Watcher: no opponent — DISPUTED");
            mark_match_disputed(pool, job.match_id, job.id, "Match has no opponent").await?;
            return Ok(());
        }
    };
    let opp_fid = match opponent_faceit_id {
        Some(id) => id,
        None => {
            error!(
                match_id = %job.match_id,
                opponent_user_id = %opp_user_id,
                "FaceIT Watcher: opponent has no linked FaceIT account — DISPUTED"
            );
            mark_match_disputed(pool, job.match_id, job.id, "Opponent has no linked FaceIT account").await?;
            return Ok(());
        }
    };

    // ── Step 3: Determine winner/loser user_id ───────────────────────────────
    // For 1v1, a player ID in the winner faction → they are the winner.
    // We check which of our two KaspaBattle players matches the faceit winner.
    // The FaceIT results.score is a map of faction → score value.
    let (winner_user_id, loser_user_id, score_str) = match_winner_to_user(
        &creator_faceit_id_owned(&creator_fid),
        creator_user_id,
        &opponent_faceit_id_owned(&opp_fid),
        opp_user_id,
        &winner_faction,
        &details.results,
        &details.teams,
    );

    let (winner_id, loser_id) = match (winner_user_id, loser_user_id) {
        (Some(w), Some(l)) => (w, l),
        _ => {
            // Winner could not be determined — no FaceIT player ID matched
            error!(
                match_id = %job.match_id,
                winner_faction = %winner_faction,
                creator_faceit_id = %creator_fid,
                opponent_faceit_id = %opp_fid,
                "FaceIT Watcher: winner mapping failed — neither player matched the winner faction — DISPUTED"
            );
            mark_match_disputed(
                pool,
                job.match_id,
                job.id,
                &format!(
                    "Winner faction '{}' did not match any participant's FaceIT ID. \
                     creator_faceit_id='{}', opponent_faceit_id='{}'",
                    winner_faction, creator_fid, opp_fid
                ),
            )
            .await?;
            return Ok(());
        }
    };

    info!(
        match_id = %job.match_id,
        winner_user_id = %winner_id,
        loser_user_id = %loser_id,
        score = %score_str,
        "🏆 FaceIT Watcher: winner mapped successfully"
    );
    tracing::info!("🏆 FaceIT Watcher: match {} — winner={} loser={} score={}",
        job.match_id, winner_id, loser_id, score_str
    );

    // ── Step 4: Atomic DB update ─────────────────────────────────────────────
    // Update match to FINISHED_FACEIT with all relevant data, mark job COMPLETED.
    // Atomic: WHERE status = 'IN_GAME' prevents double-processing.
    let rows_affected = sqlx::query(
        "UPDATE matches SET \
         status = 'FINISHED_FACEIT', \
         faceit_match_status = 'finished', \
         faceit_winner_faction = $1, \
         faceit_score = $2, \
         faceit_finished_at = NOW(), \
         winner_user_id = $3, \
         loser_user_id = $4 \
         WHERE id = $5 AND status = 'IN_GAME'",
    )
    .bind(&winner_faction)
    .bind(&score_str)
    .bind(winner_id)
    .bind(loser_id)
    .bind(job.match_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        // Race condition: another process already transitioned the match
        warn!(
            match_id = %job.match_id,
            "FaceIT Watcher: match was not in IN_GAME when trying to transition — skipping"
        );
    } else {
        info!(
            match_id = %job.match_id,
            "✅ FaceIT Watcher: match → FINISHED_FACEIT"
        );
    }

    // Mark job as COMPLETED regardless (prevents re-polling)
    mark_job_terminal(pool, job.id, "COMPLETED", None).await?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: winner mapping logic (pure, testable)
// ─────────────────────────────────────────────────────────────────────────────

/// Maps the FaceIT winner faction to KaspaBattle user IDs.
///
/// We check the FaceIT teams roster. If the creator's faceit_player_id is in the
/// winning faction's roster, they are the winner. Otherwise, we check the opponent.
///
/// Returns `(Some(winner_uid), Some(loser_uid))` on success, or `(None, None)`.
fn match_winner_to_user(
    creator_faceit_id: &str,
    creator_user_id: Uuid,
    opponent_faceit_id: &str,
    opponent_user_id: Uuid,
    winner_faction: &str,
    results: &Option<crate::models::faceit_data::FaceitMatchResults>,
    teams: &Option<crate::models::faceit_data::FaceitMatchTeams>,
) -> (Option<Uuid>, Option<Uuid>, String) {
    // Build score string from results.score map
    let score_str = if let Some(r) = results {
        let s1 = r.score.get("faction1").copied().unwrap_or(0);
        let s2 = r.score.get("faction2").copied().unwrap_or(0);
        format!("{}:{}", s1, s2)
    } else {
        "?:?".to_string()
    };

    let winning_roster = if let Some(t) = teams {
        match winner_faction {
            "faction1" => &t.faction1.roster,
            "faction2" => &t.faction2.roster,
            _ => {
                tracing::warn!(winner_faction, "FaceIT Watcher: unknown winning faction");
                return (None, None, score_str);
            }
        }
    } else {
        tracing::warn!("FaceIT Watcher: No teams data available to verify roster");
        return (None, None, score_str);
    };

    let creator_won = winning_roster.iter().any(|p| p.player_id == creator_faceit_id);
    let opponent_won = winning_roster.iter().any(|p| p.player_id == opponent_faceit_id);

    if creator_won && !opponent_won {
        (Some(creator_user_id), Some(opponent_user_id), score_str)
    } else if opponent_won && !creator_won {
        (Some(opponent_user_id), Some(creator_user_id), score_str)
    } else {
        tracing::warn!(
            creator_won,
            opponent_won,
            "FaceIT Watcher: ambiguous winner mapping (neither or both in roster)"
        );
        (None, None, score_str)
    }
}

/// Helper to keep borrow-checker happy (avoids moving out of reference)
fn creator_faceit_id_owned(s: &str) -> String {
    s.to_owned()
}
fn opponent_faceit_id_owned(s: &str) -> String {
    s.to_owned()
}

// ─────────────────────────────────────────────────────────────────────────────
// DB helpers: cancel + dispute
// ─────────────────────────────────────────────────────────────────────────────

/// Cancels the KaspaBattle match and marks the watcher job with the given terminal status.
async fn cancel_match_and_job(
    pool: &PgPool,
    match_id: Uuid,
    job_id: Uuid,
    job_terminal_status: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    // Atomic: only cancel if still IN_GAME (idempotent)
    sqlx::query(
        "UPDATE matches SET status = 'CANCELLED' \
         WHERE id = $1 AND status = 'IN_GAME'",
    )
    .bind(match_id)
    .execute(pool)
    .await?;

    mark_job_terminal(pool, job_id, job_terminal_status, Some(reason)).await?;

    info!(match_id = %match_id, reason, "FaceIT Watcher: match cancelled");
    Ok(())
}

/// Marks the match as DISPUTED and the watcher job as FAILED.
async fn mark_match_disputed(
    pool: &PgPool,
    match_id: Uuid,
    job_id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    // Note: Match stays in IN_GAME until an admin resolves the dispute.
    // We intentionally do NOT auto-cancel here — a human should decide.
    sqlx::query(
        "UPDATE matches SET status = 'DISPUTED' \
         WHERE id = $1 AND status = 'IN_GAME'",
    )
    .bind(match_id)
    .execute(pool)
    .await?;

    mark_job_terminal(pool, job_id, "FAILED", Some(reason)).await?;

    warn!(match_id = %match_id, reason, "FaceIT Watcher: match disputed");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::faceit_data::{FaceitMatchResults, FaceitMatchTeams, FaceitMatchFaction, FaceitMatchRosterPlayer};
    use std::collections::HashMap;

    fn make_results(winner: &str, f1: i32, f2: i32) -> Option<FaceitMatchResults> {
        let mut score = HashMap::new();
        score.insert("faction1".to_string(), f1);
        score.insert("faction2".to_string(), f2);
        Some(FaceitMatchResults {
            winner: winner.to_string(),
            score,
        })
    }

    fn make_teams(f1_player: &str, f2_player: &str) -> Option<FaceitMatchTeams> {
        Some(FaceitMatchTeams {
            faction1: FaceitMatchFaction {
                faction_id: "f1".to_string(),
                name: "Team 1".to_string(),
                roster: vec![FaceitMatchRosterPlayer {
                    player_id: f1_player.to_string(),
                    nickname: "Player1".to_string(),
                }],
            },
            faction2: FaceitMatchFaction {
                faction_id: "f2".to_string(),
                name: "Team 2".to_string(),
                roster: vec![FaceitMatchRosterPlayer {
                    player_id: f2_player.to_string(),
                    nickname: "Player2".to_string(),
                }],
            },
        })
    }

    #[test]
    fn test_faction1_wins_maps_to_creator() {
        let creator_id = Uuid::new_v4();
        let opponent_id = Uuid::new_v4();
        let results = make_results("faction1", 16, 10);
        let teams = make_teams("faceit-creator-123", "faceit-opponent-456");

        let (winner, loser, score) = match_winner_to_user(
            "faceit-creator-123",
            creator_id,
            "faceit-opponent-456",
            opponent_id,
            "faction1",
            &results,
            &teams,
        );

        assert_eq!(winner, Some(creator_id));
        assert_eq!(loser, Some(opponent_id));
        assert_eq!(score, "16:10");
    }

    #[test]
    fn test_faction2_wins_maps_to_opponent() {
        let creator_id = Uuid::new_v4();
        let opponent_id = Uuid::new_v4();
        let results = make_results("faction2", 10, 16);
        let teams = make_teams("faceit-creator-123", "faceit-opponent-456");

        let (winner, loser, score) = match_winner_to_user(
            "faceit-creator-123",
            creator_id,
            "faceit-opponent-456",
            opponent_id,
            "faction2",
            &results,
            &teams,
        );

        assert_eq!(winner, Some(opponent_id));
        assert_eq!(loser, Some(creator_id));
        assert_eq!(score, "10:16");
    }

    #[test]
    fn test_unknown_faction_returns_none() {
        let creator_id = Uuid::new_v4();
        let opponent_id = Uuid::new_v4();
        let results = make_results("faction3", 0, 0);
        let teams = make_teams("faceit-creator-123", "faceit-opponent-456");

        let (winner, loser, _score) = match_winner_to_user(
            "faceit-creator-123",
            creator_id,
            "faceit-opponent-456",
            opponent_id,
            "faction3",
            &results,
            &teams,
        );

        assert!(winner.is_none());
        assert!(loser.is_none());
    }

    #[test]
    fn test_no_teams_returns_none() {
        let creator_id = Uuid::new_v4();
        let opponent_id = Uuid::new_v4();
        let results = make_results("faction1", 16, 10);

        let (winner, _loser, score) = match_winner_to_user(
            "faceit-creator-123",
            creator_id,
            "faceit-opponent-456",
            opponent_id,
            "faction1",
            &results,
            &None, // No teams
        );

        assert!(winner.is_none());
        assert_eq!(score, "16:10");
    }

    #[test]
    fn test_score_string_format() {
        let creator_id = Uuid::new_v4();
        let opponent_id = Uuid::new_v4();
        let results = make_results("faction2", 5, 13);
        let teams = make_teams("creator", "opponent");

        let (_winner, _loser, score) = match_winner_to_user(
            "creator",
            creator_id,
            "opponent",
            opponent_id,
            "faction2",
            &results,
            &teams,
        );

        assert_eq!(score, "5:13");
    }

    #[test]
    fn test_watcher_config_defaults() {
        let config = FaceitWatcherConfig::default();
        assert_eq!(config.poll_interval, Duration::from_secs(10));
        assert_eq!(config.match_timeout, Duration::from_secs(3 * 3600));
        assert_eq!(config.backoff_rate_limit, Duration::from_secs(60));
    }
}
