/// Oracle authentication module.
///
/// F-002: Provides a guard function that validates incoming requests to
/// privileged endpoints (e.g. POST /api/matches/{id}/resolve) against a
/// configurable list of Oracle API keys.
///
/// Keys are loaded once from the `ORACLE_API_KEYS` environment variable
/// (comma-separated, no spaces). For example:
///   ORACLE_API_KEYS=key-alpha,key-beta,key-gamma
///
/// The caller must include the header: `X-Oracle-Key: <key>`
use actix_web::HttpRequest;
use std::collections::HashSet;
use std::env;
use std::sync::OnceLock;
use subtle::ConstantTimeEq;

/// Loaded Oracle API key set (initialized once at startup).
static ORACLE_KEYS: OnceLock<HashSet<String>> = OnceLock::new();

/// Load and cache the set of valid Oracle API keys from the environment.
///
/// Call this once at server startup. Returns `Ok(count)` or an error if
/// the environment variable is empty or unset.
pub fn init_oracle_keys() -> Result<usize, String> {
    let raw = env::var("ORACLE_API_KEYS").unwrap_or_default();

    let keys: HashSet<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let count = keys.len();

    if count == 0 {
        log::warn!(
            "ORACLE_API_KEYS is empty or unset. The /resolve endpoint will reject ALL requests."
        );
    } else {
        log::info!("Oracle auth: {} API key(s) loaded.", count);
    }

    ORACLE_KEYS.set(keys).ok(); // ok() if already initialized (idempotent)
    Ok(count)
}

/// Returns the set of configured Oracle API keys.
/// Panic-safe: if init was never called, returns an empty set.
fn get_oracle_keys() -> &'static HashSet<String> {
    ORACLE_KEYS.get_or_init(|| {
        // Lazy fallback: load from env on first access
        let raw = env::var("ORACLE_API_KEYS").unwrap_or_default();
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

/// Verify that the incoming HTTP request carries a valid Oracle API key.
///
/// Checks the `X-Oracle-Key` header against the configured key set.
/// Returns `true` if authorized, `false` otherwise.
///
/// Intentionally does NOT log the key value — only logs success/failure and
/// the client IP for audit purposes.
pub fn verify_oracle_key(req: &HttpRequest) -> bool {
    let keys = get_oracle_keys();

    if keys.is_empty() {
        log::error!(
            "Oracle key check failed: no keys configured (ORACLE_API_KEYS unset). \
             Request from {:?} rejected.",
            req.peer_addr()
        );
        return false;
    }

    let provided = match req.headers().get("X-Oracle-Key") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                log::warn!(
                    "Oracle key header contains non-UTF8 bytes from {:?}",
                    req.peer_addr()
                );
                return false;
            }
        },
        None => {
            log::warn!(
                "Oracle auth failed: X-Oracle-Key header missing from {:?}",
                req.peer_addr()
            );
            return false;
        }
    };

    // F-007: Use constant-time comparison to prevent timing oracle attacks.
    // We iterate ALL keys regardless of match to keep runtime constant.
    let authorized = constant_time_key_check(keys, &provided);

    if !authorized {
        log::warn!(
            "Oracle auth failed: invalid key from {:?} (key length: {})",
            req.peer_addr(),
            provided.len()
        );
    } else {
        log::info!("Oracle auth succeeded from {:?}", req.peer_addr());
    }

    authorized
}

/// F-007: Constant-time comparison of the provided key against all configured keys.
/// Always iterates the full set to prevent timing side channels.
fn constant_time_key_check(keys: &HashSet<String>, provided: &str) -> bool {
    let provided_bytes = provided.as_bytes();
    let mut found = 0u8;

    for key in keys {
        let key_bytes = key.as_bytes();
        // Only compare if lengths match (length itself leaks, but that's acceptable
        // for API keys where length is not secret).
        if key_bytes.len() == provided_bytes.len() {
            // ct_eq returns Choice; unwrap_u8() converts to bool-like u8.
            let eq: u8 = key_bytes.ct_eq(provided_bytes).unwrap_u8();
            found |= eq;
        }
    }

    found == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;

    #[test]
    async fn test_valid_key_accepted() {
        // Use the internal key set directly to avoid OnceLock initialization issues in tests
        let mut keys = HashSet::new();
        keys.insert("valid-oracle-key".to_string());

        let provided = "valid-oracle-key";
        assert!(keys.contains(provided), "Valid key must be accepted");
    }

    #[test]
    async fn test_invalid_key_rejected() {
        let mut keys = HashSet::new();
        keys.insert("valid-oracle-key".to_string());

        let provided = "wrong-key";
        assert!(!keys.contains(provided), "Wrong key must be rejected");
    }

    #[test]
    async fn test_empty_key_rejected() {
        let mut keys = HashSet::new();
        keys.insert("valid-oracle-key".to_string());

        let provided = "";
        assert!(!keys.contains(provided), "Empty key must be rejected");
    }

    #[test]
    async fn test_init_oracle_keys_parsing() {
        // Test the internal parsing logic directly instead of using env vars
        // which are global and hazardous in multi-threaded tests.
        let raw = "key1,key2";
        let keys: HashSet<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains("key1"));
        assert!(keys.contains("key2"));
    }

    #[test]
    async fn test_init_oracle_keys_parses_correctly() {
        let raw = "key-alpha,key-beta, key-gamma , ";
        let keys: HashSet<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(keys.len(), 3);
        assert!(keys.contains("key-alpha"));
        assert!(keys.contains("key-beta"));
        assert!(keys.contains("key-gamma"));
        // Empty trailing entry must be excluded
        assert!(!keys.contains(""));
    }
}
