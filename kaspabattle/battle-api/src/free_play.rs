//! # Free Play – off-chain Connect Four (no wallet, no stake, no Kaspa node)
//!
//! Persistence + lifecycle around the shared pure engine (`battle_core::native_games`). Free Play
//! deliberately lives in its own tables (`free_play_games`, `free_play_moves`) without any foreign
//! key to `matches` / `multisig_escrows` / `payments`, and nothing in this module touches the
//! Kaspa RPC, a wallet or the escrow code: a free-play game *cannot* cause a payment.
//!
//! The server is authoritative. A client only sends a column; the server validates participant,
//! turn, status and version, computes the board with the engine and persists everything in one
//! transaction (`SELECT … FOR UPDATE` on the game row). Events are published after commit.

use battle_core::native_games::connect_four::{
    ConnectFour, ConnectFourState, GameStatus, MoveError, COLUMNS, ROWS,
};
use battle_core::native_games::connect_four_bot::{choose_column, BotParams, Difficulty};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const P1: &str = "p1";
const P2: &str = "p2";
const MAX_OPEN_LOBBIES_PER_USER: i64 = 3;
const MAX_ACTIVE_GAMES_PER_USER: i64 = 8;

// ── Configuration ───────────────────────────────────────────────────────────

fn env_secs(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

/// A human player who does not move within this time forfeits (`FREE_PLAY_TURN_TIMEOUT_SECS`).
/// This is the documented disconnect rule: reconnecting inside the window keeps the game intact.
pub fn turn_timeout() -> Duration {
    Duration::seconds(env_secs("FREE_PLAY_TURN_TIMEOUT_SECS", 180))
}
/// Bot games are abandoned (no winner, not counted as win/loss) after this idle time.
pub fn bot_idle_timeout() -> Duration {
    Duration::seconds(env_secs("FREE_PLAY_BOT_IDLE_SECS", 3600))
}
/// Open lobbies nobody joined are closed after this time.
pub fn lobby_ttl() -> Duration {
    Duration::seconds(env_secs("FREE_PLAY_LOBBY_TTL_SECS", 1800))
}
fn bot_budget() -> std::time::Duration {
    std::time::Duration::from_millis(env_secs("FREE_PLAY_BOT_BUDGET_MS", 250) as u64)
}

// ── Presence (who has a live WebSocket on which game) ──────────────────────

#[derive(Clone, Default)]
pub struct Presence(Arc<Mutex<HashMap<Uuid, HashMap<Uuid, usize>>>>);

impl Presence {
    /// Returns true if this is the user's first connection to the game.
    pub fn join(&self, game: Uuid, user: Uuid) -> bool {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let c = m.entry(game).or_default().entry(user).or_insert(0);
        *c += 1;
        *c == 1
    }
    /// Returns true if that was the user's last connection.
    pub fn leave(&self, game: Uuid, user: Uuid) -> bool {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(g) = m.get_mut(&game) else {
            return false;
        };
        let Some(c) = g.get_mut(&user) else {
            return false;
        };
        *c = c.saturating_sub(1);
        let gone = *c == 0;
        if gone {
            g.remove(&user);
            if g.is_empty() {
                m.remove(&game);
            }
        }
        gone
    }
    pub fn is_connected(&self, game: Uuid, user: Uuid) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(&game)
            .map(|g| g.contains_key(&user))
            .unwrap_or(false)
    }
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum FpError {
    NotFound,
    NotParticipant,
    WrongStatus(&'static str),
    VersionConflict { current_version: i64 },
    Move(MoveError),
    BadRequest(&'static str),
    Conflict(&'static str),
    TooMany(&'static str),
    Db(sqlx::Error),
    Internal(String),
}

impl From<sqlx::Error> for FpError {
    fn from(e: sqlx::Error) -> Self {
        FpError::Db(e)
    }
}

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpPlayer {
    pub slot: u8,
    pub user_id: Option<Uuid>,
    pub display_name: String,
    pub is_bot: bool,
    pub color: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpSnapshot {
    pub id: Uuid,
    /// Always "free_play": no stake, no escrow, no wallet.
    pub mode: &'static str,
    pub game_type: &'static str,
    pub opponent_kind: String,
    pub bot_difficulty: Option<String>,
    /// open | active | finished | cancelled
    pub status: String,
    pub version: i64,
    /// `board[row][column]`, row 0 = bottom. 0 empty, 1 = player one (blue), 2 = player two (red).
    pub board: Vec<Vec<u8>>,
    pub columns: usize,
    pub rows: usize,
    pub move_count: i32,
    pub current_slot: Option<u8>,
    pub current_player_id: Option<Uuid>,
    pub players: Vec<FpPlayer>,
    pub winner_slot: Option<u8>,
    pub winner_user_id: Option<Uuid>,
    /// win | draw | abandoned
    pub result: Option<String>,
    /// connect_four | draw | resignation | timeout | left
    pub end_reason: Option<String>,
    pub winning_line: Vec<(u8, u8)>,
    pub last_move: Option<FpLastMove>,
    pub turn_deadline: Option<DateTime<Utc>>,
    pub state_hash: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub rematch_requested_by: Option<Uuid>,
    pub rematch_game_id: Option<Uuid>,
    pub turn_timeout_secs: i64,
    pub server_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpLastMove {
    pub column: i16,
    pub row: i16,
    pub slot: u8,
}

#[derive(Debug)]
pub struct Outcome {
    pub snapshot: FpSnapshot,
    pub events: Vec<serde_json::Value>,
    pub replay: bool,
}

#[derive(Debug, Clone)]
pub struct MoveRequest {
    pub column: i64,
    pub expected_version: i64,
    pub client_nonce: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Opponent {
    Human,
    Bot(Difficulty),
}

// ── Row access ──────────────────────────────────────────────────────────────

struct Game {
    id: Uuid,
    opponent_kind: String,
    bot_difficulty: Option<String>,
    status: String,
    p1: Uuid,
    p2: Option<Uuid>,
    board_state: String,
    current_slot: Option<i16>,
    move_count: i32,
    version: i64,
    state_hash: String,
    last_col: Option<i16>,
    last_row: Option<i16>,
    winner_slot: Option<i16>,
    winner_user: Option<Uuid>,
    result: Option<String>,
    end_reason: Option<String>,
    turn_deadline: Option<DateTime<Utc>>,
    rematch_requested_by: Option<Uuid>,
    rematch_game_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

const COLS: &str = "id, opponent_kind, bot_difficulty, status, player_one_id, player_two_id, \
    board_state::text AS board_state, current_slot, move_count, version, state_hash, \
    last_move_column, last_move_row, winner_slot, winner_user_id, result, end_reason, turn_deadline, \
    rematch_requested_by, rematch_game_id, created_at, started_at, finished_at";

fn game_from(r: &sqlx::postgres::PgRow) -> Result<Game, sqlx::Error> {
    Ok(Game {
        id: r.try_get("id")?,
        opponent_kind: r.try_get("opponent_kind")?,
        bot_difficulty: r.try_get("bot_difficulty")?,
        status: r.try_get("status")?,
        p1: r.try_get("player_one_id")?,
        p2: r.try_get("player_two_id")?,
        board_state: r.try_get("board_state")?,
        current_slot: r.try_get("current_slot")?,
        move_count: r.try_get("move_count")?,
        version: r.try_get("version")?,
        state_hash: r.try_get("state_hash")?,
        last_col: r.try_get("last_move_column")?,
        last_row: r.try_get("last_move_row")?,
        winner_slot: r.try_get("winner_slot")?,
        winner_user: r.try_get("winner_user_id")?,
        result: r.try_get("result")?,
        end_reason: r.try_get("end_reason")?,
        turn_deadline: r.try_get("turn_deadline")?,
        rematch_requested_by: r.try_get("rematch_requested_by")?,
        rematch_game_id: r.try_get("rematch_game_id")?,
        created_at: r.try_get("created_at")?,
        started_at: r.try_get("started_at")?,
        finished_at: r.try_get("finished_at")?,
    })
}

async fn lock(conn: &mut PgConnection, id: Uuid) -> Result<Option<Game>, sqlx::Error> {
    let q = format!("SELECT {COLS} FROM free_play_games WHERE id = $1 FOR UPDATE");
    match sqlx::query(&q).bind(id).fetch_optional(conn).await? {
        Some(r) => Ok(Some(game_from(&r)?)),
        None => Ok(None),
    }
}

async fn fetch(conn: &mut PgConnection, id: Uuid) -> Result<Option<Game>, sqlx::Error> {
    let q = format!("SELECT {COLS} FROM free_play_games WHERE id = $1");
    match sqlx::query(&q).bind(id).fetch_optional(conn).await? {
        Some(r) => Ok(Some(game_from(&r)?)),
        None => Ok(None),
    }
}

fn engine_of(g: &Game) -> Result<ConnectFour, FpError> {
    let state: ConnectFourState = serde_json::from_str(&g.board_state)
        .map_err(|e| FpError::Internal(format!("corrupt board: {e}")))?;
    ConnectFour::from_state(P1, P2, state).map_err(|e| FpError::Internal(e.to_string()))
}

fn board_json(game: &ConnectFour) -> Result<String, FpError> {
    serde_json::to_string(game.state()).map_err(|e| FpError::Internal(e.to_string()))
}

fn slot_of(g: &Game, user: Uuid) -> Option<u8> {
    if g.p1 == user {
        Some(1)
    } else if g.p2 == Some(user) {
        Some(2)
    } else {
        None
    }
}

fn is_bot(g: &Game) -> bool {
    g.opponent_kind == "bot"
}

// ── Snapshot & events ───────────────────────────────────────────────────────

async fn snapshot_of(conn: &mut PgConnection, g: &Game) -> Result<FpSnapshot, FpError> {
    let engine = engine_of(g)?;
    let ids: Vec<Uuid> = std::iter::once(g.p1).chain(g.p2).collect();
    let names: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, COALESCE(username, display_name) FROM users WHERE id = ANY($1)")
            .bind(ids)
            .fetch_all(&mut *conn)
            .await?;
    let name = |id: Uuid| {
        names
            .iter()
            .find(|(u, _)| *u == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    };
    let mut players = vec![FpPlayer {
        slot: 1,
        user_id: Some(g.p1),
        display_name: name(g.p1),
        is_bot: false,
        color: "blue",
    }];
    players.push(match (g.p2, is_bot(g)) {
        (Some(p2), _) => FpPlayer {
            slot: 2,
            user_id: Some(p2),
            display_name: name(p2),
            is_bot: false,
            color: "red",
        },
        (None, true) => FpPlayer {
            slot: 2,
            user_id: None,
            display_name: "Bot".into(),
            is_bot: true,
            color: "red",
        },
        (None, false) => FpPlayer {
            slot: 2,
            user_id: None,
            display_name: String::new(),
            is_bot: false,
            color: "red",
        },
    });
    let current_slot = g.current_slot.map(|s| s as u8);
    let current_player_id = match current_slot {
        Some(1) => Some(g.p1),
        Some(2) => g.p2,
        _ => None,
    };
    let winning_line = match engine.status() {
        GameStatus::Won { line, .. } => line.clone(),
        _ => vec![],
    };
    let last_move = match (g.last_col, g.last_row) {
        (Some(column), Some(row)) => Some(FpLastMove {
            column,
            row,
            slot: engine.cell(row as usize, column as usize),
        }),
        _ => None,
    };
    Ok(FpSnapshot {
        id: g.id,
        mode: "free_play",
        game_type: "connect_four",
        opponent_kind: g.opponent_kind.clone(),
        bot_difficulty: g.bot_difficulty.clone(),
        status: g.status.clone(),
        version: g.version,
        board: (0..ROWS)
            .map(|r| engine.state().cells[r * COLUMNS..(r + 1) * COLUMNS].to_vec())
            .collect(),
        columns: COLUMNS,
        rows: ROWS,
        move_count: g.move_count,
        current_slot,
        current_player_id,
        players,
        winner_slot: g.winner_slot.map(|s| s as u8),
        winner_user_id: g.winner_user,
        result: g.result.clone(),
        end_reason: g.end_reason.clone(),
        winning_line,
        last_move,
        turn_deadline: g.turn_deadline,
        state_hash: g.state_hash.clone(),
        created_at: g.created_at,
        started_at: g.started_at,
        finished_at: g.finished_at,
        rematch_requested_by: g.rematch_requested_by,
        rematch_game_id: g.rematch_game_id,
        turn_timeout_secs: turn_timeout().num_seconds(),
        server_time: Utc::now(),
    })
}

fn ev(kind: &str, s: &FpSnapshot) -> serde_json::Value {
    serde_json::json!({ "type": kind, "gameId": s.id, "version": s.version, "state": s })
}

fn lobby_ev(action: &str, id: Uuid) -> serde_json::Value {
    serde_json::json!({ "type": "free_play_lobby", "action": action, "gameId": id })
}

pub fn presence_ev(game: Uuid, user: Uuid, connected: bool) -> serde_json::Value {
    serde_json::json!({ "type": "free_play_presence", "gameId": game, "userId": user, "connected": connected })
}

/// Sends events to all WebSocket clients – call only **after** the transaction committed.
pub fn publish(tx: &tokio::sync::broadcast::Sender<String>, events: &[serde_json::Value]) {
    for e in events {
        let _ = tx.send(e.to_string());
    }
}

// ── Create / list / join / leave ────────────────────────────────────────────

fn initial_state() -> (ConnectFour, String, String) {
    let g = ConnectFour::new(P1, P2);
    let board = serde_json::to_string(g.state()).expect("static state serialises");
    let hash = g.state_hash();
    (g, board, hash)
}

pub async fn create_game(
    pool: &PgPool,
    user: Uuid,
    opponent: Opponent,
) -> Result<Outcome, FpError> {
    let mut tx = pool.begin().await?;
    // Serialise per-user limit checks.
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    let open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM free_play_games WHERE player_one_id = $1 AND status = 'open'",
    )
    .bind(user)
    .fetch_one(&mut *tx)
    .await?;
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM free_play_games WHERE status = 'active' AND (player_one_id = $1 OR player_two_id = $1)",
    )
    .bind(user)
    .fetch_one(&mut *tx)
    .await?;
    if active >= MAX_ACTIVE_GAMES_PER_USER {
        return Err(FpError::TooMany("too_many_active_games"));
    }
    let (_, board, hash) = initial_state();
    let id = Uuid::new_v4();
    match opponent {
        Opponent::Human => {
            if open >= MAX_OPEN_LOBBIES_PER_USER {
                return Err(FpError::TooMany("too_many_open_lobbies"));
            }
            sqlx::query(
                "INSERT INTO free_play_games (id, opponent_kind, status, player_one_id, board_state, state_hash) \
                 VALUES ($1, 'human', 'open', $2, $3::text::jsonb, $4)",
            )
            .bind(id)
            .bind(user)
            .bind(&board)
            .bind(&hash)
            .execute(&mut *tx)
            .await?;
        }
        Opponent::Bot(d) => {
            sqlx::query(
                "INSERT INTO free_play_games (id, opponent_kind, bot_difficulty, status, player_one_id, board_state, state_hash, \
                 current_slot, started_at, turn_deadline, version) \
                 VALUES ($1, 'bot', $2, 'active', $3, $4::text::jsonb, $5, 1, NOW(), $6, 1)",
            )
            .bind(id)
            .bind(d.as_str())
            .bind(user)
            .bind(&board)
            .bind(&hash)
            .bind(Utc::now() + bot_idle_timeout())
            .execute(&mut *tx)
            .await?;
        }
    }
    let g = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let snapshot = snapshot_of(&mut tx, &g).await?;
    tx.commit().await?;
    let events = match opponent {
        Opponent::Human => vec![lobby_ev("created", id)],
        Opponent::Bot(_) => vec![],
    };
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyItem {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub creator_name: String,
    pub created_at: DateTime<Utc>,
    pub mine: bool,
}

pub async fn list_open(pool: &PgPool, viewer: Uuid) -> Result<Vec<LobbyItem>, FpError> {
    let rows = sqlx::query(
        "SELECT g.id, g.player_one_id, COALESCE(u.username, u.display_name) AS name, g.created_at \
         FROM free_play_games g JOIN users u ON u.id = g.player_one_id \
         WHERE g.status = 'open' ORDER BY g.created_at DESC LIMIT 50",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let creator_id: Uuid = r.get("player_one_id");
            LobbyItem {
                id: r.get("id"),
                creator_id,
                creator_name: r.get("name"),
                created_at: r.get("created_at"),
                mine: creator_id == viewer,
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveItem {
    pub id: Uuid,
    pub opponent_kind: String,
    pub bot_difficulty: Option<String>,
    pub opponent_name: String,
    pub your_turn: bool,
    pub move_count: i32,
}

/// Running games of this user (so a closed tab never loses a game).
pub async fn list_active(pool: &PgPool, user: Uuid) -> Result<Vec<ActiveItem>, FpError> {
    let rows = sqlx::query(
        "SELECT g.id, g.opponent_kind, g.bot_difficulty, g.player_one_id, g.current_slot, g.move_count, \
                COALESCE(o.username, o.display_name) AS opp_name \
         FROM free_play_games g \
         LEFT JOIN users o ON o.id = CASE WHEN g.player_one_id = $1 THEN g.player_two_id ELSE g.player_one_id END \
         WHERE g.status = 'active' AND (g.player_one_id = $1 OR g.player_two_id = $1) \
         ORDER BY g.updated_at DESC LIMIT 20",
    )
    .bind(user)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let p1: Uuid = r.get("player_one_id");
            let slot: Option<i16> = r.get("current_slot");
            let kind: String = r.get("opponent_kind");
            ActiveItem {
                id: r.get("id"),
                opponent_name: if kind == "bot" {
                    "Bot".into()
                } else {
                    r.get::<Option<String>, _>("opp_name").unwrap_or_default()
                },
                opponent_kind: kind,
                bot_difficulty: r.get("bot_difficulty"),
                your_turn: slot == Some(if p1 == user { 1 } else { 2 }),
                move_count: r.get("move_count"),
            }
        })
        .collect())
}

pub async fn join(pool: &PgPool, id: Uuid, user: Uuid) -> Result<Outcome, FpError> {
    let mut tx = pool.begin().await?;
    let g = lock(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    if g.p1 == user {
        return Err(FpError::Conflict("own_lobby"));
    }
    if g.status != "open" || g.opponent_kind != "human" || g.p2.is_some() {
        return Err(FpError::Conflict("lobby_not_open"));
    }
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM free_play_games WHERE status = 'active' AND (player_one_id = $1 OR player_two_id = $1)",
    )
    .bind(user)
    .fetch_one(&mut *tx)
    .await?;
    if active >= MAX_ACTIVE_GAMES_PER_USER {
        return Err(FpError::TooMany("too_many_active_games"));
    }
    // The row is locked: exactly one concurrent joiner gets here.
    sqlx::query(
        "UPDATE free_play_games SET status = 'active', player_two_id = $2, joined_at = NOW(), started_at = NOW(), \
         current_slot = 1, turn_deadline = $3, version = version + 1, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .bind(user)
    .bind(Utc::now() + turn_timeout())
    .execute(&mut *tx)
    .await?;
    let g = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let snapshot = snapshot_of(&mut tx, &g).await?;
    tx.commit().await?;
    let events = vec![
        ev("free_play_game_started", &snapshot),
        lobby_ev("joined", id),
    ];
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

/// Creator closes an open lobby, or a player leaves a running game (= resigns).
pub async fn leave(pool: &PgPool, id: Uuid, user: Uuid) -> Result<Outcome, FpError> {
    let mut tx = pool.begin().await?;
    let g = lock(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let slot = slot_of(&g, user).ok_or(FpError::NotParticipant)?;
    match g.status.as_str() {
        "open" => {
            sqlx::query("UPDATE free_play_games SET status = 'cancelled', current_slot = NULL, turn_deadline = NULL, version = version + 1, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            let g = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
            let snapshot = snapshot_of(&mut tx, &g).await?;
            tx.commit().await?;
            Ok(Outcome {
                snapshot,
                events: vec![lobby_ev("closed", id)],
                replay: false,
            })
        }
        "active" => {
            let winner = if slot == 1 { 2 } else { 1 };
            forfeit_tx(&mut tx, &g, winner, "left").await?;
            let g = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
            let snapshot = snapshot_of(&mut tx, &g).await?;
            tx.commit().await?;
            let events = vec![ev("free_play_finished", &snapshot)];
            Ok(Outcome {
                snapshot,
                events,
                replay: false,
            })
        }
        _ => Err(FpError::WrongStatus("game_not_running")),
    }
}

/// Ends an active game: `winner_slot` wins because the other side left / timed out / resigned.
async fn forfeit_tx(
    conn: &mut PgConnection,
    g: &Game,
    winner_slot: i16,
    reason: &str,
) -> Result<(), FpError> {
    let winner_user = if winner_slot == 1 { Some(g.p1) } else { g.p2 }; // None when the bot wins
    finish_tx(
        conn,
        g.id,
        "win",
        reason,
        Some(winner_slot),
        winner_user,
        true,
    )
    .await
}

async fn finish_tx(
    conn: &mut PgConnection,
    id: Uuid,
    result: &str,
    reason: &str,
    winner_slot: Option<i16>,
    winner_user: Option<Uuid>,
    bump_version: bool,
) -> Result<(), FpError> {
    sqlx::query(
        "UPDATE free_play_games SET status = 'finished', result = $2, end_reason = $3, winner_slot = $4, winner_user_id = $5, \
         current_slot = NULL, turn_deadline = NULL, finished_at = NOW(), updated_at = NOW(), \
         version = version + CASE WHEN $6 THEN 1 ELSE 0 END WHERE id = $1 AND status = 'active'",
    )
    .bind(id)
    .bind(result)
    .bind(reason)
    .bind(winner_slot)
    .bind(winner_user)
    .bind(bump_version)
    .execute(conn)
    .await?;
    Ok(())
}

// ── Reading ─────────────────────────────────────────────────────────────────

/// Snapshot of one game. Spectators may read; the board is not secret. Also completes a pending bot
/// move (e.g. if the server restarted between the human move and the bot reply).
pub async fn get(pool: &PgPool, id: Uuid) -> Result<FpSnapshot, FpError> {
    if let Some(out) = bot_turn_if_needed(pool, id).await? {
        return Ok(out.snapshot);
    }
    let mut conn = pool.acquire().await?;
    let g = fetch(&mut conn, id).await?.ok_or(FpError::NotFound)?;
    snapshot_of(&mut conn, &g).await
}

// ── Moves ───────────────────────────────────────────────────────────────────

pub async fn apply_move(
    pool: &PgPool,
    id: Uuid,
    user: Uuid,
    req: MoveRequest,
) -> Result<Outcome, FpError> {
    if Uuid::parse_str(&req.client_nonce).is_err() {
        return Err(FpError::BadRequest("clientNonce must be a UUID"));
    }
    let nonce = req.client_nonce.to_ascii_lowercase();
    let mut tx = pool.begin().await?;
    let g = lock(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let slot = slot_of(&g, user).ok_or(FpError::NotParticipant)?;

    // Idempotency first: a retried move is a success, not a stale-version error.
    let seen: Option<i32> = sqlx::query_scalar(
        "SELECT sequence_number FROM free_play_moves WHERE game_id = $1 AND player_id = $2 AND client_nonce = $3",
    )
    .bind(id)
    .bind(user)
    .bind(&nonce)
    .fetch_optional(&mut *tx)
    .await?;
    if seen.is_some() {
        let snapshot = snapshot_of(&mut tx, &g).await?;
        tx.rollback().await?;
        return Ok(Outcome {
            snapshot,
            events: vec![],
            replay: true,
        });
    }
    if g.status != "active" {
        return Err(FpError::WrongStatus(if g.status == "finished" {
            "game_over"
        } else {
            "game_not_running"
        }));
    }
    if req.expected_version != g.version {
        return Err(FpError::VersionConflict {
            current_version: g.version,
        });
    }
    if req.column < 0 {
        return Err(FpError::Move(MoveError::InvalidColumn(usize::MAX)));
    }
    let mut engine = engine_of(&g)?;
    let player = if slot == 1 { P1 } else { P2 };
    let applied = engine
        .execute(player, req.column as usize)
        .map_err(FpError::Move)?;
    persist_move(
        &mut tx,
        &g,
        &engine,
        applied.column,
        applied.row,
        slot,
        Some(user),
        Some(&nonce),
    )
    .await?;

    let g2 = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let snapshot = snapshot_of(&mut tx, &g2).await?;
    tx.commit().await?;
    let mut events = vec![ev("free_play_move", &snapshot)];
    if snapshot.status == "finished" {
        events.push(ev("free_play_finished", &snapshot));
    }
    let mut out = Outcome {
        snapshot,
        events,
        replay: false,
    };

    // The bot answers immediately (in its own transaction, same validation path).
    if out.snapshot.status == "active" {
        if let Some(bot) = bot_turn_if_needed(pool, id).await? {
            out.events.extend(bot.events);
            out.snapshot = bot.snapshot;
        }
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
async fn persist_move(
    conn: &mut PgConnection,
    g: &Game,
    engine: &ConnectFour,
    column: u8,
    row: u8,
    slot: u8,
    player: Option<Uuid>,
    nonce: Option<&str>,
) -> Result<(), FpError> {
    let hash = engine.state_hash();
    let seq = engine.move_count() as i32;
    sqlx::query(
        "INSERT INTO free_play_moves (game_id, sequence_number, slot, player_id, column_index, row_index, client_nonce, state_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(g.id)
    .bind(seq)
    .bind(slot as i16)
    .bind(player)
    .bind(column as i16)
    .bind(row as i16)
    .bind(nonce)
    .bind(&hash)
    .execute(&mut *conn)
    .await?;

    let next_slot: Option<i16> = engine.current_player().map(|p| if p == P1 { 1 } else { 2 });
    let deadline = next_slot.map(|s| {
        let bot_next = is_bot(g) && s == 2;
        let human_vs_bot_idle = is_bot(g) && s == 1;
        if bot_next || human_vs_bot_idle {
            Utc::now() + bot_idle_timeout()
        } else {
            Utc::now() + turn_timeout()
        }
    });
    sqlx::query(
        "UPDATE free_play_games SET board_state = $2::text::jsonb, current_slot = $3, move_count = $4, version = version + 1, \
         state_hash = $5, last_move_column = $6, last_move_row = $7, turn_deadline = $8, updated_at = NOW() WHERE id = $1",
    )
    .bind(g.id)
    .bind(board_json(engine)?)
    .bind(next_slot)
    .bind(seq)
    .bind(&hash)
    .bind(column as i16)
    .bind(row as i16)
    .bind(deadline)
    .execute(&mut *conn)
    .await?;

    match engine.status() {
        GameStatus::InProgress => {}
        GameStatus::Won { winner, .. } => {
            let ws = winner.cell_value() as i16;
            let wu = if ws == 1 { Some(g.p1) } else { g.p2 };
            finish_tx(conn, g.id, "win", "connect_four", Some(ws), wu, false).await?;
        }
        GameStatus::Draw => {
            finish_tx(conn, g.id, "draw", "draw", None, None, false).await?;
        }
    }
    Ok(())
}

/// If it is the bot's turn in an active bot game, plays exactly one bot move.
pub async fn bot_turn_if_needed(pool: &PgPool, id: Uuid) -> Result<Option<Outcome>, FpError> {
    // Cheap pre-check without a lock.
    {
        let mut conn = pool.acquire().await?;
        match fetch(&mut conn, id).await? {
            Some(g) if is_bot(&g) && g.status == "active" && g.current_slot == Some(2) => {}
            _ => return Ok(None),
        }
    }
    let mut tx = pool.begin().await?;
    let Some(g) = lock(&mut tx, id).await? else {
        return Ok(None);
    };
    if !(is_bot(&g) && g.status == "active" && g.current_slot == Some(2)) {
        return Ok(None); // someone else already moved
    }
    let mut engine = engine_of(&g)?;
    let difficulty = g
        .bot_difficulty
        .as_deref()
        .and_then(Difficulty::parse)
        .unwrap_or(Difficulty::Easy);
    // Reproducible per game position.
    let seed = g.id.as_u128() as u64 ^ (g.move_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let snapshot_engine = engine.clone();
    let column = tokio::task::spawn_blocking(move || {
        choose_column(
            &snapshot_engine,
            P2,
            BotParams {
                difficulty,
                seed,
                budget: bot_budget(),
            },
        )
    })
    .await
    .map_err(|e| FpError::Internal(e.to_string()))?
    .ok_or_else(|| FpError::Internal("bot found no move".into()))?;
    // Same engine, same rules as for humans: an illegal bot move is impossible.
    let applied = engine
        .execute(P2, column)
        .map_err(|e| FpError::Internal(format!("bot move rejected: {e}")))?;
    persist_move(
        &mut tx,
        &g,
        &engine,
        applied.column,
        applied.row,
        2,
        None,
        None,
    )
    .await?;
    let g2 = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let snapshot = snapshot_of(&mut tx, &g2).await?;
    tx.commit().await?;
    let mut events = vec![ev("free_play_move", &snapshot)];
    if snapshot.status == "finished" {
        events.push(ev("free_play_finished", &snapshot));
    }
    Ok(Some(Outcome {
        snapshot,
        events,
        replay: false,
    }))
}

// ── Rematch ─────────────────────────────────────────────────────────────────

pub async fn request_rematch(pool: &PgPool, id: Uuid, user: Uuid) -> Result<Outcome, FpError> {
    let mut tx = pool.begin().await?;
    let g = lock(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let _slot = slot_of(&g, user).ok_or(FpError::NotParticipant)?;
    if g.status != "finished" {
        return Err(FpError::WrongStatus("game_not_finished"));
    }
    if g.rematch_game_id.is_some() {
        let snapshot = snapshot_of(&mut tx, &g).await?;
        tx.rollback().await?;
        return Ok(Outcome {
            snapshot,
            events: vec![],
            replay: true,
        });
    }
    let (_, board, hash) = initial_state();
    let new_id = Uuid::new_v4();
    let mut events = vec![];
    if is_bot(&g) {
        // Against the bot a rematch starts immediately; the human keeps the first move.
        sqlx::query(
            "INSERT INTO free_play_games (id, opponent_kind, bot_difficulty, status, player_one_id, board_state, state_hash, \
             current_slot, started_at, turn_deadline, version, rematch_of) \
             VALUES ($1, 'bot', $2, 'active', $3, $4::text::jsonb, $5, 1, NOW(), $6, 1, $7)",
        )
        .bind(new_id)
        .bind(&g.bot_difficulty)
        .bind(g.p1)
        .bind(&board)
        .bind(&hash)
        .bind(Utc::now() + bot_idle_timeout())
        .bind(g.id)
        .execute(&mut *tx)
        .await?;
    } else if g.rematch_requested_by.is_some() && g.rematch_requested_by != Some(user) {
        // Both want it: new game with swapped colours/first move.
        let p2 =
            g.p2.ok_or_else(|| FpError::Internal("finished human game without player two".into()))?;
        let (first, second) = (p2, g.p1);
        sqlx::query(
            "INSERT INTO free_play_games (id, opponent_kind, status, player_one_id, player_two_id, board_state, state_hash, \
             current_slot, joined_at, started_at, turn_deadline, version, rematch_of) \
             VALUES ($1, 'human', 'active', $2, $3, $4::text::jsonb, $5, 1, NOW(), NOW(), $6, 1, $7)",
        )
        .bind(new_id)
        .bind(first)
        .bind(second)
        .bind(&board)
        .bind(&hash)
        .bind(Utc::now() + turn_timeout())
        .bind(g.id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query("UPDATE free_play_games SET rematch_requested_by = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(user)
            .execute(&mut *tx)
            .await?;
        let g2 = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
        let snapshot = snapshot_of(&mut tx, &g2).await?;
        tx.commit().await?;
        events.push(ev("free_play_game_state", &snapshot));
        return Ok(Outcome {
            snapshot,
            events,
            replay: false,
        });
    }
    sqlx::query("UPDATE free_play_games SET rematch_game_id = $2, rematch_requested_by = COALESCE(rematch_requested_by, $3), updated_at = NOW() WHERE id = $1")
        .bind(id)
        .bind(new_id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let g2 = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
    let snapshot = snapshot_of(&mut tx, &g2).await?;
    tx.commit().await?;
    events.push(ev("free_play_game_state", &snapshot));
    Ok(Outcome {
        snapshot,
        events,
        replay: false,
    })
}

// ── History & stats ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: Uuid,
    pub opponent_kind: String,
    pub bot_difficulty: Option<String>,
    pub opponent_name: String,
    pub you_slot: u8,
    /// win | loss | draw | abandoned
    pub outcome: String,
    pub end_reason: Option<String>,
    pub move_count: i32,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub async fn history(pool: &PgPool, user: Uuid, limit: i64) -> Result<Vec<HistoryItem>, FpError> {
    let rows = sqlx::query(
        "SELECT g.id, g.opponent_kind, g.bot_difficulty, g.player_one_id, g.result, g.winner_user_id, g.end_reason, \
                g.move_count, g.created_at, g.finished_at, \
                COALESCE(o.username, o.display_name) AS opp_name \
         FROM free_play_games g \
         LEFT JOIN users o ON o.id = CASE WHEN g.player_one_id = $1 THEN g.player_two_id ELSE g.player_one_id END \
         WHERE g.status = 'finished' AND (g.player_one_id = $1 OR g.player_two_id = $1) \
         ORDER BY g.finished_at DESC LIMIT $2",
    )
    .bind(user)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let p1: Uuid = r.get("player_one_id");
            let result: Option<String> = r.get("result");
            let winner: Option<Uuid> = r.get("winner_user_id");
            let outcome = match result.as_deref() {
                Some("win") if winner == Some(user) => "win",
                Some("win") => "loss",
                Some("draw") => "draw",
                _ => "abandoned",
            };
            let kind: String = r.get("opponent_kind");
            HistoryItem {
                id: r.get("id"),
                opponent_name: if kind == "bot" {
                    "Bot".into()
                } else {
                    r.get::<Option<String>, _>("opp_name").unwrap_or_default()
                },
                opponent_kind: kind,
                bot_difficulty: r.get("bot_difficulty"),
                you_slot: if p1 == user { 1 } else { 2 },
                outcome: outcome.into(),
                end_reason: r.get("end_reason"),
                move_count: r.get("move_count"),
                created_at: r.get("created_at"),
                finished_at: r.get("finished_at"),
            }
        })
        .collect())
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub played: i64,
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub abandoned: i64,
    pub vs_humans: i64,
    pub vs_bots: i64,
}

/// Computed on the server from finished games only – clients cannot report results.
pub async fn stats(pool: &PgPool, user: Uuid) -> Result<Stats, FpError> {
    let r = sqlx::query(
        "SELECT COUNT(*) AS played, \
                COUNT(*) FILTER (WHERE result = 'win' AND winner_user_id = $1) AS wins, \
                COUNT(*) FILTER (WHERE result = 'win' AND winner_user_id IS DISTINCT FROM $1) AS losses, \
                COUNT(*) FILTER (WHERE result = 'draw') AS draws, \
                COUNT(*) FILTER (WHERE result = 'abandoned') AS abandoned, \
                COUNT(*) FILTER (WHERE opponent_kind = 'human') AS vs_humans, \
                COUNT(*) FILTER (WHERE opponent_kind = 'bot') AS vs_bots \
         FROM free_play_games WHERE status = 'finished' AND (player_one_id = $1 OR player_two_id = $1)",
    )
    .bind(user)
    .fetch_one(pool)
    .await?;
    Ok(Stats {
        played: r.get("played"),
        wins: r.get("wins"),
        losses: r.get("losses"),
        draws: r.get("draws"),
        abandoned: r.get("abandoned"),
        vs_humans: r.get("vs_humans"),
        vs_bots: r.get("vs_bots"),
    })
}

// ── Housekeeping (background worker) ────────────────────────────────────────

/// Applies the documented timeout rules; returns events to publish. Safe to call repeatedly.
pub async fn expire_due(pool: &PgPool) -> Result<Vec<serde_json::Value>, FpError> {
    let mut events = vec![];

    // Open lobbies nobody joined.
    let stale: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE free_play_games SET status = 'cancelled', version = version + 1, updated_at = NOW() \
         WHERE status = 'open' AND created_at < $1 RETURNING id",
    )
    .bind(Utc::now() - lobby_ttl())
    .fetch_all(pool)
    .await?;
    events.extend(stale.into_iter().map(|id| lobby_ev("closed", id)));

    // Games whose player-to-move ran out of time.
    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM free_play_games WHERE status = 'active' AND turn_deadline IS NOT NULL AND turn_deadline < NOW() LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    for id in due {
        let mut tx = pool.begin().await?;
        let Some(g) = lock(&mut tx, id).await? else {
            continue;
        };
        let overdue =
            g.status == "active" && g.turn_deadline.map(|d| d < Utc::now()).unwrap_or(false);
        if !overdue {
            continue;
        }
        if is_bot(&g) {
            if g.current_slot == Some(2) {
                // A pending bot move (e.g. after a restart) – play it instead of abandoning.
                tx.rollback().await?;
                if let Some(out) = bot_turn_if_needed(pool, id).await? {
                    events.extend(out.events);
                }
                continue;
            }
            // Human walked away from a bot game: no winner, not counted as win/loss.
            finish_tx(&mut tx, id, "abandoned", "timeout", None, None, true).await?;
        } else {
            let loser = g.current_slot.unwrap_or(1);
            forfeit_tx(&mut tx, &g, if loser == 1 { 2 } else { 1 }, "timeout").await?;
        }
        let g2 = fetch(&mut tx, id).await?.ok_or(FpError::NotFound)?;
        let snapshot = snapshot_of(&mut tx, &g2).await?;
        tx.commit().await?;
        events.push(ev("free_play_finished", &snapshot));
    }
    Ok(events)
}

/// Spawns the housekeeping loop (every 15 s).
pub fn spawn_worker(pool: PgPool, tx: tokio::sync::broadcast::Sender<String>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            match expire_due(&pool).await {
                Ok(events) => publish(&tx, &events),
                Err(e) => tracing::warn!(error = ?e, "free-play housekeeping failed"),
            }
        }
    });
}
