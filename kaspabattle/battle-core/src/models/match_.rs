use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    Open,
    Funded,
    Locked,
    Resolved,
    PaidOut,
    Disputed,
    Cancelled,
    Refunded,
}

impl MatchStatus {
    /// F-018: Returns true ONLY when a payout to the winner is permitted.
    /// Only `Resolved` matches have a verified winner and can be paid out.
    pub fn allows_payout(&self) -> bool {
        matches!(self, MatchStatus::Resolved)
    }

    /// F-009: Returns true when a refund operation is permitted.
    /// Refunds are allowed in pre-resolution states (deposits not yet locked
    /// into a game) and when disputes are active.
    pub fn allows_refund(&self) -> bool {
        matches!(
            self,
            MatchStatus::Open | MatchStatus::Funded | MatchStatus::Locked | MatchStatus::Disputed | MatchStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMatch {
    pub id: Uuid,
    pub player_a_kas_address: String,
    pub player_b_kas_address: String,
    pub player_a_faceit_id: String,
    pub player_b_faceit_id: String,
    pub faceit_match_id: Option<String>,
    pub wager_amount_sompi: u64, // in Sompi (1 KAS = 100_000_000 Sompi = 10^8)
    pub escrow_address: String,
    pub status: MatchStatus,
    pub winner_kas_address: Option<String>,
    pub payout_tx_hash: Option<String>,
    pub oracle_result_signature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub timeout_at: DateTime<Utc>, // created_at + 90 Minuten
}
