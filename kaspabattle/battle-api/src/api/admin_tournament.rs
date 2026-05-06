//! Admin API — Phase 3: Dispute resolution, force-cancel, audit log.
//!
//! All endpoints require Admin API-Key authentication (existing AdminApiKey guard).
//!
//! ## Routes (all under /api/v1/admin)
//!
//! | Method | Path                                                | Description                        |
//! |--------|-----------------------------------------------------|------------------------------------|
//! | GET    | /admin/tournaments                                  | List all tournaments (with status) |
//! | GET    | /admin/tournaments/disputed                         | List disputed tournaments          |
//! | POST   | /admin/tournaments/:id/resolve-dispute              | Resolve tournament-level dispute   |
//! | POST   | /admin/tournaments/:id/bracket/:slot_id/resolve     | Resolve bracket-slot dispute       |
//! | POST   | /admin/tournaments/:id/force-cancel                 | Force-cancel any tournament        |
//! | POST   | /admin/tournaments/:id/trigger-payout               | Manually trigger payout            |
//! | GET    | /admin/audit-log                                    | List recent audit events           |

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::api::{AppState, ApiErrorResponse, admin_guard::AdminApiKey};

type ApiError = (StatusCode, Json<ApiErrorResponse>);

fn db_err(e: sqlx::Error) -> ApiError {
    tracing::error!(error = %e, "Admin DB error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiErrorResponse {
            error: "db_error",
            message: "Database operation failed.",
        }),
    )
}

// ─── GET /admin/tournaments ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TournamentListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn admin_list_tournaments(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Query(q): Query<TournamentListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = q.limit.unwrap_or(100).min(500);
    let status_filter = q.status.as_deref().unwrap_or("%");

    let rows = sqlx::query(
        "SELECT t.id, t.title, t.game_type, t.status, t.max_teams, t.buy_in_sompi, \
                t.total_prize_pool_sompi, t.organizer_user_id, t.created_at, \
                t.winner_team_id_ref, t.dispute_reason, t.dispute_filed_at, \
                t.payout_tx_hash, t.payout_executed_at, \
                COUNT(tt.id) AS team_count \
         FROM tournaments t \
         LEFT JOIN tournament_teams tt ON tt.tournament_id = t.id \
         WHERE t.status ILIKE $1 \
         GROUP BY t.id \
         ORDER BY t.created_at DESC \
         LIMIT $2",
    )
    .bind(status_filter)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let tournaments: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "id": r.try_get::<Uuid, _>("id").unwrap().to_string(),
            "title": r.try_get::<String, _>("title").unwrap_or_default(),
            "game_type": r.try_get::<String, _>("game_type").unwrap_or_default(),
            "status": r.try_get::<String, _>("status").unwrap_or_default(),
            "max_teams": r.try_get::<i32, _>("max_teams").unwrap_or(0),
            "buy_in_sompi": r.try_get::<i64, _>("buy_in_sompi").unwrap_or(0),
            "total_prize_pool_sompi": r.try_get::<i64, _>("total_prize_pool_sompi").unwrap_or(0),
            "team_count": r.try_get::<i64, _>("team_count").unwrap_or(0),
            "organizer_user_id": r.try_get::<Uuid, _>("organizer_user_id").unwrap().to_string(),
            "created_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").unwrap().to_rfc3339(),
            "winner_team_id": r.try_get::<Option<Uuid>, _>("winner_team_id_ref").unwrap_or(None).map(|u| u.to_string()),
            "dispute_reason": r.try_get::<Option<String>, _>("dispute_reason").unwrap_or(None),
            "dispute_filed_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("dispute_filed_at").unwrap_or(None).map(|d| d.to_rfc3339()),
            "payout_tx_hash": r.try_get::<Option<String>, _>("payout_tx_hash").unwrap_or(None),
            "payout_executed_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("payout_executed_at").unwrap_or(None).map(|d| d.to_rfc3339()),
        })
    }).collect();

    Ok(Json(serde_json::json!({
        "tournaments": tournaments,
        "count": tournaments.len(),
    })))
}

// ─── GET /admin/tournaments/disputed ─────────────────────────────────────────

pub async fn admin_list_disputed(
    State(state): State<AppState>,
    _admin: AdminApiKey,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query(
        "SELECT t.id, t.title, t.status, t.dispute_reason, t.dispute_filed_at \
         FROM tournaments t \
         WHERE t.status = 'DISPUTED' \
         ORDER BY t.dispute_filed_at ASC \
         LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let slot_rows = sqlx::query(
        "SELECT b.id, b.tournament_id, b.round, b.slot_index, \
                b.dispute_reason, b.dispute_filed_at \
         FROM tournament_bracket b \
         WHERE b.disputed = TRUE AND b.dispute_resolved_at IS NULL \
         ORDER BY b.dispute_filed_at ASC \
         LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    Ok(Json(serde_json::json!({
        "disputed_tournaments": rows.iter().map(|r| serde_json::json!({
            "id": r.try_get::<Uuid, _>("id").unwrap().to_string(),
            "title": r.try_get::<String, _>("title").unwrap_or_default(),
            "status": r.try_get::<String, _>("status").unwrap_or_default(),
            "dispute_reason": r.try_get::<Option<String>, _>("dispute_reason").unwrap_or(None),
            "dispute_filed_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("dispute_filed_at").unwrap_or(None).map(|d| d.to_rfc3339()),
        })).collect::<Vec<_>>(),
        "disputed_bracket_slots": slot_rows.iter().map(|r| serde_json::json!({
            "id": r.try_get::<Uuid, _>("id").unwrap().to_string(),
            "tournament_id": r.try_get::<Uuid, _>("tournament_id").unwrap().to_string(),
            "round": r.try_get::<i32, _>("round").unwrap_or(0),
            "slot_index": r.try_get::<i32, _>("slot_index").unwrap_or(0),
            "dispute_reason": r.try_get::<Option<String>, _>("dispute_reason").unwrap_or(None),
            "dispute_filed_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("dispute_filed_at").unwrap_or(None).map(|d| d.to_rfc3339()),
        })).collect::<Vec<_>>(),
    })))
}

// ─── POST /admin/tournaments/:id/resolve-dispute ──────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ResolveTournamentDisputeReq {
    /// Resolution: "winner_team_a", "winner_team_b", "cancel", "force_complete"
    pub resolution: String,
    pub winner_team_id: Option<Uuid>,
    pub reason: String,
}

pub async fn admin_resolve_tournament_dispute(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Path(id): Path<Uuid>,
    Json(req): Json<ResolveTournamentDisputeReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.reason.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "reason_required",
                message: "A reason for the dispute resolution is required.",
            }),
        ));
    }

    let new_status = match req.resolution.as_str() {
        "cancel" => "CANCELLED",
        "force_complete" => "COMPLETED",
        "override_winner" => "COMPLETED",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    error: "invalid_resolution",
                    message: "resolution must be one of: cancel, force_complete, override_winner",
                }),
            ));
        }
    };

    sqlx::query(
        "UPDATE tournaments \
         SET status = $1, \
             dispute_resolved_at = NOW(), \
             winner_team_id_ref = COALESCE($2, winner_team_id_ref), \
             updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(new_status)
    .bind(req.winner_team_id)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    // Audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
         VALUES ('tournament', $1, 'dispute_resolved', 'admin', $2)",
    )
    .bind(id)
    .bind(serde_json::json!({
        "resolution": req.resolution,
        "reason": req.reason,
        "winner_team_id": req.winner_team_id,
        "new_status": new_status,
    }))
    .execute(&state.pool)
    .await;

    tracing::info!(
        tournament_id = %id,
        resolution = %req.resolution,
        "🔨 Admin: tournament dispute resolved"
    );

    // Broadcast WebSocket event
    let event = serde_json::json!({
        "type": "tournament_dispute_resolved",
        "tournament_id": id,
        "resolution": req.resolution,
        "new_status": new_status,
    });
    let _ = state.tx.send(event.to_string());

    Ok(Json(serde_json::json!({
        "status": "ok",
        "tournament_id": id,
        "new_status": new_status,
        "resolution": req.resolution,
    })))
}

// ─── POST /admin/tournaments/:id/bracket/:slot_id/resolve ─────────────────────

#[derive(Debug, Deserialize)]
pub struct ResolveBracketDisputeReq {
    pub winner_team_id: Uuid,
    pub reason: String,
}

pub async fn admin_resolve_bracket_dispute(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Path((tournament_id, slot_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ResolveBracketDisputeReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.reason.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "reason_required",
                message: "A reason is required for bracket dispute resolution.",
            }),
        ));
    }

    // Update the bracket slot
    let rows = sqlx::query(
        "UPDATE tournament_bracket \
         SET winner_team_id = $1, status = 'COMPLETED', disputed = FALSE, \
             dispute_resolved_at = NOW(), \
             updated_at = NOW() \
         WHERE id = $2 AND tournament_id = $3",
    )
    .bind(req.winner_team_id)
    .bind(slot_id)
    .bind(tournament_id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?
    .rows_affected();

    if rows == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: "not_found",
                message: "Bracket slot not found.",
            }),
        ));
    }

    // Re-run bracket advancement for this slot
    // Load slot info for advancement
    let slot = sqlx::query(
        "SELECT round, slot_index FROM tournament_bracket WHERE id = $1",
    )
    .bind(slot_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?;

    if let Some(slot) = slot {
        let round: i32 = slot.try_get("round").unwrap_or(1);
        let slot_index: i32 = slot.try_get("slot_index").unwrap_or(0);

        if let Err(e) = battle_core::oracle::tournament_oracle::advance_winner_for_admin(
            &state.pool,
            tournament_id,
            round,
            slot_index,
            req.winner_team_id,
        )
        .await
        {
            tracing::warn!(error = %e, "Admin resolve: bracket advance partial failure (non-fatal)");
        }
    }

    // Audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
         VALUES ('bracket_slot', $1, 'dispute_resolved', 'admin', $2)",
    )
    .bind(slot_id)
    .bind(serde_json::json!({
        "tournament_id": tournament_id,
        "winner_team_id": req.winner_team_id,
        "reason": req.reason,
    }))
    .execute(&state.pool)
    .await;

    let event = serde_json::json!({
        "type": "bracket_dispute_resolved",
        "tournament_id": tournament_id,
        "slot_id": slot_id,
        "winner_team_id": req.winner_team_id,
    });
    let _ = state.tx.send(event.to_string());

    tracing::info!(
        tournament_id = %tournament_id,
        slot_id = %slot_id,
        winner_team_id = %req.winner_team_id,
        "🔨 Admin: bracket dispute resolved"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "slot_id": slot_id,
        "winner_team_id": req.winner_team_id,
    })))
}

// ─── POST /admin/tournaments/:id/force-cancel ─────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ForceCancelReq {
    pub reason: String,
}

pub async fn admin_force_cancel_tournament(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Path(id): Path<Uuid>,
    Json(req): Json<ForceCancelReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query(
        "UPDATE tournaments \
         SET status = 'CANCELLED', cancelled_reason = $1, updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(req.reason.trim())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
         VALUES ('tournament', $1, 'force_cancelled', 'admin', $2)",
    )
    .bind(id)
    .bind(serde_json::json!({ "reason": req.reason }))
    .execute(&state.pool)
    .await;

    let _ = state.tx.send(serde_json::json!({
        "type": "tournament_cancelled",
        "tournament_id": id,
        "reason": req.reason,
    }).to_string());

    tracing::info!(tournament_id = %id, reason = %req.reason, "🔨 Admin: tournament force-cancelled");

    Ok(Json(serde_json::json!({
        "status": "ok",
        "tournament_id": id,
        "message": "Tournament force-cancelled. Refunds must be processed manually or via payout worker.",
    })))
}

// ─── POST /admin/tournaments/:id/trigger-payout ───────────────────────────────

pub async fn admin_trigger_payout(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify tournament is COMPLETED with a winner
    let row = sqlx::query(
        "SELECT status, total_prize_pool_sompi, winner_team_id_ref, runner_up_team_id_ref, \
                escrow_address, payout_tx_hash, \
                prize_winner_pct, prize_runner_up_pct, platform_fee_pct \
         FROM tournaments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or((
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse { error: "not_found", message: "Tournament not found." }),
    ))?;

    let status: String = row.try_get("status").unwrap_or_default();
    if status != "COMPLETED" {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "not_completed",
                message: "Tournament must be in COMPLETED status to trigger payout.",
            }),
        ));
    }

    let already_paid: Option<String> = row.try_get("payout_tx_hash").unwrap_or(None);
    if already_paid.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "already_paid",
                message: "Payout already executed for this tournament.",
            }),
        ));
    }

    let payout_service = state.payout_service.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiErrorResponse {
            error: "payout_unavailable",
            message: "PayoutService not available (Kaspa RPC not connected).",
        }),
    ))?;

    let pool_sompi: i64 = row.try_get("total_prize_pool_sompi").unwrap_or(0);
    let winner_pct: i64 = row.try_get::<i16, _>("prize_winner_pct").unwrap_or(70) as i64;
    let runner_up_pct: i64 = row.try_get::<i16, _>("prize_runner_up_pct").unwrap_or(20) as i64;
    let escrow_address: Option<String> = row.try_get("escrow_address").unwrap_or(None);
    let winner_team_id: Option<Uuid> = row.try_get("winner_team_id_ref").unwrap_or(None);
    let runner_up_team_id: Option<Uuid> = row.try_get("runner_up_team_id_ref").unwrap_or(None);

    let escrow = escrow_address.ok_or((
        StatusCode::CONFLICT,
        Json(ApiErrorResponse {
            error: "no_escrow",
            message: "Tournament has no escrow address configured.",
        }),
    ))?;

    // Resolve Kaspa addresses for winner and runner-up team captains
    let winner_addr = resolve_captain_address(&state.pool, winner_team_id).await.map_err(db_err)?;
    let runner_up_addr = resolve_captain_address(&state.pool, runner_up_team_id).await.map_err(db_err)?;
    // Use treasury address from env or derive from escrow wallet
    let treasury_addr = state.escrow_wallet.as_ref()
        .and_then(|w| w.derive_escrow_address("treasury").ok().map(|(a, _)| a.to_string()))
        .unwrap_or_else(|| std::env::var("TREASURY_ADDRESS").unwrap_or_default());

    if winner_addr.is_none() || runner_up_addr.is_none() {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "missing_kaspa_addresses",
                message: "Winner or runner-up captain has no Kaspa address registered.",
            }),
        ));
    }

    let winner_sompi = (pool_sompi * winner_pct / 100) as u64;
    let runner_up_sompi = (pool_sompi * runner_up_pct / 100) as u64;
    let fee_sompi = pool_sompi.saturating_sub((winner_sompi + runner_up_sompi) as i64) as u64;

    let mut outputs = vec![
        (winner_addr.unwrap(), winner_sompi),
        (runner_up_addr.unwrap(), runner_up_sompi),
    ];
    if fee_sompi > 0 && !treasury_addr.is_empty() {
        outputs.push((treasury_addr, fee_sompi));
    }

    let params = battle_kaspa::payout::TournamentPayoutParams {
        escrow_address: escrow.clone(),
        outputs,
    };

    match payout_service.execute_tournament_payout(&params).await {
        Ok(result) => {
            // Record payout TX
            sqlx::query(
                "UPDATE tournaments SET payout_tx_hash = $1, payout_executed_at = NOW(), updated_at = NOW() WHERE id = $2",
            )
            .bind(&result.tx_id)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(db_err)?;

            let _ = sqlx::query(
                "INSERT INTO audit_log (entity_type, entity_id, action, actor_role, details) \
                 VALUES ('tournament', $1, 'payout_executed', 'admin', $2)",
            )
            .bind(id)
            .bind(serde_json::json!({
                "tx_id": result.tx_id,
                "total_sompi": result.total_sompi,
                "fee_sompi": result.fee_sompi,
            }))
            .execute(&state.pool)
            .await;

            let _ = state.tx.send(serde_json::json!({
                "type": "tournament_payout_sent",
                "tournament_id": id,
                "tx_id": result.tx_id,
                "total_sompi": result.total_sompi,
            }).to_string());

            tracing::info!(
                tournament_id = %id,
                tx_id = %result.tx_id,
                "💸 Admin: tournament payout executed"
            );

            Ok(Json(serde_json::json!({
                "status": "ok",
                "tournament_id": id,
                "tx_id": result.tx_id,
                "total_sompi": result.total_sompi,
                "fee_sompi": result.fee_sompi,
            })))
        }
        Err(e) => {
            tracing::error!(tournament_id = %id, error = %e, "Admin: tournament payout failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    error: "payout_failed",
                    message: "Payout transaction failed. Check logs for details.",
                }),
            ))
        }
    }
}

// ─── GET /admin/audit-log ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub async fn admin_audit_log(
    State(state): State<AppState>,
    _admin: AdminApiKey,
    Query(q): Query<AuditLogQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = q.limit.unwrap_or(100).min(500);

    let rows = sqlx::query(
        "SELECT id, entity_type, entity_id, action, actor_user_id, actor_role, details, created_at \
         FROM audit_log \
         WHERE ($1::text IS NULL OR entity_type = $1) \
           AND ($2::uuid IS NULL OR entity_id = $2) \
         ORDER BY created_at DESC \
         LIMIT $3",
    )
    .bind(&q.entity_type)
    .bind(q.entity_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "id": r.try_get::<Uuid, _>("id").unwrap().to_string(),
            "entity_type": r.try_get::<String, _>("entity_type").unwrap_or_default(),
            "entity_id": r.try_get::<Uuid, _>("entity_id").unwrap().to_string(),
            "action": r.try_get::<String, _>("action").unwrap_or_default(),
            "actor_user_id": r.try_get::<Option<Uuid>, _>("actor_user_id").unwrap_or(None).map(|u| u.to_string()),
            "actor_role": r.try_get::<Option<String>, _>("actor_role").unwrap_or(None),
            "details": r.try_get::<Option<serde_json::Value>, _>("details").unwrap_or(None),
            "created_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").unwrap().to_rfc3339(),
        })
    }).collect();

    Ok(Json(serde_json::json!({
        "entries": entries,
        "count": entries.len(),
    })))
}

// ─── Helper: resolve captain's Kaspa address ─────────────────────────────────

async fn resolve_captain_address(
    pool: &sqlx::PgPool,
    team_id: Option<Uuid>,
) -> Result<Option<String>, sqlx::Error> {
    let Some(team_id) = team_id else {
        return Ok(None);
    };

    let row = sqlx::query(
        "SELECT u.kaspa_address FROM tournament_teams tt \
         JOIN users u ON u.id = tt.captain_user_id \
         WHERE tt.id = $1",
    )
    .bind(team_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|r| r.try_get::<Option<String>, _>("kaspa_address").unwrap_or(None)))
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/tournaments", get(admin_list_tournaments))
        .route("/tournaments/disputed", get(admin_list_disputed))
        .route("/tournaments/:id/resolve-dispute", post(admin_resolve_tournament_dispute))
        .route("/tournaments/:id/bracket/:slot_id/resolve", post(admin_resolve_bracket_dispute))
        .route("/tournaments/:id/force-cancel", post(admin_force_cancel_tournament))
        .route("/tournaments/:id/trigger-payout", post(admin_trigger_payout))
        .route("/audit-log", get(admin_audit_log))
}
