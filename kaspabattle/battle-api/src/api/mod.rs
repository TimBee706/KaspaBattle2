pub mod admin_guard;
pub mod auth_guard;
pub mod csrf_guard;
pub mod rate_limit;
pub mod faceit;
pub mod multisig;
/// Handler sub-modules (CQ-01: Phase 1 — types extracted; full handler migration: post-beta).
/// See `src/api/handlers/` for the target module structure.
pub mod handlers;

use crate::api::{admin_guard::AdminApiKey, auth_guard::SessionUser};
use crate::models::{Match, MatchStatus};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_hashes::PersonalMessageSigningHash;
use serde::{Deserialize, Serialize};
use secp256k1::{schnorr::Signature, PublicKey, XOnlyPublicKey};
use sqlx::{PgPool, Row};
use tokio::sync::broadcast;
use uuid::Uuid;


// NOTE: These types are defined here; they will move to handlers::matches in the post-beta CQ-01 sprint.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CreateReq {
    pub game_id: String,
    pub wager_sompi: i64,
    pub mode: crate::models::MatchMode,
    pub escrow_address: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DepositReq {
    pub tx_hash: String,
    pub player_role: String, // "A" or "B"
}

use battle_core::auth::AuthService;
use battle_core::faceit_oauth::FaceitOAuthService;
use battle_core::faceit_data::FaceitDataService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
    pub auth_service: Arc<AuthService>,
    pub faceit_service: Arc<FaceitOAuthService>,
    pub faceit_data_service: Option<Arc<FaceitDataService>>,
    pub escrow_wallet: Option<Arc<battle_kaspa::wallet::EscrowWallet>>,
    pub escrow_service: Option<Arc<battle_kaspa::escrow::EscrowService>>,
    pub kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaBackend>>,
    pub payout_service: Option<Arc<battle_kaspa::payout::PayoutService>>,
    /// Watcher used by the payment-status endpoint and episode runner
    pub blockchain_watcher: Option<Arc<battle_kaspa::watcher::BlockchainWatcher>>,
    pub multisig_service: Option<Arc<battle_kaspa::multisig::service::MultisigEscrowService>>,
}

#[derive(Serialize)]
pub struct ApiErrorResponse {
    pub error: &'static str,
    pub message: &'static str,
}


pub fn router() -> Router<AppState> {
    // Build rate limiters from env vars at startup
    let auth_limiter = rate_limit::build_ip_limiter(rate_limit::auth_rpm());
    let match_limiter = rate_limit::build_ip_limiter(rate_limit::match_rpm());
    let global_limiter = rate_limit::build_ip_limiter(rate_limit::global_rpm());

    tracing::info!(
        "Rate limits — auth: {}/min, match-create: {}/min, global: {}/min",
        rate_limit::auth_rpm(),
        rate_limit::match_rpm(),
        rate_limit::global_rpm(),
    );

    // ── Auth sub-router (rate-limited: 5 req/min per IP) ──────────────────────
    let auth_lim = auth_limiter.clone();
    let auth_router = Router::new()
        .route("/auth/wallet-challenge", post(create_wallet_login_challenge))
        .route("/auth/wallet-verify", post(verify_wallet_login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(get_me))
        .route("/auth/me/wallet", axum::routing::patch(update_wallet_address))
        .route("/auth/me/wallet/disconnect", post(disconnect_wallet))
        .layer(axum::middleware::from_fn(move |req, next| {
            let lim = auth_lim.clone();
            async move { rate_limit::rate_limit_middleware(lim, req, next).await }
        }));

    // ── Match-create sub-router (rate-limited: 3 req/min per IP) ─────────────
    let match_lim = match_limiter.clone();
    let match_create_router = Router::new()
        .route("/challenges", post(create_challenge))
        .layer(axum::middleware::from_fn(move |req, next| {
            let lim = match_lim.clone();
            async move { rate_limit::rate_limit_middleware(lim, req, next).await }
        }));

    // ── Main router (global rate limit: 120 req/min per IP) ───────────────────
    let global_lim = global_limiter.clone();
    Router::new()
        .route("/health", get(health))
        .route("/lobbies", get(get_lobbies))
        .route("/history", get(get_history))
        .route("/matches/:id", get(get_match))
        .route("/matches/:id/accept", post(join_challenge))
        .route("/matches/:id/deposit", post(submit_deposit))
        .route("/matches/:id/deposits", get(check_deposits))
        .route("/matches/:id/payment-status", get(get_payment_status))
        .route("/matches/:id/resolve", post(admin_resolve_match))
        .route("/matches/:id/faceid", post(submit_faceid_handler))
        .route("/matches/:id/cancel", post(cancel_match_handler))
        .route("/webhook/faceit", post(faceit_webhook))
        .route("/matches/:id/faceit-match-id", post(submit_faceit_match_id))
        .route("/matches/:id/payout/pskt", get(get_payout_pskt))
        .route("/matches/:id/payout/submit-signature", post(submit_payout_signature))
        .route("/matches/:id/payout/status", get(get_payout_status))
        .route("/me", get(get_me))
        .route("/me/wallet", axum::routing::patch(update_wallet_address))
        .route("/ws", get(ws_handler))
        .nest("/faceit", faceit::router())
        .nest("/multisig", multisig::router())
        // Merge in rate-limited sub-routers
        .merge(auth_router)
        .merge(match_create_router)
        .layer(axum::middleware::from_fn(move |req, next| {
            let lim = global_lim.clone();
            async move { rate_limit::rate_limit_middleware(lim, req, next).await }
        }))
}

pub async fn health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiErrorResponse>)> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "health check DB ping failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiErrorResponse {
                    error: "db_unavailable",
                    message: "Database connection check failed.",
                }),
            )
        })?;

    // SEC-15: Do not expose pool internals in production responses.
    // Pool metrics are logged server-side for observability instead.
    tracing::debug!(
        pool_size = state.pool.size(),
        pool_idle = state.pool.num_idle(),
        "health check: pool metrics"
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "db": "ok",
    })))
}

pub async fn get_me(
    State(state): State<AppState>,
    user_opt: Option<crate::api::auth_guard::SessionUserNoWallet>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match user_opt {
        Some(crate::api::auth_guard::SessionUserNoWallet(user)) => {
            // Enrich with FaceIT data from faceit_links table
            let faceit_row = sqlx::query(
                "SELECT faceit_player_id, faceit_nickname, faceit_avatar_url, faceit_elo, faceit_skill_level FROM faceit_links WHERE user_id = $1::uuid"
            )
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();

            let (faceit_connected, faceit_id, faceit_nickname, faceit_avatar, faceit_elo, faceit_skill_level) = if let Some(r) = &faceit_row {
                use sqlx::Row;
                let fid: String = r.try_get("faceit_player_id").unwrap_or_default();
                let fnick: String = r.try_get("faceit_nickname").unwrap_or_default();
                let favatar: Option<String> = r.try_get("faceit_avatar_url").unwrap_or(None);
                let felo: Option<i32> = r.try_get("faceit_elo").unwrap_or(None);
                let fskill: Option<i32> = r.try_get("faceit_skill_level").unwrap_or(None);
                (true, fid, fnick, favatar.unwrap_or_default(), felo, fskill)
            } else {
                // NOT connected: do NOT fallback to display_name for faceit_nickname
                (false, String::new(), String::new(), String::new(), None, None)
            };

            Ok(Json(serde_json::json!({
                "id": user.id,
                "email": user.email,
                "display_name": user.display_name,
                "kaspa_address": user.kaspa_address,
                "faceit_connected": faceit_connected,
                "faceit_id": faceit_id,
                "faceit_nickname": faceit_nickname,
                "faceit_avatar": faceit_avatar,
                "faceit_elo": faceit_elo,
                "faceit_skill_level": faceit_skill_level,
                "created_at": user.created_at,
                "total_matches": 0,
                "wins": 0,
                "losses": 0,
                "total_wagered_sompi": 0,
                "total_won_sompi": 0,
            })))
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Serialize)]
pub struct WalletAuthResponse {
    pub user_id: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct WalletChallengeReq {
    pub kaspa_address: String,
}

#[derive(Serialize)]
pub struct WalletChallengeResponse {
    pub challenge_id: String,
    pub message: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct WalletVerifyReq {
    pub challenge_id: String,
    pub kaspa_address: String,
    pub signature: String,
    pub public_key: String,
    #[serde(default)]
    pub link_to_existing_user: bool,
}

fn auth_cookie_security_attrs(target_url: Option<&str>) -> (&'static str, &'static str) {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    
    // If FRONTEND_URL is an external HTTPS proxy (e.g. ngrok), we MUST force SameSite=None + Secure
    // regardless of the redirect target url, otherwise Chrome blocks the auth cookie.
    if frontend_url.contains("ngrok-free.dev") || frontend_url.starts_with("https://") {
        return ("None", "; Secure");
    }

    let effective_url = target_url.unwrap_or(&frontend_url);
    let is_secure = effective_url.starts_with("https://");
    let same_site = if is_secure { "None" } else { "Lax" };
    let secure_flag = if is_secure { "; Secure" } else { "" };

    (same_site, secure_flag)
}

pub fn build_auth_cookie(session_token: &str) -> String {
    let (same_site, secure_flag) = auth_cookie_security_attrs(None);

    format!(
        "kaspabattle-auth={}; HttpOnly; Path=/; SameSite={}{}; Max-Age=604800",
        session_token, same_site, secure_flag
    )
}

pub fn build_auth_cookie_for_target(session_token: &str, target_url: &str) -> String {
    let (same_site, secure_flag) = auth_cookie_security_attrs(Some(target_url));

    format!(
        "kaspabattle-auth={}; HttpOnly; Path=/; SameSite={}{}; Max-Age=604800",
        session_token, same_site, secure_flag
    )
}

fn build_clear_auth_cookie() -> String {
    let (same_site, secure_flag) = auth_cookie_security_attrs(None);

    format!(
        "kaspabattle-auth=; HttpOnly; Path=/; SameSite={}{}; Max-Age=0",
        same_site, secure_flag
    )
}

fn extract_session_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());
    if let Some(header) = auth_header {
        if header.starts_with("Bearer ") {
            return Some(header["Bearer ".len()..].to_string());
        }
    }

    headers
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
}

fn challenge_is_active(
    expires_at: chrono::DateTime<chrono::Utc>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    used_at.is_none() && expires_at > now
}

fn parse_xonly_public_key(public_key_hex: &str) -> Result<XOnlyPublicKey, StatusCode> {
    let public_key_bytes =
        hex::decode(public_key_hex.trim()).map_err(|_| StatusCode::BAD_REQUEST)?;

    match public_key_bytes.len() {
        32 => XOnlyPublicKey::from_slice(&public_key_bytes).map_err(|_| StatusCode::BAD_REQUEST),
        33 | 65 => {
            let public_key =
                PublicKey::from_slice(&public_key_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
            Ok(public_key.x_only_public_key().0)
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

fn verify_wallet_signature(
    kaspa_address: &str,
    public_key_hex: &str,
    message: &str,
    signature_hex: &str,
) -> Result<(), StatusCode> {
    let prefix = if kaspa_address.starts_with("kaspatest:") {
        Prefix::Testnet
    } else if kaspa_address.starts_with("kaspa:") {
        Prefix::Mainnet
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let xonly_public_key = parse_xonly_public_key(public_key_hex)?;
    let derived_address = Address::new(prefix, Version::PubKey, &xonly_public_key.serialize());
    if derived_address.to_string() != kaspa_address {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let signature_bytes = hex::decode(signature_hex.trim()).map_err(|_| StatusCode::BAD_REQUEST)?;
    if signature_bytes.len() != 64 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let signature = Signature::from_slice(&signature_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut hasher = PersonalMessageSigningHash::new();
    hasher.write(message.as_bytes());
    let message_hash = hasher.finalize();
    let secp_message = secp256k1::Message::from_digest_slice(message_hash.as_bytes().as_slice())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    signature
        .verify(&secp_message, &xonly_public_key)
        .map_err(|_| StatusCode::UNAUTHORIZED)
}

pub async fn create_wallet_login_challenge(
    State(state): State<AppState>,
    Json(payload): Json<WalletChallengeReq>,
) -> Result<Json<WalletChallengeResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    tracing::info!("[wallet-challenge] Request for address: {}", payload.kaspa_address);

    if !payload.kaspa_address.starts_with("kaspa:") && !payload.kaspa_address.starts_with("kaspatest:") {
        tracing::info!("[wallet-challenge] Rejected: invalid address prefix");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_kaspa_address",
                message: "Kaspa address must start with kaspa: or kaspatest:.",
            }),
        ));
    }

    let challenge_id = Uuid::new_v4();
    let nonce = battle_core::auth::AuthService::generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let message = format!(
        "KaspaBattle Login Challenge\nAddress: {}\nNonce: {}\nChallenge ID: {}\nExpires At: {}",
        payload.kaspa_address,
        nonce,
        challenge_id,
        expires_at.to_rfc3339(),
    );

    sqlx::query(
        "INSERT INTO wallet_login_challenges (id, kaspa_address, challenge_message, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(challenge_id)
    .bind(&payload.kaspa_address)
    .bind(&message)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        let pool_size = state.pool.size();
        let pool_idle = state.pool.num_idle();
        match &e {
            sqlx::Error::PoolTimedOut => {
                tracing::warn!(
                    pool_size,
                    pool_idle,
                    kaspa_address = %payload.kaspa_address,
                    "wallet-challenge DB pool timed out"
                );
                tracing::info!("[wallet-challenge] DB pool timeout: pool_size={}, pool_idle={}",
                    pool_size, pool_idle
                );
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ApiErrorResponse {
                        error: "db_pool_exhausted",
                        message: "Authentication service is temporarily busy. Please retry in a moment.",
                    }),
                )
            }
            _ => {
                tracing::error!(
                    error = %e,
                    pool_size,
                    pool_idle,
                    kaspa_address = %payload.kaspa_address,
                    "wallet-challenge DB insert failed"
                );
                tracing::error!("[wallet-challenge] DB insert failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse {
                        error: "db_error",
                        message: "Failed to create wallet login challenge.",
                    }),
                )
            }
        }
    })?;

    tracing::info!("[wallet-challenge] ✅ Challenge created: {} for {}", challenge_id, payload.kaspa_address);

    Ok(Json(WalletChallengeResponse {
        challenge_id: challenge_id.to_string(),
        message,
        expires_at: expires_at.to_rfc3339(),
    }))
}

pub async fn verify_wallet_login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<WalletVerifyReq>,
) -> Result<axum::response::Response, StatusCode> {
    tracing::info!("[wallet-verify] Request: challenge_id={}, addr={}", payload.challenge_id, payload.kaspa_address);

    let challenge_id = Uuid::parse_str(&payload.challenge_id).map_err(|e| {
        tracing::info!("[wallet-verify] Invalid challenge_id: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    let challenge_row = sqlx::query(
        "SELECT kaspa_address, challenge_message, expires_at, used_at FROM wallet_login_challenges WHERE id = $1"
    )
    .bind(challenge_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("[wallet-verify] DB query failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        tracing::info!("[wallet-verify] Challenge not found: {}", challenge_id);
        StatusCode::UNAUTHORIZED
    })?;

    let challenge_address: String = challenge_row
        .try_get("kaspa_address")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let challenge_message: String = challenge_row
        .try_get("challenge_message")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let challenge_expires_at: chrono::DateTime<chrono::Utc> = challenge_row
        .try_get("expires_at")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let challenge_used_at: Option<chrono::DateTime<chrono::Utc>> = challenge_row
        .try_get("used_at")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if challenge_address != payload.kaspa_address {
        tracing::info!("[wallet-verify] Address mismatch: challenge={} vs payload={}", challenge_address, payload.kaspa_address);
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !challenge_is_active(challenge_expires_at, challenge_used_at, chrono::Utc::now()) {
        tracing::info!("[wallet-verify] Challenge expired or used: expires_at={}, used_at={:?}", challenge_expires_at, challenge_used_at);
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!("[wallet-verify] Verifying signature for {} (pubkey={}...)", payload.kaspa_address, &payload.public_key[..16.min(payload.public_key.len())]);
    verify_wallet_signature(
        &payload.kaspa_address,
        &payload.public_key,
        &challenge_message,
        &payload.signature,
    ).map_err(|status| {
        tracing::error!("[wallet-verify] ❌ Signature verification failed (status={})", status);
        status
    })?;
    tracing::info!("[wallet-verify] ✅ Signature valid for {}", payload.kaspa_address);

    sqlx::query("UPDATE wallet_login_challenges SET used_at = NOW() WHERE id = $1")
        .bind(challenge_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("[wallet-verify] DB update failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("[wallet-verify] Challenge marked as used, proceeding to user lookup...");

    // 2. Prüfen ob der Caller BEREITS authentifiziert ist (FaceIT-Session aktiv?)
    let existing_token_opt = extract_session_token_from_headers(&headers);
    tracing::info!("[wallet-verify] Existing session token: {}", if existing_token_opt.is_some() { "found" } else { "none" });

    let auth_service = battle_core::auth::AuthService::new(state.pool.clone());

    if payload.link_to_existing_user && existing_token_opt.is_none() {
        tracing::info!("[wallet-verify] Wallet link requested without active session");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Wenn bereits authentifiziert → Wallet an bestehenden User verknüpfen
    if payload.link_to_existing_user {
        let existing_token = existing_token_opt.clone().ok_or(StatusCode::UNAUTHORIZED)?;
        if let Ok(existing_user) = auth_service.validate_session(&existing_token).await {
            // Erst: kaspa_address von einem alten Guest-User freigeben falls UNIQUE-Konflikt besteht
            // (z.B. wenn diese Adresse bereits einem Wallet-only User gehörte)
            let conflict = sqlx::query(
                "SELECT id FROM users WHERE kaspa_address = $1 AND id != $2::uuid"
            )
            .bind(&payload.kaspa_address)
            .bind(&existing_user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("[wallet-verify] DB conflict check failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            if conflict.is_some() {
                tracing::info!("[wallet_login] Clearing kaspa_address from conflicting guest user");
                sqlx::query(
                    "UPDATE users SET kaspa_address = NULL WHERE kaspa_address = $1 AND id != $2::uuid"
                )
                .bind(&payload.kaspa_address)
                .bind(&existing_user.id)
                .execute(&state.pool)
                .await
                .map_err(|e| {
                    tracing::error!("[wallet-verify] DB clear conflict failed: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            }

            // Jetzt sicher: kaspa_address am bestehenden User setzen
            sqlx::query("UPDATE users SET kaspa_address = $1 WHERE id = $2::uuid")
                .bind(&payload.kaspa_address)
                .bind(&existing_user.id)
                .execute(&state.pool)
                .await
                .map_err(|e| {
                    tracing::error!("[wallet_login] UPDATE kaspa_address failed: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let mut response = Json(WalletAuthResponse {
                user_id: existing_user.id.to_string(),
                display_name: existing_user.display_name,
            })
            .into_response();
            let cookie_value = build_auth_cookie(&existing_token);
            response.headers_mut().insert(
                axum::http::header::SET_COOKIE,
                axum::http::HeaderValue::from_str(&cookie_value)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            );
            return Ok(response);
        }
    }

    // 3. Nicht eingeloggt → wie bisher: User suchen oder neu anlegen
    if let Some(existing_token) = existing_token_opt {
        let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(&existing_token)
            .execute(&state.pool)
            .await;
    }

    let row = sqlx::query("SELECT id FROM users WHERE kaspa_address = $1")
        .bind(&payload.kaspa_address)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("[wallet-verify] DB user lookup failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let (user_id, display_name_str) = if let Some(r) = row {
        let uid: uuid::Uuid = r.try_get("id").unwrap_or_else(|_| uuid::Uuid::new_v4());
        let dn: String = r.try_get("display_name").unwrap_or_else(|_|
            format!("Player_{}", &payload.kaspa_address.chars().skip(6).take(6).collect::<String>())
        );
        (uid, dn)
    } else {
        let new_user_id = uuid::Uuid::new_v4();
        let display_name = format!(
            "Player_{}",
            &payload.kaspa_address.chars().skip(6).take(6).collect::<String>()
        );
        let email = format!("{}@wallet.local", new_user_id);
        let password_hash = battle_core::auth::AuthService::hash_password(&uuid::Uuid::new_v4().to_string())
            .unwrap_or_default();

        sqlx::query("INSERT INTO users (id, email, password_hash, display_name, kaspa_address) VALUES ($1, $2, $3, $4, $5)")
            .bind(new_user_id).bind(email).bind(password_hash).bind(&display_name).bind(&payload.kaspa_address)
            .execute(&state.pool).await.map_err(|e| {
                tracing::error!("Error creating user: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        (new_user_id, display_name)
    };

    // 4. Session generieren
    let session_token = battle_core::auth::AuthService::generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(battle_core::constants::session_lifetime_days());
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&session_token).bind(&user_id).bind(expires_at)
        .execute(&state.pool).await.map_err(|e| {
            tracing::error!("[wallet-verify] DB session insert failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("[wallet-verify] ✅ Session created for user {} ({})", user_id, display_name_str);

    let mut response = Json(WalletAuthResponse {
        user_id: user_id.to_string(),
        display_name: display_name_str,
    })
    .into_response();
    let cookie_value = build_auth_cookie(&session_token);
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie_value)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok(response)
}

pub async fn disconnect_wallet(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUserNoWallet(user): crate::api::auth_guard::SessionUserNoWallet,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user_id = user.id;

    sqlx::query("UPDATE users SET kaspa_address = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Error clearing kaspa_address: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "kaspa_address": serde_json::Value::Null,
    })))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(token) = extract_session_token_from_headers(&headers) {
        let auth_service = battle_core::auth::AuthService::new(state.pool.clone());
        let _ = auth_service.logout(&token).await;
    }

    let mut response = axum::response::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&build_clear_auth_cookie())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct UpdateWalletReq {
    pub kaspa_address: String,
}

pub async fn update_wallet_address(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Json(payload): Json<UpdateWalletReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user_id = user.id;

    // SEC-08/14: Validate Kaspa address using the kaspa-addresses crate.
    // Accepts both mainnet (kaspa:) and testnet (kaspatest:) addresses.
    let addr_str = payload.kaspa_address.trim();
    if kaspa_addresses::Address::try_from(addr_str).is_err() {
        tracing::warn!(
            user_id = %user_id,
            address = %addr_str,
            "update_wallet_address: invalid Kaspa address rejected"
        );
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    sqlx::query("UPDATE users SET kaspa_address = $1 WHERE id = $2")
        .bind(addr_str)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Error updating kaspa_address: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "kaspa_address": addr_str,
    })))
}

pub async fn get_lobbies(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>(&format!(
        "SELECT {} FROM matches \
         WHERE status IN (\
           'OPEN', 'AWAITING_FUNDING', 'FUNDED', 'LOCKED', \
           'GAME_ID_INPUT', 'IN_GAME', 'FINISHED_FACEIT', \
           'READY_FOR_PAYOUT', 'DISPUTED'\
         ) ORDER BY created_at DESC LIMIT 100",
        crate::models::MATCH_COLUMNS,
    ))
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    for m in &mut matches {
        m.calculate_wager();
    }

    Ok(Json(matches))
}

pub async fn get_match(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let mut m = sqlx::query_as::<_, Match>(&format!(
        "SELECT {} FROM matches WHERE id = $1",
        crate::models::MATCH_COLUMNS,
    ))
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(match_id = %id, "get_match failed: {:?}", e);
        StatusCode::NOT_FOUND
    })?;

    m.calculate_wager();
    Ok(Json(m))
}

pub async fn get_history(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>(&format!(
        "SELECT {} FROM matches \
         WHERE status IN ('RESOLVED', 'PAID_OUT', 'CANCELLED') \
         ORDER BY created_at DESC LIMIT 200",
        crate::models::MATCH_COLUMNS,
    ))
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("SQL Error in get_history: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for m in &mut matches {
        m.calculate_wager();
    }
    Ok(Json(matches))
}

pub async fn create_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Json(payload): Json<CreateReq>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = user.id;

    // Require wallet — address derivation is purely cryptographic (no Kaspa node needed).
    let wallet = state.escrow_wallet.as_ref().ok_or_else(|| {
        tracing::error!("EscrowWallet not initialized — cannot generate escrow address");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Use a DB transaction so that if ANYTHING fails after INSERT,
    // the half-created match row is automatically rolled back.
    let mut db_tx = state.pool.begin().await.map_err(|e| {
        tracing::error!("Failed to start DB transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 1. INSERT match (escrow_address will be set below — use a temp placeholder).
    let record = sqlx::query_as::<_, Match>(&format!("INSERT INTO matches (creator_user_id, game_id, wager_sompi, wager_amount_sompi, mode) VALUES ($1, $2, $3, $3, $4) RETURNING {}", crate::models::MATCH_SELECT_COLS))
    .bind(user_id)
    .bind(&payload.game_id)
    .bind(payload.wager_sompi)
    .bind(payload.mode)
    .fetch_one(&mut *db_tx)
    .await
    .map_err(|e| {
        tracing::error!("SQL Error in create_challenge INSERT: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 2. Derive escrow address deterministically from match UUID.
    //    Pure BIP44 derivation — no Kaspa node connection required.
    let (escrow_addr_obj, derivation_index) = wallet
        .derive_escrow_address(&record.id.to_string())
        .map_err(|e| {
            tracing::error!("Failed to derive escrow address for match {}: {}", record.id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let escrow_addr = escrow_addr_obj.to_string();

    tracing::info!(
        match_id = %record.id,
        escrow_address = %escrow_addr,
        derivation_index,
        "Escrow address derived"
    );

    // 3. UPDATE match with the freshly-derived escrow address.
    let mut updated = sqlx::query_as::<_, Match>(
        "UPDATE matches SET escrow_address = $1 WHERE id = $2 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, wager_sompi, mode, status, external_match_id, created_at",
    )
    .bind(&escrow_addr)
    .bind(record.id)
    .fetch_one(&mut *db_tx)
    .await
    .map_err(|e| {
        tracing::error!("SQL Error updating escrow_address for match {}: {:?}", record.id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 4. Commit the transaction — both rows are now consistent.
    db_tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit match creation transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    updated.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&updated).unwrap_or_default());
    Ok(Json(updated))
}

pub async fn join_challenge(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    tracing::info!(match_id = %id, caller = %user.id, "join_challenge called");

    let joiner_id = user.id;

    let current_match: Match = sqlx::query_as(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, wager_sompi, mode, status, external_match_id, created_at FROM matches WHERE id = $1"
    )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(match_id = %id, "join_challenge: failed to load match: {:?}", e);
            StatusCode::NOT_FOUND
        })?;

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    current_match
        .validate_action(&MatchAction::Join {
            player_id: joiner_id.to_string(),
            kaspa_address: user.kaspa_address.clone().unwrap_or_default(),
        })
        .map_err(|e| {
            tracing::warn!("State machine rejected Join: {}", e);
            StatusCode::CONFLICT
        })?;

    // SICHERHEIT: AND creator_user_id != $1 verhindert, dass man gegen sich selbst spielt!
    let mut updated = sqlx::query_as::<_, Match>(&format!("UPDATE matches SET opponent_user_id = $1, status = $3 WHERE id = $2 AND status = $4 AND creator_user_id != $1 RETURNING {}", crate::models::MATCH_SELECT_COLS))
    .bind(joiner_id)
    .bind(id)
    .bind(crate::models::MatchStatus::AwaitingFunding)
    .bind(crate::models::MatchStatus::Open)
    .fetch_one(&state.pool).await.map_err(|e| {
        tracing::error!("Failed to update match status in join_challenge: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    updated.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&updated).unwrap_or_default());
    Ok(Json(updated))
}

pub async fn submit_deposit(
    State(state): State<AppState>,
    session: SessionUser, // M-05: authentication required
    Path(id): Path<Uuid>,
    Json(payload): Json<DepositReq>,
) -> Result<Json<Match>, StatusCode> {
    let caller_id = &session.0.id;
    tracing::info!(
        "Deposit received: match={}, player_role={} caller={}",
        id,
        payload.player_role,
        caller_id
    );

    // Step 1: Fetch the current match to determine player roles and current state
    let m: Match = sqlx::query_as(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, wager_sompi, mode, status, external_match_id, created_at, player_a_deposit_tx_hash, player_b_deposit_tx_hash FROM matches WHERE id = $1"
    )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("submit_deposit: match {} not found: {:?}", id, e);
            StatusCode::NOT_FOUND
        })?;

    // Step 2: Allow deposits when match is OPEN or AWAITING_FUNDING.
    // OPEN = player depositing before opponent has joined (valid — escrow address exists).
    // The state machine transition to FUNDED is handled by the episode runner after UTXO confirmation.
    let is_deposit_allowed = matches!(m.status, MatchStatus::Open | MatchStatus::AwaitingFunding);
    if !is_deposit_allowed {
        tracing::warn!(
            match_id = %id,
            status = ?m.status,
            "Deposit rejected: match not in a depositable state (OPEN/AWAITING_FUNDING)"
        );
        tracing::error!("❌ Deposit rejected for match {}: status is {:?}",
            id, m.status
        );
        return Err(StatusCode::CONFLICT);
    }

    // Step 3: Double-deposit guard + column update based on player_role
    // player_role "A" = creator, "B" = opponent
    match payload.player_role.to_uppercase().as_str() {
        "A" => {
            if let Some(ref existing) = m.player_a_deposit_tx_hash {
                tracing::warn!(
                    match_id = %id,
                    existing_tx = %existing,
                    new_tx = %payload.tx_hash,
                    "⚠️ Double deposit attempt blocked (player A)"
                );
                return Err(StatusCode::CONFLICT);
            }
            sqlx::query(
                "UPDATE matches SET player_a_deposit_tx_hash = $1 WHERE id = $2"
            )
            .bind(&payload.tx_hash)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("❌ SQL error recording deposit (player A): {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        "B" => {
            if let Some(ref existing) = m.player_b_deposit_tx_hash {
                tracing::warn!(
                    match_id = %id,
                    existing_tx = %existing,
                    new_tx = %payload.tx_hash,
                    "⚠️ Double deposit attempt blocked (player B)"
                );
                return Err(StatusCode::CONFLICT);
            }
            sqlx::query(
                "UPDATE matches SET player_b_deposit_tx_hash = $1 WHERE id = $2"
            )
            .bind(&payload.tx_hash)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("❌ SQL error recording deposit (player B): {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        _ => {
            tracing::error!("❌ submit_deposit: invalid player_role '{}'", payload.player_role);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Step 5: Check whether BOTH players have now deposited
    let updated: Match = sqlx::query_as(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, wager_sompi, mode, status, external_match_id, created_at FROM matches WHERE id = $1"
    )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("submit_deposit: failed to re-fetch match {}: {:?}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let _a_confirmed = updated.player_a_deposit_confirmed.unwrap_or(false);
    let _b_confirmed = updated.player_b_deposit_confirmed.unwrap_or(false);

    // We no longer transition to FUNDED here. The MatchEpisode (Blockchain Watcher)
    // is responsible for confirming the actual UTXO and setting the status.
    tracing::debug!("⏳ Deposit TX recorded for match {}. Waiting for blockchain confirmation.",
        id
    );

    let final_match = updated;

    // Broadcast updated state via WebSocket
    let _ = state
        .tx
        .send(serde_json::to_string(&final_match).unwrap_or_default());

    Ok(Json(final_match))
}

/// Check deposit status for a match via EscrowService (on-chain balance check)
pub async fn check_deposits(
    State(state): State<AppState>,
    SessionUser(_user): SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let escrow_svc = state.escrow_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ EscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Look up the match to get escrow_address and wager_sompi
    let row: (Option<String>, i64) =
        sqlx::query_as("SELECT escrow_address, wager_sompi FROM matches WHERE id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = row.0.unwrap_or_default();
    let wager_sompi = row.1 as u64;

    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "NO_ESCROW",
            "message": "No escrow address assigned to this match"
        })));
    }

    match escrow_svc.check_deposits(&escrow_address, wager_sompi).await {
        Ok(status) => Ok(Json(serde_json::to_value(status).unwrap())),
        Err(e) => {
            tracing::error!("❌ check_deposits failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Detailed per-player payment status from the `payments` table.
///
/// GET /api/v1/matches/:id/payment-status
///
/// Returns:
/// ```json
/// {
///   "escrow_address": "kaspatest:...",
///   "required_per_player_sompi": 5000000,
///   "min_confirmations_required": 10,
///   "playerA": { "paid": true, "confirmed_sompi": 5000000, "payment_count": 1, "min_confirmations": 15 },
///   "playerB": { "paid": false, "confirmed_sompi": 0, "payment_count": 0, "min_confirmations": 0 },
///   "both_paid": false
/// }
/// ```
pub async fn get_payment_status(
    State(state): State<AppState>,
    SessionUser(_user): SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Fetch match basics
    let row: (Option<String>, i64, Option<bool>, Option<bool>) = sqlx::query_as(
        "SELECT escrow_address, wager_sompi, player_a_deposit_confirmed, player_b_deposit_confirmed \
         FROM matches WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = row.0.unwrap_or_default();
    let required_per_player_sompi = row.1 as u64;
    let player_a_db_confirmed = row.2.unwrap_or(false);
    let player_b_db_confirmed = row.3.unwrap_or(false);

    const MIN_CONF: i32 = 10;

    // Aggregate from payments table
    let payment_rows: Vec<(String, Option<i64>, Option<i32>, Option<i64>)> = sqlx::query_as(
        "SELECT player_role, \
         SUM(amount_sompi)::BIGINT AS total_sompi, \
         MIN(confirmations) AS min_confirmations, \
         COUNT(*)::BIGINT AS payment_count \
         FROM payments WHERE match_id = $1 \
         GROUP BY player_role",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let find_role = |role: &str| -> (u64, i32, i64) {
        payment_rows
            .iter()
            .find(|r| r.0 == role)
            .map(|r| (
                r.1.unwrap_or(0) as u64,
                r.2.unwrap_or(0),
                r.3.unwrap_or(0),
            ))
            .unwrap_or((0, 0, 0))
    };

    let (a_sompi, a_min_conf, a_count) = find_role("A");
    let (b_sompi, b_min_conf, b_count) = find_role("B");

    // A player is considered "paid" if either:
    // (a) the DB confirms it via deposit_confirmed flag (set by episode runner), or
    // (b) the payments table shows enough sompi with enough confirmations
    let player_a_paid = player_a_db_confirmed
        || (a_sompi >= required_per_player_sompi && a_min_conf >= MIN_CONF);
    let player_b_paid = player_b_db_confirmed
        || (b_sompi >= required_per_player_sompi && b_min_conf >= MIN_CONF);

    Ok(Json(serde_json::json!({
        "escrow_address": escrow_address,
        "required_per_player_sompi": required_per_player_sompi,
        "min_confirmations_required": MIN_CONF,
        "playerA": {
            "paid": player_a_paid,
            "confirmed_sompi": a_sompi,
            "payment_count": a_count,
            "min_confirmations": a_min_conf,
        },
        "playerB": {
            "paid": player_b_paid,
            "confirmed_sompi": b_sompi,
            "payment_count": b_count,
            "min_confirmations": b_min_conf,
        },
        "both_paid": player_a_paid && player_b_paid,
    })))
}

/// Request body for admin manual match resolution
#[derive(Deserialize)]
pub struct ResolveReq {
    pub winner: String, // "A" (creator) or "B" (opponent)
}

/// Faceit webhook payload (simplified)
#[derive(Deserialize)]
pub struct FaceitWebhookPayload {
    pub event: Option<String>,
    pub match_id: Option<String>,
    pub winner_faceit_id: Option<String>,
}

/// Admin manual match resolution: POST /matches/:id/resolve
/// Body: { "winner": "A" } or { "winner": "B" }
pub async fn admin_resolve_match(
    State(state): State<AppState>,
    _admin: AdminApiKey, // M-03: admin API key required
    Path(id): Path<Uuid>,
    Json(payload): Json<ResolveReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("Admin resolve match {} → winner: {}", id, payload.winner);

    // Get the match
    let m = sqlx::query_as::<_, Match>(&format!("SELECT {} FROM matches WHERE id = $1", crate::models::MATCH_SELECT_COLS))
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Determine winner address
    let creator_addr = get_user_kaspa_address(&state.pool, m.creator_user_id).await?;
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => get_user_kaspa_address(&state.pool, uid).await?,
        None => return Err(StatusCode::BAD_REQUEST), // No opponent yet
    };

    let (winner_address, winner_id) = match payload.winner.as_str() {
        "A" | "a" | "creator" => (&creator_addr, m.creator_user_id.to_string()),
        "B" | "b" | "opponent" => (&opponent_addr, m.opponent_user_id.unwrap().to_string()),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    m.validate_action(&MatchAction::ResolveWinner {
        winner_id: winner_id.clone(),
    })
    .map_err(|e| {
        tracing::warn!("State machine rejected ResolveWinner: {}", e);
        StatusCode::CONFLICT
    })?;

    // Execute payout via PayoutService
    let result = execute_payout_for_match(&state, &m, &escrow_address, winner_address).await?;

    // Update match status to PAID_OUT
    sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(result))
}

/// Faceit webhook handler: POST /webhook/faceit
/// Receives match completion events from Faceit
///
/// M-04: Validates the FaceIT webhook HMAC signature from the
/// `Faceit-Signature` header to ensure requests are authentic.
pub async fn faceit_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Validate HMAC-SHA256 signature
    let webhook_secret = std::env::var("FACEIT_WEBHOOK_SECRET").unwrap_or_default();
    if webhook_secret.is_empty() {
        tracing::error!("FACEIT_WEBHOOK_SECRET not set — rejecting webhook request");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let signature = headers
        .get("Faceit-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
        
    if !verify_faceit_hmac(&body, &webhook_secret, signature) {
        tracing::warn!("Faceit webhook: HMAC signature mismatch — rejecting request");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Deserialize body now that signature is verified
    let payload: FaceitWebhookPayload =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let event = payload.event.unwrap_or_default();
    let match_id = payload.match_id.unwrap_or_default();
    let winner_faceit_id = payload.winner_faceit_id.unwrap_or_default();

    tracing::info!(
        "Faceit webhook verified: event={}, match_id={}, winner={}",
        event,
        match_id,
        winner_faceit_id
    );

    if event != "match_status_finished" && !event.is_empty() {
        return Ok(Json(
            serde_json::json!({ "status": "ignored", "event": event }),
        ));
    }

    // Find match by external_match_id (Faceit match ID)
    let m = sqlx::query_as::<_, Match>(&format!("SELECT {} FROM matches WHERE external_match_id = $1", crate::models::MATCH_SELECT_COLS))
    .bind(&match_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let m = match m {
        Some(m) => m,
        None => {
            tracing::warn!("⚠️ No match found for faceit match_id: {}", match_id);
            return Ok(Json(
                serde_json::json!({ "status": "no_match", "match_id": match_id }),
            ));
        }
    };

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({ "status": "no_escrow" })));
    }

    // Determine winner address from faceit_player_id via faceit_links join
    let winner_user: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT u.id, COALESCE(u.kaspa_address, '') \
         FROM faceit_links fl \
         JOIN users u ON u.id = fl.user_id \
         WHERE fl.faceit_player_id = $1"
    )
    .bind(&winner_faceit_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let winner_address = match winner_user {
        Some((_uid, addr)) if !addr.is_empty() => addr,
        Some((_uid, _)) => {
            tracing::warn!(
                winner_faceit_id = %winner_faceit_id,
                winner_faceit_id = %winner_faceit_id,
                "⚠️ Webhook winner has no Kaspa address linked"
            );
            return Ok(Json(serde_json::json!({
                "status": "winner_no_kaspa_address",
                "winner_faceit_id": winner_faceit_id
            })));
        }
        None => {
            tracing::warn!(
                "⚠️ Winner faceit_player_id {} not found in faceit_links",
                winner_faceit_id
            );
            return Ok(Json(serde_json::json!({
                "status": "winner_not_found",
                "winner_faceit_id": winner_faceit_id
            })));
        }
    };

    // Execute payout
    let result = execute_payout_for_match(&state, &m, &escrow_address, &winner_address).await?;

    // Update match status
    sqlx::query("UPDATE matches SET status = 'PAID_OUT' WHERE id = $1")
        .bind(m.id)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(result))
}

/// Helper: get a user's Kaspa address from their UUID
async fn get_user_kaspa_address(pool: &PgPool, user_id: Uuid) -> Result<String, StatusCode> {
    let row: (String,) = sqlx::query_as("SELECT kaspa_address FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(row.0)
}

/// Helper: execute payout for a match using PayoutService
async fn execute_payout_for_match(
    state: &AppState,
    m: &Match,
    escrow_address: &str,
    winner_address: &str,
) -> Result<serde_json::Value, StatusCode> {
    let payout_svc = state.payout_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ PayoutService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Register escrow key with PayoutService (derive from wallet)
    if let Some(ref wallet) = state.escrow_wallet {
        let match_id_str = m.id.to_string();
        if let Ok((addr, _)) = wallet.derive_escrow_address(&match_id_str) {
            if let Ok(privkey) = wallet.get_private_key(&addr) {
                payout_svc
                    .register_escrow_key(escrow_address.to_string(), privkey)
                    .await;
                tracing::info!("🔑 Escrow key registered for {}", escrow_address);
            }
        }
    }

    // Build a BattleMatch from DB data
    let creator_addr = get_user_kaspa_address(&state.pool, m.creator_user_id)
        .await
        .unwrap_or_else(|_| "unknown".to_string());
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => get_user_kaspa_address(&state.pool, uid)
            .await
            .unwrap_or_else(|_| "unknown".to_string()),
        None => "unknown".to_string(),
    };

    let battle_match = battle_core::models::match_::BattleMatch {
        id: m.id,
        player_a_kas_address: creator_addr,
        player_b_kas_address: opponent_addr,
        player_a_faceit_id: String::new(),
        player_b_faceit_id: String::new(),
        faceit_match_id: m.external_match_id.clone(),
        // IMPORTANT: wager_sompi stores the wager in Sompi (the frontend converts
        // KAS → Sompi before POSTing to /challenges, so no multiplication here).
        // See battle-frontend/src/hooks/useLobby.ts: wager_sompi = stakeKas * 100_000_000
        wager_amount_sompi: m.wager_sompi as u64,
        escrow_address: escrow_address.to_string(),
        status: battle_core::models::match_::MatchStatus::Resolved,
        winner_kas_address: Some(winner_address.to_string()),
        payout_tx_hash: None,
        oracle_result_signature: None,
        created_at: m.created_at.unwrap_or_else(chrono::Utc::now),
        locked_at: None,
        resolved_at: Some(chrono::Utc::now()),
        timeout_at: m.created_at.unwrap_or_else(chrono::Utc::now) + chrono::Duration::minutes(90),
    };

    match payout_svc
        .execute_payout(&battle_match, winner_address)
        .await
    {
        Ok(result) => {
            tracing::info!("✅ Payout executed: TX={}, winner={} sompi, treasury={} sompi",
                result.winner_tx_id, result.winner_amount_sompi, result.treasury_amount_sompi
            );
            Ok(serde_json::json!({
                "status": "PAYOUT_COMPLETE",
                "tx_id": result.winner_tx_id,
                "winner_amount_sompi": result.winner_amount_sompi,
                "treasury_amount_sompi": result.treasury_amount_sompi,
                "fee_sompi": result.fee_sompi,
            }))
        }
        Err(e) => {
            tracing::error!("❌ Payout failed: {:?}", e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

// ── v0.2: FaceID Endpoint ─────────────────────────────────────────────────

/// Request body for FaceID hash submission
#[derive(Deserialize)]
pub struct FaceIdReq {
    pub hash: String, // SHA-256 of biometric template (off-chain, anti-fraud only)
}

/// Global WebSocket connection counter for abuse prevention.
static WS_CONNECTION_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const WS_MAX_CONNECTIONS: usize = 200;
const WS_IDLE_TIMEOUT_SECS: u64 = 60;

pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> axum::response::Response {
    let count = WS_CONNECTION_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    if count >= WS_MAX_CONNECTIONS {
        tracing::warn!("WebSocket connection rejected: limit reached ({}/{})", count, WS_MAX_CONNECTIONS);
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many WebSocket connections").into_response();
    }
    ws.on_upgrade(|socket| websocket(socket, state)).into_response()
}

async fn websocket(stream: axum::extract::ws::WebSocket, state: AppState) {
    WS_CONNECTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (mut sender, mut receiver) = stream.split();
    let mut rx = state.tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(axum::extract::ws::Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        let idle_timeout = std::time::Duration::from_secs(WS_IDLE_TIMEOUT_SECS);
        loop {
            match tokio::time::timeout(idle_timeout, receiver.next()).await {
                Ok(Some(Ok(_))) => { /* keep alive */ }
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    tracing::debug!("WebSocket idle timeout ({}s), closing", WS_IDLE_TIMEOUT_SECS);
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
    WS_CONNECTION_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
}

/// POST /matches/:id/faceid — optional FaceID hash upload
///
/// Player submits a hash of their FaceID verification. Not required for match to
/// proceed (no blocker). Useful for dispute resolution.
pub async fn submit_faceid_handler(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<FaceIdReq>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = user.id;

    let m = load_match_full(&state.pool, id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Determine player role (A = creator, B = opponent) and update accordingly
    if m.creator_user_id == user_id {
        sqlx::query(
            "UPDATE matches SET player_a_faceid_hash = $1 WHERE id = $2"
        )
        .bind(&payload.hash)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tracing::info!("🪪  Match {}: player_a_faceid_hash recorded", id);
    } else if m.opponent_user_id == Some(user_id) {
        sqlx::query(
            "UPDATE matches SET player_b_faceid_hash = $1 WHERE id = $2"
        )
        .bind(&payload.hash)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        tracing::info!("🪪  Match {}: player_b_faceid_hash recorded", id);
    } else {
        return Err(StatusCode::FORBIDDEN);
    };

    let updated = load_match_full(&state.pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(updated))
}

// ── v0.2: Cancel Endpoint ─────────────────────────────────────────────────

/// POST /matches/:id/cancel — cancel an open or pending match
///
/// Only the match creator can cancel, and only while status is OPEN or AWAITING_FUNDING.
/// Does NOT auto-refund deposits (manual process for now).
pub async fn cancel_match_handler(
    State(state): State<AppState>,
    crate::api::auth_guard::SessionUser(user): crate::api::auth_guard::SessionUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    let user_id = user.id;

    let current_match = load_match_full(&state.pool, id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // M-08 Validation
    use battle_core::match_state::MatchAction;
    current_match
        .validate_action(&MatchAction::Cancel {
            player_id: user_id.to_string(),
            reason: "User cancelled match".to_string(),
        })
        .map_err(|e| {
            tracing::warn!("State machine rejected Cancel: {}", e);
            // Special case: if it conflicts, return 409
            StatusCode::CONFLICT
        })?;

    // Transition authorized. EITHER player can cancel if the state machine allows it.
    sqlx::query("UPDATE matches SET status = 'CANCELLED' WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let updated = load_match_full(&state.pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("❌ Match {} cancelled by {}", id, user_id);

    // M-09: Trigger refund process if an escrow address exists
    // PayoutService will check on-chain balance and execute refund if deposits exist
    if let Some(escrow) = updated.escrow_address.as_ref() {
        if !escrow.is_empty() {
            let state_clone = state.clone();
            let m_clone = updated.clone();
            let escrow_clone = escrow.clone();
            tokio::spawn(async move {
                let _ = execute_refund_for_match(&state_clone, &m_clone, &escrow_clone).await;
            });
        }
    }

    Ok(Json(updated))
}


/// Verify a FaceIT webhook HMAC-SHA256 signature.
///
/// FaceIT signs POST bodies with HMAC-SHA256 using the webhook secret and
/// sends the hex-encoded signature in the `Faceit-Signature` header.
/// Returns `true` if the signature matches, `false` otherwise.
fn verify_faceit_hmac(body: &[u8], secret: &str, signature: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let result = mac.finalize().into_bytes();
    let expected_hex = hex::encode(result);

    // Constant-time comparison
    let sig_bytes = signature.as_bytes();
    let exp_bytes = expected_hex.as_bytes();
    if sig_bytes.len() != exp_bytes.len() {
        return false;
    }
    sig_bytes
        .iter()
        .zip(exp_bytes.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Helper: execute refund for a cancelled match using PayoutService
async fn execute_refund_for_match(
    state: &AppState,
    m: &Match,
    escrow_address: &str,
) -> Result<(String, String), StatusCode> {
    let payout_svc = state.payout_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ PayoutService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Register escrow key with PayoutService (derive from wallet)
    if let Some(ref wallet) = state.escrow_wallet {
        let match_id_str = m.id.to_string();
        if let Ok((addr, _)) = wallet.derive_escrow_address(&match_id_str) {
            if let Ok(privkey) = wallet.get_private_key(&addr) {
                payout_svc
                    .register_escrow_key(escrow_address.to_string(), privkey)
                    .await;
                tracing::info!("🔑 Escrow key registered for refund {}", escrow_address);
            }
        }
    }

    // Build a BattleMatch from DB data
    let creator_addr = crate::api::get_user_kaspa_address(&state.pool, m.creator_user_id)
        .await
        .unwrap_or_else(|_| "unknown".to_string());
    let opponent_addr = match m.opponent_user_id {
        Some(uid) => crate::api::get_user_kaspa_address(&state.pool, uid)
            .await
            .unwrap_or_else(|_| "unknown".to_string()),
        None => "unknown".to_string(), // Can be none if cancelled before join
    };

    let battle_match = battle_core::models::match_::BattleMatch {
        id: m.id,
        player_a_kas_address: creator_addr,
        player_b_kas_address: opponent_addr,
        player_a_faceit_id: String::new(),
        player_b_faceit_id: String::new(),
        faceit_match_id: m.external_match_id.clone(),
        wager_amount_sompi: m.wager_sompi as u64,
        escrow_address: escrow_address.to_string(),
        status: battle_core::models::match_::MatchStatus::Cancelled,
        winner_kas_address: None,
        payout_tx_hash: None,
        oracle_result_signature: None,
        created_at: m.created_at.unwrap_or_else(chrono::Utc::now),
        locked_at: None,
        resolved_at: None,
        timeout_at: m.created_at.unwrap_or_else(chrono::Utc::now) + chrono::Duration::minutes(90),
    };

    match payout_svc.execute_refund(&battle_match).await {
        Ok(res) => {
            tracing::info!("✅ Refund executed: TX A={}, TX B={}", res.0, res.1);
            Ok(res)
        }
        Err(e) => {
            tracing::error!("❌ Refund failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_wallet_message(message: &str, private_key_bytes: [u8; 32]) -> (String, String, String) {
        let secret_key =
            secp256k1::SecretKey::from_slice(&private_key_bytes).expect("valid secret key");
        let keypair = secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &secret_key);
        let xonly_public_key = keypair.x_only_public_key().0;

        let mut hasher = PersonalMessageSigningHash::new();
        hasher.write(message.as_bytes());
        let message_hash = hasher.finalize();
        let secp_message = secp256k1::Message::from_digest_slice(message_hash.as_bytes().as_slice())
            .expect("valid digest");
        let signature = keypair.sign_schnorr(secp_message);

        let address = Address::new(Prefix::Testnet, Version::PubKey, &xonly_public_key.serialize());

        (
            address.to_string(),
            hex::encode(xonly_public_key.serialize()),
            hex::encode(signature.as_ref()),
        )
    }

    #[test]
    fn wallet_signature_verification_accepts_valid_signature() {
        let message = "KaspaBattle test login message";
        let (address, public_key, signature) = sign_wallet_message(message, [7u8; 32]);

        let result = verify_wallet_signature(&address, &public_key, message, &signature);

        assert!(result.is_ok());
    }

    #[test]
    fn wallet_signature_verification_rejects_mismatched_address() {
        let message = "KaspaBattle test login message";
        let (_address, public_key, signature) = sign_wallet_message(message, [8u8; 32]);
        let wrong_address = "kaspatest:qp8kz8m4z4v7c5y3m5k0l5u6wqg9h3h6m2y4f8m8z5p4a6f2l3d0gryd5f7c5";

        let result = verify_wallet_signature(wrong_address, &public_key, message, &signature);

        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn wallet_signature_verification_rejects_tampered_message() {
        let (address, public_key, signature) =
            sign_wallet_message("KaspaBattle original message", [9u8; 32]);

        let result = verify_wallet_signature(
            &address,
            &public_key,
            "KaspaBattle tampered message",
            &signature,
        );

        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn challenge_is_active_rejects_used_or_expired_challenges() {
        let now = chrono::Utc::now();

        assert!(challenge_is_active(now + chrono::Duration::minutes(5), None, now));
        assert!(!challenge_is_active(
            now + chrono::Duration::minutes(5),
            Some(now),
            now,
        ));
        assert!(!challenge_is_active(
            now - chrono::Duration::seconds(1),
            None,
            now,
        ));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 2: FaceIT Match-ID Submit (F-010)
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/v1/matches/{id}/faceit-match-id
///
/// A match participant submits the FaceIT match ID for the active lobby.
/// - First submission: stores per-player field, transitions FUNDED→GAME_ID_INPUT.
/// - Second submission (same ID): sets faceit_match_id_final, transitions →IN_GAME,
///   creates faceit_watcher_jobs row.
/// - Mismatch: returns 409 Conflict.
pub async fn submit_faceit_match_id(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
    Json(payload): Json<SubmitFaceitMatchIdReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let api_err = |status: StatusCode, code: &'static str, msg: &str| {
        (status, Json(serde_json::json!({"error": code, "message": msg})))
    };

    let caller_id = user.id;

    // Validate UUID format
    let faceit_id = payload.faceit_match_id.trim().to_string();
    if Uuid::parse_str(&faceit_id).is_err() {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "invalid_faceit_match_id",
            "faceit_match_id must be a valid UUID (e.g. from the FaceIT match URL)",
        ));
    }

    // Load match
    let m = load_match_full(&state.pool, match_id).await.map_err(|_| {
        api_err(StatusCode::NOT_FOUND, "match_not_found", "Match not found")
    })?;

    // Auth: caller must be a participant
    let is_creator = m.creator_user_id == caller_id;
    let is_opponent = m.opponent_user_id.map(|id| id == caller_id).unwrap_or(false);
    if !is_creator && !is_opponent {
        return Err(api_err(
            StatusCode::FORBIDDEN,
            "not_participant",
            "Only match participants can submit the FaceIT match ID",
        ));
    }

    // Status check: must be FUNDED or GAME_ID_INPUT
    use crate::models::MatchStatus;
    match m.status {
        MatchStatus::Funded | MatchStatus::GameIdInput => {}
        _ => {
            return Err(api_err(
                StatusCode::CONFLICT,
                "wrong_status",
                &format!("Cannot submit FaceIT match ID when match is in status {:?}", m.status),
            ));
        }
    }

    // Optional: verify match exists on FaceIT (use FaceitDataService if available)
    // We skip the live API call here to keep this path fast and avoid blocking on
    // network errors — the FaceIT Watcher will detect invalid IDs via 404/errors.

    // Determine which player column to update
    let player_field = if is_creator { "faceit_match_id_player_a" } else { "faceit_match_id_player_b" };

    // Update this player's submission field + transition to GAME_ID_INPUT if still FUNDED
    sqlx::query(&format!(
        "UPDATE matches SET {player_field} = $1, \
         status = CASE WHEN status = 'FUNDED' THEN 'GAME_ID_INPUT'::match_status ELSE status END \
         WHERE id = $2",
    ))
    .bind(&faceit_id)
    .bind(match_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(match_id = %match_id, error = %e, "submit_faceit_match_id: DB error");
        api_err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "Failed to save FaceIT match ID")
    })?;

    // Reload to check if both players have submitted
    let updated = load_match_full(&state.pool, match_id).await.map_err(|_| {
        api_err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "Failed to reload match")
    })?;

    // Read the *other* player's already-submitted ID from the struct
    let other_val_opt = if is_creator {
        updated.faceit_match_id_player_b.clone()
    } else {
        updated.faceit_match_id_player_a.clone()
    };
    let other_val = other_val_opt.as_deref().unwrap_or("");

    // Both submitted?
    if !other_val.is_empty() {
        if other_val != faceit_id.as_str() {
            // Mismatch — notify via WS but don't block
            let _ = state.tx.send(serde_json::json!({
                "type": "faceit_id_mismatch",
                "match_id": match_id.to_string(),
            }).to_string());
            return Err(api_err(
                StatusCode::CONFLICT,
                "faceit_id_mismatch",
                "Both players submitted different FaceIT match IDs. Please verify and resubmit.",
            ));
        }

        // IDs match → finalize and transition to IN_GAME
        sqlx::query(
            "UPDATE matches \
             SET faceit_match_id_final = $1, \
                 status = 'IN_GAME'::match_status \
             WHERE id = $2 AND status = 'GAME_ID_INPUT'",
        )
        .bind(&faceit_id)
        .bind(match_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(match_id = %match_id, error = %e, "submit_faceit_match_id: finalize error");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "Failed to finalize match ID")
        })?;

        // Create watcher job (idempotent via ON CONFLICT DO NOTHING)
        sqlx::query(
            "INSERT INTO faceit_watcher_jobs (match_id, faceit_match_id) \
             VALUES ($1, $2) ON CONFLICT (match_id) DO NOTHING",
        )
        .bind(match_id)
        .bind(&faceit_id)
        .execute(&state.pool)
        .await
        .ok(); // non-fatal

        tracing::info!(
            match_id = %match_id,
            faceit_match_id = %faceit_id,
            "✅ FaceIT match ID confirmed by both players → IN_GAME"
        );

        let _ = state.tx.send(serde_json::json!({
            "type": "match_status",
            "match_id": match_id.to_string(),
            "status": "IN_GAME",
            "faceit_match_id": faceit_id,
        }).to_string());

        return Ok(Json(serde_json::json!({
            "status": "confirmed",
            "both_submitted": true,
            "match_status": "IN_GAME",
            "faceit_match_id": faceit_id,
        })));
    }

    // Only one player has submitted so far
    let _ = state.tx.send(serde_json::json!({
        "type": "faceit_id_submitted",
        "match_id": match_id.to_string(),
        "submitted_by": caller_id.to_string(),
    }).to_string());

    tracing::info!(
        match_id = %match_id,
        caller = %caller_id,
        faceit_match_id = %faceit_id,
        "FaceIT match ID submitted (waiting for other player)"
    );

    Ok(Json(serde_json::json!({
        "status": "submitted",
        "both_submitted": false,
        "match_status": "GAME_ID_INPUT",
        "message": "Waiting for the other player to submit the same FaceIT match ID",
    })))
}

#[derive(Deserialize)]
pub struct SubmitFaceitMatchIdReq {
    pub faceit_match_id: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4b: Payout Flow Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Request body for the payout trigger endpoint (MVP: backend-held-keys, signature not verified).
#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SubmitSignatureReq {
    pub signature_hex: Option<String>,
    pub signed_tx_hex: Option<String>,
}

/// GET /api/v1/matches/{id}/payout/pskt
///
/// Returns the oracle-signed PSKT to the winner.
pub async fn get_payout_pskt(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let api_err = |status: StatusCode, code: &'static str, msg: &str| {
        (status, Json(serde_json::json!({"error": code, "message": msg})))
    };

    let caller_id = user.id;

    let m = load_match_full(&state.pool, match_id).await.map_err(|_| {
        api_err(StatusCode::NOT_FOUND, "match_not_found", "Match not found")
    })?;

    let winner_id = m.winner_user_id.ok_or_else(|| {
        api_err(StatusCode::CONFLICT, "no_winner", "No winner determined yet")
    })?;
    if winner_id != caller_id {
        return Err(api_err(StatusCode::FORBIDDEN, "not_winner", "Only the winner may access the PSKT"));
    }

    use crate::models::MatchStatus;
    if m.status != MatchStatus::ReadyForPayout {
        return Err(api_err(
            StatusCode::CONFLICT,
            "wrong_status",
            &format!("PSKT only available when READY_FOR_PAYOUT. Current: {:?}", m.status),
        ));
    }

    let pskt_hex = m.payout_pskt_hex.clone().ok_or_else(|| {
        api_err(StatusCode::INTERNAL_SERVER_ERROR, "pskt_missing", "PSKT not generated yet")
    })?;

    tracing::info!(match_id = %match_id, caller = %caller_id, "get_payout_pskt");

    let pskt_json: serde_json::Value = hex::decode(&pskt_hex)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .unwrap_or_else(|| serde_json::json!({"raw_hex": &pskt_hex}));

    Ok(Json(serde_json::json!({
        "match_id": match_id.to_string(),
        "pskt_hex": pskt_hex,
        "pskt": pskt_json,
        "payout_status": m.payout_status,
        "winner_user_id": winner_id.to_string(),
    })))
}

/// POST /api/v1/matches/{id}/payout/submit-signature
///
/// Backend-held-keys MVP: winner triggers payout, backend signs + broadcasts TX → RESOLVED.
/// Note: signature_hex is NOT verified in this MVP — auth is session + status guard only.
pub async fn submit_payout_signature(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
    Json(payload): Json<SubmitSignatureReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let api_err = |status: StatusCode, code: &'static str, msg: &str| {
        (status, Json(serde_json::json!({"error": code, "message": msg})))
    };

    let caller_id = user.id;

    let m = load_match_full(&state.pool, match_id).await.map_err(|_| {
        api_err(StatusCode::NOT_FOUND, "match_not_found", "Match not found")
    })?;

    let winner_id = m.winner_user_id.ok_or_else(|| {
        api_err(StatusCode::CONFLICT, "no_winner", "No winner determined yet")
    })?;
    if winner_id != caller_id {
        return Err(api_err(StatusCode::FORBIDDEN, "not_winner", "Only the winner may submit the signature"));
    }

    use crate::models::MatchStatus;
    if m.status != MatchStatus::ReadyForPayout {
        return Err(api_err(
            StatusCode::CONFLICT,
            "wrong_status",
            &format!("Signature only accepted when READY_FOR_PAYOUT. Current: {:?}", m.status),
        ));
    }

    if m.payout_tx_hash.is_some() {
        return Err(api_err(StatusCode::CONFLICT, "already_broadcast", "Payout TX already broadcast"));
    }

    tracing::info!(
        match_id = %match_id, caller = %caller_id,
        has_sig = payload.signature_hex.is_some(),
        "submit_payout_signature"
    );

    if let Some(ref multisig_svc) = state.multisig_service {
        // Load winner kaspa address
        let winner_addr = sqlx::query("SELECT kaspa_address FROM users WHERE id = $1")
            .bind(winner_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "submit_payout_signature: DB error");
                api_err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "Failed to load winner address")
            })?
            .and_then(|row| row.try_get::<Option<String>, _>("kaspa_address").ok().flatten())
            .ok_or_else(|| api_err(StatusCode::CONFLICT, "no_kaspa_address", "Winner has no Kaspa address linked"))?;

        // Restore escrow from DB if needed
        if multisig_svc.get_escrow(&match_id).await.is_none() {
            if let Ok(Some(er)) = sqlx::query(
                "SELECT pubkey_a_hex, pubkey_b_hex, pubkey_oracle_hex, redeem_script_hex, \
                 p2sh_address, wager_per_player_sompi, timelock_timestamp \
                 FROM multisig_escrows WHERE match_id = $1",
            )
            .bind(match_id)
            .fetch_optional(&state.pool)
            .await {
                let pk_a: String     = er.try_get("pubkey_a_hex").unwrap_or_default();
                let pk_b: String     = er.try_get("pubkey_b_hex").unwrap_or_default();
                let pk_o: String     = er.try_get("pubkey_oracle_hex").unwrap_or_default();
                let rs: String       = er.try_get("redeem_script_hex").unwrap_or_default();
                let p2sh: String     = er.try_get("p2sh_address").unwrap_or_default();
                let wager: i64       = er.try_get("wager_per_player_sompi").unwrap_or(0);
                let tl: Option<i64>  = er.try_get("timelock_timestamp").unwrap_or(None);
                multisig_svc.restore_escrow_from_row(
                    match_id, pk_a, pk_b, pk_o, rs, p2sh,
                    wager as u64, tl.map(|v| v as u64),
                ).await;
            }
        }

        match multisig_svc.execute_payout(&match_id, &winner_addr).await {
            Ok(result) => {
                tracing::info!(
                    match_id = %match_id, tx_id = %result.tx_id,
                    winner_sompi = result.winner_amount_sompi,
                    "✅ Payout TX broadcast"
                );
                sqlx::query(
                    "UPDATE matches SET payout_tx_hash = $1, payout_status = 'broadcast', \
                     status = 'RESOLVED' WHERE id = $2 AND status = 'READY_FOR_PAYOUT'",
                )
                .bind(&result.tx_id)
                .bind(match_id)
                .execute(&state.pool)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "submit_payout_signature: DB update failed after broadcast");
                    api_err(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "TX broadcast OK but DB update failed")
                })?;

                let _ = state.tx.send(serde_json::json!({
                    "type": "payout_broadcast",
                    "match_id": match_id.to_string(),
                    "tx_id": result.tx_id,
                    "status": "RESOLVED",
                }).to_string());

                return Ok(Json(serde_json::json!({
                    "status": "broadcast",
                    "tx_id": result.tx_id,
                    "winner_amount_sompi": result.winner_amount_sompi,
                    "platform_fee_sompi": result.platform_fee_sompi,
                    "network_fee_sompi": result.network_fee_sompi,
                    "match_status": "RESOLVED",
                })));
            }
            Err(e) => {
                tracing::error!(match_id = %match_id, error = %e, "execute_payout failed");
                return Err(api_err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "payout_failed",
                    &format!("Payout failed: {}", e),
                ));
            }
        }
    }

    Err(api_err(
        StatusCode::SERVICE_UNAVAILABLE,
        "no_multisig_service",
        "Multisig payout service not available",
    ))
}

/// GET /api/v1/matches/{id}/payout/status
///
/// Returns current payout progress to any match participant.
pub async fn get_payout_status(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,
    Path(match_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let api_err = |status: StatusCode, code: &'static str, msg: &str| {
        (status, Json(serde_json::json!({"error": code, "message": msg})))
    };

    let caller_id = user.id;

    let m = load_match_full(&state.pool, match_id).await.map_err(|_| {
        api_err(StatusCode::NOT_FOUND, "match_not_found", "Match not found")
    })?;

    let is_participant = m.creator_user_id == caller_id
        || m.opponent_user_id.map(|id| id == caller_id).unwrap_or(false);
    if !is_participant {
        return Err(api_err(StatusCode::FORBIDDEN, "not_participant", "Only match participants may view payout status"));
    }

    let kaspa_network = std::env::var("KASPA_NETWORK").unwrap_or_else(|_| "testnet-12".to_string());
    let explorer_base = if kaspa_network.contains("mainnet") {
        "https://explorer.kaspa.org/txs"
    } else {
        "https://explorer-tn12.kaspa.org/txs"
    };
    let explorer_url = m.payout_tx_hash.as_ref().map(|tx| format!("{}/{}", explorer_base, tx));

    Ok(Json(serde_json::json!({
        "match_id": match_id.to_string(),
        "match_status": format!("{:?}", m.status),
        "payout_status": m.payout_status,
        "payout_tx_hash": m.payout_tx_hash,
        "winner_user_id": m.winner_user_id.map(|id| id.to_string()),
        "loser_user_id": m.loser_user_id.map(|id| id.to_string()),
        "faceit_score": m.faceit_score,
        "faceit_winner_faction": m.faceit_winner_faction,
        "explorer_url": explorer_url,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: load_match_full — loads all v1.0 columns for a single match
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_match_full(
    pool: &sqlx::PgPool,
    match_id: Uuid,
) -> Result<crate::models::Match, sqlx::Error> {
    // CQ-02: Uses canonical MATCH_COLUMNS — single source of truth for all match SELECT queries.
    sqlx::query_as::<_, crate::models::Match>(&format!(
        "SELECT {} FROM matches WHERE id = $1",
        crate::models::MATCH_COLUMNS,
    ))
    .bind(match_id)
    .fetch_one(pool)
    .await
}
