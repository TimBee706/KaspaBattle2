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
}

#[derive(Serialize, Deserialize)]
pub struct DepositReq {
    pub tx_hash: String,
    pub player_role: String, // "A" oder "B"
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
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lobbies", get(get_lobbies))
        .route("/history", get(get_history))
        .route("/challenges", post(create_challenge))
        .route("/matches/:id/accept", post(join_challenge))
        .route("/matches/:id/deposit", post(submit_deposit))
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
    let rows = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, 'kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2' as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status IN ('OPEN', 'LOCKED', 'AWAITING_FUNDING')")
        .fetch_all(&state.pool).await.map_err(|e| {
            eprintln!("SQL Error in get_lobbies: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(rows))
}

pub async fn get_history(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let rows = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, 'kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2' as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status = 'RESOLVED'")
        .fetch_all(&state.pool).await.map_err(|e| {
            eprintln!("SQL Error in get_history: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(rows))
}

pub async fn create_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Json(payload): Json<CreateReq>,
) -> Result<Json<Match>, StatusCode> {
    // 1. Reale ID aus der verifizierten Session extrahieren
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record = sqlx::query_as::<_, Match>(
        "INSERT INTO matches (creator_user_id, game_id, stake_kas, mode) VALUES ($1, $2, $3, $4) RETURNING id, onchain_match_id, 'kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2' as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(user_id).bind(&payload.game_id).bind(payload.stake_kas).bind(payload.mode)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("SQL Error in create_challenge: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
    Ok(Json(record))
}

pub async fn join_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let joiner_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // SICHERHEIT: AND creator_user_id != $1 verhindert, dass man gegen sich selbst spielt!
    let record = sqlx::query_as::<_, Match>(
        "UPDATE matches SET status = 'AWAITING_FUNDING', opponent_user_id = $1 WHERE id = $2 AND status = 'OPEN' AND creator_user_id != $1 RETURNING id, onchain_match_id, 'kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2' as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(joiner_id).bind(id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::FORBIDDEN)?; // Fehlschlag (z.B. Self-Join) liefert 403

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
    Ok(Json(record))
}

pub async fn submit_deposit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<DepositReq>,
) -> Result<Json<Match>, StatusCode> {
    let field = if payload.player_role == "A" {
        "player_a_deposit_tx_hash"
    } else {
        "player_b_deposit_tx_hash"
    };

    let query = format!(
        "UPDATE matches SET {} = $1, status = CASE WHEN (player_a_deposit_tx_hash IS NOT NULL OR player_b_deposit_tx_hash IS NOT NULL) THEN 'FUNDED' ELSE status END WHERE id = $2 RETURNING id, onchain_match_id, 'kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2' as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at",
        field
    );

    let record = sqlx::query_as::<_, Match>(&query)
        .bind(&payload.tx_hash)
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("SQL Error in submit_deposit: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(record))
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
