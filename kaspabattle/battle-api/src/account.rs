//! E-mail / password accounts (Free Play sign-up) on top of the existing `users` + `sessions` tables.
//!
//! Security properties implemented here (and covered by tests):
//! * Argon2id with explicit OWASP-minimum parameters, per-password random salt, PHC strings and
//!   transparent re-hash on login when the parameters change.
//! * Tokens (verification / reset) are 256-bit random, stored only as SHA-256, single use, expiring.
//! * No user enumeration: registration, resend, forgot-password and login answer identically for
//!   unknown and known e-mail addresses (a dummy hash keeps timing comparable).
//! * Password reset revokes every session; password change revokes all but the current one.
//! * Per-IP and per-account sliding-window throttling with progressive delay on failures.
//! * Nothing secret (password, token, e-mail) is logged; the audit trail stores a salted IP hash.

use crate::mail::{password_reset_mail, verification_mail, Mailer};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const NEWSLETTER_CONSENT_VERSION: &str = "2026-10-v1";
const VERIFICATION_TTL_HOURS: i64 = 24;
const RESET_TTL_MINUTES: i64 = 60;
const MAX_SESSIONS_PER_USER: i64 = 10;

// Argon2id, OWASP minimum profile (19 MiB, t=2, p=1). Raise here; old hashes upgrade on next login.
const ARGON_M_KIB: u32 = 19_456;
const ARGON_T: u32 = 2;
const ARGON_P: u32 = 1;

// ── Configuration & shared runtime ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct AccountConfig {
    /// `REQUIRE_EMAIL_VERIFICATION`: login only after the e-mail address is confirmed.
    pub require_email_verification: bool,
    /// Base URL used in mail links (`PUBLIC_APP_URL`).
    pub public_app_url: String,
    pub mail_available: bool,
    /// Salt for the audit IP hash.
    pub audit_salt: String,
}

impl AccountConfig {
    /// `REQUIRE_EMAIL_VERIFICATION` unset → on iff SMTP is configured (never lock people out when
    /// no mail can be delivered). An explicit value always wins and is switchable without code changes.
    pub fn from_env(mail_available: bool, get: impl Fn(&str) -> Option<String>) -> Self {
        let explicit = get("REQUIRE_EMAIL_VERIFICATION").map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        });
        let public_app_url = get("PUBLIC_APP_URL")
            .or_else(|| get("FRONTEND_URL"))
            .unwrap_or_else(|| "http://localhost:5173".into())
            .trim_end_matches('/')
            .to_string();
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        AccountConfig {
            require_email_verification: explicit.unwrap_or(mail_available),
            public_app_url,
            mail_available,
            audit_salt: get("AUDIT_IP_SALT").unwrap_or_else(|| URL_SAFE_NO_PAD.encode(salt)),
        }
    }
}

/// Sliding-window counters keyed by arbitrary strings (ip, account hash, …).
#[derive(Default)]
pub struct Throttle {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Throttle {
    /// A panic while holding the lock must not turn every later login attempt into a panic too
    /// (the map only holds counters, so recovering the inner value is safe).
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Instant>>> {
        self.hits.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records a hit and returns whether it is within `limit` per `window`.
    pub fn allow(&self, key: &str, limit: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = self.map();
        if map.len() > 20_000 {
            map.retain(|_, v| {
                v.last()
                    .map(|t| now.duration_since(*t) < Duration::from_secs(3600))
                    .unwrap_or(false)
            });
        }
        let v = map.entry(key.to_string()).or_default();
        v.retain(|t| now.duration_since(*t) < window);
        if v.len() >= limit {
            return false;
        }
        v.push(now);
        true
    }

    /// Number of hits currently inside the window (without recording one).
    pub fn count(&self, key: &str, window: Duration) -> usize {
        let now = Instant::now();
        let map = self.map();
        map.get(key)
            .map(|v| {
                v.iter()
                    .filter(|t| now.duration_since(**t) < window)
                    .count()
            })
            .unwrap_or(0)
    }

    pub fn record(&self, key: &str) {
        self.map()
            .entry(key.to_string())
            .or_default()
            .push(Instant::now());
    }

    pub fn clear(&self, key: &str) {
        self.map().remove(key);
    }
}

/// Everything the account handlers need besides the pool.
#[derive(Clone)]
pub struct AccountRuntime {
    pub cfg: Arc<AccountConfig>,
    pub mailer: Arc<dyn Mailer>,
    pub throttle: Arc<Throttle>,
    /// Bounds concurrent Argon2 work so a login flood cannot exhaust CPU/RAM.
    pub hash_slots: Arc<tokio::sync::Semaphore>,
}

impl AccountRuntime {
    pub fn new(cfg: AccountConfig, mailer: Arc<dyn Mailer>) -> Self {
        AccountRuntime {
            cfg: Arc::new(cfg),
            mailer,
            throttle: Arc::new(Throttle::default()),
            hash_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
}

// ── Validation (pure) ───────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FieldError {
    Required,
    TooShort,
    TooLong,
    InvalidCharacters,
    Reserved,
    NotAllowed,
    Invalid,
    TooCommon,
    ContainsPersonalData,
}

impl FieldError {
    pub fn code(&self) -> &'static str {
        match self {
            FieldError::Required => "required",
            FieldError::TooShort => "too_short",
            FieldError::TooLong => "too_long",
            FieldError::InvalidCharacters => "invalid_characters",
            FieldError::Reserved => "reserved",
            FieldError::NotAllowed => "not_allowed",
            FieldError::Invalid => "invalid",
            FieldError::TooCommon => "too_common",
            FieldError::ContainsPersonalData => "contains_personal_data",
        }
    }
}

/// Trim + lowercase. Deliberately *no* provider-specific folding (dots/plus-tags in Gmail etc.):
/// merging different addresses would be wrong.
pub fn normalize_email(raw: &str) -> Result<String, FieldError> {
    let e = raw.trim().to_ascii_lowercase();
    if e.is_empty() {
        return Err(FieldError::Required);
    }
    if e.len() > 254 {
        return Err(FieldError::TooLong);
    }
    if !e.is_ascii() || e.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(FieldError::InvalidCharacters);
    }
    let (local, domain) = e.split_once('@').ok_or(FieldError::Invalid)?;
    if local.is_empty() || local.len() > 64 || domain.contains('@') {
        return Err(FieldError::Invalid);
    }
    let labels: Vec<&str> = domain.split('.').collect();
    let domain_ok = labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .map(|t| t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphabetic()))
            .unwrap_or(false);
    let local_ok = local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._%+-'".contains(c))
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..");
    if !domain_ok || !local_ok {
        return Err(FieldError::Invalid);
    }
    // Synthetic addresses of wallet / FACEIT accounts must never be claimable.
    if domain == "wallet.local" || domain.ends_with(".local") {
        return Err(FieldError::NotAllowed);
    }
    Ok(e)
}

const RESERVED_NAMES: &[&str] = &[
    "admin",
    "administrator",
    "root",
    "support",
    "kaspabattle",
    "kaspa",
    "moderator",
    "mod",
    "system",
    "bot",
    "staff",
    "official",
    "faceit",
    "info",
    "null",
    "undefined",
    "anonymous",
    "guest",
    "owner",
    "team",
    "security",
    "help",
    "service",
    "api",
    "www",
    "mail",
    "postmaster",
    "webmaster",
    "freeplay",
    "computer",
    "ai",
    "everyone",
    "nobody",
];

// Only unambiguous severe terms; substring match on the normalised name keeps legit names usable.
const BLOCKED_SUBSTRINGS: &[&str] = &[
    "nigger",
    "nigga",
    "faggot",
    "hitler",
    "heilhitler",
    "nazi",
    "fotze",
    "hurensohn",
    "wichser",
    "arschloch",
    "rapist",
];

fn fold_confusables(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '_' && *c != '-')
        .map(|c| match c {
            '0' => 'o',
            '1' | '!' => 'i',
            '3' => 'e',
            '4' | '@' => 'a',
            '5' | '$' => 's',
            '7' => 't',
            _ => c,
        })
        .collect()
}

pub fn validate_username(raw: &str) -> Result<String, FieldError> {
    let u = raw.trim();
    if u.is_empty() {
        return Err(FieldError::Required);
    }
    if u.chars().count() < 3 {
        return Err(FieldError::TooShort);
    }
    if u.chars().count() > 24 {
        return Err(FieldError::TooLong);
    }
    // ASCII letters, digits, `_` and `-`; must start with a letter/digit. This excludes invisible
    // and control characters, bidi tricks and look-alike scripts by construction.
    let mut chars = u.chars();
    let first_ok = chars
        .next()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false);
    if !first_ok
        || !u
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(FieldError::InvalidCharacters);
    }
    let folded = fold_confusables(u);
    if RESERVED_NAMES.iter().any(|r| folded == *r)
        || folded.starts_with("player")  // wallet accounts are auto-named Player_xxxxxx
        || folded.starts_with("kaspabattle")
        || folded.starts_with("admin")
        || folded.starts_with("support")
    {
        return Err(FieldError::Reserved);
    }
    if BLOCKED_SUBSTRINGS.iter().any(|b| folded.contains(b)) {
        return Err(FieldError::NotAllowed);
    }
    Ok(u.to_string())
}

const COMMON_PASSWORDS: &[&str] = &[
    "password1234",
    "passwort1234",
    "password12345",
    "123456789012",
    "1234567890123",
    "12345678901234",
    "qwertzuiop12",
    "qwertyuiop12",
    "qwertyuiop123",
    "iloveyou1234",
    "letmein12345",
    "welcome12345",
    "administrator",
    "passwordpassword",
    "abcdefghijkl",
    "abcdefgh1234",
    "kaspabattle1",
    "kaspabattle12",
    "kaspabattle123",
    "monkey123456",
    "dragon123456",
    "football1234",
    "baseball1234",
    "superman1234",
    "trustno1trustno1",
    "changeme1234",
    "changemenow",
    "p@ssw0rd1234",
    "passw0rd1234",
    "p@ssword1234",
    "111111111111",
    "000000000000",
    "aaaaaaaaaaaa",
    "1q2w3e4r5t6y",
    "1qaz2wsx3edc",
    "q1w2e3r4t5y6",
    "zaq12wsxcde3",
    "mypassword123",
    "meinpasswort1",
    "hallo1234567",
    "passwort12345",
    "geheim123456",
];

pub fn validate_password(
    pw: &str,
    username: Option<&str>,
    email: Option<&str>,
) -> Result<(), FieldError> {
    let len = pw.chars().count();
    if len == 0 {
        return Err(FieldError::Required);
    }
    if len < 12 {
        return Err(FieldError::TooShort);
    }
    // 128 chars is generous for passphrases and keeps Argon2 input bounded (DoS guard).
    if len > 128 || pw.len() > 512 {
        return Err(FieldError::TooLong);
    }
    let lower = pw.to_lowercase();
    let first = lower.chars().next().unwrap();
    if lower.chars().all(|c| c == first) {
        return Err(FieldError::TooCommon);
    }
    if COMMON_PASSWORDS.iter().any(|c| lower == *c) {
        return Err(FieldError::TooCommon);
    }
    // Pure ascending/descending digit or letter runs (e.g. 123456789012, abcdefghijkl).
    let bytes: Vec<i32> = lower.bytes().map(|b| b as i32).collect();
    if bytes.windows(2).all(|w| w[1] - w[0] == 1) || bytes.windows(2).all(|w| w[0] - w[1] == 1) {
        return Err(FieldError::TooCommon);
    }
    // A password that is the username/e-mail name plus filler is weak.
    let mut personal: Vec<String> = vec![];
    if let Some(u) = username {
        personal.push(u.to_lowercase());
    }
    if let Some(e) = email {
        personal.push(e.split('@').next().unwrap_or("").to_lowercase());
    }
    if personal
        .iter()
        .any(|p| p.len() >= 4 && lower.contains(p.as_str()))
    {
        return Err(FieldError::ContainsPersonalData);
    }
    Ok(())
}

// ── Crypto helpers ──────────────────────────────────────────────────────────

fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(ARGON_M_KIB, ARGON_T, ARGON_P, None).expect("static argon2 params"),
    )
}

pub fn hash_password(pw: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Returns `(valid, new_hash_if_params_outdated)`.
pub fn verify_password(pw: &str, phc: &str) -> (bool, Option<String>) {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return (false, None);
    };
    // Argon2 verification reads the cost parameters from the PHC string itself, so hashes created
    // with older parameters still verify.
    if argon2().verify_password(pw.as_bytes(), &parsed).is_err() {
        return (false, None);
    }
    // Only the cost parameters matter (the parsed params also carry the output length).
    let outdated = parsed.algorithm.as_str() != "argon2id"
        || Params::try_from(&parsed)
            .map(|p| p.m_cost() != ARGON_M_KIB || p.t_cost() != ARGON_T || p.p_cost() != ARGON_P)
            .unwrap_or(true);
    (
        true,
        if outdated {
            hash_password(pw).ok()
        } else {
            None
        },
    )
}

/// `(plaintext, sha256-hex)`; only the hash is persisted.
pub fn new_token() -> (String, String) {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    let plain = URL_SAFE_NO_PAD.encode(b);
    let hash = hash_token(&plain);
    (plain, hash)
}

pub fn hash_token(plain: &str) -> String {
    let d = Sha256::digest(plain.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn ip_hash(salt: &str, ip: &str) -> String {
    let d = Sha256::digest(format!("{salt}|{ip}").as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Stable, non-reversible throttle key for an e-mail address.
pub fn email_key(email: &str) -> String {
    hash_token(&email.trim().to_ascii_lowercase())[..16].to_string()
}

async fn audit(pool: &PgPool, cfg: &AccountConfig, user: Option<Uuid>, event: &str, ip: &str) {
    let _ = sqlx::query("INSERT INTO auth_audit_log (user_id, event, ip_hash) VALUES ($1, $2, $3)")
        .bind(user)
        .bind(event)
        .bind(ip_hash(&cfg.audit_salt, ip))
        .execute(pool)
        .await;
}

async fn hash_blocking(rt: &AccountRuntime, pw: String) -> Result<String, String> {
    let _permit = rt.hash_slots.acquire().await.map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(move || hash_password(&pw))
        .await
        .map_err(|e| e.to_string())?
}

async fn verify_blocking(rt: &AccountRuntime, pw: String, phc: String) -> (bool, Option<String>) {
    let Ok(_permit) = rt.hash_slots.acquire().await else {
        return (false, None);
    };
    tokio::task::spawn_blocking(move || verify_password(&pw, &phc))
        .await
        .unwrap_or((false, None))
}

// ── Sessions ────────────────────────────────────────────────────────────────

/// Creates a *new* session token (rotation: tokens are never reused across logins) and trims the
/// user's oldest sessions beyond [`MAX_SESSIONS_PER_USER`].
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    lifetime: ChronoDuration,
) -> Result<(String, DateTime<Utc>), sqlx::Error> {
    let token = battle_core::auth::AuthService::generate_session_token();
    let expires_at = Utc::now() + lifetime;
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM sessions WHERE user_id = $1 AND id NOT IN \
         (SELECT id FROM sessions WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2)",
    )
    .bind(user_id)
    .bind(MAX_SESSIONS_PER_USER)
    .execute(pool)
    .await?;
    Ok((token, expires_at))
}

pub async fn revoke_sessions(
    pool: &PgPool,
    user_id: Uuid,
    keep: Option<&str>,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND ($2::text IS NULL OR id <> $2)")
            .bind(user_id)
            .bind(keep)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

// ── Registration ────────────────────────────────────────────────────────────

pub struct RegisterInput {
    pub username: String,
    pub email: String,
    pub password: String,
    pub newsletter: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// New account created (verification mail sent when mail is available).
    Created(Uuid),
    /// E-mail already registered: the caller must answer exactly like `Created`.
    EmailExists,
    UsernameTaken,
}

fn is_unique_violation(e: &sqlx::Error) -> Option<String> {
    match e {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            Some(db.constraint().unwrap_or("").to_string())
        }
        _ => None,
    }
}

pub async fn register(
    pool: &PgPool,
    rt: &AccountRuntime,
    input: RegisterInput,
    ip: &str,
) -> Result<RegisterOutcome, String> {
    // Always hash first: identical work for new and existing e-mails (no timing oracle).
    let hash = hash_blocking(rt, input.password.clone()).await?;

    let email_taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE lower(email) = $1)")
            .bind(&input.email)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    if email_taken {
        audit(pool, &rt.cfg, None, "register_email_exists", ip).await;
        // A courtesy mail to the real owner (never reveals anything to the requester).
        return Ok(RegisterOutcome::EmailExists);
    }
    let username_taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE lower(username) = lower($1) OR lower(display_name) = lower($1))")
            .bind(&input.username)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    if username_taken {
        return Ok(RegisterOutcome::UsernameTaken);
    }

    let id = Uuid::new_v4();
    let res = sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, username, has_password_login, \
         email_verified, password_changed_at, newsletter_consent_at, newsletter_consent_version, newsletter_consent_source) \
         VALUES ($1, $2, $3, $4, $4, TRUE, FALSE, NOW(), \
                 CASE WHEN $5 THEN NOW() END, CASE WHEN $5 THEN $6 END, CASE WHEN $5 THEN 'registration' END)",
    )
    .bind(id)
    .bind(&input.email)
    .bind(&hash)
    .bind(&input.username)
    .bind(input.newsletter)
    .bind(NEWSLETTER_CONSENT_VERSION)
    .execute(pool)
    .await;
    match res {
        Ok(_) => {}
        Err(e) => {
            return match is_unique_violation(&e) {
                Some(c) if c.contains("username") || c.contains("display") => {
                    Ok(RegisterOutcome::UsernameTaken)
                }
                Some(_) => Ok(RegisterOutcome::EmailExists), // lost a race on the e-mail index
                None => Err(e.to_string()),
            };
        }
    }
    audit(pool, &rt.cfg, Some(id), "register", ip).await;

    if rt.mailer.is_configured() {
        if let Err(e) = send_verification(pool, rt, id, &input.email, &input.username).await {
            tracing::warn!(error = %e, "verification mail could not be sent");
        }
    }
    Ok(RegisterOutcome::Created(id))
}

async fn send_verification(
    pool: &PgPool,
    rt: &AccountRuntime,
    user_id: Uuid,
    email: &str,
    username: &str,
) -> Result<(), String> {
    let (plain, hash) = new_token();
    // Invalidate older unused tokens: only the newest link works.
    sqlx::query("UPDATE email_verification_tokens SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&hash)
        .bind(Utc::now() + ChronoDuration::hours(VERIFICATION_TTL_HOURS))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    let link = format!("{}/verify-email?token={}", rt.cfg.public_app_url, plain);
    rt.mailer
        .send(verification_mail(email, username, &link))
        .await
        .map_err(|e| e.to_string())
}

/// Consumes a verification token. `true` only the first time a valid, unexpired token is presented.
pub async fn verify_email(
    pool: &PgPool,
    rt: &AccountRuntime,
    token: &str,
    ip: &str,
) -> Result<bool, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let row = sqlx::query(
        "UPDATE email_verification_tokens SET used_at = NOW() \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW() RETURNING user_id",
    )
    .bind(hash_token(token))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let Some(row) = row else { return Ok(false) };
    let user_id: Uuid = row.try_get("user_id").map_err(|e| e.to_string())?;
    sqlx::query("UPDATE users SET email_verified = TRUE, email_verified_at = COALESCE(email_verified_at, NOW()) WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    audit(pool, &rt.cfg, Some(user_id), "email_verified", ip).await;
    Ok(true)
}

/// Always "succeeds" from the caller's perspective; sends only for existing, unverified accounts.
pub async fn resend_verification(pool: &PgPool, rt: &AccountRuntime, email: &str) {
    if !rt.mailer.is_configured() {
        return;
    }
    let row = sqlx::query(
        "SELECT id, COALESCE(username, display_name) AS name FROM users \
         WHERE lower(email) = $1 AND has_password_login AND NOT email_verified",
    )
    .bind(email)
    .fetch_optional(pool)
    .await;
    if let Ok(Some(r)) = row {
        let (id, name): (Uuid, String) = (r.get("id"), r.get("name"));
        if let Err(e) = send_verification(pool, rt, id, email, &name).await {
            tracing::warn!(error = %e, "verification mail could not be re-sent");
        }
    }
}

// ── Login ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum LoginError {
    /// Wrong e-mail or password (identical for both).
    Invalid,
    /// Correct credentials but the address is not confirmed and verification is required.
    NotVerified,
    Throttled {
        retry_after_secs: u64,
    },
    Internal(String),
}

pub struct LoginOk {
    pub user_id: Uuid,
    pub username: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub remember: bool,
}

pub const LOGIN_WINDOW: Duration = Duration::from_secs(15 * 60);
pub const LOGIN_MAX_PER_ACCOUNT: usize = 8;

pub async fn login(
    pool: &PgPool,
    rt: &AccountRuntime,
    email: &str,
    password: &str,
    remember: bool,
    ip: &str,
) -> Result<LoginOk, LoginError> {
    let akey = format!("login:acct:{}", email_key(email));
    let failures = rt.throttle.count(&akey, LOGIN_WINDOW);
    if failures >= LOGIN_MAX_PER_ACCOUNT {
        // Temporary (sliding window) – never a permanent lockout an attacker could abuse.
        return Err(LoginError::Throttled {
            retry_after_secs: LOGIN_WINDOW.as_secs(),
        });
    }
    // Progressive delay: each recent failure costs the next attempt 300 ms (max 2 s).
    if failures > 0 {
        tokio::time::sleep(Duration::from_millis((failures as u64 * 300).min(2000))).await;
    }

    let row = sqlx::query(
        "SELECT id, COALESCE(username, display_name) AS name, password_hash, email_verified, has_password_login \
         FROM users WHERE lower(email) = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(|e| LoginError::Internal(e.to_string()))?;

    // Dummy hash keeps the unknown-user path as slow as the known-user path.
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let dummy = DUMMY
        .get_or_init(|| hash_password("dummy-password-for-timing").unwrap_or_default())
        .clone();

    let (user, phc) = match &row {
        Some(r) if r.get::<bool, _>("has_password_login") => {
            (Some(r), r.get::<String, _>("password_hash"))
        }
        _ => (None, dummy),
    };
    let (ok, rehash) = verify_blocking(rt, password.to_string(), phc).await;
    let Some(r) = user.filter(|_| ok) else {
        rt.throttle.record(&akey);
        audit(pool, &rt.cfg, None, "login_failed", ip).await;
        return Err(LoginError::Invalid);
    };
    let user_id: Uuid = r.get("id");
    let name: String = r.get("name");
    let verified: bool = r.get("email_verified");

    if rt.cfg.require_email_verification && !verified {
        audit(pool, &rt.cfg, Some(user_id), "login_unverified", ip).await;
        return Err(LoginError::NotVerified);
    }
    if let Some(new_hash) = rehash {
        let _ = sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
            .bind(user_id)
            .bind(new_hash)
            .execute(pool)
            .await;
    }
    rt.throttle.clear(&akey);
    let lifetime = if remember {
        ChronoDuration::days(30)
    } else {
        ChronoDuration::hours(24)
    };
    let (token, expires_at) = create_session(pool, user_id, lifetime)
        .await
        .map_err(|e| LoginError::Internal(e.to_string()))?;
    let _ = sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    audit(pool, &rt.cfg, Some(user_id), "login", ip).await;
    Ok(LoginOk {
        user_id,
        username: name,
        token,
        expires_at,
        remember,
    })
}

// ── Password reset / change ─────────────────────────────────────────────────

/// Sends a reset mail iff the account exists and mail delivery is configured. The caller answers
/// identically either way (and refuses upfront with 503 when mail is not configured at all).
pub async fn forgot_password(pool: &PgPool, rt: &AccountRuntime, email: &str, ip: &str) {
    if !rt.mailer.is_configured() {
        return;
    }
    let row = sqlx::query(
        "SELECT id, COALESCE(username, display_name) AS name FROM users WHERE lower(email) = $1 AND has_password_login",
    )
    .bind(email)
    .fetch_optional(pool)
    .await;
    let Ok(Some(r)) = row else { return };
    let (id, name): (Uuid, String) = (r.get("id"), r.get("name"));
    let (plain, hash) = new_token();
    let stored = async {
        sqlx::query("UPDATE password_reset_tokens SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL")
            .bind(id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(&hash)
            .bind(Utc::now() + ChronoDuration::minutes(RESET_TTL_MINUTES))
            .execute(pool)
            .await
    }
    .await;
    if stored.is_err() {
        return;
    }
    audit(pool, &rt.cfg, Some(id), "password_reset_requested", ip).await;
    let link = format!("{}/reset-password?token={}", rt.cfg.public_app_url, plain);
    if let Err(e) = rt
        .mailer
        .send(password_reset_mail(email, &name, &link))
        .await
    {
        tracing::warn!(error = %e, "password reset mail could not be sent");
    }
}

pub async fn reset_password(
    pool: &PgPool,
    rt: &AccountRuntime,
    token: &str,
    new_password: &str,
    ip: &str,
) -> Result<bool, String> {
    let hash = hash_blocking(rt, new_password.to_string()).await?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let row = sqlx::query(
        "UPDATE password_reset_tokens SET used_at = NOW() \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW() RETURNING user_id",
    )
    .bind(hash_token(token))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let Some(row) = row else { return Ok(false) };
    let user_id: Uuid = row.get("user_id");
    sqlx::query("UPDATE users SET password_hash = $2, password_changed_at = NOW() WHERE id = $1")
        .bind(user_id)
        .bind(&hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    // Every other reset link dies with the password, and so does every session.
    sqlx::query(
        "UPDATE password_reset_tokens SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    audit(pool, &rt.cfg, Some(user_id), "password_reset", ip).await;
    Ok(true)
}

pub async fn change_password(
    pool: &PgPool,
    rt: &AccountRuntime,
    user_id: Uuid,
    current: &str,
    new_password: &str,
    keep_session: &str,
    ip: &str,
) -> Result<bool, String> {
    let phc: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1 AND has_password_login")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let Some(phc) = phc else { return Ok(false) };
    let (ok, _) = verify_blocking(rt, current.to_string(), phc).await;
    if !ok {
        return Ok(false);
    }
    let hash = hash_blocking(rt, new_password.to_string()).await?;
    sqlx::query("UPDATE users SET password_hash = $2, password_changed_at = NOW() WHERE id = $1")
        .bind(user_id)
        .bind(hash)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    revoke_sessions(pool, user_id, Some(keep_session))
        .await
        .map_err(|e| e.to_string())?;
    audit(pool, &rt.cfg, Some(user_id), "password_changed", ip).await;
    Ok(true)
}

pub async fn set_newsletter(
    pool: &PgPool,
    user_id: Uuid,
    subscribe: bool,
) -> Result<(), sqlx::Error> {
    if subscribe {
        // Consent only; marketing mail additionally needs a confirmed double-opt-in (not built yet).
        sqlx::query(
            "UPDATE users SET newsletter_consent_at = NOW(), newsletter_consent_version = $2, \
             newsletter_consent_source = 'profile', newsletter_revoked_at = NULL, newsletter_doi_confirmed_at = NULL WHERE id = $1",
        )
        .bind(user_id)
        .bind(NEWSLETTER_CONSENT_VERSION)
        .execute(pool)
        .await?;
    } else {
        sqlx::query("UPDATE users SET newsletter_revoked_at = NOW(), newsletter_doi_confirmed_at = NULL WHERE id = $1 AND newsletter_consent_at IS NOT NULL")
            .bind(user_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn emails_are_normalised_and_validated() {
        assert_eq!(
            normalize_email("  Alice@Example.COM ").unwrap(),
            "alice@example.com"
        );
        // provider-specific folding must NOT happen
        assert_eq!(
            normalize_email("a.l.i.c.e+tag@gmail.com").unwrap(),
            "a.l.i.c.e+tag@gmail.com"
        );
        for bad in [
            "",
            "no-at",
            "a@b",
            "a@@b.com",
            "a b@c.com",
            "@c.com",
            "a@-c.com",
            "a@c..com",
            "a@c.c",
            "ä@c.com",
            ".a@c.com",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
        assert_eq!(
            normalize_email(&format!("{}@c.com", "a".repeat(70))),
            Err(FieldError::Invalid)
        );
        assert_eq!(
            normalize_email(&format!("{}@c.com", "a".repeat(260))),
            Err(FieldError::TooLong)
        );
        // synthetic wallet addresses cannot be claimed
        assert_eq!(
            normalize_email("x@wallet.local"),
            Err(FieldError::NotAllowed)
        );
    }

    #[test]
    fn usernames_follow_the_policy() {
        assert_eq!(validate_username(" Alice_01 ").unwrap(), "Alice_01");
        assert_eq!(validate_username("ab"), Err(FieldError::TooShort));
        assert_eq!(validate_username(&"a".repeat(25)), Err(FieldError::TooLong));
        assert_eq!(
            validate_username("_alice"),
            Err(FieldError::InvalidCharacters)
        );
        for bad in [
            "al ice",
            "ali\u{200b}ce",
            "alice\u{202e}",
            "Алиса",
            "ali.ce",
            "al<ice>",
            "al\nice",
        ] {
            assert_eq!(
                validate_username(bad),
                Err(FieldError::InvalidCharacters),
                "{bad:?}"
            );
        }
        // reserved + impersonation variants
        for bad in [
            "admin",
            "Admin",
            "4dmin",
            "a_d_m_i_n",
            "support",
            "KaspaBattle",
            "Kaspa_Battle_Team",
            "Player_ab12cd",
            "root",
            "B0T",
        ] {
            assert_eq!(validate_username(bad), Err(FieldError::Reserved), "{bad}");
        }
        assert_eq!(validate_username("xXHitlerXx"), Err(FieldError::NotAllowed));
        // legit names stay usable (no Scunthorpe problem for short substrings)
        for ok in [
            "Classic",
            "Assassin",
            "Scunthorpe",
            "Mike_Hunt99",
            "kaspafan",
            "Anna-Lena",
        ] {
            assert!(validate_username(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn passwords_follow_the_policy() {
        assert!(
            validate_password("correct horse battery", Some("alice"), Some("a@b.test")).is_ok()
        );
        assert_eq!(
            validate_password("short", None, None),
            Err(FieldError::TooShort)
        );
        assert_eq!(
            validate_password(&"x".repeat(129), None, None),
            Err(FieldError::TooLong)
        );
        assert_eq!(validate_password("", None, None), Err(FieldError::Required));
        assert_eq!(
            validate_password("aaaaaaaaaaaa", None, None),
            Err(FieldError::TooCommon)
        );
        assert_eq!(
            validate_password("Password1234", None, None),
            Err(FieldError::TooCommon)
        );
        assert_eq!(
            validate_password("123456789012", None, None),
            Err(FieldError::TooCommon)
        );
        assert_eq!(
            validate_password("abcdefghijkl", None, None),
            Err(FieldError::TooCommon)
        );
        assert_eq!(
            validate_password("alice-loves-cats", Some("alice"), None),
            Err(FieldError::ContainsPersonalData)
        );
        assert_eq!(
            validate_password("bobby.tables!!!", None, Some("bobby@x.test")),
            Err(FieldError::ContainsPersonalData)
        );
        // no forced special characters
        assert!(validate_password("alllowercasewordsonly", None, None).is_ok());
        // unicode passphrases are fine and length is counted in characters
        assert!(validate_password("Käsekuchen-schmeckt-gut", None, None).is_ok());
    }

    #[test]
    fn hashing_is_argon2id_with_random_salt_and_verifies() {
        let a = hash_password("correct horse battery").unwrap();
        let b = hash_password("correct horse battery").unwrap();
        assert_ne!(a, b, "salt must be random");
        assert!(a.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{a}");
        let (ok, rehash) = verify_password("correct horse battery", &a);
        assert!(ok && rehash.is_none());
        assert!(!verify_password("wrong", &a).0);
        assert!(!verify_password("x", "not-a-phc-string").0);
    }

    #[test]
    fn outdated_parameters_trigger_a_rehash() {
        let salt = SaltString::generate(&mut OsRng);
        let weak = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(8, 1, 1, None).unwrap(),
        )
        .hash_password(b"correct horse battery", &salt)
        .unwrap()
        .to_string();
        let (ok, rehash) = verify_password("correct horse battery", &weak);
        assert!(ok);
        let new = rehash.expect("weak params must be upgraded");
        assert!(new.contains("m=19456,t=2,p=1"));
        // wrong password never yields a rehash
        assert_eq!(verify_password("nope nope nope", &weak), (false, None));
    }

    #[test]
    fn tokens_are_random_and_only_their_hash_is_derivable() {
        let (p1, h1) = new_token();
        let (p2, h2) = new_token();
        assert_ne!(p1, p2);
        assert_eq!(p1.len(), 43); // 256 bit, base64url
        assert_eq!(h1, hash_token(&p1));
        assert_ne!(h1, h2);
        assert_ne!(h1, p1);
    }

    #[test]
    fn throttle_is_a_sliding_window() {
        let t = Throttle::default();
        let w = Duration::from_millis(80);
        assert!(t.allow("k", 2, w));
        assert!(t.allow("k", 2, w));
        assert!(!t.allow("k", 2, w));
        assert!(t.allow("other", 2, w));
        std::thread::sleep(Duration::from_millis(100));
        assert!(t.allow("k", 2, w), "window must slide");
    }

    #[test]
    fn verification_default_follows_mail_availability() {
        let none = |_: &str| None::<String>;
        assert!(AccountConfig::from_env(true, none).require_email_verification);
        assert!(!AccountConfig::from_env(false, none).require_email_verification);
        let off = |k: &str| (k == "REQUIRE_EMAIL_VERIFICATION").then(|| "false".to_string());
        assert!(!AccountConfig::from_env(true, off).require_email_verification);
        let on = |k: &str| (k == "REQUIRE_EMAIL_VERIFICATION").then(|| "true".to_string());
        assert!(AccountConfig::from_env(false, on).require_email_verification);
    }
}
