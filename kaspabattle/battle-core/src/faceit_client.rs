//! # FaceitClient — Zentraler HTTP-Client für die FACEIT Data API (F-07 / F-10)
//!
//! Dieser Client kapselt alle HTTP-Calls an `https://open.faceit.com/data/v4`
//! und bietet zusätzlich:
//!
//! - **Einheitliche Fehlerbehandlung** per `FaceitApiError` (ersetzt verteilte `anyhow!`-Calls)
//! - **Circuit-Breaker** (F-10): Nach `CIRCUIT_OPEN_THRESHOLD` aufeinanderfolgenden Fehlern
//!   werden weitere Calls für `CIRCUIT_OPEN_DURATION` blockiert, um FACEIT-Ausfälle
//!   abzufangen und Rate-Limit-Budget zu schonen.
//! - **Request-Metriken**: Zählt Erfolge, Fehler und Circuit-Trips via `tracing`.
//!
//! ## Verwendung
//!
//! ```no_run
//! use battle_core::faceit_client::FaceitClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), battle_core::faceit_client::FaceitApiError> {
//!     let client = FaceitClient::new("your_api_key".to_string());
//!     let profile = client.get_player_by_id("player-uuid-here").await?;
//!     drop(profile);
//!     Ok(())
//! }
//! ```
//!
//! Der `FaceitDataService` delegiert seit F-07 alle HTTP-Calls an diesen Client.

use crate::models::faceit_data::{
    FaceitMatchDetails, FaceitMatchHistory, FaceitPlayerProfile, FaceitPlayerStats,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// Fehlertyp für alle FACEIT Data API Calls.
#[derive(Debug, Error)]
pub enum FaceitApiError {
    #[error("HTTP {status}: {message}")]
    HttpError { status: u16, message: String },

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    /// Der Circuit-Breaker ist offen — FACEIT API ist als nicht erreichbar markiert.
    #[error("Circuit open: FACEIT API temporarily unavailable (resets in {remaining_secs}s)")]
    CircuitOpen { remaining_secs: u64 },
}

impl FaceitApiError {
    /// Returns true if this is a transient error that should be retried.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            FaceitApiError::NetworkError(_)
                | FaceitApiError::HttpError { status: 429, .. }
                | FaceitApiError::HttpError { status: 503, .. }
                | FaceitApiError::HttpError { status: 502, .. }
        )
    }

    /// Returns true if the error should trip / keep the circuit open.
    pub fn should_trip_circuit(&self) -> bool {
        matches!(
            self,
            FaceitApiError::NetworkError(_)
                | FaceitApiError::HttpError { status: 429, .. }
                | FaceitApiError::HttpError { status: 500, .. }
                | FaceitApiError::HttpError { status: 502, .. }
                | FaceitApiError::HttpError { status: 503, .. }
                | FaceitApiError::HttpError { status: 504, .. }
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Circuit-Breaker state (F-10)
// ─────────────────────────────────────────────────────────────────────────────

/// Anzahl aufeinanderfolgender Fehler, nach denen der Circuit geöffnet wird.
const CIRCUIT_OPEN_THRESHOLD: u32 = 5;
/// Dauer in Sekunden, die der Circuit offen bleibt.
const CIRCUIT_OPEN_DURATION_SECS: u64 = 60;

/// Atomarer Circuit-Breaker-Zustand.
/// Wird von Arc geteilt sodass er thread-safe und Clone-fähig ist.
#[derive(Debug)]
struct CircuitBreakerState {
    /// Aufeinanderfolgende Fehler seit dem letzten Erfolg.
    consecutive_errors: AtomicU32,
    /// Unix-Timestamp (Sekunden) ab dem der Circuit geöffnet ist. 0 = geschlossen.
    open_since: AtomicI64,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            consecutive_errors: AtomicU32::new(0),
            open_since: AtomicI64::new(0),
        }
    }
}

impl CircuitBreakerState {
    /// Prüft ob der Circuit offen ist. Schließt ihn automatisch nach der Timeout-Dauer.
    fn is_open(&self) -> Option<u64> {
        let open_since = self.open_since.load(Ordering::SeqCst);
        if open_since == 0 {
            return None;
        }

        let now = chrono::Utc::now().timestamp();
        let elapsed = (now - open_since).max(0) as u64;

        if elapsed >= CIRCUIT_OPEN_DURATION_SECS {
            // Reset: Circuit schließt sich wieder
            self.open_since.store(0, Ordering::SeqCst);
            self.consecutive_errors.store(0, Ordering::SeqCst);
            tracing::info!(
                "⚡ FACEIT Circuit Breaker: CLOSED (timeout elapsed, resuming requests)"
            );
            None
        } else {
            Some(CIRCUIT_OPEN_DURATION_SECS - elapsed)
        }
    }

    /// Registriert einen Erfolg → setzt Fehlerzähler zurück.
    fn record_success(&self) {
        let prev = self.consecutive_errors.swap(0, Ordering::SeqCst);
        if prev > 0 {
            tracing::info!(
                "⚡ FACEIT Circuit Breaker: success after {} consecutive errors, counter reset",
                prev
            );
        }
    }

    /// Registriert einen Fehler → öffnet den Circuit wenn Schwellenwert erreicht.
    fn record_failure(&self) {
        let count = self.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= CIRCUIT_OPEN_THRESHOLD && self.open_since.load(Ordering::SeqCst) == 0 {
            let now = chrono::Utc::now().timestamp();
            self.open_since.store(now, Ordering::SeqCst);
            tracing::error!(
                "⚡ FACEIT Circuit Breaker: OPEN after {} consecutive errors — blocking for {}s",
                count,
                CIRCUIT_OPEN_DURATION_SECS
            );
        } else if count < CIRCUIT_OPEN_THRESHOLD {
            tracing::warn!(
                "⚡ FACEIT Circuit Breaker: {} consecutive errors ({} until OPEN)",
                count,
                CIRCUIT_OPEN_THRESHOLD - count
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FaceitClient
// ─────────────────────────────────────────────────────────────────────────────

/// Zentraler FACEIT Data API v4 HTTP-Client mit Circuit-Breaker.
#[derive(Clone)]
pub struct FaceitClient {
    http: Client,
    api_key: String,
    base_url: String,
    circuit: Arc<CircuitBreakerState>,
}

impl FaceitClient {
    /// Erstellt einen neuen Client mit Production Base-URL.
    pub fn new(api_key: String) -> Self {
        Self::new_with_base_url(api_key, "https://open.faceit.com/data/v4".to_string())
    }

    /// Erstellt einen neuen Client mit angepasster Base-URL (z.B. für Tests mit Mock-Server).
    pub fn new_with_base_url(api_key: String, base_url: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("KaspaBattle/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to build FaceitClient HTTP client");

        Self {
            http,
            api_key,
            base_url,
            circuit: Arc::new(CircuitBreakerState::default()),
        }
    }

    /// Returns the configured API key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// Prüft den Circuit-Breaker-Status. Gibt Err(CircuitOpen) zurück wenn offen.
    fn check_circuit(&self) -> Result<(), FaceitApiError> {
        if let Some(remaining) = self.circuit.is_open() {
            return Err(FaceitApiError::CircuitOpen {
                remaining_secs: remaining,
            });
        }
        Ok(())
    }

    /// Führt einen GET-Request aus und verarbeitet Fehler inkl. Circuit-Breaker-Aktualisierung.
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, FaceitApiError> {
        self.check_circuit()?;

        let response = self
            .http
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| {
                let err = FaceitApiError::NetworkError(e);
                self.circuit.record_failure();
                err
            })?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let err = match status.as_u16() {
                401 => FaceitApiError::HttpError {
                    status: 401,
                    message: "Unauthorized — invalid or expired FACEIT Data API key".to_string(),
                },
                403 => FaceitApiError::HttpError {
                    status: 403,
                    message: "Forbidden — insufficient permissions for this resource".to_string(),
                },
                404 => FaceitApiError::HttpError {
                    status: 404,
                    message: format!("Not found: {}", body),
                },
                429 => FaceitApiError::HttpError {
                    status: 429,
                    message: "Rate limit exceeded — please retry later".to_string(),
                },
                503 => FaceitApiError::HttpError {
                    status: 503,
                    message: "FACEIT service temporarily unavailable".to_string(),
                },
                s => FaceitApiError::HttpError {
                    status: s,
                    message: format!("Unexpected error from FACEIT API: {}", body),
                },
            };

            // Structured log with context
            tracing::warn!(
                faceit_status = status.as_u16(),
                url = %url,
                error = %err,
                "FACEIT API request failed"
            );

            if err.should_trip_circuit() {
                self.circuit.record_failure();
            } else {
                // 4xx client errors (e.g. 404) sind keine transienten Fehler → kein Circuit-Trip
                self.circuit.record_success();
            }

            return Err(err);
        }

        let result = response.json::<T>().await.map_err(|e| {
            FaceitApiError::ParseError(format!("JSON parse error for {}: {}", url, e))
        })?;

        self.circuit.record_success();
        Ok(result)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // FACEIT Data API Methoden
    // ─────────────────────────────────────────────────────────────────────────

    /// `GET /players/{player_id}` — Spielerprofil inkl. ELO und Level.
    pub async fn get_player_by_id(
        &self,
        player_id: &str,
    ) -> Result<FaceitPlayerProfile, FaceitApiError> {
        let url = format!("{}/players/{}", self.base_url, player_id);
        self.get(&url).await
    }

    /// `GET /players/{player_id}/stats/{game_id}` — Lifetime Stats (Matches, KD, Win Rate etc.)
    pub async fn get_player_stats(
        &self,
        player_id: &str,
        game_id: &str,
    ) -> Result<FaceitPlayerStats, FaceitApiError> {
        let url = format!("{}/players/{}/stats/{}", self.base_url, player_id, game_id);
        self.get(&url).await
    }

    /// `GET /players/{player_id}/history` — Match-Historie (mit Pagination).
    pub async fn get_player_history(
        &self,
        player_id: &str,
        game_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<FaceitMatchHistory, FaceitApiError> {
        let url = format!(
            "{}/players/{}/history?game={}&offset={}&limit={}",
            self.base_url, player_id, game_id, offset, limit
        );
        self.get(&url).await
    }

    /// `GET /matches/{match_id}` — Match-Details inkl. Status, Teams und Results.
    pub async fn get_match_details(
        &self,
        match_id: &str,
    ) -> Result<FaceitMatchDetails, FaceitApiError> {
        let url = format!("{}/matches/{}", self.base_url, match_id);
        self.get(&url).await
    }

    /// Returns current circuit breaker stats for monitoring/health checks.
    pub fn circuit_stats(&self) -> CircuitStats {
        CircuitStats {
            consecutive_errors: self.circuit.consecutive_errors.load(Ordering::SeqCst),
            is_open: self.circuit.is_open().is_some(),
        }
    }
}

/// Circuit-Breaker-Status für Monitoring/Health-Endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct CircuitStats {
    pub consecutive_errors: u32,
    pub is_open: bool,
}
