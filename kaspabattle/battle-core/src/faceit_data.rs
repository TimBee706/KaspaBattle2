//! # FaceitDataService
//!
//! High-level service für FACEIT Data API Calls. Delegiert seit F-07 alle
//! HTTP-Calls an [`FaceitClient`] (zentraler Client mit Circuit-Breaker).
//!
//! Die öffentliche API bleibt `anyhow::Result`-basiert für Backward-Kompatibilität
//! mit Handlern und Tests die bereits `?` auf `anyhow::Error` nutzen.

use crate::faceit_client::{FaceitApiError, FaceitClient};
use crate::models::faceit_data::{
    FaceitMatchDetails, FaceitMatchHistory, FaceitPlayerProfile, FaceitPlayerStats,
};
use anyhow::{anyhow, Result};
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Service
// ─────────────────────────────────────────────────────────────────────────────

/// Adapter-Service der `FaceitClient` nutzt und `anyhow::Result` zurückgibt.
/// Kann gecloned werden (Arc-backed internen Client).
#[derive(Clone)]
pub struct FaceitDataService {
    client: Arc<FaceitClient>,
}

impl FaceitDataService {
    /// Erstellt einen neuen Service mit der Production FACEIT API URL.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Arc::new(FaceitClient::new(api_key)),
        }
    }

    /// Erstellt einen neuen Service mit angepasster Base-URL (z.B. für Test-Mock-Server).
    pub fn new_with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: Arc::new(FaceitClient::new_with_base_url(api_key, base_url)),
        }
    }

    /// Erstellt einen Service aus einem bereits vorhandenen `FaceitClient`.
    pub fn from_client(client: Arc<FaceitClient>) -> Self {
        Self { client }
    }

    /// Gibt eine Referenz auf den zugrundeliegenden `FaceitClient` zurück.
    /// Nützlich wenn Code direkt den `FaceitApiError` Typ benötigt (z.B. Oracle).
    pub fn client(&self) -> &Arc<FaceitClient> {
        &self.client
    }

    /// Legacy: gibt den API-Key zurück (benötigt von FaceitOracleService::with_base_url).
    pub fn api_key(&self) -> &str {
        self.client.api_key()
    }

    #[deprecated(note = "Use new_with_base_url() instead")]
    pub fn with_base_url(self, base_url: String) -> Self {
        Self {
            client: Arc::new(FaceitClient::new_with_base_url(
                self.client.api_key().to_owned(),
                base_url,
            )),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Public API (anyhow::Result für Backward-Kompatibilität)
    // ─────────────────────────────────────────────────────────────────────────

    /// `GET /players/{player_id}` — Spielerprofil (ELO, Level, Avatar, Games).
    pub async fn get_player_by_id(&self, player_id: &str) -> Result<FaceitPlayerProfile> {
        self.client
            .get_player_by_id(player_id)
            .await
            .map_err(|e| map_api_error(e, "get_player_by_id", player_id))
    }

    /// `GET /players/{player_id}/history` — Match-Historie mit Pagination.
    pub async fn get_player_history(
        &self,
        player_id: &str,
        game_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<FaceitMatchHistory> {
        self.client
            .get_player_history(player_id, game_id, offset, limit)
            .await
            .map_err(|e| map_api_error(e, "get_player_history", player_id))
    }

    /// `GET /players/{player_id}/stats/{game_id}` — Lifetime Stats (KD, Win-Rate, etc.)
    pub async fn get_player_stats(
        &self,
        player_id: &str,
        game_id: &str,
    ) -> Result<FaceitPlayerStats> {
        self.client
            .get_player_stats(player_id, game_id)
            .await
            .map_err(|e| map_api_error(e, "get_player_stats", player_id))
    }

    /// `GET /matches/{match_id}` — Match-Details inkl. Teams und Results.
    pub async fn get_match_details(&self, match_id: &str) -> Result<FaceitMatchDetails> {
        self.client
            .get_match_details(match_id)
            .await
            .map_err(|e| map_api_error(e, "get_match_details", match_id))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Konvertiert `FaceitApiError` nach `anyhow::Error` mit lesbaren Fehlermeldungen.
fn map_api_error(e: FaceitApiError, method: &str, id: &str) -> anyhow::Error {
    match e {
        FaceitApiError::HttpError { status: 401, .. } => {
            anyhow!(
                "FACEIT API: Unauthorized — invalid or expired Data API key (HTTP 401) [{method}]"
            )
        }
        FaceitApiError::HttpError { status: 403, .. } => {
            anyhow!("FACEIT API: Forbidden — insufficient permissions (HTTP 403) [{method}]")
        }
        FaceitApiError::HttpError { status: 404, .. } => {
            anyhow!("FACEIT API: Resource not found for '{id}' (HTTP 404) [{method}]")
        }
        FaceitApiError::HttpError { status: 429, .. } => {
            anyhow!("FACEIT API: Rate limit exceeded — please retry later (HTTP 429) [{method}]")
        }
        FaceitApiError::HttpError { status: 503, .. } => {
            anyhow!("FACEIT API: Service temporarily unavailable (HTTP 503) [{method}]")
        }
        FaceitApiError::CircuitOpen { remaining_secs } => {
            anyhow!(
                "FACEIT API: Circuit breaker OPEN — service blocked for {}s more [{method}]",
                remaining_secs
            )
        }
        other => anyhow!("FACEIT API error [{method}] for '{id}': {other}"),
    }
}
