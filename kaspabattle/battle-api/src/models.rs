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
    InGame,          // Actively in game on platform
    Resolving,       // Oracle querying result
    Resolved,        // Winner determined
    PaidOut,         // Payout executed
    Disputed,        // Dispute filed
    Cancelled,
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

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Match {
    pub id: Uuid,
    pub onchain_match_id: Option<String>,
    pub escrow_address: Option<String>,
    pub creator_user_id: Uuid,
    pub opponent_user_id: Option<Uuid>,
    pub game_id: String,
    pub stake_kas: i64,
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
}

impl Match {
    pub fn calculate_wager(&mut self) {
        // The DB field stake_kas actually stores the value in Sompi
        // because the frontend converts it before sending.
        self.wager_amount_sompi = self.stake_kas;
    }

    /// Returns true if this match has both players' deposits confirmed
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
            MatchStatus::Funded
            | MatchStatus::Locked
            | MatchStatus::InGame
            | MatchStatus::Resolving => MatchState::Locked,
            MatchStatus::Resolved | MatchStatus::PaidOut => MatchState::Resolved {
                // Approximate winner_id (pure transition validation only requires the variant for most checks)
                winner_id: String::new(),
            },
            MatchStatus::Disputed => MatchState::Disputed {
                reason: String::new(),
                disputed_by: String::new(),
            },
            MatchStatus::Cancelled => MatchState::Cancelled {
                reason: String::new(),
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
