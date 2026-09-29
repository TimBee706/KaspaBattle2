use battle_core::match_state::{transition, MatchAction, MatchState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Type, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[sqlx(type_name = "match_status", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchStatus {
    Draft,           // Locally prepared, not yet published
    Open,            // WAITING_FOR_PLAYER
    AwaitingFunding, // PENDING_DEPOSITS
    Funded,          // READY_TO_LOCK (both deposits confirmed)
    Locked,          // IN_GAME (escrow locked, FACEIT running)
    GameIdInput,     // F-010: Waiting for both players to enter FaceIT match ID
    InGame,          // Actively in game on platform, FaceIT watcher polling
    FinishedFaceit,  // F-010: FaceIT match finished, winner identified
    ReadyForPayout,  // F-010: PSKT created, waiting for winner signature
    Resolving,       // Oracle querying result (legacy)
    Resolved,        // Winner determined
    PaidOut,         // Payout executed
    Disputed,        // Dispute filed
    Cancelled,
    Refunded,        // Refund executed — deposits returned to players
    ReadyToPlay,     // NATIVE: both deposits confirmed, session created, waiting for start
    FinishedGame,    // NATIVE: server engine decided the game, winner known (payout worker input)
    RefundPending,   // NATIVE: draw — both stakes are to be refunded (refund worker input)
}

/// Who runs the game (snapshotted on the match when it is created).
#[derive(Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchProvider {
    /// External competitive game verified through FACEIT (legacy behaviour).
    #[default]
    Faceit,
    /// Browser game played directly on KaspaBattle; result decided by the server engine.
    Native,
}

#[derive(Type, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[sqlx(type_name = "match_mode", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchMode {
    Bo1,
    Bo3,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: Uuid,
    pub kaspa_address: String,
    pub faceit_id: Option<String>,
    pub nickname: Option<String>,
}

/// Canonical column list for `SELECT` queries on the `matches` table.
///
/// - `COALESCE(escrow_address, '')` ensures non-null strings in Rust
/// - `COALESCE(wager_amount_sompi, wager_sompi)` handles legacy rows
///
/// Single source of truth — used by `load_match_full()`, all lobby/history
/// queries, and INSERT/UPDATE RETURNING clauses.
pub const MATCH_COLUMNS: &str = concat!(
    "id, onchain_match_id, COALESCE(escrow_address, '') AS escrow_address, ",
    "creator_user_id, opponent_user_id, game_id, wager_sompi, mode, status, ",
    "external_match_id, created_at, ",
    "COALESCE(wager_amount_sompi, wager_sompi) AS wager_amount_sompi, ",
    "player_a_deposit_tx_hash, player_b_deposit_tx_hash, ",
    "player_a_deposit_confirmed, player_b_deposit_confirmed, ",
    "player_a_faceid_hash, player_b_faceid_hash, ",
    "player_a_deposit_amount_sompi, player_b_deposit_amount_sompi, ",
    "faceit_match_id_player_a, faceit_match_id_player_b, ",
    "faceit_match_id_final, faceit_match_status, ",
    "faceit_finished_at, faceit_winner_faction, faceit_score, ",
    "winner_user_id, loser_user_id, ",
    "payout_pskt_hex, payout_tx_hash, payout_status, ",
    "refund_tx_hash, refund_status, cancelled_at, ",
    "provider, requires_faceit, native_game_type, result_source, game_finished_at, result_hash"
);

/// Backwards-compat alias (use `MATCH_COLUMNS` in new code).
pub const MATCH_SELECT_COLS: &str = MATCH_COLUMNS;

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Match {
    pub id: Uuid,
    pub onchain_match_id: Option<String>,
    pub escrow_address: Option<String>,
    pub creator_user_id: Uuid,
    pub opponent_user_id: Option<Uuid>,
    pub game_id: String,
    pub wager_sompi: i64,
    pub mode: MatchMode,
    pub status: MatchStatus,
    pub external_match_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub wager_amount_sompi: i64,

    // ── v0.2 Deposit Tracking ──
    #[sqlx(default)]
    pub player_a_deposit_tx_hash: Option<String>,
    #[sqlx(default)]
    pub player_b_deposit_tx_hash: Option<String>,
    #[sqlx(default)]
    pub player_a_deposit_confirmed: Option<bool>,
    #[sqlx(default)]
    pub player_b_deposit_confirmed: Option<bool>,

    // ── v0.2 FaceID (optional, off-chain hash) ──
    #[sqlx(default)]
    pub player_a_faceid_hash: Option<String>,
    #[sqlx(default)]
    pub player_b_faceid_hash: Option<String>,

    // ── v0.4 Per-player deposit amount tracking ──
    #[sqlx(default)]
    pub player_a_deposit_amount_sompi: Option<i64>,
    #[sqlx(default)]
    pub player_b_deposit_amount_sompi: Option<i64>,

    // ── v1.0 FaceIT Match-ID Input (F-010) ──
    #[sqlx(default)]
    pub faceit_match_id_player_a: Option<String>,
    #[sqlx(default)]
    pub faceit_match_id_player_b: Option<String>,
    /// Confirmed match ID — only set when both players submitted the same value
    #[sqlx(default)]
    pub faceit_match_id_final: Option<String>,
    /// Last known FaceIT match status (e.g. "created", "ongoing", "finished", "cancelled")
    #[sqlx(default)]
    pub faceit_match_status: Option<String>,
    /// Unix timestamp (from FaceIT) when the match ended
    #[sqlx(default)]
    pub faceit_finished_at: Option<DateTime<Utc>>,
    /// Winning faction: "faction1" or "faction2"
    #[sqlx(default)]
    pub faceit_winner_faction: Option<String>,
    /// Score string, e.g. "16:10"
    #[sqlx(default)]
    pub faceit_score: Option<String>,

    // ── v1.0 Winner/Loser tracking (F-010) ──
    #[sqlx(default)]
    pub winner_user_id: Option<Uuid>,
    #[sqlx(default)]
    pub loser_user_id: Option<Uuid>,

    // ── v1.0 Payout PSKT (F-010) ──
    /// Hex-encoded Partially Signed Kaspa Transaction (with Oracle signature)
    #[sqlx(default)]
    pub payout_pskt_hex: Option<String>,
    /// Payout TX hash after broadcast
    #[sqlx(default)]
    pub payout_tx_hash: Option<String>,
    /// Status of the payout process: "pending_winner_sig", "broadcast", "confirmed"
    #[sqlx(default)]
    pub payout_status: Option<String>,

    // ── v1.2 Refund Tracking ──
    /// On-chain TX ID of the refund transaction
    #[sqlx(default)]
    pub refund_tx_hash: Option<String>,
    /// Refund process status: "none", "pending", "pending_manual", "success", "failed"
    #[sqlx(default)]
    pub refund_status: Option<String>,
    /// Timestamp when the match was cancelled
    #[sqlx(default)]
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,

    // ── v2.0 Provider snapshot + native settlement metadata ──
    /// Snapshot of the game provider at creation time (never changes afterwards).
    #[sqlx(default)]
    pub provider: MatchProvider,
    #[sqlx(default)]
    pub requires_faceit: Option<bool>,
    /// e.g. "CONNECT_FOUR" for provider NATIVE
    #[sqlx(default)]
    pub native_game_type: Option<String>,
    /// "NATIVE_ENGINE" for natively decided matches
    #[sqlx(default)]
    pub result_source: Option<String>,
    #[sqlx(default)]
    pub game_finished_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub result_hash: Option<String>,

    // ── v1.1 FACEIT Profile Enrichment (not stored in matches table) ──
    // These fields are populated in-memory after the DB load by joining faceit_links.
    // They are NOT included in MATCH_COLUMNS or any sqlx query.
    /// FACEIT nickname of the creator (Player A)
    #[sqlx(skip)]
    pub player_a_faceit_nickname: Option<String>,
    /// FACEIT nickname of the opponent (Player B)
    #[sqlx(skip)]
    pub player_b_faceit_nickname: Option<String>,
    /// Avatar URL of Player A (from faceit_links.faceit_avatar_url)
    #[sqlx(skip)]
    pub player_a_avatar_url: Option<String>,
    /// Avatar URL of Player B (from faceit_links.faceit_avatar_url)
    #[sqlx(skip)]
    pub player_b_avatar_url: Option<String>,
    /// FACEIT profile URL of Player A (https://www.faceit.com/en/players/{nickname})
    #[sqlx(skip)]
    pub player_a_faceit_profile_url: Option<String>,
    /// FACEIT profile URL of Player B
    #[sqlx(skip)]
    pub player_b_faceit_profile_url: Option<String>,
    /// FACEIT ID (player_id) of Player A
    #[sqlx(skip)]
    pub player_a_faceit_id: Option<String>,
    /// FACEIT ID (player_id) of Player B
    #[sqlx(skip)]
    pub player_b_faceit_id: Option<String>,
    /// Wallet-account display names (both providers; native players have no FACEIT profile).
    #[sqlx(skip)]
    pub player_a_display_name: Option<String>,
    #[sqlx(skip)]
    pub player_b_display_name: Option<String>,
}

impl Match {
    pub fn calculate_wager(&mut self) {
        // Obsolete legacy mapping, keep no-op or align:
        self.wager_amount_sompi = self.wager_sompi;
    }

    /// Returns true if this match has both players' deposits confirmed
    #[allow(dead_code)]
    pub fn both_deposits_confirmed(&self) -> bool {
        self.player_a_deposit_confirmed.unwrap_or(false)
            && self.player_b_deposit_confirmed.unwrap_or(false)
    }

    /// Converts the DB representation into the pure domain MatchState
    pub fn to_state_machine(&self) -> MatchState {
        match self.status {
            MatchStatus::Draft | MatchStatus::Open => MatchState::WaitingForOpponent,
            MatchStatus::AwaitingFunding => MatchState::WaitingForDeposits {
                player_a_deposited: self.player_a_deposit_confirmed.unwrap_or(false),
                player_b_deposited: self.player_b_deposit_confirmed.unwrap_or(false),
            },
            MatchStatus::Funded | MatchStatus::Locked => MatchState::Locked,
            // NATIVE lifecycle: no external game id is involved. Cancelling is rejected from
            // READY_TO_PLAY on (funds are locked in the game) — same rule as `InGame`.
            MatchStatus::ReadyToPlay => MatchState::ReadyToPlay,
            MatchStatus::GameIdInput => MatchState::GameIdInput {
                faceit_id_a: self.faceit_match_id_player_a.clone(),
                faceit_id_b: self.faceit_match_id_player_b.clone(),
            },
            MatchStatus::InGame if self.provider == MatchProvider::Native => MatchState::NativeInGame,
            MatchStatus::InGame => MatchState::InGame {
                faceit_match_id: self
                    .faceit_match_id_final
                    .clone()
                    .unwrap_or_default(),
            },
            MatchStatus::FinishedFaceit => MatchState::FinishedFaceit {
                winner_id: self
                    .winner_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                loser_id: self
                    .loser_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                score: self.faceit_score.clone().unwrap_or_default(),
            },
            MatchStatus::FinishedGame => MatchState::FinishedGame {
                winner_id: self
                    .winner_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                loser_id: self
                    .loser_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
            },
            MatchStatus::RefundPending => MatchState::RefundPending,
            MatchStatus::ReadyForPayout => MatchState::ReadyForPayout {
                winner_id: self
                    .winner_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                pskt_hex: self.payout_pskt_hex.clone().unwrap_or_default(),
            },
            MatchStatus::Resolving => MatchState::Locked,
            MatchStatus::Resolved | MatchStatus::PaidOut => MatchState::Resolved {
                winner_id: self
                    .winner_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
            },
            MatchStatus::Disputed => MatchState::Disputed {
                reason: String::new(),
                disputed_by: String::new(),
            },
            MatchStatus::Cancelled => MatchState::Cancelled {
                reason: String::new(),
            },
            MatchStatus::Refunded => MatchState::Refunded {
                refund_tx_hash: self.refund_tx_hash.clone(),
            },
        }
    }

    /// Validates an action against the pure state machine logic (M-08)
    pub fn validate_action(&self, action: &MatchAction) -> Result<MatchState, String> {
        let current_state = self.to_state_machine();
        let opponent_id = self.opponent_user_id.map(|id| id.to_string());

        transition(
            &current_state,
            action,
            &self.creator_user_id.to_string(),
            opponent_id.as_deref(),
        )
        .map_err(|e| format!("{:?}", e))
    }
}
