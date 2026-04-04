use serde::{Deserialize, Serialize};

/// Repräsentiert einen Benutzer in der Datenbank.
/// Das password_hash Feld wird NIEMALS in API-Responses zurückgegeben.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: uuid::Uuid,
    pub email: String,
    pub email_verified: bool,
    #[serde(skip_serializing)] // NIEMALS in JSON-Output
    pub password_hash: String,
    pub display_name: String,
    pub kaspa_address: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

/// Öffentliche User-Ansicht für API-Responses (ohne sensible Daten)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: uuid::Uuid,
    pub display_name: String,
    pub kaspa_address: Option<String>,
    pub created_at: String,
}

/// Repräsentiert eine aktive Session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String, // 256-bit Token, base64url
    pub user_id: uuid::Uuid,
    pub expires_at: String, // ISO 8601 datetime
    pub created_at: String,
}

/// Request-Body für die Registrierung
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,        // Muss gültige E-Mail sein
    pub password: String,     // Mindestens 8 Zeichen
    pub display_name: String, // 3-30 Zeichen, alphanumerisch + Unterstriche
}

/// Request-Body für den Login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Response nach erfolgreichem Login/Register
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user_id: uuid::Uuid,
    pub display_name: String,
    pub session_token: String,
    pub expires_at: String,
}

/// Request zum Setzen der Kaspa-Adresse
#[derive(Debug, Deserialize)]
pub struct SetKaspaAddressRequest {
    pub kaspa_address: String,
}

impl User {
    /// Konvertiert User in die öffentliche Ansicht
    pub fn to_public(&self) -> UserPublic {
        UserPublic {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            kaspa_address: self.kaspa_address.clone(),
            created_at: self.created_at.clone(),
        }
    }
}
