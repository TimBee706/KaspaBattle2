pub mod admin_guard;
pub mod auth_guard;
pub mod faceit;

use crate::api::{admin_guard::AdminApiKey, auth_guard::SessionUser};
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
use battle_core::models::wallet_login::WalletLoginReq;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
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
use battle_core::faceit_data::FaceitDataService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
    pub auth_service: Arc<AuthService>,
    pub faceit_service: Arc<FaceitOAuthService>,
    pub faceit_data_service: Option<Arc<FaceitDataService>>,
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
        // NOTE: /lobbies/:id/simulate-deposit removed — test-only endpoint
        .route("/webhook/faceit", post(faceit_webhook))
        .route("/auth/me", get(get_me))
        .route("/auth/wallet-login", post(wallet_login))
        .route("/ws", get(ws_handler))
        .nest("/faceit", faceit::router())
}

pub async fn get_me(
    State(state): State<AppState>,
    user_opt: Option<crate::api::auth_guard::SessionUserNoWallet>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match user_opt {
        Some(crate::api::auth_guard::SessionUserNoWallet(user)) => {
            // Enrich with FaceIT data from faceit_links table
            let faceit_row = sqlx::query(
                "SELECT faceit_player_id, faceit_nickname, faceit_avatar_url, faceit_elo, faceit_skill_level FROM faceit_links WHERE user_id = $1::uuid"
            )
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();

            let (faceit_connected, faceit_id, faceit_nickname, faceit_avatar, faceit_elo, faceit_skill_level) = if let Some(r) = &faceit_row {
                use sqlx::Row;
                let fid: String = r.try_get("faceit_player_id").unwrap_or_default();
                let fnick: String = r.try_get("faceit_nickname").unwrap_or_default();
                let favatar: Option<String> = r.try_get("faceit_avatar_url").unwrap_or(None);
                let felo: Option<i32> = r.try_get("faceit_elo").unwrap_or(None);
                let fskill: Option<i32> = r.try_get("faceit_skill_level").unwrap_or(None);
                (true, fid, fnick, favatar.unwrap_or_default(), felo, fskill)
            } else {
                // NOT connected: do NOT fallback to display_name for faceit_nickname
                (false, String::new(), String::new(), String::new(), None, None)
            };

            Ok(Json(serde_json::json!({
                "id": user.id,
                "email": user.email,
                "display_name": user.display_name,
                "kaspa_address": user.kaspa_address,
                "faceit_connected": faceit_connected,
                "faceit_id": faceit_id,
                "faceit_nickname": faceit_nickname,
                "faceit_avatar": faceit_avatar,
                "faceit_elo": faceit_elo,
                "faceit_skill_level": faceit_skill_level,
                "created_at": user.created_at,
                "total_matches": 0,
                "wins": 0,
                "losses": 0,
                "total_wagered_sompi": 0,
                "total_won_sompi": 0,
            })))
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: String,
    pub session_token: String,
    pub display_name: String,
}

pub async fn wallet_login(
    State(state): State<AppState>,
    Json(payload): Json<WalletLoginReq>,
) -> Result<Json<AuthResponse>, StatusCode> {
    // 1. Kaspa Signatur verifizieren (kdapp/kaspa-wasm Logik)
    // TODO: Actually verify the Kaspa signature when Kaspa integration is fully available.
    // For now, we trust the incoming address since this is the first step of the fix
    // (In production, replace this with a proper `verify_kaspa_signature(...)`).
    let is_valid = !payload.signature.is_empty(); // dummy check

    if !is_valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 2. User in DB suchen oder neu anlegen (Guest)
    let uuid_str = format!("{:x}", md5::compute(payload.kaspa_address.as_bytes()));
    let fake_uuid = uuid::Uuid::parse_str(&format!(
        "{}-{}-{}-{}-{}",
        &uuid_str[0..8],
        &uuid_str[8..12],
        &uuid_str[12..16],
        &uuid_str[16..20],
        &uuid_str[20..32]
    ))
    .unwrap();

    // Minimal find or create fallback:
    let row = sqlx::query("SELECT id FROM users WHERE kaspa_address = $1")
        .bind(&payload.kaspa_address)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id: uuid::Uuid = if let Some(r) = row {
        r.try_get("id").unwrap_or(fake_uuid)
    } else {
        let new_user_id = uuid::Uuid::new_v4();
        let display_name = format!(
            "Player_{}",
            &payload
                .kaspa_address
                .chars()
                .skip(6)
                .take(6)
                .collect::<String>()
        );
        let email = format!("{}@wallet.local", new_user_id);
        let password_hash =
            battle_core::auth::AuthService::hash_password(&uuid::Uuid::new_v4().to_string())
                .unwrap_or_default();

        sqlx::query("INSERT INTO users (id, email, password_hash, display_name, kaspa_address) VALUES ($1, $2, $3, $4, $5)")
            .bind(new_user_id)
            .bind(email)
            .bind(password_hash)
            .bind(&display_name)
            .bind(&payload.kaspa_address)
            .execute(&state.pool)
            .await.map_err(|e| {
                eprintln!("Error creating user: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        new_user_id
    };

    // 3. Session über AuthService generieren
    let session_token = battle_core::auth::AuthService::generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&session_token)
        .bind(&user_id)
        .bind(expires_at)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AuthResponse {
        user_id: user_id.to_string(),
        session_token,
        display_name: format!(
            "Player_{}",
            &payload
                .kaspa_address
                .chars()
                .skip(6)
                .take(6)
                .collect::<String>()
        ),
    }))
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
        "INSERT INTO matches (creator_user_id, game_id, stake_kas, mode) VALUES ($1, $2, $3, $4) RETURNING *"
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
        "UPDATE matches SET escrow_address = $1 WHERE id = $2 RETURNING *",
    )
    .bind(&escrow_addr)
    .bind(record.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
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

    let current_match: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    current_match
        .validate_action(&MatchAction::Join {
            player_id: joiner_id.to_string(),
            kaspa_address: user.kaspa_address.clone().unwrap_or_default(),
        })
        .map_err(|e| {
            tracing::warn!("State machine rejected Join: {}", e);
            StatusCode::CONFLICT
        })?;

    // SICHERHEIT: AND creator_user_id != $1 verhindert, dass man gegen sich selbst spielt!
    let mut updated = sqlx::query_as::<_, Match>(
        "UPDATE matches SET opponent_user_id = $1, status = $3 WHERE id = $2 AND status = $4 AND creator_user_id != $1 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at, wager_amount_sompi, player_a_deposit_tx_hash, player_b_deposit_tx_hash, player_a_deposit_confirmed, player_b_deposit_confirmed, player_a_faceid_hash, player_b_faceid_hash"
    )
    .bind(joiner_id)
    .bind(id)
    .bind(crate::models::MatchStatus::AwaitingFunding)
    .bind(crate::models::MatchStatus::Open)
    .fetch_one(&state.pool).await.map_err(|e| {
        tracing::error!("Failed to update match status in join_challenge: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    updated.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&updated).unwrap_or_default());
    Ok(Json(updated))
}

pub async fn submit_deposit(
    State(state): State<AppState>,
    session: SessionUser, // M-05: authentication required
    Path(id): Path<Uuid>,
    Json(payload): Json<DepositReq>,
) -> Result<Json<Match>, StatusCode> {
    let caller_id = &session.0.id;
    tracing::info!(
        "Deposit received: match={}, player_role={} caller={}",
        id,
        payload.player_role,
        caller_id
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

    // Step 2: Validate state — only accept deposits in AWAITING_FUNDING/FUNDED
    // M-08 validation
    use battle_core::match_state::MatchAction;
    m.validate_action(&MatchAction::DepositConfirmed {
        player_id: caller_id.clone(),
        tx_hash: payload.tx_hash.clone(),
        amount: m.stake_kas as u64,
    })
    .map_err(|e| {
        tracing::warn!("State machine rejected DepositConfirmed: {}", e);
        StatusCode::CONFLICT
    })?;

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

    // Step 4: Record this player's deposit TX hash (DO NOT mark as confirmed yet - Episode handles this)
    sqlx::query(&format!(
        "UPDATE matches SET {} = $1 WHERE id = $2",
        tx_col
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

    // We no longer transition to FUNDED here. The MatchEpisode (Blockchain Watcher)
    // is responsible for confirming the actual UTXO and setting the status.
    eprintln!(
        "⏳ Deposit TX recorded for match {}. Waiting for blockchain confirmation.",
        id
    );

    let final_match = updated;

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
    _admin: AdminApiKey, // M-03: admin API key required
    Path(id): Path<Uuid>,
    Json(payload): Json<ResolveReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("Admin resolve match {} → winner: {}", id, payload.winner);

    // Get the match
    let m = sqlx::query_as::<_, Match>("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Determine winner address
    let creator_addr = get_user_kaspa_address(&state.pool, m.creator_user_id).await?;
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => get_user_kaspa_address(&state.pool, uid).await?,
        None => return Err(StatusCode::BAD_REQUEST), // No opponent yet
    };

    let (winner_address, winner_id) = match payload.winner.as_str() {
        "A" | "a" | "creator" => (&creator_addr, m.creator_user_id.to_string()),
        "B" | "b" | "opponent" => (&opponent_addr, m.opponent_user_id.unwrap().to_string()),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    m.validate_action(&MatchAction::ResolveWinner {
        winner_id: winner_id.clone(),
    })
    .map_err(|e| {
        tracing::warn!("State machine rejected ResolveWinner: {}", e);
        StatusCode::CONFLICT
    })?;

    // Execute payout via PayoutService
    let result = execute_payout_for_match(&state, &m, &escrow_address, winner_address).await?;

    // Update match status to PAID_OUT
    sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(result))
}

/// Faceit webhook handler: POST /webhook/faceit
/// Receives match completion events from Faceit
///
/// M-04: Validates the FaceIT webhook HMAC signature from the
/// `Faceit-Signature` header to ensure requests are authentic.
pub async fn faceit_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Validate HMAC-SHA256 signature
    let webhook_secret = std::env::var("FACEIT_WEBHOOK_SECRET").unwrap_or_default();
    if !webhook_secret.is_empty() {
        let signature = headers
            .get("Faceit-Signature")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !verify_faceit_hmac(&body, &webhook_secret, signature) {
            tracing::warn!("Faceit webhook: HMAC signature mismatch — rejecting request");
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        tracing::warn!("FACEIT_WEBHOOK_SECRET not set — skipping signature validation (insecure!)");
    }

    // Deserialize body now that signature is verified
    let payload: FaceitWebhookPayload =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let event = payload.event.unwrap_or_default();
    let match_id = payload.match_id.unwrap_or_default();
    let winner_faceit_id = payload.winner_faceit_id.unwrap_or_default();

    tracing::info!(
        "Faceit webhook verified: event={}, match_id={}, winner={}",
        event,
        match_id,
        winner_faceit_id
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
    sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
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
        // IMPORTANT: stake_kas stores the wager in Sompi (the frontend converts
        // KAS → Sompi before POSTing to /challenges, so no multiplication here).
        // See battle-frontend/src/hooks/useLobby.ts: stake_kas = stakeKas * 100_000_000
        wager_amount_sompi: m.stake_kas as u64,
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

pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: axum::extract::ws::WebSocket, state: AppState) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = state.tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // axum 0.7 requires Utf8Bytes, which implements From<String>
            if sender.send(axum::extract::ws::Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {
            // keep-alive / ignore incoming messages
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
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

    let current_match: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    current_match
        .validate_action(&MatchAction::Cancel {
            player_id: user_id.to_string(),
            reason: "User cancelled match".to_string(),
        })
        .map_err(|e| {
            tracing::warn!("State machine rejected Cancel: {}", e);
            // Special case: if it conflicts, return 409
            StatusCode::CONFLICT
        })?;

    // Transition authorized. EITHER player can cancel if the state machine allows it.
    let updated: Match = sqlx::query_as(
        "UPDATE matches SET status = 'CANCELLED' \
         WHERE id = $1 \
         RETURNING *",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("❌ Match {} cancelled by {}", id, user_id);

    // M-09: Trigger refund process if an escrow address exists
    // PayoutService will check on-chain balance and execute refund if deposits exist
    if let Some(escrow) = updated.escrow_address.as_ref() {
        if !escrow.is_empty() {
            let state_clone = state.clone();
            let m_clone = updated.clone();
            let escrow_clone = escrow.clone();
            tokio::spawn(async move {
                let _ = execute_refund_for_match(&state_clone, &m_clone, &escrow_clone).await;
            });
        }
    }

    Ok(Json(updated))
}


/// Verify a FaceIT webhook HMAC-SHA256 signature.
///
/// FaceIT signs POST bodies with HMAC-SHA256 using the webhook secret and
/// sends the hex-encoded signature in the `Faceit-Signature` header.
/// Returns `true` if the signature matches, `false` otherwise.
fn verify_faceit_hmac(body: &[u8], secret: &str, signature: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let result = mac.finalize().into_bytes();
    let expected_hex = hex::encode(result);

    // Constant-time comparison
    let sig_bytes = signature.as_bytes();
    let exp_bytes = expected_hex.as_bytes();
    if sig_bytes.len() != exp_bytes.len() {
        return false;
    }
    sig_bytes
        .iter()
        .zip(exp_bytes.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Helper: execute refund for a cancelled match using PayoutService
async fn execute_refund_for_match(
    state: &AppState,
    m: &Match,
    escrow_address: &str,
) -> Result<(String, String), StatusCode> {
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
                eprintln!("🔑 Escrow key registered for refund {}", escrow_address);
            }
        }
    }

    // Build a BattleMatch from DB data
    let creator_addr = crate::api::get_user_kaspa_address(&state.pool, m.creator_user_id)
        .await
        .unwrap_or_else(|_| "unknown".to_string());
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => crate::api::get_user_kaspa_address(&state.pool, uid)
            .await
            .unwrap_or_else(|_| "unknown".to_string()),
        None => "unknown".to_string(), // Can be none if cancelled before join
    };

    let battle_match = battle_core::models::match_::BattleMatch {
        id: m.id,
        player_a_kas_address: creator_addr,
        player_b_kas_address: opponent_addr,
        player_a_faceit_id: String::new(),
        player_b_faceit_id: String::new(),
        faceit_match_id: m.external_match_id.clone(),
        wager_amount_sompi: m.stake_kas as u64,
        escrow_address: escrow_address.to_string(),
        status: battle_core::models::match_::MatchStatus::Cancelled,
        winner_kas_address: None,
        payout_tx_hash: None,
        oracle_result_signature: None,
        created_at: m.created_at.unwrap_or_else(chrono::Utc::now),
        locked_at: None,
        resolved_at: None,
        timeout_at: m.created_at.unwrap_or_else(chrono::Utc::now) + chrono::Duration::minutes(90),
    };

    match payout_svc.execute_refund(&battle_match).await {
        Ok(res) => {
            eprintln!("✅ Refund executed: TX A={}, TX B={}", res.0, res.1);
            Ok(res)
        }
        Err(e) => {
            eprintln!("❌ Refund failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
