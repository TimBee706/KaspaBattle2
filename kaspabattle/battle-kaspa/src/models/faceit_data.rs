use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FaceitMatchDetails {
    pub match_id: String,
    pub status: String,
    pub game: String,
    pub teams: FaceitTeams,
    pub results: Option<FaceitResults>,
}

#[derive(Debug, Deserialize)]
pub struct FaceitTeams {
    pub faction1: FaceitFaction,
    pub faction2: FaceitFaction,
}

#[derive(Debug, Deserialize)]
pub struct FaceitFaction {
    pub faction_id: String,
    pub name: String,
    pub roster: Vec<FaceitPlayer>,
}

#[derive(Debug, Deserialize)]
pub struct FaceitPlayer {
    pub player_id: String,
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
pub struct FaceitResults {
    pub winner: String,
    pub score: FaceitScore,
}

#[derive(Debug, Deserialize)]
pub struct FaceitScore {
    pub faction1: i32,
    pub faction2: i32,
}
