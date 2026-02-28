use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Type, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[sqlx(type_name = "match_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchStatus {
    Open,
    AwaitingFunding,
    Locked,
    InGame,
    Resolved,
    Cancelled,
}

#[derive(Type, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[sqlx(type_name = "match_mode", rename_all = "SCREAMING_SNAKE_CASE")]
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
    pub creator_user_id: Uuid,
    pub opponent_user_id: Option<Uuid>,
    pub game_id: String,
    pub stake_kas: i64,
    pub mode: MatchMode,
    pub status: MatchStatus,
    pub external_match_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
