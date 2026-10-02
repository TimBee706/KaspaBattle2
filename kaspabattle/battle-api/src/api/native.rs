//! HTTP handlers for native (browser) games.
//!
//! | Method | Path (under `/api/v1`)                     | Auth            |
//! |--------|--------------------------------------------|-----------------|
//! | GET    | `/matches/:id/game`                        | optional        |
//! | POST   | `/matches/:id/game/start`                  | session+wallet  |
//! | POST   | `/matches/:id/connect-four/moves`          | session+wallet  |
//! | POST   | `/matches/:id/game/resign`                 | session+wallet  |
//! | GET    | `/features`                                | none            |

use crate::api::auth_guard::{SessionUser, SessionUserNoWallet};
use crate::api::AppState;
use crate::native_game::{self, MoveRequest, NativeError, Outcome};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use battle_core::native_games::connect_four::MoveError;
use serde::Deserialize;
use uuid::Uuid;

/// `POST .../connect-four/moves` body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveBody {
    pub column: i64,
    pub expected_version: i64,
    pub client_nonce: String,
}

pub struct ApiError(pub StatusCode, pub serde_json::Value);

pub fn err(status: StatusCode, code: &str, message: &str) -> ApiError {
    ApiError(
        status,
        serde_json::json!({ "error": code, "message": message }),
    )
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

impl From<NativeError> for ApiError {
    fn from(e: NativeError) -> Self {
        match e {
            NativeError::NotFound(code) => err(
                StatusCode::NOT_FOUND,
                code,
                "Match or game session not found",
            ),
            NativeError::NotParticipant => err(
                StatusCode::FORBIDDEN,
                "not_participant",
                "Only the two players of this match may do that",
            ),
            NativeError::WrongStatus(msg) => ApiError(
                StatusCode::CONFLICT,
                serde_json::json!({ "error": "wrong_status", "message": msg }),
            ),
            NativeError::VersionConflict { current_version } => ApiError(
                StatusCode::CONFLICT,
                serde_json::json!({
                    "error": "version_conflict",
                    "message": "The game changed; reload the game state",
                    "currentVersion": current_version,
                }),
            ),
            NativeError::Move(MoveError::Unauthorized) => err(
                StatusCode::FORBIDDEN,
                "not_participant",
                "You are not a player of this game",
            ),
            NativeError::Move(MoveError::GameOver) => err(
                StatusCode::CONFLICT,
                "game_over",
                "The game is already finished",
            ),
            NativeError::Move(MoveError::NotYourTurn) => {
                err(StatusCode::CONFLICT, "not_your_turn", "It is not your turn")
            }
            NativeError::Move(MoveError::InvalidColumn(_)) => err(
                StatusCode::BAD_REQUEST,
                "invalid_column",
                "Column must be between 0 and 6",
            ),
            NativeError::Move(MoveError::ColumnFull(_)) => err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "column_full",
                "That column is full",
            ),
            NativeError::BadRequest(msg) => err(StatusCode::BAD_REQUEST, "bad_request", msg),
            NativeError::Db(e) => {
                tracing::error!(error = %e, "native game: database error");
                err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "db_error",
                    "Internal error",
                )
            }
            NativeError::Internal(m) => {
                tracing::error!(error = %m, "native game: internal error");
                err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal error",
                )
            }
        }
    }
}

async fn respond(state: &AppState, match_id: Uuid, outcome: Outcome) -> Json<serde_json::Value> {
    // Events go out only now, after the service committed its transaction.
    native_game::publish(&state.pool, &state.tx, match_id, &outcome.events).await;
    let mut body = serde_json::to_value(&outcome.snapshot).unwrap_or_default();
    body["replay"] = serde_json::json!(outcome.replay);
    Json(body)
}

/// `GET /matches/:id/game` — full snapshot; the client calls this first on load and on every reconnect.
pub async fn get_game(
    State(state): State<AppState>,
    Path(match_id): Path<Uuid>,
    user: Option<SessionUserNoWallet>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(NativeError::Db)?;
    let snapshot = native_game::load_snapshot(&mut conn, match_id).await?;
    let Some(snapshot) = snapshot else {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM matches WHERE id = $1 AND provider = 'NATIVE')",
        )
        .bind(match_id)
        .fetch_one(&state.pool)
        .await
        .map_err(NativeError::Db)?;
        return Err(if exists {
            err(
                StatusCode::NOT_FOUND,
                "game_not_started",
                "The game session has not been created yet",
            )
        } else {
            err(
                StatusCode::NOT_FOUND,
                "match_not_found",
                "No native game for this match",
            )
        });
    };
    let mut body = serde_json::to_value(&snapshot).unwrap_or_default();
    body["you"] = match user {
        Some(SessionUserNoWallet(u)) => {
            let slot = snapshot
                .players
                .iter()
                .find(|p| p.user_id == u.id)
                .map(|p| p.slot);
            serde_json::json!({ "userId": u.id, "slot": slot })
        }
        None => serde_json::Value::Null,
    };
    Ok(Json(body))
}

/// `POST /matches/:id/game/start` — idempotent.
pub async fn start_game(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let outcome = native_game::start_game(&state.pool, match_id, Some(user.id)).await?;
    Ok(respond(&state, match_id, outcome).await)
}

/// `POST /matches/:id/connect-four/moves`
pub async fn post_move(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
    Json(body): Json<MoveBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let outcome = native_game::apply_move(
        &state.pool,
        match_id,
        user.id,
        MoveRequest {
            column: body.column,
            expected_version: body.expected_version,
            client_nonce: body.client_nonce,
        },
    )
    .await?;
    Ok(respond(&state, match_id, outcome).await)
}

/// `POST /matches/:id/game/resign`
pub async fn resign(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let outcome = native_game::resign(&state.pool, match_id, user.id).await?;
    Ok(respond(&state, match_id, outcome).await)
}

/// `GET /features` — lets the frontend adapt to what this server offers.
pub async fn features(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "nativeGamesEnabled": native_game::native_games_enabled() && !native_game::mainnet_blocked(),
        // Free Play needs nothing from Kaspa and is always on.
        "freePlayEnabled": true,
        "emailVerificationRequired": state.account.cfg.require_email_verification,
        "passwordResetAvailable": state.account.cfg.mail_available,
    }))
}
