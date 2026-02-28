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

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lobbies", get(get_lobbies))
        .route("/history", get(get_history))
        .route("/challenges", post(create_challenge))
        .route("/challenges/:id/join", post(join_challenge))
}

pub async fn get_lobbies(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let rows = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status IN ('OPEN', 'LOCKED', 'AWAITING_FUNDING')")
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

pub async fn get_history(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let rows = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status = 'RESOLVED'")
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

pub async fn create_challenge(
    State(state): State<AppState>,
    Json(payload): Json<CreateReq>,
) -> Result<Json<Match>, StatusCode> {
    let mock_user_id = Uuid::nil();
    let record = sqlx::query_as::<_, Match>(
        "INSERT INTO matches (creator_user_id, game_id, stake_kas, mode) VALUES ($1, $2, $3, $4) RETURNING id, onchain_match_id, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(mock_user_id).bind(&payload.game_id).bind(payload.stake_kas).bind(payload.mode)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
    Ok(Json(record))
}

pub async fn join_challenge(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let mock_user_id = Uuid::nil();
    let record = sqlx::query_as::<_, Match>(
        "UPDATE matches SET status = 'AWAITING_FUNDING', opponent_user_id = $1 WHERE id = $2 AND status = 'OPEN' RETURNING id, onchain_match_id, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(mock_user_id).bind(id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::BAD_REQUEST)?;

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
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
