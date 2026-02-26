use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use rand::Rng;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::faceit::{
    FaceitLink, FaceitLinkStatus, FaceitOAuthConfig, FaceitTokenResponse, FaceitUserInfo,
    OAuthPendingState,
};

/// Generiert einen PKCE Code Verifier (43-128 Zeichen, URL-safe)
pub fn generate_code_verifier() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Berechnet die PKCE Code Challenge (SHA256 + base64url)
pub fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

/// Generiert einen zufälligen State-Token für CSRF-Schutz
pub fn generate_state() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Erstellt den Base64url-kodierten Basic Auth Header
/// WICHTIG: Muss URL-safe sein ('+' → '-', '/' → '_', kein '=')
pub fn encode_basic_auth(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{}:{}", client_id, client_secret);
    URL_SAFE_NO_PAD.encode(credentials.as_bytes())
}

pub struct FaceitOAuthService {
    config: FaceitOAuthConfig,
    http_client: reqwest::Client,
    db: Arc<Mutex<Connection>>,
    pending_states: Arc<Mutex<HashMap<String, OAuthPendingState>>>,
}

impl FaceitOAuthService {
    pub fn new(config: FaceitOAuthConfig, db: Arc<Mutex<Connection>>) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            db,
            pending_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn generate_auth_url(
        &self,
        user_id: Option<&str>,
    ) -> Result<(String, OAuthPendingState)> {
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier);
        let state = generate_state();

        let mut url = reqwest::Url::parse(&self.config.auth_url)
            .map_err(|e| anyhow!("Fehler beim Parsen der Auth-URL: {}", e))?;

        url.query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", "openid email profile")
            .append_pair("state", &state)
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256");

        let pending_state = OAuthPendingState {
            state: state.clone(),
            code_verifier,
            user_id: user_id.map(|s| s.to_string()),
            created_at: Utc::now().to_rfc3339(),
        };

        let mut states = self.pending_states.lock().await;
        states.insert(state.clone(), pending_state.clone());

        Ok((url.to_string(), pending_state))
    }

    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(FaceitUserInfo, FaceitTokenResponse, Option<String>)> {
        let pending_state = {
            let mut states = self.pending_states.lock().await;
            states
                .remove(state)
                .ok_or_else(|| anyhow!("Ungültiger oder abgelaufener State-Parameter"))?
        };

        let tokens = self
            .exchange_code(code, &pending_state.code_verifier)
            .await?;
        let userinfo = self.get_userinfo(&tokens.access_token).await?;

        Ok((userinfo, tokens, pending_state.user_id))
    }

    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<FaceitTokenResponse> {
        let auth_header = format!(
            "Basic {}",
            encode_basic_auth(&self.config.client_id, &self.config.client_secret)
        );

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
        ];

        let response = self
            .http_client
            .post(&self.config.token_url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP-Fehler beim Token-Request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "FACEIT API Fehler bei Token-Request: {} - {}",
                status,
                text
            ));
        }

        let tokens = response
            .json::<FaceitTokenResponse>()
            .await
            .map_err(|e| anyhow!("Fehler beim Parsen der FACEIT Token-Response: {}", e))?;

        Ok(tokens)
    }

    pub async fn get_userinfo(&self, access_token: &str) -> Result<FaceitUserInfo> {
        let response = self
            .http_client
            .get(&self.config.userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP-Fehler beim UserInfo-Request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "FACEIT API Fehler bei UserInfo-Request: {} - {}",
                status,
                text
            ));
        }

        let userinfo = response
            .json::<FaceitUserInfo>()
            .await
            .map_err(|e| anyhow!("Fehler beim Parsen der FACEIT UserInfo-Response: {}", e))?;

        Ok(userinfo)
    }

    pub async fn save_faceit_link(
        &self,
        user_id: &str,
        info: &FaceitUserInfo,
        tokens: &FaceitTokenResponse,
    ) -> Result<FaceitLink> {
        let conn = self.db.lock().await;

        let link_id = Uuid::new_v4().to_string();
        let expires_at = (Utc::now() + Duration::seconds(tokens.expires_in as i64)).to_rfc3339();

        match conn.execute(
            "INSERT INTO faceit_links (id, user_id, faceit_player_id, faceit_nickname, faceit_avatar_url, access_token, refresh_token, token_expires_at, verified) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(user_id) DO UPDATE SET 
             access_token=excluded.access_token, 
             refresh_token=excluded.refresh_token, 
             token_expires_at=excluded.token_expires_at",
            (
                &link_id,
                user_id,
                &info.guid,
                &info.nickname,
                info.picture.as_deref(),
                &tokens.access_token,   // In Production sollte dies verschlüsselt werden
                &tokens.refresh_token,  // In Production sollte dies verschlüsselt werden
                &expires_at,
                1
            ),
        ) {
            Ok(_) => {},
            Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation => {
                // If it fails on faceit_player_id unique constraint
                conn.execute(
                    "UPDATE faceit_links SET access_token=?1, refresh_token=?2, token_expires_at=?3 WHERE faceit_player_id=?4",
                    (&tokens.access_token, &tokens.refresh_token, &expires_at, &info.guid),
                )?;
            }
            Err(e) => return Err(e.into()),
        }

        Ok(FaceitLink {
            id: link_id,
            user_id: user_id.to_string(),
            faceit_player_id: info.guid.clone(),
            faceit_nickname: info.nickname.clone(),
            faceit_elo: None,
            faceit_skill_level: None,
            faceit_avatar_url: info.picture.clone(),
            linked_at: Utc::now().to_rfc3339(),
            verified: true,
        })
    }

    pub async fn get_link_status(&self, user_id: &str) -> Result<FaceitLinkStatus> {
        let conn = self.db.lock().await;

        let mut stmt = conn.prepare(
            "SELECT faceit_nickname, faceit_elo, faceit_skill_level, faceit_avatar_url, linked_at 
             FROM faceit_links WHERE user_id = ?1",
        )?;

        let mut rows = stmt.query([user_id])?;

        if let Some(row) = rows.next()? {
            Ok(FaceitLinkStatus {
                linked: true,
                faceit_nickname: Some(row.get(0)?),
                faceit_elo: row.get(1)?,
                faceit_skill_level: row.get(2)?,
                faceit_avatar_url: row.get(3)?,
                linked_at: Some(row.get(4)?),
            })
        } else {
            Ok(FaceitLinkStatus {
                linked: false,
                faceit_nickname: None,
                faceit_elo: None,
                faceit_skill_level: None,
                faceit_avatar_url: None,
                linked_at: None,
            })
        }
    }

    pub async fn unlink_faceit(&self, user_id: &str) -> Result<()> {
        let conn = self.db.lock().await;
        conn.execute("DELETE FROM faceit_links WHERE user_id = ?1", [user_id])?;
        Ok(())
    }

    pub async fn get_faceit_player_id(&self, user_id: &str) -> Result<Option<String>> {
        let conn = self.db.lock().await;

        let mut stmt =
            conn.prepare("SELECT faceit_player_id FROM faceit_links WHERE user_id = ?1")?;
        let mut rows = stmt.query([user_id])?;

        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_config() -> FaceitOAuthConfig {
        FaceitOAuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/callback".to_string(),
            auth_url: "https://accounts.faceit.com/authorize".to_string(),
            token_url: "https://api.faceit.com/auth/v1/oauth/token".to_string(),
            userinfo_url: "https://api.faceit.com/auth/v1/resources/userinfo".to_string(),
        }
    }

    async fn create_test_service() -> FaceitOAuthService {
        let conn = Connection::open_in_memory().unwrap();

        conn.execute_batch(
            "CREATE TABLE users (id TEXT PRIMARY KEY);
             CREATE TABLE faceit_links (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL UNIQUE,
                faceit_player_id TEXT NOT NULL UNIQUE,
                faceit_nickname TEXT NOT NULL,
                faceit_elo INTEGER,
                faceit_skill_level INTEGER,
                faceit_avatar_url TEXT,
                access_token TEXT,
                refresh_token TEXT,
                token_expires_at TEXT,
                linked_at TEXT NOT NULL DEFAULT (datetime('now')),
                verified INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();

        FaceitOAuthService::new(mock_config(), Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn test_generate_code_verifier() {
        let v1 = generate_code_verifier();
        let v2 = generate_code_verifier();
        assert!(v1.len() >= 43 && v1.len() <= 128);
        assert!(!v1.contains('+') && !v1.contains('/') && !v1.contains('='));
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_generate_code_challenge() {
        let verifier = "test-verifier-which-is-long-enough-for-pkce";
        let challenge = generate_code_challenge(verifier);
        assert!(!challenge.contains('+') && !challenge.contains('/') && !challenge.contains('='));
        assert!(!challenge.is_empty());
    }

    #[test]
    fn test_encode_basic_auth() {
        let basic = encode_basic_auth("user", "pass");
        assert_eq!(basic, URL_SAFE_NO_PAD.encode(b"user:pass"));
        assert!(!basic.contains('+') && !basic.contains('/') && !basic.contains('='));
    }

    #[test]
    fn test_generate_state() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert!(!s1.is_empty());
        assert_ne!(s1, s2);
    }

    #[tokio::test]
    async fn test_generate_auth_url_contains_all_params() {
        let service = create_test_service().await;
        let (url, state) = service.generate_auth_url(Some("user123")).await.unwrap();

        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A3000%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid+email+profile"));
        assert!(url.contains(&format!("state={}", state.state)));
        let challenge = generate_code_challenge(&state.code_verifier);
        assert!(url.contains(&format!("code_challenge={}", challenge)));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_code_challenge_matches_verifier() {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let expected = URL_SAFE_NO_PAD.encode(hash);

        assert_eq!(challenge, expected);
    }

    #[tokio::test]
    async fn test_save_faceit_link_to_db() {
        let service = create_test_service().await;

        service
            .db
            .lock()
            .await
            .execute("INSERT INTO users (id) VALUES ('user123')", [])
            .unwrap();

        let info = FaceitUserInfo {
            guid: "faceit-guid-123".to_string(),
            nickname: "testplayer".to_string(),
            email: Some("test@example.com".to_string()),
            picture: Some("http://example.com/pic.png".to_string()),
        };

        let tokens = FaceitTokenResponse {
            access_token: "access".to_string(),
            id_token: "id".to_string(),
            refresh_token: "refresh".to_string(),
            expires_in: 3600,
            token_type: "Bearer".to_string(),
        };

        let link = service
            .save_faceit_link("user123", &info, &tokens)
            .await
            .unwrap();

        assert_eq!(link.user_id, "user123");
        assert_eq!(link.faceit_player_id, "faceit-guid-123");
        assert_eq!(link.faceit_nickname, "testplayer");
        assert_eq!(
            link.faceit_avatar_url.unwrap(),
            "http://example.com/pic.png"
        );

        let status = service.get_link_status("user123").await.unwrap();
        assert!(status.linked);
    }

    #[tokio::test]
    async fn test_prevent_duplicate_faceit_link() {
        let service = create_test_service().await;

        service
            .db
            .lock()
            .await
            .execute("INSERT INTO users (id) VALUES ('user1')", [])
            .unwrap();
        service
            .db
            .lock()
            .await
            .execute("INSERT INTO users (id) VALUES ('user2')", [])
            .unwrap();

        let info = FaceitUserInfo {
            guid: "shared-guid".to_string(),
            nickname: "player".to_string(),
            email: None,
            picture: None,
        };

        let tokens = FaceitTokenResponse {
            access_token: "acc".to_string(),
            id_token: "id".to_string(),
            refresh_token: "ref".to_string(),
            expires_in: 3600,
            token_type: "Bearer".to_string(),
        };

        let res1 = service.save_faceit_link("user1", &info, &tokens).await;
        assert!(res1.is_ok());

        let res2 = service.save_faceit_link("user2", &info, &tokens).await;
        assert!(res2.is_err());
        assert!(res2
            .unwrap_err()
            .to_string()
            .contains("ist bereits verknüpft"));
    }

    #[tokio::test]
    async fn test_unlink_faceit() {
        let service = create_test_service().await;
        service
            .db
            .lock()
            .await
            .execute("INSERT INTO users (id) VALUES ('user1')", [])
            .unwrap();

        let info = FaceitUserInfo {
            guid: "guid".to_string(),
            nickname: "player".to_string(),
            email: None,
            picture: None,
        };
        let tokens = FaceitTokenResponse {
            access_token: "acc".to_string(),
            id_token: "id".to_string(),
            refresh_token: "ref".to_string(),
            expires_in: 3600,
            token_type: "Bearer".to_string(),
        };

        service
            .save_faceit_link("user1", &info, &tokens)
            .await
            .unwrap();

        let status1 = service.get_link_status("user1").await.unwrap();
        assert!(status1.linked);

        service.unlink_faceit("user1").await.unwrap();

        let status2 = service.get_link_status("user1").await.unwrap();
        assert!(!status2.linked);
    }

    #[tokio::test]
    async fn test_get_link_status_not_linked() {
        let service = create_test_service().await;
        let status = service.get_link_status("not-linked-user").await.unwrap();
        assert!(!status.linked);
        assert!(status.faceit_nickname.is_none());
        assert!(status.faceit_elo.is_none());
    }
}
