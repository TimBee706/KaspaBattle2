use anyhow::{anyhow, Result};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
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

/// HTTP Basic (RFC 7617) requires *standard* Base64 (with padding), not URL-safe.
pub fn encode_basic_auth(client_id: &str, client_secret: &str) -> String {
    let credentials = format!("{}:{}", client_id, client_secret);
    STANDARD.encode(credentials.as_bytes())
}

/// OAuth state and PKCE verifier live at most this long.
pub const OAUTH_STATE_TTL_MINUTES: i64 = 10;

/// The state is stored hashed so a database read never yields a redeemable state value.
fn hash_state(state: &str) -> String {
    hex_encode(&Sha256::digest(state.as_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Only relative in-app routes are accepted as post-login target ("/lobby", "/lobby?x=1").
/// Anything that could leave the canonical origin (absolute URLs, `//host`, backslashes,
/// control characters) is rejected.
pub fn sanitize_return_path(candidate: &str) -> Option<String> {
    let c = candidate.trim();
    if !c.starts_with('/') || c.starts_with("//") || c.contains('\\') || c.len() > 512 {
        return None;
    }
    if c.chars().any(|ch| ch.is_control() || ch == ' ') {
        return None;
    }
    Some(c.split('#').next().unwrap_or("/").to_string())
}

pub struct FaceitOAuthService {
    config: FaceitOAuthConfig,
    http_client: reqwest::Client,
    db: PgPool,
}

impl FaceitOAuthService {
    pub fn new(config: FaceitOAuthConfig, db: PgPool) -> Self {
        Self {
            config,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            db,
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
            .append_pair("code_challenge_method", "S256");
        // Full-page redirect flow: intentionally no `redirect_popup` parameter.

        let pending_state = OAuthPendingState {
            state: state.clone(),
            code_verifier,
            user_id: user_id.map(|s| s.to_string()),
            return_to: return_to.clone(),
            created_at: Utc::now().to_rfc3339(),
        };

        let user_uuid = user_id.map(Uuid::parse_str).transpose()?;
        let return_to = return_to.as_deref().and_then(sanitize_return_path);

        // Opportunistic cleanup of expired rows, then persist the new state.
        sqlx::query("DELETE FROM faceit_oauth_states WHERE expires_at < CURRENT_TIMESTAMP")
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO faceit_oauth_states (state_hash, code_verifier, user_id, return_to, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(hash_state(&state))
        .bind(&pending_state.code_verifier)
        .bind(user_uuid)
        .bind(&return_to)
        .bind(Utc::now() + Duration::minutes(OAUTH_STATE_TTL_MINUTES))
        .execute(&self.db)
        .await?;

        Ok((url.to_string(), pending_state))
    }

    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(
        FaceitUserInfo,
        FaceitTokenResponse,
        Option<String>,
        Option<String>,
    )> {
        let pending_state = self.consume_state(state).await?;

        let tokens = self
            .exchange_code(code, &pending_state.code_verifier)
            .await?;
        let userinfo = self.get_userinfo(&tokens.access_token).await?;

        Ok((
            userinfo,
            tokens,
            pending_state.user_id,
            pending_state.return_to,
        ))
    }

    /// Atomically redeems a state: a single `DELETE .. RETURNING` guarantees single use,
    /// even across concurrent requests or several backend instances. Expired rows never match.
    pub async fn consume_state(&self, state: &str) -> Result<OAuthPendingState> {
        let row = sqlx::query(
            "DELETE FROM faceit_oauth_states
             WHERE state_hash = $1 AND expires_at > CURRENT_TIMESTAMP
             RETURNING code_verifier, user_id, return_to, created_at",
        )
        .bind(hash_state(state))
        .fetch_optional(&self.db)
        .await?;

        let row = row.ok_or_else(|| {
            tracing::warn!("FACEIT callback rejected: unknown, expired or already used state");
            anyhow!("Ungültiger oder abgelaufener State-Parameter — bitte erneut einloggen")
        })?;

        let user_id: Option<Uuid> = row.try_get("user_id")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        Ok(OAuthPendingState {
            state: String::new(),
            code_verifier: row.try_get("code_verifier")?,
            user_id: user_id.map(|u| u.to_string()),
            return_to: row.try_get("return_to")?,
            created_at: created_at.to_rfc3339(),
        })
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

        let params: [(&str, &str); 4] = [
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
            // Body is intentionally not included: error bodies may echo request parameters.
            return Err(anyhow!("FACEIT API Fehler bei Token-Request: {}", status));
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

        sqlx::query("DELETE FROM faceit_links WHERE user_id = $1 AND faceit_player_id != $2")
            .bind(uid)
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
             WHERE faceit_player_id = $8",
        )
        .bind(uid)
        .bind(&info.nickname)
        .bind(&info.picture)
        .bind(&tokens.access_token)
        .bind(&tokens.refresh_token)
        .bind(expires_at)
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
            .bind(link_id)
            .bind(uid)
            .bind(&info.guid)
            .bind(&info.nickname)
            .bind(&info.picture)
            .bind(&tokens.access_token)
            .bind(&tokens.refresh_token)
            .bind(expires_at)
            .bind(true)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE users
             SET faceit_id = $1,
                 nickname = $2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = $3",
        )
        .bind(&info.guid)
        .bind(&info.nickname)
        .bind(uid)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::faceit::FaceitOAuthConfig;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const REDIRECT: &str = "https://www.example.test/auth/faceit/callback";

    fn test_config(base: &str) -> FaceitOAuthConfig {
        FaceitOAuthConfig {
            client_id: "client".to_string(),
            client_secret: "not-a-real-secret".to_string(),
            redirect_uri: REDIRECT.to_string(),
            auth_url: "https://accounts.faceit.com".to_string(),
            token_url: format!("{base}/token"),
            userinfo_url: format!("{base}/userinfo"),
        }
    }

    /// DB tests need PostgreSQL: set TEST_DATABASE_URL (each test uses its own schema).
    /// Without it the DB-backed tests are skipped.
    async fn test_pool() -> Option<PgPool> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        let schema = format!("t_{}", Uuid::new_v4().simple());
        let admin = PgPool::connect(&url).await.ok()?;
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .ok()?;
        let s2 = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .after_connect(move |conn, _| {
                let s = s2.clone();
                Box::pin(async move {
                    sqlx::query(&format!("SET search_path TO {s}"))
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .ok()?;
        sqlx::raw_sql(include_str!(
            "../../migrations/202609290004_faceit_oauth_pending_states.sql"
        ))
        .execute(&pool)
        .await
        .ok()?;
        Some(pool)
    }

    async fn create_link_tables(pool: &PgPool) {
        sqlx::raw_sql(
            "CREATE TABLE users (id UUID PRIMARY KEY, faceit_id TEXT, nickname TEXT, updated_at TIMESTAMPTZ);
             CREATE TABLE faceit_links (id UUID PRIMARY KEY, user_id UUID NOT NULL, faceit_player_id TEXT UNIQUE NOT NULL,
               faceit_nickname TEXT NOT NULL, faceit_avatar_url TEXT, access_token TEXT, refresh_token TEXT,
               token_expires_at TIMESTAMPTZ, verified BOOLEAN, faceit_elo INTEGER, faceit_skill_level INTEGER,
               linked_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP);",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    /// Minimal FACEIT mock: records the token request, answers token + userinfo.
    async fn mock_faceit(token_status: u16) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let (mut sock, _) = listener.accept().await.unwrap();
                let seen = seen2.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let (status, body) = if req.starts_with("POST /token") {
                        seen.lock().unwrap().push(req.clone());
                        if token_status == 200 {
                            (
                                200,
                                r#"{"access_token":"at","id_token":"it","refresh_token":"rt","expires_in":3600,"token_type":"Bearer"}"#,
                            )
                        } else {
                            (token_status, r#"{"error":"invalid_grant"}"#)
                        }
                    } else {
                        (
                            200,
                            r#"{"guid":"11111111-2222-3333-4444-555555555555","nickname":"tester","email":null,"picture":null}"#,
                        )
                    };
                    let resp = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (base, seen)
    }

    fn params_of(url: &str) -> HashMap<String, String> {
        reqwest::Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn basic_auth_uses_standard_base64_known_vector() {
        // RFC 7617 example vector
        assert_eq!(
            encode_basic_auth("Aladdin", "open sesame"),
            "QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        // Bytes that differ between standard and URL-safe alphabets ('+', '/', '=')
        assert_eq!(encode_basic_auth("a?", ">>>>"), "YT86Pj4+Pg==");
    }

    #[test]
    fn pkce_s256_matches_rfc7636_vector() {
        assert_eq!(
            generate_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn return_path_validation_rejects_open_redirects() {
        assert_eq!(sanitize_return_path("/lobby"), Some("/lobby".into()));
        assert_eq!(
            sanitize_return_path("/lobby?tab=1#x"),
            Some("/lobby?tab=1".into())
        );
        for bad in [
            "https://evil.example",
            "//evil.example",
            "/\\evil.example",
            "javascript:alert(1)",
            "lobby",
            "",
            "/a b",
            "/a\r\nSet-Cookie: x=1",
        ] {
            assert_eq!(sanitize_return_path(bad), None, "{bad:?}");
        }
    }

    #[tokio::test]
    async fn auth_url_has_exact_redirect_uri_pkce_and_no_popup() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let service = FaceitOAuthService::new(test_config("http://unused"), pool);
        let (url, pending) = service
            .generate_auth_url(None, Some("/lobby".into()))
            .await
            .unwrap();
        let p = params_of(&url);
        assert_eq!(p["redirect_uri"], REDIRECT);
        assert_eq!(p["response_type"], "code");
        assert_eq!(p["code_challenge_method"], "S256");
        assert_eq!(
            p["code_challenge"],
            generate_code_challenge(&pending.code_verifier)
        );
        assert!(!p.contains_key("redirect_popup"));
    }

    #[tokio::test]
    async fn state_is_persisted_hashed_and_survives_restart() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let a = FaceitOAuthService::new(test_config("http://unused"), pool.clone());
        let (url, _) = a
            .generate_auth_url(None, Some("/lobby".into()))
            .await
            .unwrap();
        let state = params_of(&url)["state"].clone();
        drop(a); // simulated backend restart: new service instance, same database

        let raw: i64 =
            sqlx::query_scalar("SELECT count(*) FROM faceit_oauth_states WHERE state_hash = $1")
                .bind(&state)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(raw, 0, "plain state must not be stored");

        let b = FaceitOAuthService::new(test_config("http://unused"), pool);
        let pending = b.consume_state(&state).await.unwrap();
        assert_eq!(pending.return_to.as_deref(), Some("/lobby"));
    }

    #[tokio::test]
    async fn state_is_single_use_and_expires() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let service = FaceitOAuthService::new(test_config("http://unused"), pool.clone());

        let (url, _) = service.generate_auth_url(None, None).await.unwrap();
        let state = params_of(&url)["state"].clone();
        assert!(service.consume_state(&state).await.is_ok());
        assert!(
            service.consume_state(&state).await.is_err(),
            "replay must fail"
        );

        let (url, _) = service.generate_auth_url(None, None).await.unwrap();
        let state = params_of(&url)["state"].clone();
        sqlx::query(
            "UPDATE faceit_oauth_states SET expires_at = CURRENT_TIMESTAMP - interval '1 second'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            service.consume_state(&state).await.is_err(),
            "expired state must fail"
        );
        assert!(service.consume_state("never-issued").await.is_err());
    }

    #[tokio::test]
    async fn token_exchange_succeeds_against_mock_and_sends_standard_basic_auth() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let (base, seen) = mock_faceit(200).await;
        let service = FaceitOAuthService::new(test_config(&base), pool);
        let (url, pending) = service
            .generate_auth_url(None, Some("/lobby".into()))
            .await
            .unwrap();
        let state = params_of(&url)["state"].clone();

        let (info, tokens, user, return_to) =
            service.handle_callback("the-code", &state).await.unwrap();
        assert_eq!(info.nickname, "tester");
        assert_eq!(tokens.access_token, "at");
        assert!(user.is_none());
        assert_eq!(return_to.as_deref(), Some("/lobby"));

        let req = seen.lock().unwrap()[0].clone();
        let expected = format!(
            "authorization: Basic {}",
            encode_basic_auth("client", "not-a-real-secret")
        );
        assert!(req.to_lowercase().contains(&expected.to_lowercase()));
        assert!(req.contains("code=the-code"));
        assert!(req.contains(&format!("code_verifier={}", pending.code_verifier)));
        assert!(
            req.contains("redirect_uri=https%3A%2F%2Fwww.example.test%2Fauth%2Ffaceit%2Fcallback")
        );
    }

    #[tokio::test]
    async fn failed_token_exchange_is_an_error_and_burns_the_state() {
        let Some(pool) = test_pool().await else {
            return;
        };
        let (base, _) = mock_faceit(400).await;
        let service = FaceitOAuthService::new(test_config(&base), pool);
        let (url, _) = service.generate_auth_url(None, None).await.unwrap();
        let state = params_of(&url)["state"].clone();
        let err = service
            .handle_callback("bad", &state)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            !err.contains("invalid_grant"),
            "body must not leak into errors"
        );
        assert!(service.consume_state(&state).await.is_err());
    }

    #[tokio::test]
    async fn save_faceit_link_error_is_surfaced_and_rolled_back() {
        let Some(pool) = test_pool().await else {
            return;
        };
        // faceit_links/users tables intentionally missing -> must be an Err, not swallowed
        let service = FaceitOAuthService::new(test_config("http://unused"), pool.clone());
        let info = FaceitUserInfo {
            guid: "g".into(),
            nickname: "n".into(),
            email: None,
            picture: None,
        };
        let tokens = FaceitTokenResponse {
            access_token: "a".into(),
            id_token: "i".into(),
            refresh_token: "r".into(),
            expires_in: 60,
            token_type: "Bearer".into(),
        };
        assert!(service
            .save_faceit_link(&Uuid::new_v4().to_string(), &info, &tokens)
            .await
            .is_err());

        create_link_tables(&pool).await;
        let uid = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id) VALUES ($1)")
            .bind(uid)
            .execute(&pool)
            .await
            .unwrap();
        service
            .save_faceit_link(&uid.to_string(), &info, &tokens)
            .await
            .unwrap();
        let status = service.get_link_status(&uid.to_string()).await.unwrap();
        assert!(status.linked);
        assert_eq!(status.faceit_nickname.as_deref(), Some("n"));
    }
}
