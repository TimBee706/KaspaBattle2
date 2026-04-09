use crate::models::faceit_data::FaceitMatchDetails;
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

/// REST-Client für die FACEIT Data API (Server-to-Server)
pub struct FaceitApiClient {
    http_client: Client,
    api_key: String,
    base_url: String,
}

impl FaceitApiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_key,
            base_url: "https://open.faceit.com/data/v4".to_string(),
        }
    }

    /// Überschreibt die Base_URL (nützlich für Wiremock-Tests)
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Ruft Match-Details von FACEIT ab
    pub async fn get_match_details(&self, match_id: &str) -> Result<FaceitMatchDetails> {
        let url = format!("{}/matches/{}", self.base_url, match_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| anyhow!("Netzwerkfehler beim FACEIT Data Request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(anyhow!(
                "FACEIT API Fehler {}: Match nicht gefunden oder limit erreicht",
                status
            ));
        }

        let json = response
            .json::<FaceitMatchDetails>()
            .await
            .map_err(|e| anyhow!("Fehler beim Parsen der FACEIT Match-Daten: {}", e))?;

        Ok(json)
    }

    /// Überprüft, ob beide Spieler (anhand ihrer FACEIT-Guids) im Match waren
    /// und ob das Match ein 1v1 mit genau einem Spieler pro Faction ist.
    pub fn verify_players_in_match(
        &self,
        match_details: &FaceitMatchDetails,
        player_a_guid: &str,
        player_b_guid: &str,
    ) -> bool {
        let faction1 = &match_details.teams.faction1.roster;
        let faction2 = &match_details.teams.faction2.roster;

        if faction1.len() != 1 || faction2.len() != 1 {
            return false;
        }

        let combined_ids = [
            faction1[0].player_id.as_str(),
            faction2[0].player_id.as_str(),
        ];

        let a_in_match = combined_ids.contains(&player_a_guid);
        let b_in_match = combined_ids.contains(&player_b_guid);
        let exact_players = combined_ids
            .iter()
            .all(|id| *id == player_a_guid || *id == player_b_guid);

        a_in_match && b_in_match && exact_players
    }

    /// Ermittelt den Gewinner des Matches (gibt die GUID des Gewinners zurück)
    pub fn determine_winner_guid(
        &self,
        match_details: &FaceitMatchDetails,
        player_a_guid: &str,
        player_b_guid: &str,
    ) -> Result<String> {
        if match_details.status != "FINISHED" {
            return Err(anyhow!(
                "Match ist noch nicht abgeschlossen (Status: {})",
                match_details.status
            ));
        }

        let results = match_details
            .results
            .as_ref()
            .ok_or_else(|| anyhow!("Keine Results vorhanden obwohl Match beendet ist"))?;

        let winner_faction = &results.winner; // "faction1" oder "faction2"

        // Finde heraus, in welcher Faction Player A und B waren
        let a_in_f1 = match_details
            .teams
            .faction1
            .roster
            .iter()
            .any(|p| p.player_id == player_a_guid);
        let b_in_f1 = match_details
            .teams
            .faction1
            .roster
            .iter()
            .any(|p| p.player_id == player_b_guid);

        let a_in_f2 = match_details
            .teams
            .faction2
            .roster
            .iter()
            .any(|p| p.player_id == player_a_guid);
        let b_in_f2 = match_details
            .teams
            .faction2
            .roster
            .iter()
            .any(|p| p.player_id == player_b_guid);

        if winner_faction == "faction1" {
            if a_in_f1 {
                Ok(player_a_guid.to_string())
            } else if b_in_f1 {
                Ok(player_b_guid.to_string())
            } else {
                Err(anyhow!(
                    "Weder Player A noch B waren im Gewinner-Team (Faction 1)"
                ))
            }
        } else if winner_faction == "faction2" {
            if a_in_f2 {
                Ok(player_a_guid.to_string())
            } else if b_in_f2 {
                Ok(player_b_guid.to_string())
            } else {
                Err(anyhow!(
                    "Weder Player A noch B waren im Gewinner-Team (Faction 2)"
                ))
            }
        } else {
            Err(anyhow!("Unbekannte Gewinner-Fraktion: {}", winner_faction))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_match_details_success() {
        let server = MockServer::start().await;
        let mock_response = json!({
            "match_id": "test-match-123",
            "status": "FINISHED",
            "game": "cs2",
            "teams": {
                "faction1": {
                    "faction_id": "f1",
                    "name": "Team A",
                    "roster": [{"player_id": "guid-a", "nickname": "Player A"}]
                },
                "faction2": {
                    "faction_id": "f2",
                    "name": "Team B",
                    "roster": [{"player_id": "guid-b", "nickname": "Player B"}]
                }
            },
            "results": {
                "winner": "faction1",
                "score": {"faction1": 13, "faction2": 10}
            }
        });

        Mock::given(method("GET"))
            .and(path("/matches/test-match-123"))
            .and(header("Authorization", "Bearer api-secret-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&server)
            .await;

        let client = FaceitApiClient::new("api-secret-123".to_string()).with_base_url(server.uri());

        let details = client.get_match_details("test-match-123").await.unwrap();

        assert_eq!(details.match_id, "test-match-123");
        assert_eq!(details.status, "FINISHED");
        assert_eq!(details.teams.faction1.roster[0].player_id, "guid-a");
    }

    #[tokio::test]
    async fn test_get_match_details_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = FaceitApiClient::new("sec".to_string()).with_base_url(server.uri());
        let res = client.get_match_details("wrong-id").await;

        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("404"));
    }

    // --- Helper for creating fake data ---
    fn fake_details(
        status: &str,
        winner: Option<&str>,
        p_a: &str,
        p_b: &str,
        game: &str,
    ) -> FaceitMatchDetails {
        let results = winner.map(|w| crate::models::faceit_data::FaceitResults {
            winner: w.to_string(),
            score: crate::models::faceit_data::FaceitScore {
                faction1: 13,
                faction2: 10,
            },
        });

        FaceitMatchDetails {
            match_id: "m1".to_string(),
            status: status.to_string(),
            game: game.to_string(),
            teams: crate::models::faceit_data::FaceitTeams {
                faction1: crate::models::faceit_data::FaceitFaction {
                    faction_id: "f1".to_string(),
                    name: "F1".to_string(),
                    roster: vec![crate::models::faceit_data::FaceitPlayer {
                        player_id: p_a.to_string(),
                        nickname: "A".to_string(),
                    }],
                },
                faction2: crate::models::faceit_data::FaceitFaction {
                    faction_id: "f2".to_string(),
                    name: "F2".to_string(),
                    roster: vec![crate::models::faceit_data::FaceitPlayer {
                        player_id: p_b.to_string(),
                        nickname: "B".to_string(),
                    }],
                },
            },
            results,
        }
    }

    #[test]
    fn test_verify_players_in_match_both_present() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("ONGOING", None, "guid-1", "guid-2", "cs2");
        assert!(client.verify_players_in_match(&details, "guid-1", "guid-2"));
    }

    #[test]
    fn test_verify_players_in_match_one_missing() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("ONGOING", None, "guid-1", "stranger", "cs2");
        assert!(!client.verify_players_in_match(&details, "guid-1", "guid-2"));
    }

    #[test]
    fn test_verify_players_in_match_both_missing() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("ONGOING", None, "stranger1", "stranger2", "cs2");
        assert!(!client.verify_players_in_match(&details, "guid-1", "guid-2"));
    }

    #[test]
    fn test_determine_winner_guid_f1_wins() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction1"), "guid-A", "guid-B", "cs2");
        let winner = client
            .determine_winner_guid(&details, "guid-A", "guid-B")
            .unwrap();
        assert_eq!(winner, "guid-A");
    }

    #[test]
    fn test_determine_winner_guid_f2_wins() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction2"), "guid-A", "guid-B", "cs2");
        let winner = client
            .determine_winner_guid(&details, "guid-A", "guid-B")
            .unwrap();
        assert_eq!(winner, "guid-B");
    }

    #[test]
    fn test_determine_winner_guid_not_finished() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("ONGOING", None, "guid-A", "guid-B", "cs2");
        let res = client.determine_winner_guid(&details, "guid-A", "guid-B");
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("noch nicht abgeschlossen"));
    }

    #[test]
    fn test_determine_winner_guid_stranger_won() {
        let client = FaceitApiClient::new("".to_string());
        // Faction 1 wins, but neither A nor B are in Faction 1
        let mut details = fake_details("FINISHED", Some("faction1"), "guid-A", "guid-B", "cs2");
        details.teams.faction1.roster[0].player_id = "stranger".to_string(); // Replace guid-A

        let res = client.determine_winner_guid(&details, "guid-A", "guid-B");
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Weder Player A noch B"));
    }

    // Test 10-14 (simulated by making variations of the above)
    #[test]
    fn test_determine_winner_guid_invalid_faction() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("invalid_faction"), "guid-A", "guid-B", "cs2");
        let res = client.determine_winner_guid(&details, "guid-A", "guid-B");
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Unbekannte Gewinner-Fraktion"));
    }

    #[test]
    fn test_verify_players_in_match_requires_1v1_rosters() {
        let client = FaceitApiClient::new("".to_string());
        let mut details = fake_details("ONGOING", None, "guid-1", "guid-2", "cs2");
        details.teams.faction1.roster.push(crate::models::faceit_data::FaceitPlayer {
            player_id: "extra".to_string(),
            nickname: "Extra".to_string(),
        });

        assert!(!client.verify_players_in_match(&details, "guid-1", "guid-2"));
    }

    #[test]
    fn test_verify_players_in_match_valorant_1v1() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction2"), "guid-A", "guid-B", "valorant");
        assert!(client.verify_players_in_match(&details, "guid-A", "guid-B"));
    }

    #[test]
    fn test_determine_winner_guid_valorant() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction2"), "guid-A", "guid-B", "valorant");
        let winner = client
            .determine_winner_guid(&details, "guid-A", "guid-B")
            .unwrap();
        assert_eq!(winner, "guid-B");
    }

    #[test]
    fn test_verify_players_in_match_rocket_league_1v1() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction1"), "guid-X", "guid-Y", "rocket_league");
        assert!(client.verify_players_in_match(&details, "guid-X", "guid-Y"));
    }

    #[test]
    fn test_determine_winner_guid_rocket_league() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction1"), "guid-X", "guid-Y", "rocket_league");
        let winner = client
            .determine_winner_guid(&details, "guid-X", "guid-Y")
            .unwrap();
        assert_eq!(winner, "guid-X");
    }

    #[test]
    fn test_verify_players_in_match_dota2_1v1() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction2"), "guid-P", "guid-Q", "dota2");
        assert!(client.verify_players_in_match(&details, "guid-P", "guid-Q"));
    }

    #[test]
    fn test_determine_winner_guid_dota2() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction2"), "guid-P", "guid-Q", "dota2");
        let winner = client
            .determine_winner_guid(&details, "guid-P", "guid-Q")
            .unwrap();
        assert_eq!(winner, "guid-Q");
    }

    #[test]
    fn test_verify_players_in_match_lol_1v1() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction1"), "guid-R", "guid-S", "lol");
        assert!(client.verify_players_in_match(&details, "guid-R", "guid-S"));
    }

    #[test]
    fn test_determine_winner_guid_lol() {
        let client = FaceitApiClient::new("".to_string());
        let details = fake_details("FINISHED", Some("faction1"), "guid-R", "guid-S", "lol");
        let winner = client
            .determine_winner_guid(&details, "guid-R", "guid-S")
            .unwrap();
        assert_eq!(winner, "guid-R");
    }
}
