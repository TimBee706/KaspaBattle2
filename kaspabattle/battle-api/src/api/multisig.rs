//! Multisig Escrow API Routes
//!
//! Axum route handlers for 2-of-3 P2SH multisig escrow management.
//! These routes provide the HTTP interface for:
//! - Creating multisig escrows for new matches
//! - Checking deposit status
//! - Executing payouts (after match resolution)
//! - Executing refunds

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::AppState;

// ─── Request / Response Types ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateEscrowReq {
    pub match_id: Uuid,
    pub wager_per_player_sompi: u64,
    pub timelock_timestamp: Option<u64>,
}

#[derive(Deserialize)]
pub struct CheckDepositsReq {
    pub address: String,
    pub wager: u64,
}

#[derive(Deserialize)]
pub struct ExecutePayoutReq {
    pub match_id: Uuid,
    pub winner_address: String,
}

#[derive(Deserialize)]
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

// ─── Handlers ────────────────────────────────────────────────────────────

/// POST /api/v1/multisig/create
///
/// Creates a new 2-of-3 multisig escrow for a match.
/// Returns the P2SH escrow address, redeem script, and public keys.
async fn create_escrow(
    State(state): State<AppState>,
    Json(payload): Json<CreateEscrowReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        eprintln!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .create_escrow(
            payload.match_id,
            payload.wager_per_player_sompi,
            payload.timelock_timestamp,
        )
        .await
    {
        Ok(info) => {
            eprintln!(
                "✅ Multisig escrow created: match={}, address={}",
                info.match_id, info.escrow_address
            );
            Ok(Json(serde_json::to_value(info).unwrap()))
        }
        Err(e) => {
            eprintln!("❌ Failed to create multisig escrow: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/multisig/deposits?address=X&wager=Y
///
/// Check deposit status for a multisig escrow address.
async fn check_deposits(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<CheckDepositsReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        eprintln!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .check_deposits(&params.address, params.wager)
        .await
    {
        Ok(status) => Ok(Json(serde_json::to_value(status).unwrap())),
        Err(e) => {
            eprintln!("❌ check_deposits failed: {}", e);
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
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        StatusCode::SERVICE_UNAVAILABLE
    })?;

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
    Json(payload): Json<ExecutePayoutReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        eprintln!("❌ MultisigEscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    match multisig_svc
        .execute_payout(&match_id, &payload.winner_address)
        .await
    {
        Ok(result) => {
            eprintln!(
                "✅ Multisig payout: match={}, tx={}, winner_sompi={}",
                match_id, result.tx_id, result.winner_amount_sompi
            );

            // Update match status in DB
            let _ = sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
                .bind(match_id)
                .execute(&state.pool)
                .await;

            Ok(Json(serde_json::to_value(result).unwrap()))
        }
        Err(e) => {
            eprintln!("❌ Multisig payout failed: {}", e);
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
    Json(payload): Json<ExecuteRefundReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let multisig_svc = state.multisig_service.as_ref().ok_or_else(|| {
        eprintln!("❌ MultisigEscrowService not available");
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
            eprintln!(
                "💸 Multisig refund: match={}, tx={}",
                match_id, result.tx_id
            );

            // Update match status in DB
            let _ = sqlx::query("UPDATE matches SET status = 'REFUNDED' WHERE id = $1")
                .bind(match_id)
                .execute(&state.pool)
                .await;

            Ok(Json(serde_json::to_value(result).unwrap()))
        }
        Err(e) => {
            eprintln!("❌ Multisig refund failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
