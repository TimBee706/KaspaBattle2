pub mod auth_guard;
pub mod faceit;

use crate::models::{Match, MatchMode};
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct CreateReq {
    pub game_id: String,
    pub stake_kas: i64,
    pub mode: MatchMode,
    pub escrow_address: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DepositReq {
    pub tx_hash: String,
    pub player_role: String, // "A" oder "B"
}

pub async fn simulate_deposit_test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    if std::env::var("TEST_MODE").unwrap_or_default() != "true" {
        return Err(StatusCode::FORBIDDEN);
    }

    let dummy_tx = "fake_tx_testmode_123".to_string();

    let mut record = sqlx::query_as::<_, Match>(
        "UPDATE matches SET status = 'LOCKED' WHERE id = $1 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    record.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
    Ok(Json(record))
}

use battle_core::auth::AuthService;
use battle_core::faceit_oauth::FaceitOAuthService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
    pub auth_service: Arc<AuthService>,
    pub faceit_service: Arc<FaceitOAuthService>,
    pub escrow_wallet: Option<Arc<battle_kaspa::wallet::EscrowWallet>>,
    pub escrow_service: Option<Arc<battle_kaspa::escrow::EscrowService>>,
    pub kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaRpc>>,
    pub payout_service: Option<Arc<battle_kaspa::payout::PayoutService>>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lobbies", get(get_lobbies))
        .route("/history", get(get_history))
        .route("/challenges", post(create_challenge))
        .route("/matches/:id/accept", post(join_challenge))
        .route("/matches/:id/deposit", post(submit_deposit))
        .route("/matches/:id/deposits", get(check_deposits))
        .route("/matches/:id/resolve", post(admin_resolve_match))
        // ── v0.2 ──
        .route("/matches/:id/faceid", post(submit_faceid_handler))
        .route("/matches/:id/cancel", post(cancel_match_handler))
        .route("/lobbies/:id/simulate-deposit", post(simulate_deposit_test))
        .route("/webhook/faceit", post(faceit_webhook))
        .route("/auth/me", get(get_me))
        .nest("/faceit", faceit::router())
}

pub async fn get_me(
    State(_state): State<AppState>,
    user_opt: Option<crate::api::auth_guard::SessionUser>,
) -> Result<Json<battle_core::models::user::User>, StatusCode> {
    if std::env::var("TEST_MODE").unwrap_or_default() == "true" {
        return Ok(Json(battle_core::models::user::User {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            email: "test@example.com".to_string(),
            email_verified: true,
            password_hash: "".to_string(),
            display_name: "TestUser".to_string(),
            kaspa_address: Some(
                "kaspatest:qzh86re35m2re7k7sxc6uqlp40st4vvmefsc0sv0uev49v3axm7jwcst6p7xs"
                    .to_string(),
            ),
            created_at: "".to_string(),
            updated_at: "".to_string(),
            last_login_at: None,
        }));
    }

    match user_opt {
        Some(crate::api::auth_guard::SessionUser(user)) => Ok(Json(user)),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

pub async fn get_lobbies(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status IN ('OPEN', 'AWAITING_FUNDING', 'LOCKED', 'IN_GAME') ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    for m in &mut matches {
        m.calculate_wager();
    }

    Ok(Json(matches))
}

pub async fn get_history(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status = 'RESOLVED'")
        .fetch_all(&state.pool).await.map_err(|e| {
            eprintln!("SQL Error in get_history: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    for m in &mut matches {
        m.calculate_wager();
    }
    Ok(Json(matches))
}

pub async fn create_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Json(payload): Json<CreateReq>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 1. INSERT match first (without escrow_address)
    let record = sqlx::query_as::<_, Match>(
        "INSERT INTO matches (creator_user_id, game_id, stake_kas, mode) VALUES ($1, $2, $3, $4) RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(user_id).bind(&payload.game_id).bind(payload.stake_kas).bind(payload.mode)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("SQL Error in create_challenge: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 2. Generate escrow address using EscrowService (deterministic from match ID)
    let escrow_addr = if let Some(ref escrow_svc) = state.escrow_service {
        match escrow_svc.create_escrow(&record.id.to_string(), payload.stake_kas as u64) {
            Ok(info) => {
                eprintln!(
                    "📝 Escrow created via EscrowService: {} (index {})",
                    info.escrow_address, info.derivation_index
                );
                info.escrow_address
            }
            Err(e) => {
                eprintln!("⚠️ EscrowService failed, using client address: {}", e);
                payload.escrow_address.clone().unwrap_or_default()
            }
        }
    } else {
        // Fallback: use client-provided address
        payload.escrow_address.clone().unwrap_or_default()
    };

    // 3. UPDATE match with escrow address
    let mut updated = sqlx::query_as::<_, Match>(
        "UPDATE matches SET escrow_address = $1 WHERE id = $2 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(&escrow_addr).bind(record.id)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("SQL Error updating escrow_address: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    updated.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&updated).unwrap_or_default());
    Ok(Json(updated))
}

pub async fn join_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let joiner_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // SICHERHEIT: AND creator_user_id != $1 verhindert, dass man gegen sich selbst spielt!
    let mut updated = sqlx::query_as::<_, Match>(
        "UPDATE matches SET opponent_user_id = $1, status = 'AWAITING_FUNDING' WHERE id = $2 AND status = 'OPEN' AND creator_user_id != $1 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(joiner_id).bind(id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    updated.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&updated).unwrap_or_default());
    Ok(Json(updated))
}

pub async fn submit_deposit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<DepositReq>,
) -> Result<Json<Match>, StatusCode> {
    eprintln!(
        "📥 Deposit received: match={}, tx_hash={}, player_role={}",
        id, payload.tx_hash, payload.player_role
    );

    // Step 1: Fetch the current match to determine player roles and current state
    let m: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| {
            eprintln!("❌ submit_deposit: match {} not found", id);
            StatusCode::NOT_FOUND
        })?;

    // Step 2: Validate state — only accept deposits in AWAITING_FUNDING
    use crate::models::MatchStatus;
    if m.status != MatchStatus::AwaitingFunding {
        eprintln!(
            "⚠️ submit_deposit: match {} is in state {:?}, not AWAITING_FUNDING — rejecting deposit",
            id, m.status
        );
        return Err(StatusCode::CONFLICT);
    }

    // Step 3: Determine which player column to update based on player_role
    // player_role "A" = creator, "B" = opponent
    let (tx_col, confirmed_col) = match payload.player_role.to_uppercase().as_str() {
        "A" => ("player_a_deposit_tx_hash", "player_a_deposit_confirmed"),
        "B" => ("player_b_deposit_tx_hash", "player_b_deposit_confirmed"),
        _ => {
            eprintln!(
                "❌ submit_deposit: invalid player_role '{}'",
                payload.player_role
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Step 4: Record this player's deposit TX hash and mark as confirmed
    sqlx::query(&format!(
        "UPDATE matches SET {} = $1, {} = true WHERE id = $2",
        tx_col, confirmed_col
    ))
    .bind(&payload.tx_hash)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        eprintln!("❌ SQL error recording deposit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Step 5: Check whether BOTH players have now deposited
    let updated: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let a_confirmed = updated.player_a_deposit_confirmed.unwrap_or(false);
    let b_confirmed = updated.player_b_deposit_confirmed.unwrap_or(false);

    let final_match = if a_confirmed && b_confirmed {
        // ✅ Both deposits received → advance to FUNDED (READY_TO_LOCK)
        // Episode-runner will then transition FUNDED → LOCKED when it calls execute()
        eprintln!("💰 Both deposits confirmed for match {} → FUNDED", id);
        sqlx::query_as("UPDATE matches SET status = 'FUNDED' WHERE id = $1 RETURNING *")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        // ⏳ Only one deposit so far → stay in AWAITING_FUNDING (PENDING_DEPOSITS)
        eprintln!(
            "⏳ Deposit 1/2 confirmed for match {} → staying AWAITING_FUNDING (A={}, B={})",
            id, a_confirmed, b_confirmed
        );
        updated
    };

    // Broadcast updated state via WebSocket
    let _ = state
        .tx
        .send(serde_json::to_string(&final_match).unwrap_or_default());

    Ok(Json(final_match))
}

/// Check deposit status for a match via EscrowService (on-chain balance check)
pub async fn check_deposits(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let escrow_svc = state.escrow_service.as_ref().ok_or_else(|| {
        eprintln!("❌ EscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Look up the match to get escrow_address and stake_kas
    let row: (Option<String>, i64) =
        sqlx::query_as("SELECT escrow_address, stake_kas FROM matches WHERE id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = row.0.unwrap_or_default();
    let stake_kas = row.1 as u64;

    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "NO_ESCROW",
            "message": "No escrow address assigned to this match"
        })));
    }

    match escrow_svc.check_deposits(&escrow_address, stake_kas).await {
        Ok(status) => Ok(Json(serde_json::to_value(status).unwrap())),
        Err(e) => {
            eprintln!("❌ check_deposits failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Request body for admin manual match resolution
#[derive(Deserialize)]
pub struct ResolveReq {
    pub winner: String, // "A" (creator) or "B" (opponent)
}

/// Faceit webhook payload (simplified)
#[derive(Deserialize)]
pub struct FaceitWebhookPayload {
    pub event: Option<String>,
    pub match_id: Option<String>,
    pub winner_faceit_id: Option<String>,
}

/// Admin manual match resolution: POST /matches/:id/resolve
/// Body: { "winner": "A" } or { "winner": "B" }
pub async fn admin_resolve_match(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ResolveReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    eprintln!("🔧 Admin resolve match {} → winner: {}", id, payload.winner);

    // Get the match
    let m = sqlx::query_as::<_, Match>(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE id = $1"
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Determine winner and loser addresses
    let creator_addr = get_user_kaspa_address(&state.pool, m.creator_user_id).await?;
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => get_user_kaspa_address(&state.pool, uid).await?,
        None => return Err(StatusCode::BAD_REQUEST), // No opponent yet
    };

    let winner_address = match payload.winner.as_str() {
        "A" | "a" | "creator" => &creator_addr,
        "B" | "b" | "opponent" => &opponent_addr,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Execute payout via PayoutService
    let result = execute_payout_for_match(&state, &m, &escrow_address, winner_address).await?;

    // Update match status to RESOLVED
    sqlx::query("UPDATE matches SET status = 'RESOLVED' WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(result))
}

/// Faceit webhook handler: POST /webhook/faceit
/// Receives match completion events from Faceit
pub async fn faceit_webhook(
    State(state): State<AppState>,
    Json(payload): Json<FaceitWebhookPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let event = payload.event.unwrap_or_default();
    let match_id = payload.match_id.unwrap_or_default();
    let winner_faceit_id = payload.winner_faceit_id.unwrap_or_default();

    eprintln!(
        "📨 Faceit webhook: event={}, match_id={}, winner={}",
        event, match_id, winner_faceit_id
    );

    if event != "match_status_finished" && !event.is_empty() {
        return Ok(Json(
            serde_json::json!({ "status": "ignored", "event": event }),
        ));
    }

    // Find match by external_match_id (Faceit match ID)
    let m = sqlx::query_as::<_, Match>(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE external_match_id = $1"
    )
    .bind(&match_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let m = match m {
        Some(m) => m,
        None => {
            eprintln!("⚠️ No match found for faceit match_id: {}", match_id);
            return Ok(Json(
                serde_json::json!({ "status": "no_match", "match_id": match_id }),
            ));
        }
    };

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({ "status": "no_escrow" })));
    }

    // Determine winner address from faceit_id
    // Look up which user has this faceit_id
    let winner_user: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, kaspa_address FROM users WHERE faceit_id = $1")
            .bind(&winner_faceit_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let winner_address = match winner_user {
        Some((_, addr)) => addr,
        None => {
            eprintln!(
                "⚠️ Winner faceit_id {} not found in users",
                winner_faceit_id
            );
            return Ok(Json(serde_json::json!({
                "status": "winner_not_found",
                "winner_faceit_id": winner_faceit_id
            })));
        }
    };

    // Execute payout
    let result = execute_payout_for_match(&state, &m, &escrow_address, &winner_address).await?;

    // Update match status
    sqlx::query("UPDATE matches SET status = 'RESOLVED' WHERE id = $1")
        .bind(m.id)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(result))
}

/// Helper: get a user's Kaspa address from their UUID
async fn get_user_kaspa_address(pool: &PgPool, user_id: Uuid) -> Result<String, StatusCode> {
    let row: (String,) = sqlx::query_as("SELECT kaspa_address FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(row.0)
}

/// Helper: execute payout for a match using PayoutService
async fn execute_payout_for_match(
    state: &AppState,
    m: &Match,
    escrow_address: &str,
    winner_address: &str,
) -> Result<serde_json::Value, StatusCode> {
    let payout_svc = state.payout_service.as_ref().ok_or_else(|| {
        eprintln!("❌ PayoutService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Register escrow key with PayoutService (derive from wallet)
    if let Some(ref wallet) = state.escrow_wallet {
        let match_id_str = m.id.to_string();
        if let Ok((addr, _)) = wallet.derive_escrow_address(&match_id_str) {
            if let Ok(privkey) = wallet.get_private_key(&addr) {
                payout_svc
                    .register_escrow_key(escrow_address.to_string(), privkey)
                    .await;
                eprintln!("🔑 Escrow key registered for {}", escrow_address);
            }
        }
    }

    // Build a BattleMatch from DB data
    let creator_addr = get_user_kaspa_address(&state.pool, m.creator_user_id)
        .await
        .unwrap_or_else(|_| "unknown".to_string());
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => get_user_kaspa_address(&state.pool, uid)
            .await
            .unwrap_or_else(|_| "unknown".to_string()),
        None => "unknown".to_string(),
    };

    let battle_match = battle_core::models::match_::BattleMatch {
        id: m.id,
        player_a_kas_address: creator_addr,
        player_b_kas_address: opponent_addr,
        player_a_faceit_id: String::new(),
        player_b_faceit_id: String::new(),
        faceit_match_id: m.external_match_id.clone(),
        wager_amount_sompi: (m.stake_kas as u64) * 100_000, // KAS → sompi
        escrow_address: escrow_address.to_string(),
        status: battle_core::models::match_::MatchStatus::Resolved,
        winner_kas_address: Some(winner_address.to_string()),
        payout_tx_hash: None,
        oracle_result_signature: None,
        created_at: m.created_at.unwrap_or_else(chrono::Utc::now),
        locked_at: None,
        resolved_at: Some(chrono::Utc::now()),
        timeout_at: m.created_at.unwrap_or_else(chrono::Utc::now) + chrono::Duration::minutes(90),
    };

    match payout_svc
        .execute_payout(&battle_match, winner_address)
        .await
    {
        Ok(result) => {
            eprintln!(
                "✅ Payout executed: TX={}, winner={} sompi, treasury={} sompi",
                result.winner_tx_id, result.winner_amount_sompi, result.treasury_amount_sompi
            );
            Ok(serde_json::json!({
                "status": "PAYOUT_COMPLETE",
                "tx_id": result.winner_tx_id,
                "winner_amount_sompi": result.winner_amount_sompi,
                "treasury_amount_sompi": result.treasury_amount_sompi,
                "fee_sompi": result.fee_sompi,
            }))
        }
        Err(e) => {
            eprintln!("❌ Payout failed: {:?}", e);
            Ok(serde_json::json!({
                "status": "PAYOUT_FAILED",
                "error": format!("{}", e),
            }))
        }
    }
}

// ── v0.2: FaceID Endpoint ─────────────────────────────────────────────────

/// Request body for FaceID hash submission
#[derive(Deserialize)]
pub struct FaceIdReq {
    pub hash: String, // SHA-256 of biometric template (off-chain, anti-fraud only)
}

/// POST /matches/:id/faceid — optional FaceID hash upload
///
/// Player submits a hash of their FaceID verification. Not required for match to
/// proceed (no blocker). Useful for dispute resolution.
pub async fn submit_faceid_handler(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<FaceIdReq>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let m: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Determine player role (A = creator, B = opponent)
    let col = if m.creator_user_id == user_id {
        "player_a_faceid_hash"
    } else if m.opponent_user_id == Some(user_id) {
        "player_b_faceid_hash"
    } else {
        return Err(StatusCode::FORBIDDEN);
    };

    let updated: Match = sqlx::query_as(&format!(
        "UPDATE matches SET {} = $1 WHERE id = $2 RETURNING *",
        col
    ))
    .bind(&payload.hash)
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    eprintln!("🪪  Match {}: {} FaceID hash recorded", id, col);
    Ok(Json(updated))
}

// ── v0.2: Cancel Endpoint ─────────────────────────────────────────────────

/// POST /matches/:id/cancel — cancel an open or pending match
///
/// Only the match creator can cancel, and only while status is OPEN or AWAITING_FUNDING.
/// Does NOT auto-refund deposits (manual process for now).
pub async fn cancel_match_handler(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Only creator can cancel, only from non-locked states
    let updated: Match = sqlx::query_as(
        "UPDATE matches SET status = 'CANCELLED' \
         WHERE id = $1 \
           AND creator_user_id = $2 \
           AND status IN ('OPEN', 'AWAITING_FUNDING') \
         RETURNING *",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    eprintln!("❌ Match {} cancelled by creator {}", id, user_id);
    // TODO: trigger EscrowService.refund() when deposits exist
    Ok(Json(updated))
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        let mut rx = state.tx.subscribe();
        let (mut sender, _) = socket.split();
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    })
}
