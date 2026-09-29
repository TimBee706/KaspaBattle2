//! # Native games service (server-authoritative)
//!
//! Persistence + lifecycle glue around the pure domain engine in
//! `battle_core::native_games`. No HTTP types live here (see `api::native` for the handlers), so every
//! function can be tested against a real Postgres with `#[sqlx::test]`.
//!
//! ## Locking order
//! Every mutating path locks **session row first, match row second** (`FOR UPDATE`). The episode
//! runner never touches sessions while holding the match lock (it calls the `*_if_due` helpers
//! *after* its own transaction committed), so there is no lock-order inversion.
//!
//! ## Events
//! Mutating functions return the events to publish; callers broadcast them **after** the
//! transaction committed, so a client can never observe state that was rolled back.

use crate::models::{Match, MatchProvider, MatchStatus, MATCH_COLUMNS};
use battle_core::match_state::MatchAction;
use battle_core::native_games::connect_four::{
    ConnectFour, ConnectFourState, GameStatus, MoveError, COLUMNS, ROWS,
};
use battle_core::native_games::{result_hash, EndReason};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

pub const GAME_TYPE_CONNECT_FOUR: &str = "CONNECT_FOUR";
pub const RESULT_SOURCE_NATIVE_ENGINE: &str = "NATIVE_ENGINE";

// ── Configuration ────────────────────────────────────────────────────────────

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Feature flag `NATIVE_GAMES_ENABLED` (default: off). Gates creating / joining native matches.
/// Already running native matches stay playable when the flag is switched off so no funds get stuck.
pub fn native_games_enabled() -> bool {
    env_flag("NATIVE_GAMES_ENABLED")
}

/// Native games are testnet-only until the escrow key handling (docs/09-NATIVE-GAMES.md, B1) is fixed.
pub fn mainnet_blocked() -> bool {
    let network = std::env::var("KASPA_NETWORK")
        .unwrap_or_default()
        .to_ascii_lowercase();
    network.contains("mainnet") && !env_flag("NATIVE_GAMES_ALLOW_MAINNET")
}

/// Seconds a player has for one move before forfeiting (`NATIVE_TURN_TIMEOUT_SECS`, default 120).
pub fn turn_timeout_secs() -> i64 {
    std::env::var("NATIVE_TURN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(120)
}

/// Grace period before the server starts a `READY_TO_PLAY` game by itself
/// (`NATIVE_AUTOSTART_SECS`, default 20). Clients normally start it immediately.
pub fn autostart_secs() -> i64 {
    std::env::var("NATIVE_AUTOSTART_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(20)
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum NativeError {
    /// No such match, or the match has no native session (yet).
    NotFound(&'static str),
    NotParticipant,
    /// The match / session is in the wrong lifecycle state.
    WrongStatus(String),
    /// `expectedVersion` does not match the server; the client must reload the snapshot.
    VersionConflict {
        current_version: i64,
    },
    Move(MoveError),
    BadRequest(&'static str),
    Db(sqlx::Error),
    Internal(String),
}

impl From<sqlx::Error> for NativeError {
    fn from(e: sqlx::Error) -> Self {
        NativeError::Db(e)
    }
}

// ── Public DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    pub user_id: Uuid,
    pub display_name: String,
    /// 1 = creator (moves first, blue), 2 = opponent (red)
    pub slot: u8,
    pub color: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastMove {
    pub column: i16,
    pub row: i16,
    pub player_id: Option<Uuid>,
}

/// Full, self-contained view of a game. This is what `GET /game` returns and what every WebSocket
/// event carries, so a client can always replace its local state wholesale.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSnapshot {
    pub match_id: Uuid,
    pub game_type: String,
    /// READY | ACTIVE | FINISHED
    pub status: String,
    pub match_status: MatchStatus,
    pub version: i64,
    /// `board[row][column]`, row 0 = bottom. 0 empty, 1 = player one (blue), 2 = player two (red).
    pub board: Vec<Vec<u8>>,
    pub columns: usize,
    pub rows: usize,
    pub move_count: i32,
    pub current_player_id: Option<Uuid>,
    pub players: Vec<PlayerView>,
    pub winner_user_id: Option<Uuid>,
    /// WIN | DRAW once finished
    pub result: Option<String>,
    /// CONNECT_FOUR | DRAW | RESIGNATION | TIMEOUT
    pub end_reason: Option<String>,
    /// `[row, column]` pairs of the winning run.
    pub winning_line: Vec<(u8, u8)>,
    pub last_move: Option<LastMove>,
    pub turn_deadline: Option<DateTime<Utc>>,
    pub state_hash: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub server_time: DateTime<Utc>,
}

/// Result of a mutating call. `events` must be broadcast by the caller **after** commit.
#[derive(Debug)]
pub struct Outcome {
    pub snapshot: GameSnapshot,
    pub events: Vec<serde_json::Value>,
    /// True if the request was an idempotent replay (same clientNonce) and changed nothing.
    pub replay: bool,
}

#[derive(Debug, Clone)]
pub struct MoveRequest {
    pub column: i64,
    pub expected_version: i64,
    pub client_nonce: String,
}

// ── Internal session row ─────────────────────────────────────────────────────

struct SessionRow {
    match_id: Uuid,
    game_type: String,
    status: String,
    player_one: Uuid,
    player_two: Uuid,
    board_state: String,
    current_player: Option<Uuid>,
    winner: Option<Uuid>,
    result: Option<String>,
    end_reason: Option<String>,
    move_count: i32,
    version: i64,
    state_hash: String,
    last_move_column: Option<i16>,
    last_move_row: Option<i16>,
    turn_deadline: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

const SESSION_COLUMNS: &str = "match_id, game_type, status, player_one_id, player_two_id, \
    board_state::text AS board_state, current_player_id, winner_user_id, result, end_reason, \
    move_count, version, state_hash, last_move_column, last_move_row, turn_deadline, \
    started_at, finished_at";

fn session_from_row(r: &sqlx::postgres::PgRow) -> Result<SessionRow, sqlx::Error> {
    Ok(SessionRow {
        match_id: r.try_get("match_id")?,
        game_type: r.try_get("game_type")?,
        status: r.try_get("status")?,
        player_one: r.try_get("player_one_id")?,
        player_two: r.try_get("player_two_id")?,
        board_state: r.try_get("board_state")?,
        current_player: r.try_get("current_player_id")?,
        winner: r.try_get("winner_user_id")?,
        result: r.try_get("result")?,
        end_reason: r.try_get("end_reason")?,
        move_count: r.try_get("move_count")?,
        version: r.try_get("version")?,
        state_hash: r.try_get("state_hash")?,
        last_move_column: r.try_get("last_move_column")?,
        last_move_row: r.try_get("last_move_row")?,
        turn_deadline: r.try_get("turn_deadline")?,
        started_at: r.try_get("started_at")?,
        finished_at: r.try_get("finished_at")?,
    })
}

async fn lock_session(
    conn: &mut PgConnection,
    match_id: Uuid,
) -> Result<Option<SessionRow>, sqlx::Error> {
    let q = format!(
        "SELECT {SESSION_COLUMNS} FROM native_game_sessions WHERE match_id = $1 FOR UPDATE"
    );
    match sqlx::query(&q).bind(match_id).fetch_optional(conn).await? {
        Some(r) => Ok(Some(session_from_row(&r)?)),
        None => Ok(None),
    }
}

async fn lock_match(conn: &mut PgConnection, match_id: Uuid) -> Result<Option<Match>, sqlx::Error> {
    let q = format!("SELECT {MATCH_COLUMNS} FROM matches WHERE id = $1 FOR UPDATE");
    sqlx::query_as::<_, Match>(&q)
        .bind(match_id)
        .fetch_optional(conn)
        .await
}

fn engine_of(s: &SessionRow) -> Result<ConnectFour, NativeError> {
    let state: ConnectFourState = serde_json::from_str(&s.board_state)
        .map_err(|e| NativeError::Internal(format!("corrupt board_state: {e}")))?;
    ConnectFour::from_state(s.player_one.to_string(), s.player_two.to_string(), state)
        .map_err(|e| NativeError::Internal(e.to_string()))
}

fn board_state_json(game: &ConnectFour) -> Result<String, NativeError> {
    serde_json::to_string(game.state()).map_err(|e| NativeError::Internal(e.to_string()))
}

fn is_participant(m: &Match, user: Uuid) -> bool {
    m.creator_user_id == user || m.opponent_user_id == Some(user)
}

// ── Snapshot ─────────────────────────────────────────────────────────────────

fn board_rows(cells: &[u8]) -> Vec<Vec<u8>> {
    (0..ROWS)
        .map(|r| cells[r * COLUMNS..(r + 1) * COLUMNS].to_vec())
        .collect()
}

/// Loads the snapshot for `match_id` (`Ok(None)` if there is no native session).
pub async fn load_snapshot(
    conn: &mut PgConnection,
    match_id: Uuid,
) -> Result<Option<GameSnapshot>, NativeError> {
    let q = format!("SELECT {SESSION_COLUMNS} FROM native_game_sessions WHERE match_id = $1");
    let Some(row) = sqlx::query(&q)
        .bind(match_id)
        .fetch_optional(&mut *conn)
        .await?
    else {
        return Ok(None);
    };
    let s = session_from_row(&row)?;
    let match_status: MatchStatus = sqlx::query_scalar("SELECT status FROM matches WHERE id = $1")
        .bind(match_id)
        .fetch_one(&mut *conn)
        .await?;
    snapshot_from(conn, &s, match_status).await.map(Some)
}

async fn snapshot_from(
    conn: &mut PgConnection,
    s: &SessionRow,
    match_status: MatchStatus,
) -> Result<GameSnapshot, NativeError> {
    let game = engine_of(s)?;
    let names: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, display_name FROM users WHERE id = ANY($1)")
            .bind(vec![s.player_one, s.player_two])
            .fetch_all(&mut *conn)
            .await?;
    let name_of = |id: Uuid| {
        names
            .iter()
            .find(|(uid, _)| *uid == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    };
    let winning_line = match game.status() {
        GameStatus::Won { line, .. } => line.clone(),
        _ => vec![],
    };
    let last_move = match (s.last_move_column, s.last_move_row) {
        (Some(column), Some(row)) => Some(LastMove {
            column,
            row,
            // The mover is whoever owns the disc on that cell.
            player_id: match game.cell(row as usize, column as usize) {
                1 => Some(s.player_one),
                2 => Some(s.player_two),
                _ => None,
            },
        }),
        _ => None,
    };
    Ok(GameSnapshot {
        match_id: s.match_id,
        game_type: s.game_type.clone(),
        status: s.status.clone(),
        match_status,
        version: s.version,
        board: board_rows(&game.state().cells),
        columns: COLUMNS,
        rows: ROWS,
        move_count: s.move_count,
        current_player_id: s.current_player,
        players: vec![
            PlayerView {
                user_id: s.player_one,
                display_name: name_of(s.player_one),
                slot: 1,
                color: "blue",
            },
            PlayerView {
                user_id: s.player_two,
                display_name: name_of(s.player_two),
                slot: 2,
                color: "red",
            },
        ],
        winner_user_id: s.winner,
        result: s.result.clone(),
        end_reason: s.end_reason.clone(),
        winning_line,
        last_move,
        turn_deadline: s.turn_deadline,
        state_hash: s.state_hash.clone(),
        started_at: s.started_at,
        finished_at: s.finished_at,
        server_time: Utc::now(),
    })
}

// ── Events ───────────────────────────────────────────────────────────────────

fn ev_state(snap: &GameSnapshot) -> serde_json::Value {
    serde_json::json!({
        "type": "native_game_state",
        "match_id": snap.match_id,
        "version": snap.version,
        "state": snap,
    })
}

fn ev_started(snap: &GameSnapshot) -> serde_json::Value {
    serde_json::json!({
        "type": "native_game_started",
        "match_id": snap.match_id,
        "version": snap.version,
        "state": snap,
    })
}

fn ev_move(
    snap: &GameSnapshot,
    sequence: i32,
    column: u8,
    row: u8,
    player: Uuid,
) -> serde_json::Value {
    serde_json::json!({
        "type": "native_game_move",
        "match_id": snap.match_id,
        "version": snap.version,
        "move": {
            "sequence": sequence,
            "column": column,
            "row": row,
            "playerId": player,
            "stateHash": snap.state_hash,
        },
        "state": snap,
    })
}

fn ev_finished(snap: &GameSnapshot) -> serde_json::Value {
    serde_json::json!({
        "type": "native_game_finished",
        "match_id": snap.match_id,
        "version": snap.version,
        "result": snap.result,
        "endReason": snap.end_reason,
        "winnerUserId": snap.winner_user_id,
        "state": snap,
    })
}

/// Typed `match_update` envelope (same shape the episode runner already emits).
pub async fn match_update_event(pool: &PgPool, match_id: Uuid) -> Option<serde_json::Value> {
    let mut m = crate::api::load_match_full(pool, match_id).await.ok()?;
    m.calculate_wager();
    crate::api::enrich_match_with_profiles(pool, &mut m).await;
    Some(serde_json::json!({ "type": "match_update", "match": m }))
}

/// Sends events to all WebSocket clients. Call **after** the DB transaction committed.
pub async fn publish(
    pool: &PgPool,
    tx: &tokio::sync::broadcast::Sender<String>,
    match_id: Uuid,
    events: &[serde_json::Value],
) {
    for e in events {
        let _ = tx.send(e.to_string());
    }
    if let Some(e) = match_update_event(pool, match_id).await {
        let _ = tx.send(e.to_string());
    }
}

// ── Lifecycle: funded → ready to play ────────────────────────────────────────

/// Creates the session for a fully funded native match and moves it `FUNDED → READY_TO_PLAY`.
///
/// Called by the episode runner inside its transaction. Idempotent: an existing session is kept.
pub async fn prepare_session_tx(
    conn: &mut PgConnection,
    match_id: Uuid,
    creator: Uuid,
    opponent: Uuid,
) -> Result<(), NativeError> {
    let game = ConnectFour::new(creator.to_string(), opponent.to_string());
    let board = board_state_json(&game)?;
    sqlx::query(
        "INSERT INTO native_game_sessions \
         (match_id, game_type, status, player_one_id, player_two_id, board_state, \
          current_player_id, state_hash) \
         VALUES ($1, $2, 'READY', $3, $4, $5::text::jsonb, $3, $6) \
         ON CONFLICT (match_id) DO NOTHING",
    )
    .bind(match_id)
    .bind(GAME_TYPE_CONNECT_FOUR)
    .bind(creator)
    .bind(opponent)
    .bind(board)
    .bind(game.state_hash())
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "UPDATE matches SET status = 'READY_TO_PLAY' \
         WHERE id = $1 AND status = 'FUNDED' AND provider = 'NATIVE'",
    )
    .bind(match_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ── Lifecycle: start ─────────────────────────────────────────────────────────

/// Starts a `READY` session. `actor == None` means the server itself (autostart).
/// Idempotent: starting a running game returns the current snapshot without events.
pub async fn start_game(
    pool: &PgPool,
    match_id: Uuid,
    actor: Option<Uuid>,
) -> Result<Outcome, NativeError> {
    let mut tx = pool.begin().await?;
    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let m = lock_match(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("match_not_found"))?;
    if let Some(user) = actor {
        if !is_participant(&m, user) {
            return Err(NativeError::NotParticipant);
        }
    }

    if session.status == "ACTIVE" {
        let snapshot = snapshot_from(&mut tx, &session, m.status.clone()).await?;
        tx.rollback().await?;
        return Ok(Outcome {
            snapshot,
            events: vec![],
            replay: true,
        });
    }
    if session.status != "READY" {
        return Err(NativeError::WrongStatus(format!(
            "session is {}",
            session.status
        )));
    }
    m.validate_action(&MatchAction::StartNativeGame)
        .map_err(NativeError::WrongStatus)?;

    let deadline = Utc::now() + Duration::seconds(turn_timeout_secs());
    sqlx::query(
        "UPDATE native_game_sessions \
         SET status = 'ACTIVE', started_at = NOW(), updated_at = NOW(), \
             turn_deadline = $2, version = version + 1 \
         WHERE match_id = $1",
    )
    .bind(match_id)
    .bind(deadline)
    .execute(&mut *tx)
    .await?;
    let moved = sqlx::query(
        "UPDATE matches SET status = 'IN_GAME' \
         WHERE id = $1 AND status = 'READY_TO_PLAY' AND provider = 'NATIVE'",
    )
    .bind(match_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if moved != 1 {
        return Err(NativeError::WrongStatus(
            "match is not READY_TO_PLAY".into(),
        ));
    }

    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let snapshot = snapshot_from(&mut tx, &session, MatchStatus::InGame).await?;
    tx.commit().await?;
    let events = vec![ev_started(&snapshot)];
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

// ── Lifecycle: moves ─────────────────────────────────────────────────────────

pub async fn apply_move(
    pool: &PgPool,
    match_id: Uuid,
    user: Uuid,
    req: MoveRequest,
) -> Result<Outcome, NativeError> {
    if Uuid::parse_str(&req.client_nonce).is_err() {
        return Err(NativeError::BadRequest("clientNonce must be a UUID"));
    }
    let nonce = req.client_nonce.to_ascii_lowercase();

    let mut tx = pool.begin().await?;
    // 1. lock session row
    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let m = lock_match(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("match_not_found"))?;

    // 2. membership
    if !is_participant(&m, user) {
        return Err(NativeError::NotParticipant);
    }

    // 5 (before 3/4, see docs): idempotency — a retry of an already applied move is a success and
    // must not be rejected as a stale version.
    let existing: Option<i32> = sqlx::query_scalar(
        "SELECT sequence_number FROM native_game_moves \
         WHERE match_id = $1 AND player_id = $2 AND client_nonce = $3",
    )
    .bind(match_id)
    .bind(user)
    .bind(&nonce)
    .fetch_optional(&mut *tx)
    .await?;
    if existing.is_some() {
        let snapshot = snapshot_from(&mut tx, &session, m.status.clone()).await?;
        tx.rollback().await?;
        return Ok(Outcome {
            snapshot,
            events: vec![],
            replay: true,
        });
    }

    // 3. match status
    if m.status != MatchStatus::InGame || m.provider != MatchProvider::Native {
        return Err(NativeError::WrongStatus(format!("match is {:?}", m.status)));
    }
    if session.status != "ACTIVE" {
        return Err(NativeError::WrongStatus(format!(
            "session is {}",
            session.status
        )));
    }

    // 4. optimistic concurrency
    if req.expected_version != session.version {
        return Err(NativeError::VersionConflict {
            current_version: session.version,
        });
    }

    // 6. domain engine
    let mut game = engine_of(&session)?;
    if req.column < 0 {
        return Err(NativeError::Move(MoveError::InvalidColumn(usize::MAX)));
    }
    let applied = game
        .execute(&user.to_string(), req.column as usize)
        .map_err(NativeError::Move)?;
    let state_hash = game.state_hash();
    let sequence = game.move_count() as i32;

    // 7. audit
    sqlx::query(
        "INSERT INTO native_game_moves \
         (match_id, sequence_number, player_id, column_index, row_index, client_nonce, state_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(match_id)
    .bind(sequence)
    .bind(user)
    .bind(applied.column as i16)
    .bind(applied.row as i16)
    .bind(&nonce)
    .bind(&state_hash)
    .execute(&mut *tx)
    .await?;

    // 8. session
    let board = board_state_json(&game)?;
    let next_player = game.current_player().and_then(|p| Uuid::parse_str(p).ok());
    let deadline = next_player.map(|_| Utc::now() + Duration::seconds(turn_timeout_secs()));
    sqlx::query(
        "UPDATE native_game_sessions SET \
         board_state = $2::text::jsonb, current_player_id = $3, move_count = $4, \
         version = version + 1, state_hash = $5, last_move_column = $6, last_move_row = $7, \
         turn_deadline = $8, updated_at = NOW() \
         WHERE match_id = $1",
    )
    .bind(match_id)
    .bind(board)
    .bind(next_player)
    .bind(sequence)
    .bind(&state_hash)
    .bind(applied.column as i16)
    .bind(applied.row as i16)
    .bind(deadline)
    .execute(&mut *tx)
    .await?;

    // 9./10. game over → settle the match
    let mut finished = false;
    match game.status() {
        GameStatus::InProgress => {}
        GameStatus::Won { winner, .. } => {
            let winner_id = Uuid::parse_str(game.player_at(*winner))
                .map_err(|e| NativeError::Internal(e.to_string()))?;
            let loser_id = Uuid::parse_str(game.player_at(winner.other()))
                .map_err(|e| NativeError::Internal(e.to_string()))?;
            settle_tx(
                &mut tx,
                &m,
                match_id,
                Some((winner_id, loser_id)),
                EndReason::ConnectFour,
                &state_hash,
            )
            .await?;
            finished = true;
        }
        GameStatus::Draw => {
            settle_tx(&mut tx, &m, match_id, None, EndReason::Draw, &state_hash).await?;
            finished = true;
        }
    }

    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let match_status: MatchStatus = sqlx::query_scalar("SELECT status FROM matches WHERE id = $1")
        .bind(match_id)
        .fetch_one(&mut *tx)
        .await?;
    let snapshot = snapshot_from(&mut tx, &session, match_status).await?;
    // 11. commit
    tx.commit().await?;

    // 12. events (caller publishes)
    let mut events = vec![ev_move(
        &snapshot,
        sequence,
        applied.column,
        applied.row,
        user,
    )];
    if finished {
        events.push(ev_finished(&snapshot));
    } else {
        events.push(ev_state(&snapshot));
    }
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

/// Writes the final result to session + match (inside the caller's transaction).
///
/// `winner_loser == None` is a draw: the match goes to `REFUND_PENDING` and **no winner is
/// recorded**, so the payout worker (which requires `winner_user_id`) can never pick it up.
async fn settle_tx(
    conn: &mut PgConnection,
    m: &Match,
    match_id: Uuid,
    winner_loser: Option<(Uuid, Uuid)>,
    reason: EndReason,
    final_state_hash: &str,
) -> Result<(), NativeError> {
    let action = match winner_loser {
        Some((w, l)) => MatchAction::NativeGameFinished {
            winner_id: w.to_string(),
            loser_id: l.to_string(),
        },
        None => MatchAction::NativeGameDrawn,
    };
    m.validate_action(&action)
        .map_err(NativeError::WrongStatus)?;

    let hash = result_hash(
        &match_id.to_string(),
        GAME_TYPE_CONNECT_FOUR,
        final_state_hash,
        winner_loser.map(|(w, _)| w.to_string()).as_deref(),
        reason,
    );
    let result = if winner_loser.is_some() {
        "WIN"
    } else {
        "DRAW"
    };

    sqlx::query(
        "UPDATE native_game_sessions SET \
         status = 'FINISHED', result = $2, end_reason = $3, winner_user_id = $4, \
         current_player_id = NULL, turn_deadline = NULL, finished_at = NOW(), updated_at = NOW() \
         WHERE match_id = $1",
    )
    .bind(match_id)
    .bind(result)
    .bind(reason.as_str())
    .bind(winner_loser.map(|(w, _)| w))
    .execute(&mut *conn)
    .await?;

    let rows = match winner_loser {
        Some((w, l)) => sqlx::query(
            "UPDATE matches SET status = 'FINISHED_GAME', winner_user_id = $2, loser_user_id = $3, \
             result_source = $4, game_finished_at = NOW(), result_hash = $5 \
             WHERE id = $1 AND status = 'IN_GAME' AND provider = 'NATIVE'",
        )
        .bind(match_id)
        .bind(w)
        .bind(l)
        .bind(RESULT_SOURCE_NATIVE_ENGINE)
        .bind(&hash)
        .execute(&mut *conn)
        .await?
        .rows_affected(),
        None => sqlx::query(
            "UPDATE matches SET status = 'REFUND_PENDING', winner_user_id = NULL, loser_user_id = NULL, \
             result_source = $2, game_finished_at = NOW(), result_hash = $3 \
             WHERE id = $1 AND status = 'IN_GAME' AND provider = 'NATIVE'",
        )
        .bind(match_id)
        .bind(RESULT_SOURCE_NATIVE_ENGINE)
        .bind(&hash)
        .execute(&mut *conn)
        .await?
        .rows_affected(),
    };
    if rows != 1 {
        return Err(NativeError::WrongStatus("match was not IN_GAME".into()));
    }
    Ok(())
}

// ── Resign / timeout ─────────────────────────────────────────────────────────

/// Ends an active game because `loser` gave up or ran out of time; the opponent wins.
async fn forfeit_locked(
    tx: &mut PgConnection,
    session: &SessionRow,
    m: &Match,
    loser: Uuid,
    reason: EndReason,
) -> Result<(), NativeError> {
    let winner = if loser == session.player_one {
        session.player_two
    } else {
        session.player_one
    };
    // Every state change bumps the version (moves bump it themselves before settling).
    sqlx::query("UPDATE native_game_sessions SET version = version + 1 WHERE match_id = $1")
        .bind(session.match_id)
        .execute(&mut *tx)
        .await?;
    settle_tx(
        tx,
        m,
        session.match_id,
        Some((winner, loser)),
        reason,
        &session.state_hash,
    )
    .await
}

pub async fn resign(pool: &PgPool, match_id: Uuid, user: Uuid) -> Result<Outcome, NativeError> {
    let mut tx = pool.begin().await?;
    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let m = lock_match(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("match_not_found"))?;
    if !is_participant(&m, user) {
        return Err(NativeError::NotParticipant);
    }
    if m.status != MatchStatus::InGame || session.status != "ACTIVE" {
        return Err(NativeError::WrongStatus(format!("match is {:?}", m.status)));
    }
    forfeit_locked(&mut tx, &session, &m, user, EndReason::Resignation).await?;
    finish_outcome(tx, match_id).await
}

/// If the current player's turn deadline passed, they forfeit. Returns `None` when nothing was due.
/// Called by the episode runner after its own transaction has committed.
pub async fn expire_if_due(pool: &PgPool, match_id: Uuid) -> Result<Option<Outcome>, NativeError> {
    let mut tx = pool.begin().await?;
    let Some(session) = lock_session(&mut tx, match_id).await? else {
        return Ok(None);
    };
    let due = session.status == "ACTIVE"
        && session
            .turn_deadline
            .map(|d| d <= Utc::now())
            .unwrap_or(false);
    if !due {
        return Ok(None);
    }
    let m = lock_match(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("match_not_found"))?;
    let Some(loser) = session.current_player else {
        return Ok(None);
    };
    forfeit_locked(&mut tx, &session, &m, loser, EndReason::Timeout).await?;
    finish_outcome(tx, match_id).await.map(Some)
}

/// Server-side autostart for a `READY` game nobody started within the grace period.
pub async fn autostart_if_due(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Option<Outcome>, NativeError> {
    let created: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT created_at FROM native_game_sessions WHERE match_id = $1 AND status = 'READY'",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await?;
    match created {
        Some(at) if Utc::now() - at >= Duration::seconds(autostart_secs()) => {
            start_game(pool, match_id, None).await.map(Some)
        }
        _ => Ok(None),
    }
}

async fn finish_outcome(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
) -> Result<Outcome, NativeError> {
    let session = lock_session(&mut tx, match_id)
        .await?
        .ok_or(NativeError::NotFound("game_not_started"))?;
    let match_status: MatchStatus = sqlx::query_scalar("SELECT status FROM matches WHERE id = $1")
        .bind(match_id)
        .fetch_one(&mut *tx)
        .await?;
    let snapshot = snapshot_from(&mut tx, &session, match_status).await?;
    tx.commit().await?;
    let events = vec![ev_finished(&snapshot)];
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeError::NotFound(c) => write!(f, "not found: {c}"),
            NativeError::NotParticipant => write!(f, "not a participant"),
            NativeError::WrongStatus(s) => write!(f, "wrong status: {s}"),
            NativeError::VersionConflict { current_version } => {
                write!(f, "version conflict (current {current_version})")
            }
            NativeError::Move(e) => write!(f, "{e}"),
            NativeError::BadRequest(m) => write!(f, "bad request: {m}"),
            NativeError::Db(e) => write!(f, "database error: {e}"),
            NativeError::Internal(m) => write!(f, "internal error: {m}"),
        }
    }
}

impl std::error::Error for NativeError {}

/// True if the user has linked a FACEIT account (`faceit_links`), the same signal `/auth/me` reports
/// as `faceit_connected`.
pub async fn user_has_faceit_link(pool: &PgPool, user: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM faceit_links WHERE user_id = $1)")
        .bind(user)
        .fetch_one(pool)
        .await
}
