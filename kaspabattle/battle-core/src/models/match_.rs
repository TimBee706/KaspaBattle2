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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMatch {
    pub id: Uuid,
    pub player_a_kas_address: String,
    pub player_b_kas_address: String,
    pub player_a_faceit_id: String,
    pub player_b_faceit_id: String,
    pub faceit_match_id: Option<String>,
    pub wager_amount_sompi: u64, // in Sompi (1 KAS = 100_000 Sompi)
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
