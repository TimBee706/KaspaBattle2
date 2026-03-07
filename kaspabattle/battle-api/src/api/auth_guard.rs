use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use battle_core::{auth::AuthService, models::user::User};

use crate::api::AppState;

/// Axum extractor that enforces authenticated, wallet-linked sessions.
///
/// Validates:
///   1. A valid Bearer token is present in the Authorization header.
///   2. The token resolves to an active entry in the `sessions` table.
///   3. The resolved user has a linked Kaspa wallet address (required for gameplay).
///
/// ⚠️ Security note: There is NO development bypass for this extractor.
/// To write integration tests that require an authenticated user, use
/// the `#[cfg(test)]` helper `test_session_user()` defined at the bottom
/// of this file, or mock the extractor via `axum::test` infrastructure.
pub struct SessionUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for SessionUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract Bearer token from Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization Header"))?;

        if !auth_header.starts_with("Bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "Invalid Token Format"));
        }
        let token = &auth_header["Bearer ".len()..];

        // 2. Validate token against the sessions table
        let auth_service = AuthService::new(state.pool.clone());
        let user = auth_service
            .validate_session(token)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Session invalid or expired"))?;

        // 3. Wallet guard: Kaspa address is required for any gameplay action
        if user.kaspa_address.is_none() {
            return Err((
                StatusCode::FORBIDDEN,
                "You must link a Kaspa wallet before participating in battles",
            ));
        }

        Ok(SessionUser(user))
    }
}

/// Unit-test helper: a pre-built User that passes all guards.
///
/// Only available inside `#[cfg(test)]` — never compiled into production builds.
#[cfg(test)]
pub fn test_session_user() -> SessionUser {
    SessionUser(User {
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        email: "test@example.com".to_string(),
        email_verified: true,
        password_hash: "".to_string(),
        display_name: "TestUser".to_string(),
        kaspa_address: Some(
            "kaspatest:qzh86re35m2re7k7sxc6uqlp40st4vvmefsc0sv0uev49v3axm7jwcst6p7xs".to_string(),
        ),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        last_login_at: None,
    })
}
