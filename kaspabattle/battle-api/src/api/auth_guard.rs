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
        // 1. Extract Bearer token from Authorization header or Cookie
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok());

        let token_opt = if let Some(header) = auth_header {
            if let Some(stripped) = header.strip_prefix("Bearer ") {
                Some(stripped.to_string())
            } else {
                None
            }
        } else {
            // Fallback: check "kaspabattle-auth" Cookie
            parts
                .headers
                .get(axum::http::header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|cookie_str| {
                    cookie_str.split(';').find_map(|pair| {
                        let mut kv = pair.splitn(2, '=');
                        let key = kv.next()?.trim();
                        let val = kv.next()?.trim();
                        if key == "kaspabattle-auth" {
                            Some(val.to_string())
                        } else {
                            None
                        }
                    })
                })
        };

        let token_str = token_opt.ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization Header or Cookie",
        ))?;
        let token = token_str.as_str();

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

/// Lightweight session extractor: validates token but does NOT require a Kaspa wallet.
/// Used for endpoints like `/auth/me` where FaceIT-only users must be allowed.
pub struct SessionUserNoWallet(pub User);

#[async_trait]
impl FromRequestParts<AppState> for SessionUserNoWallet {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Same token extraction logic as SessionUser
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok());

        let token_opt = if let Some(header) = auth_header {
            if let Some(stripped) = header.strip_prefix("Bearer ") {
                Some(stripped.to_string())
            } else {
                None
            }
        } else {
            parts
                .headers
                .get(axum::http::header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|cookie_str| {
                    cookie_str.split(';').find_map(|pair| {
                        let mut kv = pair.splitn(2, '=');
                        let key = kv.next()?.trim();
                        let val = kv.next()?.trim();
                        if key == "kaspabattle-auth" {
                            Some(val.to_string())
                        } else {
                            None
                        }
                    })
                })
        };

        let token_str = token_opt.ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization Header or Cookie",
        ))?;

        let auth_service = AuthService::new(state.pool.clone());
        let user = auth_service
            .validate_session(&token_str)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Session invalid or expired"))?;

        // NO wallet guard here — FaceIT-only users are allowed
        Ok(SessionUserNoWallet(user))
    }
}

/// Unit-test helper: a pre-built User that passes all guards.
///
/// Only available inside `#[cfg(test)]` — never compiled into production builds.
#[cfg(test)]
#[allow(dead_code)]
pub fn test_session_user() -> SessionUser {
    SessionUser(User {
        id: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
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
