//! FACEIT Data API response models.
//!
//! # Dev-Schema-Validation (F-13)
//!
//! Compile with `--features strict-faceit-schema` (or in CI) to enable
//! `#[serde(deny_unknown_fields)]` on all FACEIT response structs.
//! This causes deserialization to fail if FACEIT unexpectedly changes their
//! response schema, catching drift early in development.
//!
//! In production builds the attribute is omitted for forward-compatibility.

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
#[cfg_attr(feature = "strict-faceit-schema", serde(deny_unknown_fields))]
pub struct FaceitPlayerStats {
    pub player_id: String,
    pub game_id: String,
    pub lifetime: FaceitLifetimeStats,
}

/// Lifetime-Statistiken aus `GET /players/{id}/stats/{game_id}`.
///
/// Felder mit `Option<String>` sind in der FACEIT API vorhanden aber nicht
/// garantiert befüllt (z.B. wenn ein Spieler noch nie gespielt hat).
///
/// **F-12**: Vollständige Erfassung aller von FACEIT bereitgestellten Felder.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FaceitLifetimeStats {
    // ── Kern-Felder (immer vorhanden) ─────────────────────────────────
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

    // ── Erweiterte Felder (F-12) ───────────────────────────────────────
    /// Aktuelle Siegesserie (seit letzter Niederlage).
    #[serde(rename = "Current Win Streak", default)]
    pub current_win_streak: Option<String>,
    /// Längste jemals erreichte Siegesserie.
    #[serde(rename = "Longest Win Streak", default)]
    pub longest_win_streak: Option<String>,
    /// Gesamtanteil an Headshots über alle Partien (präziser als Average).
    #[serde(rename = "Total Headshots %", default)]
    pub total_headshots_percent: Option<String>,
    /// Durchschnittliches Kill/Round-Verhältnis.
    #[serde(rename = "Average K/R Ratio", default)]
    pub average_kr_ratio: Option<String>,
    /// Verlassene Partien (Early Exits / Abandons).
    #[serde(rename = "Leaves", default)]
    pub leaves: Option<String>,
    /// Durchschnittliche Kills pro Partie.
    #[serde(rename = "Average Kills", default)]
    pub average_kills: Option<String>,
    /// Durchschnittliche Deaths pro Partie.
    #[serde(rename = "Average Deaths", default)]
    pub average_deaths: Option<String>,
    /// Durchschnittliche Assists pro Partie.
    #[serde(rename = "Average Assists", default)]
    pub average_assists: Option<String>,
    /// Gespielte Maps als Häufigkeitsliste.
    #[serde(rename = "Maps Played", default)]
    pub maps_played: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaceitMatchDetails {
    pub match_id: String,
    pub status: String,
    pub game_id: String,
    pub results: Option<FaceitMatchResults>,
    pub teams: Option<FaceitMatchTeams>,
    /// Unix timestamp when the match finished (populated by FACEIT API).
    pub finished_at: Option<i64>,
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
