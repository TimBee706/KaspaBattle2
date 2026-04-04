use crate::models::faceit_data::{
    FaceitMatchDetails, FaceitMatchHistory, FaceitPlayerProfile, FaceitPlayerStats,
};
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

#[derive(Clone)]
pub struct FaceitDataService {
    client: Client,
    api_key: String,
    base_url: String,
}

impl FaceitDataService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .pool_idle_timeout(Duration::from_secs(90))
                .build()
                .expect("Failed to build FACEIT HTTP client"),
            api_key,
            base_url: "https://open.faceit.com/data/v4".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// Abrufen des Spielerprofils
    pub async fn get_player_by_id(&self, player_id: &str) -> Result<FaceitPlayerProfile> {
        let url = format!("{}/players/{}", self.base_url, player_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| anyhow!("Network error calling FACEIT API: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("FACEIT API Error {}: {}", status, text));
        }

        response
            .json::<FaceitPlayerProfile>()
            .await
            .map_err(|e| anyhow!("Failed to parse FACEIT stats: {}", e))
    }

    /// Abrufen der Match-Historie für ein bestimmtes Game (z.B. "cs2")
    pub async fn get_player_history(
        &self,
        player_id: &str,
        game_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<FaceitMatchHistory> {
        let url = format!("{}/players/{}/history", self.base_url, player_id);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("game", game_id),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| anyhow!("Network error calling FACEIT API: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("FACEIT API Error {}: {}", status, text));
        }

        response
            .json::<FaceitMatchHistory>()
            .await
            .map_err(|e| anyhow!("Failed to parse FACEIT history: {}", e))
    }

    /// Abrufen der Lifetime Stats für einen Spieler
    pub async fn get_player_stats(
        &self,
        player_id: &str,
        game_id: &str,
    ) -> Result<FaceitPlayerStats> {
        let url = format!("{}/players/{}/stats/{}", self.base_url, player_id, game_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| anyhow!("Network error calling FACEIT API: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("FACEIT API Error {}: {}", status, text));
        }

        response
            .json::<FaceitPlayerStats>()
            .await
            .map_err(|e| anyhow!("Failed to parse FACEIT stats: {}", e))
    }

    /// Optional: Match Details abrufen
    pub async fn get_match_details(&self, match_id: &str) -> Result<FaceitMatchDetails> {
        let url = format!("{}/matches/{}", self.base_url, match_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| anyhow!("Network error calling FACEIT API: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("FACEIT API Error {}: {}", status, text));
        }

        response
            .json::<FaceitMatchDetails>()
            .await
            .map_err(|e| anyhow!("Failed to parse FACEIT match details: {}", e))
    }
}
