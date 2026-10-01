//! Multisig Escrow API Routes
//!
//! Axum route handlers for 2-of-3 P2SH multisig escrow management.
//! These routes provide the HTTP interface for:
//! - Creating multisig escrows for new matches
//! - Checking deposit status
//! - Executing payouts (after match resolution)
//! - Executing refunds

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::admin_guard::AdminApiKey;
use crate::api::auth_guard::SessionUser;
use crate::api::AppState;

// ─── Request / Response Types ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateEscrowReq {
    pub match_id: Uuid,
    pub wager_per_player_sompi: u64,
    /// Accepted for wire compatibility only; deliberately ignored (see `create_escrow`).
    #[allow(dead_code)]
    pub timelock_timestamp: Option<u64>,
}

#[derive(Deserialize)]
pub struct CheckDepositsReq {
    pub address: String,
    pub wager: u64,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct ExecutePayoutReq {
    pub match_id: Uuid,
    pub winner_address: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct ExecuteRefundReq {
    pub match_id: Uuid,
    pub player_a_address: String,
    pub player_b_address: String,
}

// ─── Router ──────────────────────────────────────────────────────────────

/// Returns the Router for multisig escrow endpoints.
///
/// Mounted at `/api/v1/multisig`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/create", post(create_escrow))
        .route("/deposits", get(check_deposits))
        .route("/:match_id", get(get_escrow))
        .route("/:match_id/payout", post(execute_payout))
        .route("/:match_id/refund", post(execute_refund))
}

/// Decides whether `caller` may request the escrow of a match (AUDIT F-01).
///
/// Pure so the rule is unit-testable without a database: the caller must be the
/// creator or the joined opponent, and the requested wager must equal the wager
/// stored on the match row.
fn authorize_escrow_request(
    caller: Uuid,
    creator: Uuid,
    opponent: Option<Uuid>,
    db_wager_sompi: i64,
    requested_wager_sompi: u64,
) -> Result<(), StatusCode> {
    if caller != creator && Some(caller) != opponent {
        return Err(StatusCode::FORBIDDEN);
    }
    if db_wager_sompi <= 0 || db_wager_sompi as u64 != requested_wager_sompi {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

// ─── Handlers ────────────────────────────────────────────────────────────

/// POST /api/v1/multisig/create
///
/// Creates a new 2-of-3 multisig escrow for a match.
/// Returns the P2SH escrow address, redeem script, and public keys.
async fn create_escrow(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Json(payload): Json<CreateEscrowReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // AUDIT F-01: the escrow terms come from the match row, never from the client.
    // Only a participant of an existing match may (re-)request its escrow.
    let row: Option<(Uuid, Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT creator_user_id, opponent_user_id, wager_sompi FROM matches WHERE id = $1",
    )
    .bind(payload.match_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("create_escrow: match lookup failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let (creator, opponent, db_wager) = row.ok_or(StatusCode::NOT_FOUND)?;
    authorize_escrow_request(
        user.id,
        creator,
        opponent,
        db_wager,
        payload.wager_per_player_sompi,
    )?;

    // The client-supplied timelock is deliberately ignored (escrows are created with none
    // by `create_challenge`; the service returns the existing record if there is one).
    match multisig_svc
        .create_escrow(payload.match_id, payload.wager_per_player_sompi, None)
        .await
    {
        Ok(info) => {
            tracing::info!(
                "✅ Multisig escrow created: match={}, address={}",
                info.match_id,
                info.escrow_address
            );
            Ok(Json(serde_json::to_value(info).unwrap()))
        }
        Err(e) => {
            tracing::error!("❌ Failed to create multisig escrow: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/multisig/deposits?address=X&wager=Y
///
/// Check deposit status for a multisig escrow address.
async fn check_deposits(
    State(state): State<AppState>,
    SessionUser(_user): SessionUser,
    Query(params): Query<CheckDepositsReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .check_deposits(&params.address, params.wager)
        .await
    {
        Ok(status) => Ok(Json(serde_json::to_value(status).unwrap())),
        Err(e) => {
            tracing::error!("❌ check_deposits failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/multisig/:match_id
///
/// Get full details about a multisig escrow.
async fn get_escrow(
    State(state): State<AppState>,
    Path(match_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state
        .multisig_service
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    match multisig_svc.get_escrow(&match_id).await {
        Some(escrow) => Ok(Json(serde_json::json!({
            "match_id": escrow.match_id.to_string(),
            "p2sh_address": escrow.p2sh_address,
            "status": format!("{:?}", escrow.status),
            "redeem_script_hex": escrow.redeem_script_hex,
            "pubkey_a": escrow.pubkey_a_hex,
            "pubkey_b": escrow.pubkey_b_hex,
            "pubkey_oracle": escrow.pubkey_oracle_hex,
            "threshold": escrow.config.threshold,
            "wager_per_player_sompi": escrow.wager_per_player_sompi,
            "timelock_timestamp": escrow.timelock_timestamp,
            "created_at": escrow.created_at.to_rfc3339(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /api/v1/multisig/:match_id/payout
///
/// Execute a payout to the match winner.
/// Signs with 2-of-3 keys (winner + oracle) and broadcasts.
async fn execute_payout(
    State(state): State<AppState>,
    Path(match_id): Path<Uuid>,
    _admin: AdminApiKey,
    Json(payload): Json<ExecutePayoutReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .execute_payout(&match_id, &payload.winner_address)
        .await
    {
        Ok(result) => {
            tracing::info!(
                "✅ Multisig payout: match={}, tx={}, winner_sompi={}",
                match_id,
                result.tx_id,
                result.winner_amount_sompi
            );

            // Update match status in DB
            // The TX is already broadcast: a failed status write must be loud (a stale
            // status invites a second payout attempt), so log at error level.
            if let Err(e) = sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
                .bind(match_id)
                .execute(&state.pool)
                .await
            {
                tracing::error!(
                    match_id = %match_id,
                    tx = %result.tx_id,
                    error = %e,
                    "TX broadcast but marking match PAID_OUT failed — reconcile manually"
                );
            }

            Ok(Json(serde_json::to_value(result).unwrap()))
        }
        Err(e) => {
            tracing::error!("❌ Multisig payout failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "PAYOUT_FAILED",
                "error": format!("{}", e),
            })))
        }
    }
}

/// POST /api/v1/multisig/:match_id/refund
///
/// Execute a refund — splits escrow equally back to both players.
async fn execute_refund(
    State(state): State<AppState>,
    Path(match_id): Path<Uuid>,
    _admin: AdminApiKey,
    Json(payload): Json<ExecuteRefundReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .execute_refund(
            &match_id,
            &payload.player_a_address,
            &payload.player_b_address,
        )
        .await
    {
        Ok(result) => {
            tracing::info!(
                "💸 Multisig refund: match={}, tx={}",
                match_id,
                result.tx_id
            );

            // Update match status in DB
            if let Err(e) = sqlx::query("UPDATE matches SET status = 'REFUNDED' WHERE id = $1")
                .bind(match_id)
                .execute(&state.pool)
                .await
            {
                tracing::error!(
                    match_id = %match_id,
                    tx = %result.tx_id,
                    error = %e,
                    "TX broadcast but marking match REFUNDED failed — reconcile manually"
                );
            }

            Ok(Json(serde_json::to_value(result).unwrap()))
        }
        Err(e) => {
            tracing::error!("❌ Multisig refund failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn creator_and_opponent_may_request_escrow() {
        let (a, b) = (id(), id());
        assert_eq!(authorize_escrow_request(a, a, Some(b), 100, 100), Ok(()));
        assert_eq!(authorize_escrow_request(b, a, Some(b), 100, 100), Ok(()));
        assert_eq!(authorize_escrow_request(a, a, None, 100, 100), Ok(()));
    }

    #[test]
    fn stranger_is_forbidden() {
        let (a, b, x) = (id(), id(), id());
        assert_eq!(
            authorize_escrow_request(x, a, Some(b), 100, 100),
            Err(StatusCode::FORBIDDEN)
        );
        // No opponent yet: a stranger must not match `None`.
        assert_eq!(
            authorize_escrow_request(x, a, None, 100, 100),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn wager_must_match_the_match_row() {
        let a = id();
        assert_eq!(
            authorize_escrow_request(a, a, None, 100, 1),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            authorize_escrow_request(a, a, None, 100, u64::MAX),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            authorize_escrow_request(a, a, None, 0, 0),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            authorize_escrow_request(a, a, None, -5, 5),
            Err(StatusCode::BAD_REQUEST)
        );
    }
}
