use serde::{Deserialize, Serialize};

pub type MatchId = String;
pub type KaspaAddress = String;

/// 1 KAS = 100_000 Sompi (Kaspa unit analogous to Satoshi)
pub const SOMPI_PER_KAS: u64 = 100_000;
pub const MIN_WAGER_KAS: u64 = 10; // 10 KAS minimum
pub const MAX_WAGER_KAS: u64 = 10_000; // 10.000 KAS maximum
pub const DEPOSIT_TIMEOUT_MINUTES: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub player_id: String,
    pub kaspa_address: KaspaAddress,
    pub display_name: Option<String>,
}

/// Supported game types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameType {
    #[serde(rename = "cs2")]
    CounterStrike2,
    #[serde(rename = "dota2")]
    Dota2,
    #[serde(rename = "valorant")]
    Valorant,
}

impl GameType {
    pub fn as_str(&self) -> &str {
        match self {
            GameType::CounterStrike2 => "cs2",
            GameType::Dota2 => "dota2",
            GameType::Valorant => "valorant",
        }
    }
}

impl std::str::FromStr for GameType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cs2" => Ok(GameType::CounterStrike2),
            "dota2" => Ok(GameType::Dota2),
            "valorant" => Ok(GameType::Valorant),
            _ => Err(()),
        }
    }
}
