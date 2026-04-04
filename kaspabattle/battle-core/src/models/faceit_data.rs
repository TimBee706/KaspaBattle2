use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitPlayerProfile {
    pub player_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub country: Option<String>,
    pub games: std::collections::HashMap<String, FaceitGameProfile>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitGameProfile {
    pub skill_level: i32,
    pub faceit_elo: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchHistory {
    pub items: Vec<FaceitMatchHistoryItem>,
    pub start: i32,
    pub end: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchHistoryItem {
    pub match_id: String,
    pub game_id: String,
    pub region: String,
    pub match_type: String,
    pub game_mode: String,
    pub map_i_ds: Vec<String>,
    pub team_id: String,
    pub playing_players: Vec<String>,
    pub started_at: i64,
    pub finished_at: i64,
    pub results: FaceitMatchResults,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchResults {
    pub winner: String,
    pub score: std::collections::HashMap<String, i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitPlayerStats {
    pub player_id: String,
    pub game_id: String,
    pub lifetime: FaceitLifetimeStats,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitLifetimeStats {
    #[serde(rename = "Matches")]
    pub matches: String,
    #[serde(rename = "Win Rate %")]
    pub win_rate: String,
    #[serde(rename = "Recent Results")]
    pub recent_results: Vec<String>,
    #[serde(rename = "Wins")]
    pub wins: String,
    #[serde(rename = "Average K/D Ratio")]
    pub average_kd: String,
    #[serde(rename = "Average Headshots %")]
    pub average_headshots: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchDetails {
    pub match_id: String,
    pub status: String,
    pub game_id: String,
    pub results: Option<FaceitMatchResults>,
    pub teams: Option<FaceitMatchTeams>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchTeams {
    pub faction1: FaceitMatchFaction,
    pub faction2: FaceitMatchFaction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchFaction {
    pub faction_id: String,
    pub name: String,
    pub roster: Vec<FaceitMatchRosterPlayer>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchRosterPlayer {
    pub player_id: String,
    pub nickname: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitStatsSnapshot {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub faceit_player_id: String,
    pub game_id: String,
    pub elo: i32,
    pub skill_level: i32,
    pub snapshot_at: String,
}
