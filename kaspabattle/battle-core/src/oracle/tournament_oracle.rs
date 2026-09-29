//! Tournament Bracket Oracle — Phase 3
//!
//! Processes FaceIT match results for tournament bracket slots.
//!
//! Called by the FaceIT watcher when a bracket-slot match finishes.
//! Handles:
//!   - Winner-to-team mapping (via `faceit_faction_a/b` stored in bracket)
//!   - Automatic winner advancement to the next bracket slot
//!   - BYE-slot auto-resolution
//!   - Finals detection → tournament COMPLETED transition
//!   - WebSocket event broadcast: `bracket_result`, `tournament_completed`

use sqlx::{PgPool, Row};
use tokio::sync::broadcast;
use uuid::Uuid;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Process a finished FaceIT match for a tournament bracket slot.
///
/// `winner_faction` — "faction1" or "faction2" (from FaceIT results.winner)
/// `score_str`      — "16:10" formatted string (from FaceIT results.score)
/// `ws_tx`          — WebSocket broadcast sender for real-time events
pub async fn process_bracket_result(
    pool: &PgPool,
    bracket_slot_id: Uuid,
    tournament_id: Uuid,
    winner_faction: &str,
    score_str: &str,
    ws_tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load slot details
    let slot = sqlx::query(
        "SELECT b.round, b.slot_index, b.team_a_id, b.team_b_id, \
                b.faceit_faction_a, b.faceit_faction_b, b.status, b.disputed \
         FROM tournament_bracket b \
         WHERE b.id = $1 AND b.tournament_id = $2",
    )
    .bind(bracket_slot_id)
    .bind(tournament_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| format!("Bracket slot {} not found", bracket_slot_id))?;

    let status: String = slot.try_get("status").unwrap_or_default();
    let disputed: bool = slot.try_get("disputed").unwrap_or(false);

    if disputed {
        tracing::warn!(
            bracket_slot_id = %bracket_slot_id,
            "Tournament oracle: slot is disputed — skipping auto-advance"
        );
        return Ok(());
    }

    if !matches!(status.as_str(), "IN_PROGRESS" | "READY") {
        tracing::warn!(
            bracket_slot_id = %bracket_slot_id,
            %status,
            "Tournament oracle: slot not in expected status — skipping"
        );
        return Ok(());
    }

    let round: i32 = slot.try_get("round")?;
    let slot_index: i32 = slot.try_get("slot_index")?;
    let team_a_id: Option<Uuid> = slot.try_get("team_a_id").unwrap_or(None);
    let team_b_id: Option<Uuid> = slot.try_get("team_b_id").unwrap_or(None);
    let faction_a: Option<String> = slot.try_get("faceit_faction_a").unwrap_or(None);
    let faction_b: Option<String> = slot.try_get("faceit_faction_b").unwrap_or(None);

    // Determine winner team based on faction mapping
    let winner_team_id = determine_winner_team(
        winner_faction,
        team_a_id,
        team_b_id,
        faction_a.as_deref(),
        faction_b.as_deref(),
    );

    let winner_team_id = match winner_team_id {
        Some(id) => id,
        None => {
            // Can't determine winner — mark slot as DISPUTED
            tracing::error!(
                bracket_slot_id = %bracket_slot_id,
                %winner_faction,
                "Tournament oracle: cannot map winner faction to team — marking DISPUTED"
            );
            mark_slot_disputed(
                pool,
                bracket_slot_id,
                "Cannot map FaceIT winner faction to team",
            )
            .await?;
            return Ok(());
        }
    };

    let loser_team_id = if winner_team_id == team_a_id.unwrap_or(Uuid::nil()) {
        team_b_id
    } else {
        team_a_id
    };

    tracing::info!(
        bracket_slot_id = %bracket_slot_id,
        tournament_id = %tournament_id,
        winner_team_id = %winner_team_id,
        score = %score_str,
        "🏆 Tournament oracle: bracket result processed"
    );

    // Update the bracket slot as COMPLETED
    sqlx::query(
        "UPDATE tournament_bracket \
         SET winner_team_id = $1, status = 'COMPLETED', \
             reported_score = $2, match_finished_at = NOW(), updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(winner_team_id)
    .bind(score_str)
    .bind(bracket_slot_id)
    .execute(pool)
    .await?;

    // Advance winner to next round
    advance_winner(pool, tournament_id, round, slot_index, winner_team_id).await?;

    // Write audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
         VALUES ('bracket_slot', $1, 'result_reported', 'system', $2)",
    )
    .bind(bracket_slot_id)
    .bind(serde_json::json!({
        "tournament_id": tournament_id,
        "winner_team_id": winner_team_id,
        "loser_team_id": loser_team_id,
        "score": score_str,
        "winner_faction": winner_faction,
    }))
    .execute(pool)
    .await;

    // Broadcast WebSocket event
    let event = serde_json::json!({
        "type": "bracket_result",
        "tournament_id": tournament_id,
        "bracket_slot_id": bracket_slot_id,
        "round": round,
        "slot_index": slot_index,
        "winner_team_id": winner_team_id,
        "score": score_str,
    });
    let _ = ws_tx.send(event.to_string());

    // Check if this was the Finals (winner_team_id is now tournament winner)
    check_and_complete_tournament(pool, tournament_id, winner_team_id, ws_tx).await?;

    Ok(())
}

// ─── Winner-faction mapping ───────────────────────────────────────────────────

fn determine_winner_team(
    winner_faction: &str,
    team_a_id: Option<Uuid>,
    team_b_id: Option<Uuid>,
    faction_a: Option<&str>, // what faction team_a was assigned
    faction_b: Option<&str>, // what faction team_b was assigned
) -> Option<Uuid> {
    match winner_faction {
        "faction1" => {
            // faction1 won — find which team was assigned faction1
            if faction_a == Some("faction1") {
                team_a_id
            } else if faction_b == Some("faction1") {
                team_b_id
            } else {
                // No faction assignment stored — fall back: faction1 = team_a (legacy)
                team_a_id
            }
        }
        "faction2" => {
            if faction_a == Some("faction2") {
                team_a_id
            } else if faction_b == Some("faction2") {
                team_b_id
            } else {
                // Fall back: faction2 = team_b (legacy)
                team_b_id
            }
        }
        _ => None,
    }
}

// ─── Advance winner to next round ────────────────────────────────────────────

/// Propagate the winner from `(round, slot_index)` to the appropriate slot in `round+1`.
///
/// Single-elimination bracket layout:
///   Round R, Slot I → Round R+1, Slot I/2
///   Feeds into slot_index/2 in the next round:
///     I=0 → next.slot=0 as team_a
///     I=1 → next.slot=0 as team_b
///     I=2 → next.slot=1 as team_a
///     I=3 → next.slot=1 as team_b
async fn advance_winner(
    pool: &PgPool,
    tournament_id: Uuid,
    round: i32,
    slot_index: i32,
    winner_team_id: Uuid,
) -> Result<(), sqlx::Error> {
    let next_round = round + 1;
    let next_slot = slot_index / 2;
    let is_team_a_slot = (slot_index % 2) == 0;

    // Check if a next-round slot exists
    let next_slot_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tournament_bracket \
         WHERE tournament_id = $1 AND round = $2 AND slot_index = $3)",
    )
    .bind(tournament_id)
    .bind(next_round)
    .bind(next_slot)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !next_slot_exists {
        tracing::debug!(
            tournament_id = %tournament_id,
            round,
            slot_index,
            "No next round slot found — this may be the finals"
        );
        return Ok(());
    }

    // Place winner into the appropriate side of the next slot
    // H-02: Accept both WAITING and READY status to allow admin dispute resolution
    // to override an incorrect auto-advance from an earlier result.
    if is_team_a_slot {
        sqlx::query(
            "UPDATE tournament_bracket \
             SET team_a_id = $1, status = CASE WHEN team_b_id IS NOT NULL THEN 'READY' ELSE status END, \
                 updated_at = NOW() \
             WHERE tournament_id = $2 AND round = $3 AND slot_index = $4 AND status IN ('WAITING', 'READY')",
        )
        .bind(winner_team_id)
        .bind(tournament_id)
        .bind(next_round)
        .bind(next_slot)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE tournament_bracket \
             SET team_b_id = $1, status = CASE WHEN team_a_id IS NOT NULL THEN 'READY' ELSE status END, \
                 updated_at = NOW() \
             WHERE tournament_id = $2 AND round = $3 AND slot_index = $4 AND status IN ('WAITING', 'READY')",
        )
        .bind(winner_team_id)
        .bind(tournament_id)
        .bind(next_round)
        .bind(next_slot)
        .execute(pool)
        .await?;
    }

    tracing::info!(
        tournament_id = %tournament_id,
        winner_team_id = %winner_team_id,
        next_round,
        next_slot,
        "⬆️ Winner advanced to next bracket round"
    );

    Ok(())
}

/// Public entry point called by the admin dispute resolution handler.
///
/// Equivalent to `advance_winner` but exposed for use outside this module.
pub async fn advance_winner_for_admin(
    pool: &PgPool,
    tournament_id: Uuid,
    round: i32,
    slot_index: i32,
    winner_team_id: Uuid,
) -> Result<(), sqlx::Error> {
    advance_winner(pool, tournament_id, round, slot_index, winner_team_id).await
}

async fn check_and_complete_tournament(
    pool: &PgPool,
    tournament_id: Uuid,
    finals_winner_id: Uuid,
    ws_tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get the max round in this tournament
    let max_round: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(round), 0) FROM tournament_bracket WHERE tournament_id = $1",
    )
    .bind(tournament_id)
    .fetch_one(pool)
    .await?;

    // Count COMPLETED slots at max round
    let max_round_completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_bracket \
         WHERE tournament_id = $1 AND round = $2 AND status = 'COMPLETED'",
    )
    .bind(tournament_id)
    .bind(max_round)
    .fetch_one(pool)
    .await?;

    // In single elimination, finals = exactly 1 slot at max round
    let max_round_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_bracket \
         WHERE tournament_id = $1 AND round = $2",
    )
    .bind(tournament_id)
    .bind(max_round)
    .fetch_one(pool)
    .await?;

    if max_round_total != 1 || max_round_completed != 1 {
        // Not the finals yet
        return Ok(());
    }

    tracing::info!(
        tournament_id = %tournament_id,
        winner_team_id = %finals_winner_id,
        "🏆 Tournament FINALS completed — determining winner and runner-up"
    );

    // Runner-up = loser of the finals slot
    let finals_slot = sqlx::query(
        "SELECT team_a_id, team_b_id FROM tournament_bracket \
         WHERE tournament_id = $1 AND round = $2 LIMIT 1",
    )
    .bind(tournament_id)
    .bind(max_round)
    .fetch_one(pool)
    .await?;

    let team_a: Option<Uuid> = finals_slot.try_get("team_a_id").unwrap_or(None);
    let team_b: Option<Uuid> = finals_slot.try_get("team_b_id").unwrap_or(None);

    let runner_up_id = if team_a == Some(finals_winner_id) {
        team_b
    } else {
        team_a
    };

    // Update tournament to COMPLETED
    sqlx::query(
        "UPDATE tournaments \
         SET status = 'COMPLETED', \
             winner_team_id_ref = $1, \
             runner_up_team_id_ref = $2, \
             updated_at = NOW() \
         WHERE id = $3 AND status IN ('IN_PROGRESS', 'BRACKET_READY')",
    )
    .bind(finals_winner_id)
    .bind(runner_up_id)
    .bind(tournament_id)
    .execute(pool)
    .await?;

    // Write audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
         VALUES ('tournament', $1, 'completed', 'system', $2)",
    )
    .bind(tournament_id)
    .bind(serde_json::json!({
        "winner_team_id": finals_winner_id,
        "runner_up_team_id": runner_up_id,
    }))
    .execute(pool)
    .await;

    // Broadcast completion event
    let event = serde_json::json!({
        "type": "tournament_completed",
        "tournament_id": tournament_id,
        "winner_team_id": finals_winner_id,
        "runner_up_team_id": runner_up_id,
    });
    let _ = ws_tx.send(event.to_string());

    tracing::info!(
        tournament_id = %tournament_id,
        winner_team_id = %finals_winner_id,
        runner_up_team_id = ?runner_up_id,
        "✅ Tournament marked COMPLETED — payout worker will execute automatically"
    );

    Ok(())
}

// ─── Dispute helper ───────────────────────────────────────────────────────────

async fn mark_slot_disputed(
    pool: &PgPool,
    bracket_slot_id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tournament_bracket \
         SET disputed = TRUE, dispute_reason = $1, dispute_filed_at = NOW(), updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(reason)
    .bind(bracket_slot_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faction1_wins_maps_to_team_a_when_faction_stored() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let winner = determine_winner_team(
            "faction1",
            Some(team_a),
            Some(team_b),
            Some("faction1"),
            Some("faction2"),
        );
        assert_eq!(winner, Some(team_a));
    }

    #[test]
    fn test_faction2_wins_maps_to_team_b_when_faction_stored() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let winner = determine_winner_team(
            "faction2",
            Some(team_a),
            Some(team_b),
            Some("faction1"),
            Some("faction2"),
        );
        assert_eq!(winner, Some(team_b));
    }

    #[test]
    fn test_faction1_wins_falls_back_to_team_a_without_faction_data() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let winner = determine_winner_team("faction1", Some(team_a), Some(team_b), None, None);
        assert_eq!(winner, Some(team_a));
    }

    #[test]
    fn test_faction2_wins_falls_back_to_team_b_without_faction_data() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let winner = determine_winner_team("faction2", Some(team_a), Some(team_b), None, None);
        assert_eq!(winner, Some(team_b));
    }

    #[test]
    fn test_unknown_faction_returns_none() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let winner = determine_winner_team(
            "faction3",
            Some(team_a),
            Some(team_b),
            Some("faction1"),
            Some("faction2"),
        );
        assert!(winner.is_none());
    }
}
