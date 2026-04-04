//! # SecretProvider — Abstraktionsschicht für Secrets (F-11)
//!
//! Lädt sensible Konfigurationswerte (API-Keys, Passwörter, Credentials) aus
//! verschiedenen Backends — je nach Deployment-Umgebung automatisch ausgewählt.
//!
//! ## Backends (Priorität absteigend)
//!
//! 1. **Docker Secrets** (`/run/secrets/<name>`): Produktionsempfehlung für
//!    Docker Swarm / Docker Compose mit `secrets:` Block. Kein Klartext in ENV.
//! 2. **Environment Variables**: Fallback für lokale Entwicklung und CI.
//!
//! ## Verwendung in `main.rs`
//!
//! ```rust
//! use battle_core::secret_provider::SecretProvider;
//!
//! let provider = SecretProvider::auto_detect();
//! let api_key = provider.require("FACEIT_DATA_API_KEY")?;
//! ```
//!
//! ## Docker Compose Setup (Beispiel)
//!
//! ```yaml
//! services:
//!   battle-api:
//!     secrets:
//!       - faceit_data_api_key
//!     environment:
//!       # Zeigt SecretProvider auf Docker-Secret-Datei
//!       USE_DOCKER_SECRETS: "true"
//!
//! secrets:
//!   faceit_data_api_key:
//!     external: true  # vorher: docker secret create faceit_data_api_key ./key.txt
//! ```

use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Konfiguration
// ─────────────────────────────────────────────────────────────────────────────

/// Standard-Pfad für Docker Secrets (Docker Swarm und Docker Compose secrets).
const DOCKER_SECRETS_PATH: &str = "/run/secrets";

// ─────────────────────────────────────────────────────────────────────────────
// Fehlertyp
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Required secret '{name}' not found in any configured backend ({backends})")]
    Missing {
        name: String,
        backends: String,
    },
    #[error("Secret '{name}' from file '{path}' could not be read: {reason}")]
    FileReadError {
        name: String,
        path: String,
        reason: String,
    },
    #[error("Secret '{name}' is empty")]
    Empty { name: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend-Typen
// ─────────────────────────────────────────────────────────────────────────────

/// Konfiguriertes Secrets-Backend.
#[derive(Debug, Clone)]
pub enum SecretBackend {
    /// Liest Secrets aus Environment-Variablen (Standard für lokale Entwicklung).
    Env,
    /// Liest Secrets aus Dateien in einem Verzeichnis (Docker Secrets / Vault Agent).
    /// Dateiname entspricht dem Secret-Namen in Kleinbuchstaben mit `_` statt `-`.
    Files { base_path: PathBuf },
}

// ─────────────────────────────────────────────────────────────────────────────
// SecretProvider
// ─────────────────────────────────────────────────────────────────────────────

/// Abstraktionsschicht für Secrets-Loading.
/// Unterstützt mehrere Backends in Prioritätsreihenfolge.
#[derive(Debug, Clone)]
pub struct SecretProvider {
    backends: Vec<SecretBackend>,
}

impl SecretProvider {
    /// Erstellt einen Provider der nur ENV-Variablen nutzt (Standard für Dev).
    pub fn env_only() -> Self {
        Self {
            backends: vec![SecretBackend::Env],
        }
    }

    /// Erstellt einen Provider der Docker Secrets zuerst probiert, dann ENV.
    pub fn with_docker_secrets(secrets_path: impl Into<PathBuf>) -> Self {
        Self {
            backends: vec![
                SecretBackend::Files {
                    base_path: secrets_path.into(),
                },
                SecretBackend::Env,
            ],
        }
    }

    /// **Auto-Detection**: Erkennt anhand der Umgebung das beste Backend.
    ///
    /// - `USE_DOCKER_SECRETS=true` oder `/run/secrets` existiert → Docker Secrets zuerst
    /// - Sonst → ENV only
    ///
    /// Dies erlaubt dieselbe Binary in Dev (ENV) und Prod (Docker Secrets) zu nutzen.
    pub fn auto_detect() -> Self {
        let docker_secrets_dir = PathBuf::from(DOCKER_SECRETS_PATH);
        let use_docker = std::env::var("USE_DOCKER_SECRETS")
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
            || docker_secrets_dir.exists();

        if use_docker {
            tracing::info!(
                "🔐 SecretProvider: Docker Secrets mode (path: {})",
                DOCKER_SECRETS_PATH
            );
            Self::with_docker_secrets(docker_secrets_dir)
        } else {
            tracing::info!("🔐 SecretProvider: Environment Variables mode");
            Self::env_only()
        }
    }

    /// Liest ein Secret. Gibt `Ok(None)` wenn nicht gefunden.
    pub fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        for backend in &self.backends {
            match backend {
                SecretBackend::Env => {
                    if let Ok(val) = std::env::var(name) {
                        let trimmed = val.trim().to_string();
                        if !trimmed.is_empty() {
                            return Ok(Some(trimmed));
                        }
                    }
                }
                SecretBackend::Files { base_path } => {
                    // Docker Secrets: Dateiname = secret name in lowercase,
                    // z.B. "FACEIT_DATA_API_KEY" → "/run/secrets/faceit_data_api_key"
                    let file_name = name.to_lowercase();
                    let file_path = base_path.join(&file_name);

                    if file_path.exists() {
                        match std::fs::read_to_string(&file_path) {
                            Ok(content) => {
                                let trimmed = content.trim().to_string();
                                if !trimmed.is_empty() {
                                    tracing::debug!(
                                        "🔐 Secret '{}' loaded from file: {}",
                                        name,
                                        file_path.display()
                                    );
                                    return Ok(Some(trimmed));
                                }
                            }
                            Err(e) => {
                                return Err(SecretError::FileReadError {
                                    name: name.to_string(),
                                    path: file_path.display().to_string(),
                                    reason: e.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Liest ein **Pflicht**-Secret. Gibt Err zurück wenn nicht gefunden oder leer.
    pub fn require(&self, name: &str) -> Result<String, SecretError> {
        match self.get(name)? {
            Some(val) => Ok(val),
            None => {
                let backends = self
                    .backends
                    .iter()
                    .map(|b| match b {
                        SecretBackend::Env => "ENV".to_string(),
                        SecretBackend::Files { base_path } => {
                            format!("FILES({})", base_path.display())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                Err(SecretError::Missing {
                    name: name.to_string(),
                    backends,
                })
            }
        }
    }

    /// Gibt an ob der Provider im Docker-Secrets-Modus läuft.
    pub fn is_docker_secrets_mode(&self) -> bool {
        self.backends
            .iter()
            .any(|b| matches!(b, SecretBackend::Files { .. }))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_env_provider_reads_env_var() {
        std::env::set_var("_TEST_SECRET_ALPHA", "test_value_123");
        let provider = SecretProvider::env_only();
        let result = provider.get("_TEST_SECRET_ALPHA").unwrap();
        assert_eq!(result, Some("test_value_123".to_string()));
        std::env::remove_var("_TEST_SECRET_ALPHA");
    }

    #[test]
    fn test_env_provider_missing_returns_none() {
        std::env::remove_var("_TEST_SECRET_MISSING");
        let provider = SecretProvider::env_only();
        let result = provider.get("_TEST_SECRET_MISSING").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_require_missing_returns_error() {
        std::env::remove_var("_TEST_SECRET_MISSING_2");
        let provider = SecretProvider::env_only();
        assert!(provider.require("_TEST_SECRET_MISSING_2").is_err());
    }

    #[test]
    fn test_file_provider_reads_secret_file() {
        let tmp = TempDir::new().unwrap();
        let secret_path = tmp.path().join("faceit_data_api_key");
        let mut f = std::fs::File::create(&secret_path).unwrap();
        writeln!(f, "  my_secret_api_key  ").unwrap(); // trailing whitespace trimmed

        let provider = SecretProvider::with_docker_secrets(tmp.path());
        let result = provider.get("FACEIT_DATA_API_KEY").unwrap();
        assert_eq!(result, Some("my_secret_api_key".to_string()));
    }

    #[test]
    fn test_file_provider_falls_back_to_env() {
        let tmp = TempDir::new().unwrap();
        // No file in tmp dir → should fall back to ENV
        std::env::set_var("_TEST_SECRET_FALLBACK", "env_value");
        let provider = SecretProvider::with_docker_secrets(tmp.path());
        let result = provider.get("_TEST_SECRET_FALLBACK").unwrap();
        assert_eq!(result, Some("env_value".to_string()));
        std::env::remove_var("_TEST_SECRET_FALLBACK");
    }
}
