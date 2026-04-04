//! # FaceitOracleService
//!
//! Signs FACEIT match results with an Ed25519 key so the on-chain oracle contract
//! can verify off-chain outcomes without trusting the backend server.
//!
//! ## Architecture (F-08 refactor)
//!
//! The service now delegates all FACEIT HTTP calls to [`FaceitDataService`]
//! (shared HTTP client with timeout, pooling, and differentiated error handling)
//! instead of maintaining a private `reqwest::Client`.
//!
//! This eliminates the duplicate HTTP-client and private type definitions
//! that previously existed alongside the canonical `models/faceit_data.rs` types.

use crate::errors::OracleError;
use crate::faceit_client::{FaceitApiError, FaceitClient};
use crate::models::faceit_data::FaceitMatchTeams;
use base64::{engine::general_purpose::STANDARD as Base64, Engine as _};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OracleResult {
    pub faceit_match_id: String,
    pub winner_faceit_id: String,
    pub loser_faceit_id: String,
    pub match_finished_at: DateTime<Utc>,
    pub score: String,
    pub signature: String,
    pub queried_at: DateTime<Utc>,
}

impl OracleResult {
    /// Creates a deterministic string representation of the result for signing
    fn signing_payload(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.faceit_match_id,
            self.winner_faceit_id,
            self.loser_faceit_id,
            self.match_finished_at.timestamp(),
            self.score
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Service
// ─────────────────────────────────────────────────────────────────────────────

pub struct FaceitOracleService {
    /// Zentraler FACEIT HTTP-Client mit Circuit-Breaker (F-07/F-10).
    client: Arc<FaceitClient>,
    oracle_signing_key: SigningKey,
}

impl FaceitOracleService {
    /// Creates a new FaceitOracleService backed by a shared FaceitClient.
    pub fn new(client: Arc<FaceitClient>, oracle_signing_key: SigningKey) -> Self {
        Self {
            client,
            oracle_signing_key,
        }
    }

    /// Standalone constructor: erstellt einen eigenen FaceitClient.
    /// Bevorzuge `new(client, signing_key)` wenn ein geteilter Client vorliegt.
    pub fn new_standalone(api_key: String, oracle_signing_key: SigningKey) -> Self {
        Self {
            client: Arc::new(FaceitClient::new(api_key)),
            oracle_signing_key,
        }
    }

    /// Override the base URL (mainly for testing with mock servers).
    pub fn with_base_url(self, base_url: String) -> Self {
        Self {
            client: Arc::new(FaceitClient::new_with_base_url(
                self.client.api_key().to_owned(),
                base_url,
            )),
            ..self
        }
    }

    /// Fetches a match result with exponential backoff retries (5s, 15s, 45s).
    pub async fn fetch_match_result(
        &self,
        faceit_match_id: &str,
    ) -> Result<Option<OracleResult>, OracleError> {
        let max_retries = 3;
        let mut retry_delay = 5;

        for attempt in 1..=max_retries {
            match self.do_fetch_match_result(faceit_match_id).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("FACEIT API fetch failed (attempt {}): {}", attempt, e);
                    if attempt == max_retries {
                        return Err(e);
                    }
                    sleep(Duration::from_secs(retry_delay)).await;
                    retry_delay *= 3; // 5s -> 15s -> 45s
                }
            }
        }

        unreachable!()
    }

    async fn do_fetch_match_result(
        &self,
        faceit_match_id: &str,
    ) -> Result<Option<OracleResult>, OracleError> {
        // Direkt FaceitClient nutzen — hat Circuit-Breaker + strukturierte Fehler
        let match_data = self
            .client
            .get_match_details(faceit_match_id)
            .await
            .map_err(|e| match e {
                FaceitApiError::HttpError { status, message } => {
                    OracleError::ApiError { status, message }
                }
                FaceitApiError::NetworkError(re) => OracleError::NetworkError(re),
                FaceitApiError::ParseError(msg) => OracleError::ParseError(msg),
                FaceitApiError::CircuitOpen { remaining_secs } => OracleError::ApiError {
                    status: 503,
                    message: format!("Circuit breaker OPEN for {}s more", remaining_secs),
                },
            })?;

        // FACEIT API returns "finished" (lowercase) for completed matches
        let normalized_status = match_data.status.to_lowercase();
        if normalized_status != "finished" {
            return Ok(None);
        }

        let results = match_data.results.ok_or_else(|| {
            OracleError::ParseError("Match is FINISHED but has no results".to_string())
        })?;

        let winner_faction = results.winner.clone();
        let loser_faction = if winner_faction == "faction1" {
            "faction2".to_string()
        } else {
            "faction1".to_string()
        };

        let teams = match_data.teams.ok_or_else(|| {
            OracleError::ParseError("Match is FINISHED but has no teams data".to_string())
        })?;

        let (winner_team, loser_team) = resolve_factions(&teams, &winner_faction, &loser_faction)?;

        // In 1v1 battles, roster size is 1. Take the first player from each roster.
        let winner_faceit_id = winner_team
            .roster
            .first()
            .map(|p| p.player_id.clone())
            .ok_or_else(|| OracleError::ParseError("Winner team roster is empty".to_string()))?;

        let loser_faceit_id = loser_team
            .roster
            .first()
            .map(|p| p.player_id.clone())
            .ok_or_else(|| OracleError::ParseError("Loser team roster is empty".to_string()))?;

        // Build score string from shared FaceitMatchResults
        let score = format!(
            "{}:{}",
            results.score.get("faction1").copied().unwrap_or(0),
            results.score.get("faction2").copied().unwrap_or(0)
        );

        let finished_at = match_data
            .finished_at
            .unwrap_or_else(|| Utc::now().timestamp());
        let match_finished_at = DateTime::from_timestamp(finished_at, 0).unwrap_or_else(Utc::now);

        let mut result = OracleResult {
            faceit_match_id: faceit_match_id.to_string(),
            winner_faceit_id,
            loser_faceit_id,
            match_finished_at,
            score,
            signature: String::new(),
            queried_at: Utc::now(),
        };

        result.signature = self.sign_result(&result);

        Ok(Some(result))
    }

    /// Fetches the result twice to ensure confirmation matches.
    pub async fn fetch_with_double_confirmation(
        &self,
        faceit_match_id: &str,
    ) -> Result<Option<OracleResult>, OracleError> {
        let first = self.fetch_match_result(faceit_match_id).await?;
        if first.is_none() {
            return Ok(None);
        }

        sleep(Duration::from_millis(500)).await;

        let second = self.fetch_match_result(faceit_match_id).await?;
        if second.is_none() {
            // Unlikely to go from FINISHED -> Not FINISHED
            return Ok(None);
        }

        let first_res = first.unwrap();
        let second_res = second.unwrap();

        if first_res.winner_faceit_id != second_res.winner_faceit_id {
            return Err(OracleError::ConfirmationMismatch {
                first: first_res.winner_faceit_id,
                second: second_res.winner_faceit_id,
            });
        }

        Ok(Some(first_res))
    }

    /// Signs the OracleResult payload with the Ed25519 oracle key.
    fn sign_result(&self, result: &OracleResult) -> String {
        let payload = result.signing_payload();
        let signature = self.oracle_signing_key.sign(payload.as_bytes());
        Base64.encode(signature.to_bytes())
    }

    /// Verifies the signature of an OracleResult.
    pub fn verify_signature(&self, result: &OracleResult, public_key: &VerifyingKey) -> bool {
        let payload = result.signing_payload();
        let signature_bytes = match Base64.decode(&result.signature) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let signature = match Signature::try_from(signature_bytes.as_slice()) {
            Ok(s) => s,
            Err(_) => return false,
        };

        public_key.verify(payload.as_bytes(), &signature).is_ok()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_factions<'a>(
    teams: &'a FaceitMatchTeams,
    winner_faction: &str,
    loser_faction: &str,
) -> Result<
    (
        &'a crate::models::faceit_data::FaceitMatchFaction,
        &'a crate::models::faceit_data::FaceitMatchFaction,
    ),
    OracleError,
> {
    let winner_team = match winner_faction {
        "faction1" => &teams.faction1,
        "faction2" => &teams.faction2,
        other => {
            return Err(OracleError::ParseError(format!(
                "Unknown winner faction: {}",
                other
            )))
        }
    };
    let loser_team = match loser_faction {
        "faction1" => &teams.faction1,
        "faction2" => &teams.faction2,
        other => {
            return Err(OracleError::ParseError(format!(
                "Unknown loser faction: {}",
                other
            )))
        }
    };
    Ok((winner_team, loser_team))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faceit_client::FaceitClient;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn create_test_service() -> (FaceitOracleService, VerifyingKey) {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let client = Arc::new(FaceitClient::new("test_api_key".to_string()));
        let service = FaceitOracleService::new(client, signing_key);
        (service, verifying_key)
    }

    #[test]
    fn test_oracle_result_sign_verify() {
        let (service, verifying_key) = create_test_service();

        let mut result = OracleResult {
            faceit_match_id: "match_123".to_string(),
            winner_faceit_id: "player_a".to_string(),
            loser_faceit_id: "player_b".to_string(),
            match_finished_at: Utc::now(),
            score: "16:7".to_string(),
            signature: String::new(),
            queried_at: Utc::now(),
        };

        result.signature = service.sign_result(&result);

        // Verification must succeed
        assert!(service.verify_signature(&result, &verifying_key));
    }

    #[test]
    fn test_oracle_result_tampered_fails() {
        let (service, verifying_key) = create_test_service();

        let mut result = OracleResult {
            faceit_match_id: "match_123".to_string(),
            winner_faceit_id: "player_a".to_string(),
            loser_faceit_id: "player_b".to_string(),
            match_finished_at: Utc::now(),
            score: "16:7".to_string(),
            signature: String::new(),
            queried_at: Utc::now(),
        };

        result.signature = service.sign_result(&result);

        // Tamper with the result
        result.winner_faceit_id = "player_c_hacker".to_string();

        // Verification must FAIL
        assert!(!service.verify_signature(&result, &verifying_key));
    }
}
