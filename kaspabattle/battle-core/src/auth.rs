use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use regex::Regex;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::faceit::FaceitLink;
use crate::models::user::{AuthResponse, LoginRequest, RegisterRequest, User};
use crate::constants::session_lifetime_days;

pub struct AuthService {
    db: PgPool,
}

impl AuthService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub fn validate_email(email: &str) -> Result<String> {
        let re = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")?;
        if !re.is_match(email) {
            return Err(anyhow!("Invalid email format"));
        }
        Ok(email.to_lowercase().trim().to_string())
    }

    pub fn validate_password(password: &str) -> Result<()> {
        if password.len() < 8 {
            return Err(anyhow!("Password must be at least 8 characters"));
        }
        Ok(())
    }

    pub fn validate_display_name(name: &str) -> Result<String> {
        let re = Regex::new(r"^[a-zA-Z0-9_-]{3,30}$")?;
        if !re.is_match(name) {
            return Err(anyhow!("Display name must be 3-30 characters and contain only alphanumeric characters, underscores, or hyphens"));
        }
        Ok(name.trim().to_string())
    }

    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?
            .to_string();
        Ok(password_hash)
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| anyhow!("Hash parse failed: {}", e))?;
        let is_valid = Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();
        Ok(is_valid)
    }

    pub fn generate_session_token() -> String {
        let random_bytes: [u8; 32] = rand::thread_rng().gen();
        general_purpose::URL_SAFE_NO_PAD.encode(random_bytes)
    }

    pub async fn register(&self, req: RegisterRequest) -> Result<AuthResponse> {
        let email = Self::validate_email(&req.email)?;
        Self::validate_password(&req.password)?;
        let display_name = Self::validate_display_name(&req.display_name)?;
        let password_hash = Self::hash_password(&req.password)?;

        let user_id = Uuid::new_v4();

        let res = sqlx::query(
            "INSERT INTO users (id, email, password_hash, display_name) VALUES ($1, $2, $3, $4)",
        )
        .bind(&user_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(&display_name)
        .execute(&self.db)
        .await;

        if let Err(e) = res {
            if e.to_string().contains("unique constraint") {
                return Err(anyhow!("An account with this email already exists"));
            }
            return Err(e.into());
        }

        let session_token = Self::generate_session_token();
        let expires_at = Utc::now() + Duration::days(session_lifetime_days());

        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(&session_token)
            .bind(&user_id)
            .bind(&expires_at)
            .execute(&self.db)
            .await?;

        Ok(AuthResponse {
            user_id,
            display_name,
            session_token,
            expires_at: expires_at.to_rfc3339(),
        })
    }

    pub async fn login(&self, req: LoginRequest) -> Result<AuthResponse> {
        let email = req.email.to_lowercase().trim().to_string();

        let row = sqlx::query("SELECT id, display_name, password_hash FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(&self.db)
            .await?;

        let (user_id, display_name, password_hash): (Uuid, String, String) = if let Some(r) = row {
            (
                r.try_get("id")?,
                r.try_get("display_name")?,
                r.try_get("password_hash")?,
            )
        } else {
            return Err(anyhow!("Invalid credentials"));
        };

        if !Self::verify_password(&req.password, &password_hash)? {
            return Err(anyhow!("Invalid credentials"));
        }

        sqlx::query("UPDATE users SET last_login_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(&user_id)
            .execute(&self.db)
            .await?;

        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(&user_id)
            .execute(&self.db)
            .await?;

        let session_token = Self::generate_session_token();
        let expires_at = Utc::now() + Duration::days(session_lifetime_days());

        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(&session_token)
            .bind(&user_id)
            .bind(&expires_at)
            .execute(&self.db)
            .await?;

        Ok(AuthResponse {
            user_id,
            display_name,
            session_token,
            expires_at: expires_at.to_rfc3339(),
        })
    }

    pub async fn validate_session(&self, token: &str) -> Result<User> {
        let row = sqlx::query(
            "SELECT u.id, u.email, u.email_verified, u.password_hash, u.display_name, u.kaspa_address, u.created_at, u.updated_at, u.last_login_at 
             FROM sessions s 
             JOIN users u ON s.user_id = u.id 
             WHERE s.id = $1 AND s.expires_at > CURRENT_TIMESTAMP"
        )
        .bind(token)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let id: Uuid = r.try_get("id")?;
            let created_at: DateTime<Utc> = r.try_get("created_at")?;
            let updated_at: DateTime<Utc> = r.try_get("updated_at")?;
            let last_login_at: Option<DateTime<Utc>> = r.try_get("last_login_at")?;

            Ok(User {
                id,
                email: r.try_get("email")?,
                email_verified: r.try_get("email_verified")?,
                password_hash: r.try_get("password_hash")?,
                display_name: r.try_get("display_name")?,
                kaspa_address: r.try_get("kaspa_address")?,
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
                last_login_at: last_login_at.map(|dt| dt.to_rfc3339()),
            })
        } else {
            Err(anyhow!("Session invalid or expired"))
        }
    }

    pub async fn logout(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(token)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn set_kaspa_address(&self, user_id: &str, address: &str) -> Result<()> {
        if !address.starts_with("kaspa:") && !address.starts_with("kaspatest:") {
            return Err(anyhow!("Invalid Kaspa address"));
        }

        let uid = Uuid::parse_str(user_id)?;

        let affected = sqlx::query(
            "UPDATE users SET kaspa_address = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(address)
        .bind(uid)
        .execute(&self.db)
        .await?
        .rows_affected();

        if affected == 0 {
            Err(anyhow!("User not found"))
        } else {
            Ok(())
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<User> {
        let uid = Uuid::parse_str(user_id)?;

        let row = sqlx::query(
            "SELECT id, email, email_verified, password_hash, display_name, kaspa_address, created_at, updated_at, last_login_at 
             FROM users WHERE id = $1"
        )
        .bind(uid)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let id: Uuid = r.try_get("id")?;
            let created_at: DateTime<Utc> = r.try_get("created_at")?;
            let updated_at: DateTime<Utc> = r.try_get("updated_at")?;
            let last_login_at: Option<DateTime<Utc>> = r.try_get("last_login_at")?;

            Ok(User {
                id,
                email: r.try_get("email")?,
                email_verified: r.try_get("email_verified")?,
                password_hash: r.try_get("password_hash")?,
                display_name: r.try_get("display_name")?,
                kaspa_address: r.try_get("kaspa_address")?,
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
                last_login_at: last_login_at.map(|dt| dt.to_rfc3339()),
            })
        } else {
            Err(anyhow!("User not found"))
        }
    }

    pub async fn get_faceit_link(&self, user_id: &str) -> Result<Option<FaceitLink>> {
        let uid = Uuid::parse_str(user_id)?;
        let row = sqlx::query(
            "SELECT id, user_id, faceit_player_id, faceit_nickname, faceit_elo, faceit_skill_level, faceit_avatar_url, linked_at, verified 
             FROM faceit_links WHERE user_id = $1"
        )
        .bind(uid)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let id: Uuid = r.try_get("id")?;
            let user_id_res: Uuid = r.try_get("user_id")? ;
            let linked_at: DateTime<Utc> = r.try_get("linked_at")?;
            let elo: Option<i32> = r.try_get("faceit_elo")?;
            let skill: Option<i32> = r.try_get("faceit_skill_level")?;

            Ok(Some(FaceitLink {
                id,
                user_id: user_id_res,
                faceit_player_id: r.try_get("faceit_player_id")?,
                faceit_nickname: r.try_get("faceit_nickname")?,
                faceit_elo: elo,
                faceit_skill_level: skill,
                faceit_avatar_url: r.try_get("faceit_avatar_url")?,
                linked_at: linked_at.to_rfc3339(),
                verified: r.try_get("verified")?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn save_faceit_snapshot(
        &self,
        user_id: &str,
        faceit_player_id: &str,
        game_id: &str,
        elo: i32,
        skill_level: i32,
    ) -> Result<()> {
        let uid = Uuid::parse_str(user_id)?;
        let snapshot_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO faceit_stats_snapshots (id, user_id, faceit_player_id, game_id, elo, skill_level) 
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(snapshot_id)
        .bind(uid)
        .bind(faceit_player_id)
        .bind(game_id)
        .bind(elo)
        .bind(skill_level)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_latest_faceit_snapshot(
        &self,
        faceit_player_id: &str,
        game_id: &str,
    ) -> Result<Option<crate::models::faceit_data::FaceitStatsSnapshot>> {
        let row = sqlx::query(
            "SELECT id, user_id, faceit_player_id, game_id, elo, skill_level, snapshot_at
             FROM faceit_stats_snapshots 
             WHERE faceit_player_id = $1 AND game_id = $2
             ORDER BY snapshot_at DESC LIMIT 1",
        )
        .bind(faceit_player_id)
        .bind(game_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let id: Uuid = r.try_get("id")?;
            let user_id: Uuid = r.try_get("user_id")?;
            let snapshot_at: DateTime<Utc> = r.try_get("snapshot_at")?;

            Ok(Some(crate::models::faceit_data::FaceitStatsSnapshot {
                id,
                user_id,
                faceit_player_id: r.try_get("faceit_player_id")?,
                game_id: r.try_get("game_id")?,
                elo: r.try_get("elo")?,
                skill_level: r.try_get("skill_level")?,
                snapshot_at: snapshot_at.to_rfc3339(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let res = sqlx::query("DELETE FROM sessions WHERE expires_at < CURRENT_TIMESTAMP")
            .execute(&self.db)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn issue_session_for_user(&self, user_id: &Uuid) -> Result<String> {
        let session_token = Self::generate_session_token();
        let expires_at = Utc::now() + Duration::days(session_lifetime_days());

        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(&session_token)
            .bind(user_id)
            .bind(&expires_at)
            .execute(&self.db)
            .await?;

        Ok(session_token)
    }

    pub async fn handle_faceit_sso(
        &self,
        info: &crate::models::faceit::FaceitUserInfo,
        preferred_user_id: Option<&str>,
    ) -> Result<(String, String)> {
        let user_id: Uuid = if let Some(preferred_user_id) = preferred_user_id {
            let preferred_uuid = Uuid::parse_str(preferred_user_id)?;

            sqlx::query("SELECT id FROM users WHERE id = $1")
                .bind(preferred_uuid)
                .fetch_one(&self.db)
                .await?;

            preferred_uuid
        } else {
            let row = sqlx::query("SELECT user_id FROM faceit_links WHERE faceit_player_id = $1")
                .bind(&info.guid)
                .fetch_optional(&self.db)
                .await?;

            if let Some(r) = row {
                r.try_get("user_id")?
            } else {
                let new_user_id = Uuid::new_v4();
                let email = info
                    .email
                    .clone()
                    .unwrap_or_else(|| format!("{}@faceit.local", info.guid));
                let display_name = info.nickname.clone();
                let dummy_pass = Self::hash_password(&Uuid::new_v4().to_string())?;

                let res = sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES ($1, $2, $3, $4)")
                    .bind(&new_user_id)
                    .bind(&email)
                    .bind(&dummy_pass)
                    .bind(&display_name)
                    .execute(&self.db)
                    .await;

                if res.is_err() {
                    let fallback_email = format!(
                        "{}-{}@faceit.local",
                        info.guid,
                        &Uuid::new_v4().to_string()[..6]
                    );
                    sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES ($1, $2, $3, $4)")
                        .bind(&new_user_id)
                        .bind(&fallback_email)
                        .bind(&dummy_pass)
                        .bind(&display_name)
                        .execute(&self.db)
                        .await?;
                }
                new_user_id
            }
        };

        let session_token = self.issue_session_for_user(&user_id).await?;

        Ok((user_id.to_string(), session_token))
    }
}
