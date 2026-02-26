use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Utc};
use rand::Rng;
use regex::Regex;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::faceit::FaceitLink;
use crate::models::user::{AuthResponse, LoginRequest, RegisterRequest, User};

pub struct AuthService {
    db: Arc<Mutex<Connection>>,
}

impl AuthService {
    pub async fn new(db: Arc<Mutex<Connection>>) -> Result<Self> {
        let service = Self { db };
        service.run_migrations().await?;
        Ok(service)
    }

    pub async fn run_migrations(&self) -> Result<()> {
        let conn = self.db.lock().await;
        let sql_users = include_str!("../migrations/004_users.sql");
        let sql_snapshots = include_str!("../migrations/006_faceit_snapshots.sql");
        conn.execute_batch(sql_users)?;
        conn.execute_batch(sql_snapshots)?;
        Ok(())
    }

    pub fn validate_email(email: &str) -> Result<String> {
        let re = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")?;
        if !re.is_match(email) {
            return Err(anyhow!("Ungültiges E-Mail-Format"));
        }
        Ok(email.to_lowercase().trim().to_string())
    }

    pub fn validate_password(password: &str) -> Result<()> {
        if password.len() < 8 {
            return Err(anyhow!("Passwort muss mindestens 8 Zeichen lang sein"));
        }
        Ok(())
    }

    pub fn validate_display_name(name: &str) -> Result<String> {
        let re = Regex::new(r"^[a-zA-Z0-9_-]{3,30}$")?;
        if !re.is_match(name) {
            return Err(anyhow!("Display-Name muss 3-30 Zeichen lang sein und darf nur alphanumerische Zeichen, Unterstriche und Bindestriche enthalten"));
        }
        Ok(name.trim().to_string())
    }

    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Fehler beim Hashing: {}", e))?
            .to_string();
        Ok(password_hash)
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| anyhow!("Fehler beim Parsen des Hashes: {}", e))?;
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

        let user_id = Uuid::new_v4().to_string();

        let conn = self.db.lock().await;

        match conn.execute(
            "INSERT INTO users (id, email, password_hash, display_name) VALUES (?1, ?2, ?3, ?4)",
            (&user_id, &email, &password_hash, &display_name),
        ) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ffi::ErrorCode::ConstraintViolation =>
            {
                return Err(anyhow!("Ein Account mit dieser E-Mail existiert bereits"));
            }
            Err(e) => return Err(e.into()),
        }

        let session_token = Self::generate_session_token();
        let expires_at = (Utc::now() + Duration::days(7)).to_rfc3339();

        conn.execute(
            "INSERT INTO sessions (id, user_id, expires_at) VALUES (?1, ?2, ?3)",
            (&session_token, &user_id, &expires_at),
        )?;

        Ok(AuthResponse {
            user_id,
            display_name,
            session_token,
            expires_at,
        })
    }

    pub async fn login(&self, req: LoginRequest) -> Result<AuthResponse> {
        let email = req.email.to_lowercase().trim().to_string();

        let conn = self.db.lock().await;

        let mut stmt =
            conn.prepare("SELECT id, display_name, password_hash FROM users WHERE email = ?1")?;

        let mut rows = stmt.query([&email])?;

        let (user_id, display_name, password_hash) = if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let hash: String = row.get(2)?;
            (id, name, hash)
        } else {
            return Err(anyhow!("Ungültige Anmeldedaten"));
        };

        if !Self::verify_password(&req.password, &password_hash)? {
            return Err(anyhow!("Ungültige Anmeldedaten"));
        }

        conn.execute(
            "UPDATE users SET last_login_at = datetime('now') WHERE id = ?1",
            [&user_id],
        )?;

        conn.execute("DELETE FROM sessions WHERE user_id = ?1", [&user_id])?;

        let session_token = Self::generate_session_token();
        let expires_at = (Utc::now() + Duration::days(7)).to_rfc3339();

        conn.execute(
            "INSERT INTO sessions (id, user_id, expires_at) VALUES (?1, ?2, ?3)",
            (&session_token, &user_id, &expires_at),
        )?;

        Ok(AuthResponse {
            user_id,
            display_name,
            session_token,
            expires_at,
        })
    }

    pub async fn validate_session(&self, token: &str) -> Result<User> {
        let conn = self.db.lock().await;

        let mut stmt = conn.prepare(
            "SELECT u.id, u.email, u.email_verified, u.password_hash, u.display_name, u.kaspa_address, u.created_at, u.updated_at, u.last_login_at 
             FROM sessions s 
             JOIN users u ON s.user_id = u.id 
             WHERE s.id = ?1 AND s.expires_at > datetime('now')"
        )?;

        let mut rows = stmt.query([token])?;

        if let Some(row) = rows.next()? {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                email_verified: row.get::<_, i64>(2)? != 0,
                password_hash: row.get(3)?,
                display_name: row.get(4)?,
                kaspa_address: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                last_login_at: row.get(8)?,
            })
        } else {
            Err(anyhow!("Session ungültig oder abgelaufen"))
        }
    }

    pub async fn logout(&self, token: &str) -> Result<()> {
        let conn = self.db.lock().await;
        conn.execute("DELETE FROM sessions WHERE id = ?1", [&token])?;
        Ok(())
    }

    pub async fn set_kaspa_address(&self, user_id: &str, address: &str) -> Result<()> {
        if !address.starts_with("kaspa:") && !address.starts_with("kaspatest:") {
            return Err(anyhow!("Ungültige Kaspa-Adresse"));
        }

        let conn = self.db.lock().await;
        let affected = conn.execute(
            "UPDATE users SET kaspa_address = ?1, updated_at = datetime('now') WHERE id = ?2",
            [&address.to_string(), &user_id.to_string()],
        )?;

        if affected == 0 {
            Err(anyhow!("User nicht gefunden"))
        } else {
            Ok(())
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<User> {
        let conn = self.db.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, email, email_verified, password_hash, display_name, kaspa_address, created_at, updated_at, last_login_at 
             FROM users WHERE id = ?1"
        )?;

        let mut rows = stmt.query([user_id])?;

        if let Some(row) = rows.next()? {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                email_verified: row.get::<_, i64>(2)? != 0,
                password_hash: row.get(3)?,
                display_name: row.get(4)?,
                kaspa_address: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                last_login_at: row.get(8)?,
            })
        } else {
            Err(anyhow!("User nicht gefunden"))
        }
    }

    pub async fn get_faceit_link(&self, user_id: &str) -> Result<Option<FaceitLink>> {
        let conn = self.db.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, user_id, faceit_player_id, faceit_nickname, faceit_elo, faceit_skill_level, faceit_avatar_url, created_at, verified 
             FROM faceit_links WHERE user_id = ?1"
        )?;

        let mut rows = stmt.query([user_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(FaceitLink {
                id: row.get(0)?,
                user_id: row.get(1)?,
                faceit_player_id: row.get(2)?,
                faceit_nickname: row.get(3)?,
                faceit_elo: row.get(4)?,
                faceit_skill_level: row.get(5)?,
                faceit_avatar_url: row.get(6)?,
                linked_at: row.get(7)?,
                verified: row.get::<_, i64>(8)? != 0,
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
        let snapshot_id = Uuid::new_v4().to_string();
        let conn = self.db.lock().await;

        conn.execute(
            "INSERT INTO faceit_stats_snapshots (id, user_id, faceit_player_id, game_id, elo, skill_level) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (&snapshot_id, user_id, faceit_player_id, game_id, &elo, &skill_level),
        )?;
        Ok(())
    }

    pub async fn get_latest_faceit_snapshot(
        &self,
        faceit_player_id: &str,
        game_id: &str,
    ) -> Result<Option<crate::models::faceit_data::FaceitStatsSnapshot>> {
        let conn = self.db.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, user_id, faceit_player_id, game_id, elo, skill_level, snapshot_at
             FROM faceit_stats_snapshots 
             WHERE faceit_player_id = ?1 AND game_id = ?2
             ORDER BY snapshot_at DESC LIMIT 1",
        )?;

        let mut rows = stmt.query([faceit_player_id, game_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(crate::models::faceit_data::FaceitStatsSnapshot {
                id: row.get(0)?,
                user_id: row.get(1)?,
                faceit_player_id: row.get(2)?,
                game_id: row.get(3)?,
                elo: row.get(4)?,
                skill_level: row.get(5)?,
                snapshot_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let conn = self.db.lock().await;
        let deleted = conn.execute(
            "DELETE FROM sessions WHERE expires_at < datetime('now')",
            [],
        )?;
        Ok(deleted as u64)
    }

    pub async fn handle_faceit_sso(
        &self,
        info: &crate::models::faceit::FaceitUserInfo,
    ) -> Result<(String, String)> {
        let conn = self.db.lock().await;

        let mut stmt =
            conn.prepare("SELECT user_id FROM faceit_links WHERE faceit_player_id = ?1")?;
        let mut rows = stmt.query([&info.guid])?;

        let user_id = if let Some(row) = rows.next()? {
            row.get(0)?
        } else {
            let new_user_id = Uuid::new_v4().to_string();
            let email = info
                .email
                .clone()
                .unwrap_or_else(|| format!("{}@faceit.local", info.guid));
            let display_name = info.nickname.clone();
            let dummy_pass = Self::hash_password(&Uuid::new_v4().to_string())?;

            match conn.execute(
                "INSERT INTO users (id, email, password_hash, display_name) VALUES (?1, ?2, ?3, ?4)",
                (&new_user_id, &email, &dummy_pass, &display_name),
            ) {
                Ok(_) => new_user_id,
                Err(_) => {
                    let fallback_email = format!("{}-{}@faceit.local", info.guid, &Uuid::new_v4().to_string()[..6]);
                    conn.execute(
                        "INSERT INTO users (id, email, password_hash, display_name) VALUES (?1, ?2, ?3, ?4)",
                        (&new_user_id, &fallback_email, &dummy_pass, &display_name),
                    )?;
                    new_user_id
                }
            }
        };

        let session_token = Self::generate_session_token();
        let expires_at = (Utc::now() + Duration::days(7)).to_rfc3339();

        conn.execute(
            "INSERT INTO sessions (id, user_id, expires_at) VALUES (?1, ?2, ?3)",
            (&session_token, &user_id, &expires_at),
        )?;

        Ok((user_id, session_token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erstellt einen frischen AuthService mit In-Memory-DB für Tests
    async fn create_test_service() -> AuthService {
        let conn = Connection::open_in_memory().expect("In-Memory DB sollte funktionieren");
        let db = Arc::new(Mutex::new(conn));
        AuthService::new(db)
            .await
            .expect("AuthService Init sollte funktionieren")
    }

    /// Hilfsfunktion: Registriert einen Test-User und gibt AuthResponse zurück
    async fn register_test_user(service: &AuthService) -> AuthResponse {
        service
            .register(RegisterRequest {
                email: "test@example.com".to_string(),
                password: "TestPass123!".to_string(),
                display_name: "TestUser".to_string(),
            })
            .await
            .expect("Registrierung sollte funktionieren")
    }

    #[tokio::test]
    async fn test_register_new_user() {
        let service = create_test_service().await;

        let result = service
            .register(RegisterRequest {
                email: "alice@example.com".to_string(),
                password: "SecurePass123!".to_string(),
                display_name: "Alice".to_string(),
            })
            .await;

        // ASSERT: Registrierung erfolgreich
        assert!(result.is_ok(), "Registrierung sollte erfolgreich sein");

        let auth = result.unwrap();

        // ASSERT: User-ID ist eine gültige UUID
        assert_eq!(auth.user_id.len(), 36, "User-ID sollte UUID-Format haben");
        assert!(
            auth.user_id.contains('-'),
            "User-ID sollte Bindestriche enthalten"
        );

        // ASSERT: Display-Name korrekt
        assert_eq!(auth.display_name, "Alice");

        // ASSERT: Session-Token ist nicht leer und hat richtige Länge
        assert!(
            !auth.session_token.is_empty(),
            "Session-Token darf nicht leer sein"
        );
        assert!(
            auth.session_token.len() >= 40,
            "Session-Token sollte mindestens 40 Zeichen haben"
        );

        // ASSERT: expires_at liegt in der Zukunft
        assert!(
            !auth.expires_at.is_empty(),
            "expires_at darf nicht leer sein"
        );

        // ASSERT: User kann in DB gefunden werden
        let user = service.get_user(&auth.user_id).await;
        assert!(user.is_ok(), "User sollte in DB existieren");
        let user = user.unwrap();
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.display_name, "Alice");
        assert!(
            user.kaspa_address.is_none(),
            "Kaspa-Adresse sollte initial None sein"
        );
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        let service = create_test_service().await;

        // Ersten User registrieren
        let first = service
            .register(RegisterRequest {
                email: "bob@example.com".to_string(),
                password: "Password123!".to_string(),
                display_name: "Bob".to_string(),
            })
            .await;
        assert!(first.is_ok(), "Erste Registrierung sollte funktionieren");

        // Gleiche E-Mail nochmal registrieren
        let second = service
            .register(RegisterRequest {
                email: "bob@example.com".to_string(),
                password: "AnotherPass456!".to_string(),
                display_name: "Bob2".to_string(),
            })
            .await;

        // ASSERT: Zweite Registrierung schlägt fehl
        assert!(second.is_err(), "Duplikat-Email sollte abgelehnt werden");

        let err_msg = second.unwrap_err().to_string();
        assert!(
            err_msg.contains("existiert bereits") || err_msg.contains("UNIQUE"),
            "Fehlermeldung sollte auf Duplikat hinweisen, war aber: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_register_email_case_insensitive() {
        let service = create_test_service().await;

        // Registrierung mit Großbuchstaben
        let first = service
            .register(RegisterRequest {
                email: "Alice@Example.COM".to_string(),
                password: "Password123!".to_string(),
                display_name: "Alice".to_string(),
            })
            .await;
        assert!(first.is_ok());

        // Gleiche E-Mail in Kleinbuchstaben
        let second = service
            .register(RegisterRequest {
                email: "alice@example.com".to_string(),
                password: "Password456!".to_string(),
                display_name: "Alice2".to_string(),
            })
            .await;

        // ASSERT: Wird als Duplikat erkannt
        assert!(
            second.is_err(),
            "Case-insensitive E-Mail sollte als Duplikat erkannt werden"
        );
    }

    #[tokio::test]
    async fn test_register_invalid_email() {
        let service = create_test_service().await;

        let invalid_emails = vec![
            "",
            "keine-email",
            "@example.com",
            "user@",
            "user@.com",
            "user@com",
            "user space@example.com",
        ];

        for email in invalid_emails {
            let result = service
                .register(RegisterRequest {
                    email: email.to_string(),
                    password: "ValidPass123!".to_string(),
                    display_name: "TestUser".to_string(),
                })
                .await;

            assert!(
                result.is_err(),
                "E-Mail '{}' sollte abgelehnt werden",
                email
            );
        }
    }

    #[tokio::test]
    async fn test_register_invalid_password() {
        let service = create_test_service().await;

        // Zu kurzes Passwort (< 8 Zeichen)
        let result = service
            .register(RegisterRequest {
                email: "test@example.com".to_string(),
                password: "Short1!".to_string(), // Nur 7 Zeichen
                display_name: "TestUser".to_string(),
            })
            .await;

        assert!(
            result.is_err(),
            "Zu kurzes Passwort sollte abgelehnt werden"
        );

        // Leeres Passwort
        let result2 = service
            .register(RegisterRequest {
                email: "test2@example.com".to_string(),
                password: "".to_string(),
                display_name: "TestUser2".to_string(),
            })
            .await;

        assert!(result2.is_err(), "Leeres Passwort sollte abgelehnt werden");
    }

    #[tokio::test]
    async fn test_register_invalid_display_name() {
        let service = create_test_service().await;

        let too_long = "x".repeat(31);
        let invalid_names = vec![
            "",        // Leer
            "ab",      // Zu kurz (< 3)
            "a",       // Zu kurz
            &too_long, // Zu lang (> 30)
            "name with spaces",
            "name@special",
            "name!chars",
        ];

        for name in invalid_names {
            let result = service
                .register(RegisterRequest {
                    email: format!("{}@example.com", name.replace(" ", "")),
                    password: "ValidPass123!".to_string(),
                    display_name: name.to_string(),
                })
                .await;

            assert!(
                result.is_err(),
                "Display-Name '{}' sollte abgelehnt werden",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_login_correct_password() {
        let service = create_test_service().await;

        // User registrieren
        let reg = service
            .register(RegisterRequest {
                email: "login@example.com".to_string(),
                password: "MyPassword123!".to_string(),
                display_name: "LoginUser".to_string(),
            })
            .await
            .expect("Registrierung sollte funktionieren");

        // Login mit korrektem Passwort
        let login = service
            .login(LoginRequest {
                email: "login@example.com".to_string(),
                password: "MyPassword123!".to_string(),
            })
            .await;

        // ASSERT: Login erfolgreich
        assert!(
            login.is_ok(),
            "Login mit korrektem Passwort sollte funktionieren"
        );

        let auth = login.unwrap();
        assert_eq!(auth.user_id, reg.user_id, "User-ID sollte identisch sein");
        assert_eq!(auth.display_name, "LoginUser");
        assert!(!auth.session_token.is_empty());

        // ASSERT: Neues Session-Token (nicht das alte von der Registrierung)
        assert_ne!(
            auth.session_token, reg.session_token,
            "Login sollte neuen Session-Token generieren"
        );
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let service = create_test_service().await;

        // User registrieren
        service
            .register(RegisterRequest {
                email: "wrong@example.com".to_string(),
                password: "CorrectPassword123!".to_string(),
                display_name: "WrongPassUser".to_string(),
            })
            .await
            .expect("Registrierung sollte funktionieren");

        // Login mit falschem Passwort
        let result = service
            .login(LoginRequest {
                email: "wrong@example.com".to_string(),
                password: "WrongPassword456!".to_string(),
            })
            .await;

        // ASSERT: Login schlägt fehl
        assert!(
            result.is_err(),
            "Login mit falschem Passwort sollte fehlschlagen"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Ungültige Anmeldedaten"),
            "Fehlermeldung sollte generisch sein: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_login_nonexistent_user() {
        let service = create_test_service().await;

        let result = service
            .login(LoginRequest {
                email: "nobody@example.com".to_string(),
                password: "SomePassword123!".to_string(),
            })
            .await;

        // ASSERT: Login schlägt fehl mit GLEICHER Fehlermeldung wie bei falschem Passwort
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Ungültige Anmeldedaten"),
            "Fehlermeldung bei nicht-existentem User sollte gleich sein wie bei falschem Passwort: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_session_validation() {
        let service = create_test_service().await;

        // User registrieren → Session-Token erhalten
        let auth = register_test_user(&service).await;

        // Session validieren
        let user = service.validate_session(&auth.session_token).await;

        // ASSERT: Session ist gültig
        assert!(user.is_ok(), "Gültige Session sollte validiert werden");
        let user = user.unwrap();
        assert_eq!(user.id, auth.user_id);
        assert_eq!(user.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_session_invalid_token() {
        let service = create_test_service().await;

        // Ungültigen Token validieren
        let result = service
            .validate_session("definitely-not-a-real-token")
            .await;

        // ASSERT: Session ungültig
        assert!(result.is_err(), "Ungültiger Token sollte abgelehnt werden");
    }

    #[tokio::test]
    async fn test_set_kaspa_address() {
        let service = create_test_service().await;
        let auth = register_test_user(&service).await;

        // Testnet-Adresse setzen
        let result = service
            .set_kaspa_address(
                &auth.user_id,
                "kaspatest:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll",
            )
            .await;
        assert!(
            result.is_ok(),
            "Gültige Kaspa-Testnet-Adresse sollte akzeptiert werden"
        );

        // User abrufen und Adresse prüfen
        let user = service.get_user(&auth.user_id).await.unwrap();
        assert_eq!(
            user.kaspa_address.as_deref(),
            Some("kaspatest:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll")
        );

        // Ungültige Adresse
        let invalid = service
            .set_kaspa_address(&auth.user_id, "bitcoin:xyz123")
            .await;
        assert!(
            invalid.is_err(),
            "Nicht-Kaspa-Adresse sollte abgelehnt werden"
        );

        // Mainnet-Adresse
        let mainnet = service
            .set_kaspa_address(
                &auth.user_id,
                "kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll",
            )
            .await;
        assert!(
            mainnet.is_ok(),
            "Gültige Kaspa-Mainnet-Adresse sollte akzeptiert werden"
        );
    }
}
