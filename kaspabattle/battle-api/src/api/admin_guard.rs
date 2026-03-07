use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

use crate::api::AppState;

/// Axum extractor for admin-only API endpoints.
///
/// Validates the `X-Admin-Api-Key` header against the `ADMIN_API_KEY`
/// environment variable. Rejects with 403 if the key is missing or wrong.
///
/// Usage:
/// ```rust
/// pub async fn admin_resolve_match(
///     State(state): State<AppState>,
///     _admin: AdminApiKey,          // <-- add this extractor
///     Path(id): Path<Uuid>,
///     ...
/// ```
///
/// Set `ADMIN_API_KEY` in your `.env` file:
///   `ADMIN_API_KEY=$(openssl rand -hex 32)`
pub struct AdminApiKey;

#[async_trait]
impl FromRequestParts<AppState> for AdminApiKey {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Read the expected key at request time (not cached at startup)
        // so that key rotation takes effect without a restart.
        let expected_key = std::env::var("ADMIN_API_KEY").unwrap_or_default();

        if expected_key.is_empty() {
            tracing::error!(
                "ADMIN_API_KEY environment variable is not set — admin endpoint blocked"
            );
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Admin API not configured. Set ADMIN_API_KEY environment variable.",
            ));
        }

        let provided_key = parts
            .headers
            .get("X-Admin-Api-Key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // Constant-time comparison to prevent timing attacks
        if !constant_time_eq(provided_key.as_bytes(), expected_key.as_bytes()) {
            tracing::warn!("Admin API: rejected request with invalid or missing API key");
            return Err((StatusCode::FORBIDDEN, "Invalid or missing admin API key"));
        }

        tracing::info!("Admin API: authenticated request from admin key");
        Ok(AdminApiKey)
    }
}

/// Constant-time byte comparison to avoid timing side-channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
