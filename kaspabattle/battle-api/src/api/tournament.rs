//! Tournament API endpoints — Phase 2
//!
//! REST interface for the KaspaBattle tournament system.
//!
//! ## Routes (all under /api/v1/tournaments)
//!
//! | Method | Path                                        | Auth     | Description                        |
//! |--------|---------------------------------------------|----------|------------------------------------|
//! | POST   | /tournaments                                | User     | Create tournament                  |
//! | GET    | /tournaments                                | Public   | List open/upcoming tournaments      |
//! | GET    | /tournaments/:id                            | Public   | Get tournament details             |
//! | POST   | /tournaments/:id/teams                      | User     | Register a team                    |
//! | GET    | /tournaments/:id/teams                      | Public   | List teams in tournament           |
//! | POST   | /tournaments/:id/teams/:team_id/members     | User     | Add member to team                 |
//! | POST   | /tournaments/:id/lock                       | Organizer| Lock bracket                       |
//! | GET    | /tournaments/:id/bracket                    | Public   | Get bracket                        |
//! | POST   | /tournaments/:id/bracket/:slot_id/match-id  | Captain  | Submit FaceIT match ID             |
//! | GET    | /tournaments/:id/payout                     | Public   | Get payout info                    |
//! | POST   | /tournaments/:id/cancel                     | Organizer| Cancel tournament                  |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::api::{AppState, ApiErrorResponse};
use crate::api::auth_guard::SessionUser;

// ─── Request/Response types ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTournamentReq {
    #[serde(rename = "name")]
    pub title: String,
    pub max_teams: i32,
    pub buy_in_sompi: i64,
    #[serde(default = "default_winner_pct")]
    pub prize_winner_pct: i16,
    #[serde(default = "default_runner_up_pct")]
    pub prize_runner_up_pct: i16,
    #[serde(default = "default_fee_pct")]
    pub platform_fee_pct: i16,
    #[serde(default = "default_game_type", rename = "game_id")]
    pub game_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_deadline: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_winner_pct() -> i16 { 70 }
fn default_runner_up_pct() -> i16 { 20 }
fn default_fee_pct() -> i16 { 10 }
fn default_game_type() -> String { "CS2".to_string() }

#[derive(Debug, Serialize)]
pub struct TournamentResponse {
    pub id: Uuid,
    #[serde(rename = "name")]
    pub title: String,
    #[serde(rename = "game_id")]
    pub game_type: String,
    pub max_teams: i32,
    pub buy_in_sompi: i64,
    pub prize_winner_pct: i16,
    pub prize_runner_up_pct: i16,
    pub platform_fee_pct: i16,
    pub escrow_address: Option<String>,
    pub total_prize_pool_sompi: i64,
    pub status: String,
    pub organizer_user_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub team_count: i64,
    pub registration_deadline: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterTeamReq {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub name: String,
    pub captain_user_id: Uuid,
    pub captain_display_name: Option<String>,
    pub deposit_status: String,
    pub deposit_tx_hash: Option<String>,
    pub deposit_confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub seed: Option<i32>,
    pub member_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberReq {
    /// The user_id to add as a team member (captain adds others)
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BracketSlotResponse {
    pub id: Uuid,
    pub round: i32,
    pub slot_index: i32,
    pub team_a: Option<BracketTeamInfo>,
    pub team_b: Option<BracketTeamInfo>,
    pub winner_team_id: Option<Uuid>,
    pub faceit_match_id: Option<String>,
    pub status: String,
    pub match_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub match_finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct BracketTeamInfo {
    pub id: Uuid,
    pub name: String,
    pub seed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitMatchIdReq {
    pub faceit_match_id: String,
}

#[derive(Debug, Serialize)]
pub struct PayoutInfoResponse {
    pub tournament_id: Uuid,
    pub status: String,
    pub total_prize_pool_sompi: i64,
    pub winner_share_sompi: i64,
    pub runner_up_share_sompi: i64,
    pub platform_fee_sompi: i64,
    pub winner_team_id: Option<Uuid>,
    pub runner_up_team_id: Option<Uuid>,
    pub payout_tx_hash: Option<String>,
    pub payout_executed_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ─── Helper: error shorthand ──────────────────────────────────────────────────

type ApiError = (StatusCode, Json<ApiErrorResponse>);

fn db_err(e: sqlx::Error) -> ApiError {
    tracing::error!(error = %e, "Tournament DB error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiErrorResponse {
            error: "db_error",
            message: "Database operation failed.",
        }),
    )
}

fn not_found() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse {
            error: "not_found",
            message: "Tournament not found.",
        }),
    )
}

fn forbidden() -> ApiError {
    (
        StatusCode::FORBIDDEN,
        Json(ApiErrorResponse {
            error: "forbidden",
            message: "You are not authorized to perform this action.",
        }),
    )
}

// ─── POST /tournaments ────────────────────────────────────────────────────────

pub async fn create_tournament(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Json(req): Json<CreateTournamentReq>,
) -> Result<(StatusCode, Json<TournamentResponse>), ApiError> {
    // Validate
    if req.title.trim().is_empty() || req.title.len() > 80 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_title",
                message: "Title must be 1–80 characters.",
            }),
        ));
    }

    let valid_sizes = [4i32, 8, 16];
    if !valid_sizes.contains(&req.max_teams) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_max_teams",
                message: "max_teams must be 4, 8, or 16.",
            }),
        ));
    }

    if req.buy_in_sompi <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_buy_in",
                message: "buy_in_sompi must be positive.",
            }),
        ));
    }

    if req.prize_winner_pct + req.prize_runner_up_pct + req.platform_fee_pct != 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_prize_split",
                message: "Prize percentages must sum to 100.",
            }),
        ));
    }

    let tournament_id = Uuid::new_v4();

    // Derive a deterministic escrow address for this tournament
    let escrow_address: Option<String> = state
        .escrow_wallet
        .as_ref()
        .and_then(|wallet| {
            wallet
                .derive_escrow_address(&tournament_id.to_string())
                .ok()
                .map(|(addr, _)| addr.to_string())
        });

    let row = sqlx::query(
        "INSERT INTO tournaments \
         (id, title, game_type, max_teams, buy_in_sompi, \
          prize_winner_pct, prize_runner_up_pct, platform_fee_pct, \
          escrow_address, organizer_user_id, registration_deadline) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) \
         RETURNING id, title, game_type, max_teams, buy_in_sompi, \
                   prize_winner_pct, prize_runner_up_pct, platform_fee_pct, \
                   escrow_address, total_prize_pool_sompi, status::text, \
                   organizer_user_id, created_at, registration_deadline",
    )
    .bind(tournament_id)
    .bind(req.title.trim())
    .bind(&req.game_type)
    .bind(req.max_teams)
    .bind(req.buy_in_sompi)
    .bind(req.prize_winner_pct)
    .bind(req.prize_runner_up_pct)
    .bind(req.platform_fee_pct)
    .bind(&escrow_address)
    .bind(user.id)
    .bind(req.registration_deadline)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    tracing::info!(
        tournament_id = %tournament_id,
        organizer_id = %user.id,
        "🏆 Tournament created"
    );

    Ok((
        StatusCode::CREATED,
        Json(TournamentResponse {
            id: row.try_get("id").unwrap(),
            title: row.try_get("title").unwrap(),
            game_type: row.try_get("game_type").unwrap(),
            max_teams: row.try_get("max_teams").unwrap(),
            buy_in_sompi: row.try_get("buy_in_sompi").unwrap(),
            prize_winner_pct: row.try_get("prize_winner_pct").unwrap(),
            prize_runner_up_pct: row.try_get("prize_runner_up_pct").unwrap(),
            platform_fee_pct: row.try_get("platform_fee_pct").unwrap(),
            escrow_address: row.try_get("escrow_address").unwrap_or(None),
            total_prize_pool_sompi: row.try_get("total_prize_pool_sompi").unwrap_or(0),
            status: row.try_get("status").unwrap(),
            organizer_user_id: row.try_get("organizer_user_id").unwrap(),
            created_at: row.try_get("created_at").unwrap(),
            team_count: 0,
            registration_deadline: row.try_get("registration_deadline").unwrap_or(None),
        }),
    ))
}

// ─── GET /tournaments ─────────────────────────────────────────────────────────

pub async fn list_tournaments(
    State(state): State<AppState>,
) -> Result<Json<Vec<TournamentResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT t.id, t.title, t.game_type, t.max_teams, t.buy_in_sompi, \
                t.prize_winner_pct, t.prize_runner_up_pct, t.platform_fee_pct, \
                t.escrow_address, t.total_prize_pool_sompi, t.status::text, \
                t.organizer_user_id, t.created_at, t.registration_deadline, \
                COUNT(tt.id) AS team_count \
         FROM tournaments t \
         LEFT JOIN tournament_teams tt ON tt.tournament_id = t.id \
         WHERE t.status IN ('REGISTRATION','FUNDED','BRACKET_READY','IN_PROGRESS') \
         GROUP BY t.id \
         ORDER BY t.created_at DESC \
         LIMIT 50",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let result = rows.iter().map(|r| TournamentResponse {
        id: r.try_get("id").unwrap(),
        title: r.try_get("title").unwrap(),
        game_type: r.try_get("game_type").unwrap(),
        max_teams: r.try_get("max_teams").unwrap(),
        buy_in_sompi: r.try_get("buy_in_sompi").unwrap(),
        prize_winner_pct: r.try_get("prize_winner_pct").unwrap(),
        prize_runner_up_pct: r.try_get("prize_runner_up_pct").unwrap(),
        platform_fee_pct: r.try_get("platform_fee_pct").unwrap(),
        escrow_address: r.try_get("escrow_address").unwrap_or(None),
        total_prize_pool_sompi: r.try_get("total_prize_pool_sompi").unwrap_or(0),
        status: r.try_get("status").unwrap(),
        organizer_user_id: r.try_get("organizer_user_id").unwrap(),
        created_at: r.try_get("created_at").unwrap(),
        team_count: r.try_get::<i64, _>("team_count").unwrap_or(0),
        registration_deadline: r.try_get("registration_deadline").unwrap_or(None),
    }).collect();

    Ok(Json(result))
}

// ─── GET /tournaments/:id ─────────────────────────────────────────────────────

pub async fn get_tournament(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TournamentResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT t.id, t.title, t.game_type, t.max_teams, t.buy_in_sompi, \
                t.prize_winner_pct, t.prize_runner_up_pct, t.platform_fee_pct, \
                t.escrow_address, t.total_prize_pool_sompi, t.status::text, \
                t.organizer_user_id, t.created_at, t.registration_deadline, \
                COUNT(tt.id) AS team_count \
         FROM tournaments t \
         LEFT JOIN tournament_teams tt ON tt.tournament_id = t.id \
         WHERE t.id = $1 \
         GROUP BY t.id",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    Ok(Json(TournamentResponse {
        id: row.try_get("id").unwrap(),
        title: row.try_get("title").unwrap(),
        game_type: row.try_get("game_type").unwrap(),
        max_teams: row.try_get("max_teams").unwrap(),
        buy_in_sompi: row.try_get("buy_in_sompi").unwrap(),
        prize_winner_pct: row.try_get("prize_winner_pct").unwrap(),
        prize_runner_up_pct: row.try_get("prize_runner_up_pct").unwrap(),
        platform_fee_pct: row.try_get("platform_fee_pct").unwrap(),
        escrow_address: row.try_get("escrow_address").unwrap_or(None),
        total_prize_pool_sompi: row.try_get("total_prize_pool_sompi").unwrap_or(0),
        status: row.try_get("status").unwrap(),
        organizer_user_id: row.try_get("organizer_user_id").unwrap(),
        created_at: row.try_get("created_at").unwrap(),
        team_count: row.try_get::<i64, _>("team_count").unwrap_or(0),
        registration_deadline: row.try_get("registration_deadline").unwrap_or(None),
    }))
}

// ─── POST /tournaments/:id/teams ─────────────────────────────────────────────

pub async fn register_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    SessionUser(user): SessionUser,
    Json(req): Json<RegisterTeamReq>,
) -> Result<(StatusCode, Json<TeamResponse>), ApiError> {
    if req.name.trim().is_empty() || req.name.len() > 60 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_name",
                message: "Team name must be 1–60 characters.",
            }),
        ));
    }

    // Verify tournament exists and is in REGISTRATION status
    let tournament = sqlx::query(
        "SELECT status::text AS status, max_teams, registration_deadline FROM tournaments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let status: String = tournament.try_get("status").unwrap_or_default();
    if status != "REGISTRATION" {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "registration_closed",
                message: "Tournament registration is closed.",
            }),
        ));
    }

    // M-04: Enforce registration_deadline server-side
    let deadline: Option<chrono::DateTime<chrono::Utc>> =
        tournament.try_get("registration_deadline").unwrap_or(None);
    if let Some(dl) = deadline {
        if chrono::Utc::now() > dl {
            return Err((
                StatusCode::CONFLICT,
                Json(ApiErrorResponse {
                    error: "registration_closed",
                    message: "Registration deadline has passed.",
                }),
            ));
        }
    }

    let max_teams: i32 = tournament.try_get("max_teams").unwrap_or(8);
    let current_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_teams WHERE tournament_id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if current_count >= max_teams as i64 {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "tournament_full",
                message: "This tournament is already full.",
            }),
        ));
    }

    let team_id = Uuid::new_v4();
    let seed = (current_count + 1) as i32;

    sqlx::query(
        "INSERT INTO tournament_teams \
         (id, tournament_id, name, captain_user_id, seed) \
         VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(team_id)
    .bind(id)
    .bind(req.name.trim())
    .bind(user.id)
    .bind(seed)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unique") || msg.contains("duplicate") {
            (
                StatusCode::CONFLICT,
                Json(ApiErrorResponse {
                    error: "team_name_taken",
                    message: "This team name is already taken in this tournament.",
                }),
            )
        } else {
            db_err(e)
        }
    })?;

    // Auto-add captain as first team member
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, is_captain) VALUES ($1,$2,true) ON CONFLICT DO NOTHING",
    )
    .bind(team_id)
    .bind(user.id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    tracing::info!(
        tournament_id = %id,
        team_id = %team_id,
        captain_id = %user.id,
        name = %req.name,
        "⚔️ Team registered"
    );

    Ok((
        StatusCode::CREATED,
        Json(TeamResponse {
            id: team_id,
            tournament_id: id,
            name: req.name.trim().to_string(),
            captain_user_id: user.id,
            captain_display_name: Some(user.display_name.clone()),
            deposit_status: "PENDING".to_string(),
            deposit_tx_hash: None,
            deposit_confirmed_at: None,
            seed: Some(seed),
            member_count: 1,
        }),
    ))
}

// ─── GET /tournaments/:id/teams ───────────────────────────────────────────────

pub async fn list_teams(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TeamResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT tt.id, tt.tournament_id, tt.name, tt.captain_user_id, \
                u.display_name AS captain_display_name, \
                tt.deposit_status::text, tt.deposit_tx_hash, tt.deposit_confirmed_at, \
                tt.seed, \
                COUNT(tm.id) AS member_count \
         FROM tournament_teams tt \
         JOIN users u ON u.id = tt.captain_user_id \
         LEFT JOIN team_members tm ON tm.team_id = tt.id \
         WHERE tt.tournament_id = $1 \
         GROUP BY tt.id, u.display_name \
         ORDER BY tt.seed ASC NULLS LAST, tt.created_at ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let teams = rows.iter().map(|r| {
        let ds: String = r.try_get("deposit_status").unwrap_or_else(|_| "PENDING".into());
        TeamResponse {
            id: r.try_get("id").unwrap(),
            tournament_id: r.try_get("tournament_id").unwrap(),
            name: r.try_get("name").unwrap(),
            captain_user_id: r.try_get("captain_user_id").unwrap(),
            captain_display_name: r.try_get("captain_display_name").ok(),
            deposit_status: ds,
            deposit_tx_hash: r.try_get("deposit_tx_hash").unwrap_or(None),
            deposit_confirmed_at: r.try_get("deposit_confirmed_at").unwrap_or(None),
            seed: r.try_get("seed").unwrap_or(None),
            member_count: r.try_get::<i64, _>("member_count").unwrap_or(0),
        }
    }).collect();

    Ok(Json(teams))
}

// ─── POST /tournaments/:id/teams/:team_id/members ─────────────────────────────

pub async fn add_team_member(
    State(state): State<AppState>,
    Path((tournament_id, team_id)): Path<(Uuid, Uuid)>,
    SessionUser(user): SessionUser,
    Json(req): Json<AddMemberReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify caller is the team captain
    let team = sqlx::query(
        "SELECT captain_user_id FROM tournament_teams \
         WHERE id = $1 AND tournament_id = $2",
    )
    .bind(team_id)
    .bind(tournament_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let captain_id: Uuid = team.try_get("captain_user_id").unwrap();
    if captain_id != user.id {
        return Err(forbidden());
    }

    // Check team size limit (5 players max)
    let current_members: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_members WHERE team_id = $1",
    )
    .bind(team_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if current_members >= 5 {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "team_full",
                message: "Teams are limited to 5 players.",
            }),
        ));
    }

    let member_user_id = req.user_id.unwrap_or(user.id);

    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, is_captain) \
         VALUES ($1,$2,false) ON CONFLICT DO NOTHING",
    )
    .bind(team_id)
    .bind(member_user_id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "team_id": team_id,
        "user_id": member_user_id,
    })))
}

// ─── POST /tournaments/:id/teams/:team_id/deposit ───────────────────────────

#[derive(Debug, Deserialize)]
pub struct TournamentDepositReq {
    pub tx_hash: String,
}

pub async fn submit_team_deposit(
    State(state): State<AppState>,
    Path((tournament_id, team_id)): Path<(Uuid, Uuid)>,
    SessionUser(user): SessionUser,
    Json(req): Json<TournamentDepositReq>,
) -> Result<(StatusCode, Json<TeamResponse>), ApiError> {
    // 1) Log incoming request for debugging
    tracing::info!(
        tournament_id = %tournament_id,
        team_id = %team_id,
        user_id = %user.id,
        tx_hash = %req.tx_hash,
        "📥 submit_team_deposit called"
    );

    // 2) Verify caller is captain and team exists in this tournament
    let team_opt = sqlx::query(
        "SELECT captain_user_id, name, deposit_status::text, seed \
         FROM tournament_teams WHERE id = $1 AND tournament_id = $2"
    )
    .bind(team_id)
    .bind(tournament_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?;

    let team = match team_opt {
        Some(t) => t,
        None => {
            tracing::warn!(
                tournament_id = %tournament_id,
                team_id = %team_id,
                user_id = %user.id,
                "❌ submit_team_deposit: team not found in tournament_teams for given (team_id, tournament_id)"
            );
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    error: "team_not_in_tournament",
                    message: "Your team does not exist in this tournament. Please reload and re-register.",
                }),
            ));
        }
    };

    let captain_id: Uuid = team.try_get("captain_user_id").unwrap();
    if captain_id != user.id {
        return Err(forbidden());
    }

    // 2) Verify tournament status
    let tournament_status: String = sqlx::query_scalar(
        "SELECT status::text FROM tournaments WHERE id = $1"
    )
    .bind(tournament_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .unwrap_or_default();

    if tournament_status != "REGISTRATION" && tournament_status != "FUNDED" && tournament_status != "BRACKET_READY" {
         return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "invalid_tournament_status",
                message: "Tournament is not open for deposits.",
            }),
        ));
    }

    // 3) Update team deposit status to PENDING
    let row = sqlx::query(
        "UPDATE tournament_teams \
         SET deposit_tx_hash = $1, deposit_status = 'PENDING', updated_at = NOW() \
         WHERE id = $2 \
         RETURNING id, tournament_id, name, captain_user_id, deposit_status::text, deposit_tx_hash, deposit_confirmed_at, seed"
    )
    .bind(&req.tx_hash)
    .bind(team_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_members WHERE team_id = $1"
    )
    .bind(team_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    tracing::info!(
        tournament_id = %tournament_id,
        team_id = %team_id,
        tx_hash = %req.tx_hash,
        "💸 Tournament team deposit submitted (PENDING)"
    );

    Ok((
        StatusCode::OK,
        Json(TeamResponse {
            id: row.try_get("id").unwrap(),
            tournament_id: row.try_get("tournament_id").unwrap(),
            name: row.try_get("name").unwrap(),
            captain_user_id: row.try_get("captain_user_id").unwrap(),
            captain_display_name: Some(user.display_name.clone()),
            deposit_status: row.try_get("deposit_status").unwrap(),
            deposit_tx_hash: row.try_get("deposit_tx_hash").unwrap_or(None),
            deposit_confirmed_at: row.try_get("deposit_confirmed_at").unwrap_or(None),
            seed: row.try_get("seed").unwrap_or(None),
            member_count,
        }),
    ))
}

// ─── POST /tournaments/:id/lock ───────────────────────────────────────────────

pub async fn lock_bracket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    SessionUser(user): SessionUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify caller is the organizer
    let tournament = sqlx::query(
        "SELECT organizer_user_id, status, max_teams FROM tournaments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let organizer_id: Uuid = tournament.try_get("organizer_user_id").unwrap();
    if organizer_id != user.id {
        return Err(forbidden());
    }

    let status: String = tournament.try_get("status").unwrap_or_default();
    if !matches!(status.as_str(), "FUNDED" | "REGISTRATION") {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "invalid_status",
                message: "Bracket can only be locked from FUNDED or REGISTRATION status.",
            }),
        ));
    }

    let max_teams: i32 = tournament.try_get("max_teams").unwrap_or(8);

    // H-01: Only seed teams with CONFIRMED deposits into the bracket.
    // Unconfirmed teams are excluded to prevent prize pool / escrow mismatch.
    let teams = sqlx::query(
        "SELECT id, seed FROM tournament_teams \
         WHERE tournament_id = $1 AND deposit_status = 'CONFIRMED' \
         ORDER BY COALESCE(seed, 9999), created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let funded_team_ids: Vec<Uuid> = teams.iter()
        .map(|r| r.try_get::<Uuid, _>("id").unwrap())
        .collect();

    if funded_team_ids.len() < 2 {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "not_enough_teams",
                message: "At least 2 teams required to lock the bracket.",
            }),
        ));
    }

    // Generate bracket slots in a transaction
    let mut tx = state.pool.begin().await.map_err(db_err)?;

    // Lock tournament
    sqlx::query(
        "UPDATE tournaments SET status = 'BRACKET_READY', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;

    // Generate single-elimination bracket slots
    let n = funded_team_ids.len();
    let half = (max_teams as usize) / 2;
    let total_rounds = (max_teams as f64).log2().ceil() as i32;

    // Round 1: pair teams
    for slot_idx in 0..half {
        let team_a_id = funded_team_ids.get(slot_idx * 2).copied();
        let team_b_id = funded_team_ids.get(slot_idx * 2 + 1).copied();

        let status = match (team_a_id, team_b_id) {
            (Some(_), Some(_)) => "READY",
            (Some(_), None) => "BYE",
            _ => "WAITING",
        };

        // AUTO-advance BYE team to next round
        let winner_for_bye = if status == "BYE" { team_a_id } else { None };

        sqlx::query(
            "INSERT INTO tournament_bracket \
             (id, tournament_id, round, slot_index, team_a_id, team_b_id, winner_team_id, status) \
             VALUES (gen_random_uuid(), $1, 1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(slot_idx as i32)
        .bind(team_a_id)
        .bind(team_b_id)
        .bind(winner_for_bye)
        .bind(status)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
    }

    // Later rounds: empty waiting slots
    let mut slots_per_round = half / 2;
    for round in 2..=total_rounds {
        for slot_idx in 0..slots_per_round {
            sqlx::query(
                "INSERT INTO tournament_bracket \
                 (id, tournament_id, round, slot_index, status) \
                 VALUES (gen_random_uuid(), $1, $2, $3, 'WAITING')",
            )
            .bind(id)
            .bind(round)
            .bind(slot_idx as i32)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        if slots_per_round > 1 {
            slots_per_round /= 2;
        }
    }

    // M-03: Propagate BYE winners into the next round slots now that all rounds exist.
    // Without this, BYE winners appear as "TBD" in the next round.
    for slot_idx in 0..half {
        let team_a_id = funded_team_ids.get(slot_idx * 2).copied();
        let team_b_id = funded_team_ids.get(slot_idx * 2 + 1).copied();
        let is_bye = team_a_id.is_some() && team_b_id.is_none();

        if is_bye {
            if let Some(bye_winner) = team_a_id {
                let next_round = 2i32;
                let next_slot = (slot_idx / 2) as i32;
                let is_team_a_side = (slot_idx % 2) == 0;

                if is_team_a_side {
                    sqlx::query(
                        "UPDATE tournament_bracket \
                         SET team_a_id = $1, \
                             status = CASE WHEN team_b_id IS NOT NULL THEN 'READY' ELSE status END, \
                             updated_at = NOW() \
                         WHERE tournament_id = $2 AND round = $3 AND slot_index = $4",
                    )
                    .bind(bye_winner)
                    .bind(id)
                    .bind(next_round)
                    .bind(next_slot)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_err)?;
                } else {
                    sqlx::query(
                        "UPDATE tournament_bracket \
                         SET team_b_id = $1, \
                             status = CASE WHEN team_a_id IS NOT NULL THEN 'READY' ELSE status END, \
                             updated_at = NOW() \
                         WHERE tournament_id = $2 AND round = $3 AND slot_index = $4",
                    )
                    .bind(bye_winner)
                    .bind(id)
                    .bind(next_round)
                    .bind(next_slot)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_err)?;
                }

                tracing::debug!(
                    tournament_id = %id,
                    bye_team = %bye_winner,
                    next_round,
                    next_slot,
                    "⬆️ BYE winner auto-advanced to next round"
                );
            }
        }
    }

    tx.commit().await.map_err(db_err)?;

    tracing::info!(
        tournament_id = %id,
        teams = n,
        rounds = total_rounds,
        "🔒 Bracket locked"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "tournament_id": id,
        "bracket_slots": half + (max_teams as usize - 1),
        "total_rounds": total_rounds,
        "teams_seeded": n,
    })))
}

// ─── GET /tournaments/:id/bracket ────────────────────────────────────────────

pub async fn get_bracket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<BracketSlotResponse>>, ApiError> {
    // Verify tournament exists
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tournaments WHERE id = $1)",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if !exists {
        return Err(not_found());
    }

    let rows = sqlx::query(
        "SELECT b.id, b.round, b.slot_index, b.status::text, \
                b.team_a_id, ta.name AS team_a_name, ta.seed AS team_a_seed, \
                b.team_b_id, tb.name AS team_b_name, tb.seed AS team_b_seed, \
                b.winner_team_id, b.faceit_match_id, \
                b.match_started_at, b.match_finished_at \
         FROM tournament_bracket b \
         LEFT JOIN tournament_teams ta ON ta.id = b.team_a_id \
         LEFT JOIN tournament_teams tb ON tb.id = b.team_b_id \
         WHERE b.tournament_id = $1 \
         ORDER BY b.round ASC, b.slot_index ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let slots = rows.iter().map(|r| {
        let team_a = r.try_get::<Uuid, _>("team_a_id").ok().map(|tid| BracketTeamInfo {
            id: tid,
            name: r.try_get("team_a_name").unwrap_or_default(),
            seed: r.try_get("team_a_seed").unwrap_or(None),
        });
        let team_b = r.try_get::<Uuid, _>("team_b_id").ok().map(|tid| BracketTeamInfo {
            id: tid,
            name: r.try_get("team_b_name").unwrap_or_default(),
            seed: r.try_get("team_b_seed").unwrap_or(None),
        });

        BracketSlotResponse {
            id: r.try_get("id").unwrap(),
            round: r.try_get("round").unwrap(),
            slot_index: r.try_get("slot_index").unwrap(),
            team_a,
            team_b,
            winner_team_id: r.try_get("winner_team_id").unwrap_or(None),
            faceit_match_id: r.try_get("faceit_match_id").unwrap_or(None),
            status: r.try_get("status").unwrap(),
            match_started_at: r.try_get("match_started_at").unwrap_or(None),
            match_finished_at: r.try_get("match_finished_at").unwrap_or(None),
        }
    }).collect();

    Ok(Json(slots))
}

// ─── POST /tournaments/:id/bracket/:slot_id/match-id ─────────────────────────

pub async fn submit_bracket_match_id(
    State(state): State<AppState>,
    Path((tournament_id, slot_id)): Path<(Uuid, Uuid)>,
    SessionUser(user): SessionUser,
    Json(req): Json<SubmitMatchIdReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let faceit_match_id = req.faceit_match_id.trim().to_string();
    if faceit_match_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_match_id",
                message: "faceit_match_id must not be empty.",
            }),
        ));
    }

    // Verify slot belongs to this tournament and is READY
    let slot = sqlx::query(
        "SELECT b.id, b.status::text, b.team_a_id, b.team_b_id \
         FROM tournament_bracket b \
         WHERE b.id = $1 AND b.tournament_id = $2",
    )
    .bind(slot_id)
    .bind(tournament_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let status: String = slot.try_get("status").unwrap_or_default();
    if !matches!(status.as_str(), "READY" | "IN_PROGRESS") {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "slot_not_ready",
                message: "This bracket slot is not ready for a match ID submission.",
            }),
        ));
    }

    // Verify caller is the captain of team_a or team_b
    let team_a_id: Option<Uuid> = slot.try_get("team_a_id").unwrap_or(None);
    let team_b_id: Option<Uuid> = slot.try_get("team_b_id").unwrap_or(None);

    let is_authorized = is_captain_of_any(&state.pool, user.id, &[team_a_id, team_b_id]).await;
    // Also allow tournament organizer
    let is_organizer = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM tournaments WHERE id = $1 AND organizer_user_id = $2)",
    )
    .bind(tournament_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !is_authorized && !is_organizer {
        return Err(forbidden());
    }

    // Update the slot
    sqlx::query(
        "UPDATE tournament_bracket \
         SET faceit_match_id = $1, status = 'IN_PROGRESS', \
             match_started_at = COALESCE(match_started_at, NOW()), \
             updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(&faceit_match_id)
    .bind(slot_id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    // Transition tournament to IN_PROGRESS if still BRACKET_READY
    sqlx::query(
        "UPDATE tournaments \
         SET status = 'IN_PROGRESS', updated_at = NOW() \
         WHERE id = $1 AND status = 'BRACKET_READY'",
    )
    .bind(tournament_id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    // Create FaceIT watcher job so the existing watcher infrastructure tracks this match
    sqlx::query(
        "INSERT INTO faceit_watcher_jobs (id, match_id, faceit_match_id, status) \
         VALUES (gen_random_uuid(), $1, $2, 'ACTIVE') \
         ON CONFLICT (match_id) DO UPDATE \
         SET faceit_match_id = EXCLUDED.faceit_match_id, status = 'ACTIVE', updated_at = NOW()",
    )
    .bind(slot_id) // We use slot_id as the match_id reference (they share UUID space)
    .bind(&faceit_match_id)
    .execute(&state.pool)
    .await
    .ok(); // Non-fatal if this fails (schema constraint might not allow it without a matches row)

    tracing::info!(
        tournament_id = %tournament_id,
        slot_id = %slot_id,
        faceit_match_id = %faceit_match_id,
        "🎮 Bracket slot match ID submitted"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "slot_id": slot_id,
        "faceit_match_id": faceit_match_id,
    })))
}

// ─── GET /tournaments/:id/payout ─────────────────────────────────────────────

pub async fn get_payout_info(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PayoutInfoResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT t.id, t.status, t.total_prize_pool_sompi, \
                t.prize_winner_pct, t.prize_runner_up_pct, t.platform_fee_pct, \
                t.winner_team_id_ref, t.runner_up_team_id_ref, \
                t.payout_tx_hash, t.payout_executed_at \
         FROM tournaments t \
         WHERE t.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let pool_sompi: i64 = row.try_get("total_prize_pool_sompi").unwrap_or(0);
    let winner_pct: i64 = row.try_get::<i16, _>("prize_winner_pct").unwrap_or(70) as i64;
    let runner_up_pct: i64 = row.try_get::<i16, _>("prize_runner_up_pct").unwrap_or(20) as i64;
    let fee_pct: i64 = row.try_get::<i16, _>("platform_fee_pct").unwrap_or(10) as i64;

    Ok(Json(PayoutInfoResponse {
        tournament_id: id,
        status: row.try_get("status").unwrap(),
        total_prize_pool_sompi: pool_sompi,
        winner_share_sompi: pool_sompi * winner_pct / 100,
        runner_up_share_sompi: pool_sompi * runner_up_pct / 100,
        platform_fee_sompi: pool_sompi * fee_pct / 100,
        winner_team_id: row.try_get("winner_team_id_ref").unwrap_or(None),
        runner_up_team_id: row.try_get("runner_up_team_id_ref").unwrap_or(None),
        payout_tx_hash: row.try_get("payout_tx_hash").unwrap_or(None),
        payout_executed_at: row.try_get("payout_executed_at").unwrap_or(None),
    }))
}

// ─── POST /tournaments/:id/cancel ────────────────────────────────────────────

pub async fn cancel_tournament(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    SessionUser(user): SessionUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tournament = sqlx::query(
        "SELECT organizer_user_id, status FROM tournaments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let organizer_id: Uuid = tournament.try_get("organizer_user_id").unwrap();
    if organizer_id != user.id {
        return Err(forbidden());
    }

    let status: String = tournament.try_get("status").unwrap_or_default();
    if !matches!(status.as_str(), "REGISTRATION" | "FUNDED") {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "cannot_cancel",
                message: "Tournament can only be cancelled before bracket is locked.",
            }),
        ));
    }

    sqlx::query(
        "UPDATE tournaments SET status = 'CANCELLED', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    tracing::info!(tournament_id = %id, organizer_id = %user.id, "❌ Tournament cancelled");

    Ok(Json(serde_json::json!({
        "status": "ok",
        "tournament_id": id,
        "message": "Tournament cancelled. Refunds will be processed.",
    })))
}

// ─── Helper: check if user is captain of any of the given teams ───────────────

async fn is_captain_of_any(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    team_ids: &[Option<Uuid>],
) -> bool {
    let ids: Vec<Uuid> = team_ids.iter().filter_map(|id| *id).collect();
    if ids.is_empty() {
        return false;
    }

    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM tournament_teams \
         WHERE id = ANY($1) AND captain_user_id = $2)",
    )
    .bind(&ids)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

// ─── POST /tournaments/:id/dispute ────────────────────────────────────────────
//
// Any participant (team captain) can file a dispute against a tournament.
// Only tournaments IN_PROGRESS or BRACKET_READY can be disputed.

#[derive(Debug, Deserialize)]
pub struct FileDisputeReq {
    pub reason: String,
}

pub async fn file_dispute(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    SessionUser(user): SessionUser,
    Json(req): Json<FileDisputeReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.reason.trim().len() < 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "reason_too_short",
                message: "Dispute reason must be at least 10 characters.",
            }),
        ));
    }

    // Must be a captain in this tournament
    let is_participant: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tournament_teams \
         WHERE tournament_id = $1 AND captain_user_id = $2)",
    )
    .bind(id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if !is_participant {
        return Err(forbidden());
    }

    let row = sqlx::query("SELECT status::text FROM tournaments WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_err)?
        .ok_or_else(not_found)?;

    let status: String = row.try_get("status").unwrap_or_default();
    if !matches!(status.as_str(), "IN_PROGRESS" | "BRACKET_READY" | "COMPLETED") {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "not_disputable",
                message: "Tournament can only be disputed when IN_PROGRESS, BRACKET_READY, or COMPLETED.",
            }),
        ));
    }

    sqlx::query(
        "UPDATE tournaments \
         SET status = 'DISPUTED', \
             dispute_reason = $1, \
             dispute_filed_at = NOW(), \
             updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(req.reason.trim())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    // Audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_user_id, actor_role, details) \
         VALUES ('tournament', $1, 'dispute_filed', $2, 'captain', $3)",
    )
    .bind(id)
    .bind(user.id)
    .bind(serde_json::json!({ "reason": req.reason }))
    .execute(&state.pool)
    .await;

    let event = serde_json::json!({
        "type": "tournament_disputed",
        "tournament_id": id,
        "filed_by": user.id,
        "reason": req.reason,
    });
    let _ = state.tx.send(event.to_string());

    tracing::info!(
        tournament_id = %id,
        user_id = %user.id,
        reason = %req.reason,
        "⚖️ Tournament dispute filed"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "tournament_id": id,
        "message": "Dispute filed. Admins have been notified.",
    })))
}

// ─── POST /tournaments/:id/bracket/:slot_id/dispute ──────────────────────────

pub async fn file_bracket_dispute(
    State(state): State<AppState>,
    Path((tournament_id, slot_id)): Path<(Uuid, Uuid)>,
    SessionUser(user): SessionUser,
    Json(req): Json<FileDisputeReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.reason.trim().len() < 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "reason_too_short",
                message: "Dispute reason must be at least 10 characters.",
            }),
        ));
    }

    // Caller must be captain of one of the teams in this slot
    let slot = sqlx::query(
        "SELECT team_a_id, team_b_id, status::text, disputed FROM tournament_bracket \
         WHERE id = $1 AND tournament_id = $2",
    )
    .bind(slot_id)
    .bind(tournament_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let already_disputed: bool = slot.try_get("disputed").unwrap_or(false);
    if already_disputed {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "already_disputed",
                message: "This bracket slot is already under dispute.",
            }),
        ));
    }

    let team_a: Option<Uuid> = slot.try_get("team_a_id").unwrap_or(None);
    let team_b: Option<Uuid> = slot.try_get("team_b_id").unwrap_or(None);
    let is_participant = is_captain_of_any(&state.pool, user.id, &[team_a, team_b]).await;
    if !is_participant {
        return Err(forbidden());
    }

    sqlx::query(
        "UPDATE tournament_bracket \
         SET disputed = TRUE, dispute_reason = $1, \
             dispute_filed_by = $2, dispute_filed_at = NOW(), \
             updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(req.reason.trim())
    .bind(user.id)
    .bind(slot_id)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    let _ = sqlx::query(
        "INSERT INTO audit_log (entity_type, entity_id, action, actor_user_id, actor_role, details) \
         VALUES ('bracket_slot', $1, 'dispute_filed', $2, 'captain', $3)",
    )
    .bind(slot_id)
    .bind(user.id)
    .bind(serde_json::json!({ "tournament_id": tournament_id, "reason": req.reason }))
    .execute(&state.pool)
    .await;

    let _ = state.tx.send(serde_json::json!({
        "type": "bracket_disputed",
        "tournament_id": tournament_id,
        "slot_id": slot_id,
        "filed_by": user.id,
    }).to_string());

    tracing::info!(
        tournament_id = %tournament_id,
        slot_id = %slot_id,
        user_id = %user.id,
        "⚖️ Bracket slot dispute filed"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "slot_id": slot_id,
        "message": "Bracket dispute filed. Admins have been notified.",
    })))
}

// ─── GET /tournaments/:id/results ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TournamentResultsResponse {
    pub tournament_id: Uuid,
    pub status: String,
    pub winner_team: Option<TeamSummary>,
    pub runner_up_team: Option<TeamSummary>,
    pub payout_tx_hash: Option<String>,
    pub payout_executed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub total_prize_pool_sompi: i64,
    pub winner_share_sompi: i64,
    pub runner_up_share_sompi: i64,
    pub platform_fee_sompi: i64,
    pub bracket: Vec<BracketSlotResponse>,
}

#[derive(Debug, Serialize)]
pub struct TeamSummary {
    pub id: Uuid,
    pub name: String,
    pub captain_display_name: Option<String>,
}

pub async fn get_tournament_results(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TournamentResultsResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT t.status, t.total_prize_pool_sompi, \
                t.prize_winner_pct, t.prize_runner_up_pct, t.platform_fee_pct, \
                t.payout_tx_hash, t.payout_executed_at, \
                t.winner_team_id_ref, t.runner_up_team_id_ref, \
                -- winner team info
                wt.name AS winner_name, wu.display_name AS winner_captain_name, \
                -- runner-up team info
                rt.name AS runner_up_name, ru.display_name AS runner_up_captain_name \
         FROM tournaments t \
         LEFT JOIN tournament_teams wt ON wt.id = t.winner_team_id_ref \
         LEFT JOIN users wu ON wu.id = wt.captain_user_id \
         LEFT JOIN tournament_teams rt ON rt.id = t.runner_up_team_id_ref \
         LEFT JOIN users ru ON ru.id = rt.captain_user_id \
         WHERE t.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(not_found)?;

    let pool_sompi: i64 = row.try_get("total_prize_pool_sompi").unwrap_or(0);
    let winner_pct: i64 = row.try_get::<i16, _>("prize_winner_pct").unwrap_or(70) as i64;
    let runner_up_pct: i64 = row.try_get::<i16, _>("prize_runner_up_pct").unwrap_or(20) as i64;
    let status: String = row.try_get("status").unwrap_or_default();
    let winner_id: Option<Uuid> = row.try_get("winner_team_id_ref").unwrap_or(None);
    let runner_up_id: Option<Uuid> = row.try_get("runner_up_team_id_ref").unwrap_or(None);

    let winner_share = pool_sompi * winner_pct / 100;
    let runner_up_share = pool_sompi * runner_up_pct / 100;
    let platform_fee = pool_sompi.saturating_sub(winner_share + runner_up_share);

    let winner_team = winner_id.map(|wid| TeamSummary {
        id: wid,
        name: row.try_get("winner_name").unwrap_or_default(),
        captain_display_name: row.try_get("winner_captain_name").unwrap_or(None),
    });

    let runner_up_team = runner_up_id.map(|rid| TeamSummary {
        id: rid,
        name: row.try_get("runner_up_name").unwrap_or_default(),
        captain_display_name: row.try_get("runner_up_captain_name").unwrap_or(None),
    });

    // Load full bracket for results page
    let bracket_rows = sqlx::query(
        "SELECT b.id, b.round, b.slot_index, b.status::text, \
                b.team_a_id, ta.name AS team_a_name, ta.seed AS team_a_seed, \
                b.team_b_id, tb.name AS team_b_name, tb.seed AS team_b_seed, \
                b.winner_team_id, b.faceit_match_id, \
                b.match_started_at, b.match_finished_at \
         FROM tournament_bracket b \
         LEFT JOIN tournament_teams ta ON ta.id = b.team_a_id \
         LEFT JOIN tournament_teams tb ON tb.id = b.team_b_id \
         WHERE b.tournament_id = $1 \
         ORDER BY b.round ASC, b.slot_index ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;

    let bracket = bracket_rows.iter().map(|r| {
        let team_a = r.try_get::<Uuid, _>("team_a_id").ok().map(|tid| BracketTeamInfo {
            id: tid,
            name: r.try_get("team_a_name").unwrap_or_default(),
            seed: r.try_get("team_a_seed").unwrap_or(None),
        });
        let team_b = r.try_get::<Uuid, _>("team_b_id").ok().map(|tid| BracketTeamInfo {
            id: tid,
            name: r.try_get("team_b_name").unwrap_or_default(),
            seed: r.try_get("team_b_seed").unwrap_or(None),
        });

        BracketSlotResponse {
            id: r.try_get("id").unwrap(),
            round: r.try_get("round").unwrap(),
            slot_index: r.try_get("slot_index").unwrap(),
            team_a,
            team_b,
            winner_team_id: r.try_get("winner_team_id").unwrap_or(None),
            faceit_match_id: r.try_get("faceit_match_id").unwrap_or(None),
            status: r.try_get("status").unwrap_or_default(),
            match_started_at: r.try_get("match_started_at").unwrap_or(None),
            match_finished_at: r.try_get("match_finished_at").unwrap_or(None),
        }
    }).collect();

    Ok(Json(TournamentResultsResponse {
        tournament_id: id,
        status,
        winner_team,
        runner_up_team,
        payout_tx_hash: row.try_get("payout_tx_hash").unwrap_or(None),
        payout_executed_at: row.try_get("payout_executed_at").unwrap_or(None),
        total_prize_pool_sompi: pool_sompi,
        winner_share_sompi: winner_share,
        runner_up_share_sompi: runner_up_share,
        platform_fee_sompi: platform_fee,
        bracket,
    }))
}

// ─── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/", get(list_tournaments).post(create_tournament))
        .route("/:id", get(get_tournament))
        .route("/:id/teams", get(list_teams).post(register_team))
        .route("/:id/teams/:team_id/members", post(add_team_member))
        .route("/:id/teams/:team_id/deposit", post(submit_team_deposit))
        .route("/:id/lock", post(lock_bracket))
        .route("/:id/bracket", get(get_bracket))
        .route("/:id/bracket/:slot_id/match-id", post(submit_bracket_match_id))
        .route("/:id/bracket/:slot_id/dispute", post(file_bracket_dispute))
        .route("/:id/payout", get(get_payout_info))
        .route("/:id/dispute", post(file_dispute))
        .route("/:id/results", get(get_tournament_results))
        .route("/:id/cancel", post(cancel_tournament))
}
