use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::faceit::{
    FaceitLink, FaceitLinkStatus, FaceitOAuthConfig, FaceitTokenResponse, FaceitUserInfo,
    OAuthPendingState,
};

pub fn generate_code_verifier() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

pub fn generate_state() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn encode_basic_auth(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{}:{}", client_id, client_secret);
    URL_SAFE_NO_PAD.encode(credentials.as_bytes())
}

pub struct FaceitOAuthService {
    config: FaceitOAuthConfig,
    http_client: reqwest::Client,
    db: PgPool,
    pending_states: Arc<Mutex<HashMap<String, OAuthPendingState>>>,
}

impl FaceitOAuthService {
    pub fn new(config: FaceitOAuthConfig, db: PgPool) -> Self {
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
        return_to: Option<String>,
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
            .append_pair("code_challenge_method", "S256")
            .append_pair("redirect_popup", "true");

        let pending_state = OAuthPendingState {
            state: state.clone(),
            code_verifier,
            user_id: user_id.map(|s| s.to_string()),
            return_to,
            created_at: Utc::now().to_rfc3339(),
        };

        let mut states = self.pending_states.lock().await;

        // Cleanup stale pending states (> 10 min) to prevent memory leaks
        states.retain(|_, ps| {
            chrono::DateTime::parse_from_rfc3339(&ps.created_at)
                .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_minutes() < 10)
                .unwrap_or(false)
        });

        states.insert(state.clone(), pending_state.clone());

        Ok((url.to_string(), pending_state))
    }

    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(FaceitUserInfo, FaceitTokenResponse, Option<String>, Option<String>)> {
        let pending_state = {
            let mut states = self.pending_states.lock().await;
            tracing::debug!("🔍 FACEIT callback: state='{}', {} pending states in memory", state, states.len());
            states
                .remove(state)
                .ok_or_else(|| {
                    tracing::error!("❌ FACEIT state '{}' not found in pending_states (backend may have restarted during OAuth flow)", state);
                    anyhow!("Ungültiger oder abgelaufener State-Parameter — bitte erneut einloggen")
                })?
        };

        let tokens = self
            .exchange_code(code, &pending_state.code_verifier)
            .await?;
        let userinfo = self.get_userinfo(&tokens.access_token).await?;

        Ok((userinfo, tokens, pending_state.user_id, pending_state.return_to))
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

        let params: [(& str, &str); 4] = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", &self.config.redirect_uri),
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
        let uid = Uuid::parse_str(user_id)?;
        let link_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(tokens.expires_in as i64);
        let mut tx = self.db.begin().await?;

        sqlx::query(
            "DELETE FROM faceit_links WHERE user_id = $1 AND faceit_player_id != $2"
        )
        .bind(&uid)
        .bind(&info.guid)
        .execute(&mut *tx)
        .await?;

        let updated = sqlx::query(
            "UPDATE faceit_links
             SET user_id = $1,
                 faceit_nickname = $2,
                 faceit_avatar_url = $3,
                 access_token = $4,
                 refresh_token = $5,
                 token_expires_at = $6,
                 verified = $7
             WHERE faceit_player_id = $8"
        )
        .bind(&uid)
        .bind(&info.nickname)
        .bind(&info.picture)
        .bind(&tokens.access_token)
        .bind(&tokens.refresh_token)
        .bind(&expires_at)
        .bind(true)
        .bind(&info.guid)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if updated == 0 {
            sqlx::query(
                "INSERT INTO faceit_links (id, user_id, faceit_player_id, faceit_nickname, faceit_avatar_url, access_token, refresh_token, token_expires_at, verified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
            )
            .bind(&link_id)
            .bind(&uid)
            .bind(&info.guid)
            .bind(&info.nickname)
            .bind(&info.picture)
            .bind(&tokens.access_token)
            .bind(&tokens.refresh_token)
            .bind(&expires_at)
            .bind(true)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE users
             SET faceit_id = $1,
                 nickname = $2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = $3"
        )
        .bind(&info.guid)
        .bind(&info.nickname)
        .bind(&uid)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(FaceitLink {
            id: link_id,
            user_id: uid,
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
        let uid = Uuid::parse_str(user_id)?;
        let row = sqlx::query(
            "SELECT faceit_nickname, faceit_elo, faceit_skill_level, faceit_avatar_url, linked_at 
             FROM faceit_links WHERE user_id = $1",
        )
        .bind(uid)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let linked_at: DateTime<Utc> = r.try_get("linked_at")?;
            Ok(FaceitLinkStatus {
                linked: true,
                faceit_nickname: Some(r.try_get("faceit_nickname")?),
                faceit_elo: r.try_get("faceit_elo")?,
                faceit_skill_level: r.try_get("faceit_skill_level")?,
                faceit_avatar_url: r.try_get("faceit_avatar_url")?,
                linked_at: Some(linked_at.to_rfc3339()),
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
        let uid = Uuid::parse_str(user_id)?;
        sqlx::query("DELETE FROM faceit_links WHERE user_id = $1")
            .bind(uid)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn get_faceit_player_id(&self, user_id: &str) -> Result<Option<String>> {
        let uid = Uuid::parse_str(user_id)?;
        let row = sqlx::query("SELECT faceit_player_id FROM faceit_links WHERE user_id = $1")
            .bind(uid)
            .fetch_optional(&self.db)
            .await?;

        if let Some(r) = row {
            let id: String = r.try_get("faceit_player_id")?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
}
