use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use battle_core::{auth::AuthService, models::user::User};

use crate::api::AppState;

/// Dies ist ein Axum Extractor, der sicherstellt, dass:
/// 1. Ein gültiges Bearer-Token im Header übergeben wurde.
/// 2. Das Token aktiv in der der `sessions` Tabelle steht.
/// 3. Der User eine verknüpfte Kaspa-Adresse für das Escrow besitzt.
pub struct SessionUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for SessionUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // --- TEST_MODE Bypass ---
        if std::env::var("TEST_MODE").unwrap_or_default() == "true" {
            return Ok(SessionUser(User {
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                email: "test@example.com".to_string(),
                email_verified: true,
                password_hash: "".to_string(),
                display_name: "TestUser".to_string(),
                kaspa_address: Some(
                    "kaspatest:qzh86re35m2re7k7sxc6uqlp40st4vvmefsc0sv0uev49v3axm7jwcst6p7xs"
                        .to_string(),
                ),
                created_at: "".to_string(),
                updated_at: "".to_string(),
                last_login_at: None,
            }));
        }

        // 1. JWT / Session aus dem Authorization Header ziehen
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization Header"))?;

        if !auth_header.starts_with("Bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "Invalid Token Format"));
        }
        let token = &auth_header["Bearer ".len()..];

        // 2. Gegen die Datenbank (Sessions-Tabelle) prüfen
        let auth_service = AuthService::new(state.pool.clone());
        let user = auth_service
            .validate_session(token)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Session invalid or expired"))?;

        // 3. Strikter Wallet Guard: Ohne verknüpfte Adresse ist Gameplay verboten!
        if user.kaspa_address.is_none() {
            return Err((
                StatusCode::FORBIDDEN,
                "You must link a Kaspa wallet before participating in battles",
            ));
        }

        Ok(SessionUser(user))
    }
}
