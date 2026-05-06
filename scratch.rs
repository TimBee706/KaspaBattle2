use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct CreateTournamentReq {
    #[serde(rename = "name")]
    pub title: String,
    pub max_teams: i32,
    pub buy_in_sompi: i64,
    pub prize_winner_pct: i16,
    pub prize_runner_up_pct: i16,
    pub platform_fee_pct: i16,
    #[serde(rename = "game_id")]
    pub game_type: String,
    pub registration_deadline: Option<chrono::DateTime<chrono::Utc>>,
}
fn main() {
    let json = #{
        "name": "test",
        "game_id": "cs2",
        "max_teams": 8,
        "buy_in_sompi": 500000000,
        "prize_winner_pct": 70,
        "prize_runner_up_pct": 20,
        "platform_fee_pct": 10
    }#;
    match serde_json::from_str::<CreateTournamentReq>(json) {
        Ok(v) => println!("OK: {:?}", v),
        Err(e) => println!("ERR: {}", e),
    }
}
