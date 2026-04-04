use serde::{Deserialize, Serialize};

/// Konfiguration für FACEIT OAuth2
#[derive(Debug, Clone)]
pub struct FaceitOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,     // https://accounts.faceit.com
    pub token_url: String,    // https://api.faceit.com/auth/v1/oauth/token
    pub userinfo_url: String, // https://api.faceit.com/auth/v1/resources/userinfo
}

/// Wird in der DB gespeichert: Zuordnung User ↔ FACEIT-Account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceitLink {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub faceit_player_id: String, // FACEIT GUID
    pub faceit_nickname: String,
    pub faceit_elo: Option<i32>,
    pub faceit_skill_level: Option<i32>,
    pub faceit_avatar_url: Option<String>,
    pub linked_at: String,
    pub verified: bool,
}

/// FACEIT Token-Response
#[derive(Debug, Deserialize)]
pub struct FaceitTokenResponse {
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// FACEIT UserInfo-Response
#[derive(Debug, Deserialize)]
pub struct FaceitUserInfo {
    pub guid: String, // FACEIT Player ID
    pub nickname: String,
    pub email: Option<String>,
    pub picture: Option<String>, // Avatar URL
}

/// Wird in der Session gespeichert während OAuth2-Flow läuft
/// (Kurzlebig, nur für die Dauer des Redirects)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthPendingState {
    pub state: String,           // Random CSRF-Token
    pub code_verifier: String,   // PKCE Code Verifier
    pub user_id: Option<String>, // Option für echtes SSO (None = Login, Some = Account-Link)
    pub return_to: Option<String>,
    pub created_at: String,
}

/// Response für den /faceit/status Endpoint
#[derive(Debug, Serialize)]
pub struct FaceitLinkStatus {
    pub linked: bool,
    pub faceit_nickname: Option<String>,
    pub faceit_elo: Option<i32>,
    pub faceit_skill_level: Option<i32>,
    pub faceit_avatar_url: Option<String>,
    pub linked_at: Option<String>,
}
