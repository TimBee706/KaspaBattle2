//! HTTP handlers for Free Play (off-chain Connect Four). Logic: `crate::free_play`.
//!
//! Every endpoint needs a logged-in session but **no wallet and no FACEIT** (`SessionUserNoWallet`),
//! and none of them can reach the Kaspa RPC, an escrow or a payout – by construction these code
//! paths never import that machinery. Request bodies reject unknown fields, so a manipulated
//! request carrying `stake`, `wager`, `escrow`, `mode` etc. is refused with 422.

use crate::api::auth_guard::SessionUserNoWallet;
use crate::api::native::{err, ApiError};
use crate::api::AppState;
use crate::free_play::{self, FpError, MoveRequest, Opponent, Outcome};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use battle_core::native_games::connect_four::MoveError;
use battle_core::native_games::connect_four_bot::Difficulty;
use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

impl From<FpError> for ApiError {
    fn from(e: FpError) -> Self {
        match e {
            FpError::NotFound => err(StatusCode::NOT_FOUND, "game_not_found", "Game not found."),
            FpError::NotParticipant => err(StatusCode::FORBIDDEN, "not_participant", "Only the players of this game may do that."),
            FpError::WrongStatus(code) => err(StatusCode::CONFLICT, code, "The game is not in a state that allows this."),
            FpError::VersionConflict { current_version } => ApiError(
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "version_conflict", "message": "The game changed; reload the state.", "currentVersion": current_version }),
            ),
            FpError::Move(MoveError::Unauthorized) => err(StatusCode::FORBIDDEN, "not_participant", "You are not a player of this game."),
            FpError::Move(MoveError::GameOver) => err(StatusCode::CONFLICT, "game_over", "The game is already finished."),
            FpError::Move(MoveError::NotYourTurn) => err(StatusCode::CONFLICT, "not_your_turn", "It is not your turn."),
            FpError::Move(MoveError::InvalidColumn(_)) => err(StatusCode::BAD_REQUEST, "invalid_column", "Column must be between 0 and 6."),
            FpError::Move(MoveError::ColumnFull(_)) => err(StatusCode::UNPROCESSABLE_ENTITY, "column_full", "That column is full."),
            FpError::BadRequest(m) => err(StatusCode::BAD_REQUEST, "bad_request", m),
            FpError::Conflict(code) => err(StatusCode::CONFLICT, code, "This is not possible right now."),
            FpError::TooMany(code) => err(StatusCode::TOO_MANY_REQUESTS, code, "You have too many games open. Finish or close one first."),
            FpError::Db(e) => {
                tracing::error!(error = %e, "free play: database error");
                err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "Internal error.")
            }
            FpError::Internal(m) => {
                tracing::error!(error = %m, "free play: internal error");
                err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Internal error.")
            }
        }
    }
}

type Res = Result<Json<serde_json::Value>, ApiError>;

fn respond(state: &AppState, out: Outcome, viewer: Uuid) -> Json<serde_json::Value> {
    free_play::publish(&state.tx, &out.events);
    let mut body = serde_json::to_value(&out.snapshot).unwrap_or_default();
    body["replay"] = serde_json::json!(out.replay);
    attach_viewer(state, &mut body, &out.snapshot, viewer);
    Json(body)
}

/// `you` + live presence of the players (derived from WebSocket connections).
fn attach_viewer(state: &AppState, body: &mut serde_json::Value, snap: &free_play::FpSnapshot, viewer: Uuid) {
    let slot = snap.players.iter().find(|p| p.user_id == Some(viewer)).map(|p| p.slot);
    body["you"] = serde_json::json!({ "userId": viewer, "slot": slot });
    let presence: serde_json::Map<String, serde_json::Value> = snap
        .players
        .iter()
        .filter_map(|p| p.user_id)
        .map(|u| (u.to_string(), serde_json::json!(state.presence.is_connected(snap.id, u))))
        .collect();
    body["presence"] = serde_json::Value::Object(presence);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateBody {
    /// "human" | "bot"
    pub opponent: String,
    pub bot_difficulty: Option<String>,
    /// Only "connect_four" exists.
    #[serde(default)]
    pub game: Option<String>,
}

fn throttle(state: &AppState, key: String, limit: usize, secs: u64) -> Result<(), ApiError> {
    if state.account.throttle.allow(&key, limit, Duration::from_secs(secs)) {
        Ok(())
    } else {
        Err(err(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many requests. Please slow down."))
    }
}

pub async fn create_game(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Json(b): Json<CreateBody>,
) -> Res {
    throttle(&state, format!("fp:create:{}", user.id), 20, 600)?;
    if b.game.as_deref().map(|g| g != "connect_four").unwrap_or(false) {
        return Err(err(StatusCode::BAD_REQUEST, "unsupported_game", "Only connect_four is available."));
    }
    let opponent = match (b.opponent.as_str(), b.bot_difficulty.as_deref()) {
        ("human", None) => Opponent::Human,
        ("bot", d) => {
            let d = Difficulty::parse(d.unwrap_or("easy"))
                .ok_or_else(|| err(StatusCode::BAD_REQUEST, "invalid_difficulty", "Difficulty must be easy, medium or hard."))?;
            Opponent::Bot(d)
        }
        ("human", Some(_)) => return Err(err(StatusCode::BAD_REQUEST, "bad_request", "botDifficulty only applies to bot games.")),
        _ => return Err(err(StatusCode::BAD_REQUEST, "invalid_opponent", "opponent must be human or bot.")),
    };
    let out = free_play::create_game(&state.pool, user.id, opponent).await?;
    Ok(respond(&state, out, user.id))
}

pub async fn lobbies(State(state): State<AppState>, SessionUserNoWallet(user): SessionUserNoWallet) -> Res {
    let items = free_play::list_open(&state.pool, user.id).await?;
    let active = free_play::list_active(&state.pool, user.id).await?;
    Ok(Json(serde_json::json!({ "lobbies": items, "active": active })))
}

pub async fn get_game(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Path(id): Path<Uuid>,
) -> Res {
    let snap = free_play::get(&state.pool, id).await?;
    let mut body = serde_json::to_value(&snap).unwrap_or_default();
    attach_viewer(&state, &mut body, &snap, user.id);
    Ok(Json(body))
}

pub async fn join_game(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Path(id): Path<Uuid>,
) -> Res {
    let out = free_play::join(&state.pool, id, user.id).await?;
    Ok(respond(&state, out, user.id))
}

pub async fn leave_game(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Path(id): Path<Uuid>,
) -> Res {
    let out = free_play::leave(&state.pool, id, user.id).await?;
    Ok(respond(&state, out, user.id))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MoveBody {
    pub column: i64,
    pub expected_version: i64,
    pub client_nonce: String,
}

pub async fn post_move(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Path(id): Path<Uuid>,
    Json(b): Json<MoveBody>,
) -> Res {
    throttle(&state, format!("fp:move:{}", user.id), 120, 60)?;
    let out = free_play::apply_move(
        &state.pool,
        id,
        user.id,
        MoveRequest { column: b.column, expected_version: b.expected_version, client_nonce: b.client_nonce },
    )
    .await?;
    Ok(respond(&state, out, user.id))
}

pub async fn rematch(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Path(id): Path<Uuid>,
) -> Res {
    throttle(&state, format!("fp:rematch:{}", user.id), 20, 600)?;
    let out = free_play::request_rematch(&state.pool, id, user.id).await?;
    Ok(respond(&state, out, user.id))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
}

pub async fn history(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Query(q): Query<HistoryQuery>,
) -> Res {
    let items = free_play::history(&state.pool, user.id, q.limit.unwrap_or(20)).await?;
    Ok(Json(serde_json::json!({ "games": items })))
}

pub async fn stats(State(state): State<AppState>, SessionUserNoWallet(user): SessionUserNoWallet) -> Res {
    let s = free_play::stats(&state.pool, user.id).await?;
    Ok(Json(serde_json::to_value(s).unwrap_or_default()))
}
