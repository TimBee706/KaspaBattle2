pub mod admin_guard;
pub mod auth_guard;
pub mod faceit;
pub mod multisig;

use crate::api::{admin_guard::AdminApiKey, auth_guard::SessionUser};
use crate::models::{Match, MatchMode, MatchStatus};
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        Path, State,
    },
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

#[derive(Serialize, Deserialize)]
pub struct CreateReq {
    pub game_id: String,
    pub stake_kas: i64,
    pub mode: MatchMode,
    pub escrow_address: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DepositReq {
    pub tx_hash: String,
    pub player_role: String, // "A" oder "B"
}

pub async fn simulate_deposit_test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Match>, StatusCode> {
    if std::env::var("TEST_MODE").unwrap_or_default() != "true" {
        return Err(StatusCode::FORBIDDEN);
    }

    let dummy_tx = "fake_tx_testmode_123".to_string();

    let mut record = sqlx::query_as::<_, Match>(
        "UPDATE matches SET status = 'LOCKED' WHERE id = $1 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    record.calculate_wager();

    let _ = state
        .tx
        .send(serde_json::to_string(&record).unwrap_or_default());
    Ok(Json(record))
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
    pub kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaRpc>>,
    pub payout_service: Option<Arc<battle_kaspa::payout::PayoutService>>,
    /// Watcher used by the payment-status endpoint and episode runner
    pub blockchain_watcher: Option<Arc<battle_kaspa::watcher::BlockchainWatcher>>,
    pub multisig_service: Option<Arc<battle_kaspa::multisig::service::MultisigEscrowService>>,
}


pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lobbies", get(get_lobbies))
        .route("/history", get(get_history))
        .route("/challenges", post(create_challenge))
        .route("/matches/:id/accept", post(join_challenge))
        .route("/matches/:id/deposit", post(submit_deposit))
        .route("/matches/:id/deposits", get(check_deposits))
        .route("/matches/:id/payment-status", get(get_payment_status))
        .route("/matches/:id/resolve", post(admin_resolve_match))
        // ── v0.2 ──
        .route("/matches/:id/faceid", post(submit_faceid_handler))
        .route("/matches/:id/cancel", post(cancel_match_handler))
        // NOTE: /lobbies/:id/simulate-deposit removed — test-only endpoint
        .route("/webhook/faceit", post(faceit_webhook))
        .route("/auth/me", get(get_me))
        .route("/auth/logout", post(logout))
        .route("/auth/me/wallet", axum::routing::patch(update_wallet_address))
        .route("/auth/wallet-challenge", post(create_wallet_login_challenge))
        .route("/auth/wallet-verify", post(verify_wallet_login))
        .route("/ws", get(ws_handler))
        .nest("/faceit", faceit::router())
        .nest("/multisig", multisig::router())
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
}

fn build_auth_cookie(session_token: &str) -> String {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let is_secure = frontend_url.starts_with("https");
    let same_site = if is_secure { "None" } else { "Lax" };
    let secure_flag = if is_secure { "; Secure" } else { "" };

    format!(
        "kaspabattle-auth={}; HttpOnly; Path=/; SameSite={}{}; Max-Age=604800",
        session_token, same_site, secure_flag
    )
}

fn build_clear_auth_cookie() -> String {
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let is_secure = frontend_url.starts_with("https");
    let same_site = if is_secure { "None" } else { "Lax" };
    let secure_flag = if is_secure { "; Secure" } else { "" };

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
) -> Result<Json<WalletChallengeResponse>, StatusCode> {
    eprintln!("[wallet-challenge] Request for address: {}", payload.kaspa_address);

    if !payload.kaspa_address.starts_with("kaspa:") && !payload.kaspa_address.starts_with("kaspatest:") {
        eprintln!("[wallet-challenge] Rejected: invalid address prefix");
        return Err(StatusCode::BAD_REQUEST);
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
        eprintln!("[wallet-challenge] DB insert failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    eprintln!("[wallet-challenge] ✅ Challenge created: {} for {}", challenge_id, payload.kaspa_address);

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
    eprintln!("[wallet-verify] Request: challenge_id={}, addr={}", payload.challenge_id, payload.kaspa_address);

    let challenge_id = Uuid::parse_str(&payload.challenge_id).map_err(|e| {
        eprintln!("[wallet-verify] Invalid challenge_id: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    let challenge_row = sqlx::query(
        "SELECT kaspa_address, challenge_message, expires_at, used_at FROM wallet_login_challenges WHERE id = $1"
    )
    .bind(challenge_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        eprintln!("[wallet-verify] DB query failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        eprintln!("[wallet-verify] Challenge not found: {}", challenge_id);
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
        eprintln!("[wallet-verify] Address mismatch: challenge={} vs payload={}", challenge_address, payload.kaspa_address);
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !challenge_is_active(challenge_expires_at, challenge_used_at, chrono::Utc::now()) {
        eprintln!("[wallet-verify] Challenge expired or used: expires_at={}, used_at={:?}", challenge_expires_at, challenge_used_at);
        return Err(StatusCode::UNAUTHORIZED);
    }

    eprintln!("[wallet-verify] Verifying signature for {} (pubkey={}...)", payload.kaspa_address, &payload.public_key[..16.min(payload.public_key.len())]);
    verify_wallet_signature(
        &payload.kaspa_address,
        &payload.public_key,
        &challenge_message,
        &payload.signature,
    ).map_err(|status| {
        eprintln!("[wallet-verify] ❌ Signature verification failed (status={})", status);
        status
    })?;
    eprintln!("[wallet-verify] ✅ Signature valid for {}", payload.kaspa_address);

    sqlx::query("UPDATE wallet_login_challenges SET used_at = NOW() WHERE id = $1")
        .bind(challenge_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("[wallet-verify] DB update failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    eprintln!("[wallet-verify] Challenge marked as used, proceeding to user lookup...");

    // 2. Prüfen ob der Caller BEREITS authentifiziert ist (FaceIT-Session aktiv?)
    let existing_token_opt = extract_session_token_from_headers(&headers);
    eprintln!("[wallet-verify] Existing session token: {}", if existing_token_opt.is_some() { "found" } else { "none" });

    let auth_service = battle_core::auth::AuthService::new(state.pool.clone());

    // Wenn bereits authentifiziert → Wallet an bestehenden User verknüpfen
    if let Some(existing_token) = existing_token_opt {
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
                eprintln!("[wallet-verify] DB conflict check failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            if conflict.is_some() {
                eprintln!("[wallet_login] Clearing kaspa_address from conflicting guest user");
                sqlx::query(
                    "UPDATE users SET kaspa_address = NULL WHERE kaspa_address = $1 AND id != $2::uuid"
                )
                .bind(&payload.kaspa_address)
                .bind(&existing_user.id)
                .execute(&state.pool)
                .await
                .map_err(|e| {
                    eprintln!("[wallet-verify] DB clear conflict failed: {}", e);
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
                    eprintln!("[wallet_login] UPDATE kaspa_address failed: {e}");
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
    let uuid_str = format!("{:x}", md5::compute(payload.kaspa_address.as_bytes()));
    let fake_uuid = uuid::Uuid::parse_str(&format!(
        "{}-{}-{}-{}-{}",
        &uuid_str[0..8], &uuid_str[8..12], &uuid_str[12..16],
        &uuid_str[16..20], &uuid_str[20..32]
    )).unwrap();

    let row = sqlx::query("SELECT id FROM users WHERE kaspa_address = $1")
        .bind(&payload.kaspa_address)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("[wallet-verify] DB user lookup failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let (user_id, display_name_str) = if let Some(r) = row {
        let uid: uuid::Uuid = r.try_get("id").unwrap_or(fake_uuid);
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
                eprintln!("Error creating user: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        (new_user_id, display_name)
    };

    // 4. Session generieren
    let session_token = battle_core::auth::AuthService::generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&session_token).bind(&user_id).bind(expires_at)
        .execute(&state.pool).await.map_err(|e| {
            eprintln!("[wallet-verify] DB session insert failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    eprintln!("[wallet-verify] ✅ Session created for user {} ({})", user_id, display_name_str);

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
    let user_id = uuid::Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("UPDATE users SET kaspa_address = $1 WHERE id = $2")
        .bind(&payload.kaspa_address)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("Error updating kaspa_address: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "kaspa_address": payload.kaspa_address,
    })))
}

pub async fn get_lobbies(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status IN ('OPEN', 'AWAITING_FUNDING', 'LOCKED', 'IN_GAME') ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    for m in &mut matches {
        m.calculate_wager();
    }

    Ok(Json(matches))
}

pub async fn get_history(State(state): State<AppState>) -> Result<Json<Vec<Match>>, StatusCode> {
    let mut matches = sqlx::query_as::<_, Match>("SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE status = 'RESOLVED'")
        .fetch_all(&state.pool).await.map_err(|e| {
            eprintln!("SQL Error in get_history: {:?}", e);
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
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    let record = sqlx::query_as::<_, Match>(
        "INSERT INTO matches (creator_user_id, game_id, stake_kas, mode) VALUES ($1, $2, $3, $4) RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
    .bind(user_id)
    .bind(&payload.game_id)
    .bind(payload.stake_kas)
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
        "UPDATE matches SET escrow_address = $1 WHERE id = $2 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at",
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

    let joiner_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_match: Match = sqlx::query_as(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE id = $1"
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
    let mut updated = sqlx::query_as::<_, Match>(
        "UPDATE matches SET opponent_user_id = $1, status = $3 WHERE id = $2 AND status = $4 AND creator_user_id != $1 RETURNING id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at"
    )
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
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at, player_a_deposit_tx_hash, player_b_deposit_tx_hash FROM matches WHERE id = $1"
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
        eprintln!(
            "❌ Deposit rejected for match {}: status is {:?}",
            id, m.status
        );
        return Err(StatusCode::CONFLICT);
    }

    // Step 3: Determine which player column to update based on player_role
    // player_role "A" = creator, "B" = opponent
    let (tx_col, confirmed_col) = match payload.player_role.to_uppercase().as_str() {
        "A" => ("player_a_deposit_tx_hash", "player_a_deposit_confirmed"),
        "B" => ("player_b_deposit_tx_hash", "player_b_deposit_confirmed"),
        _ => {
            eprintln!(
                "❌ submit_deposit: invalid player_role '{}'",
                payload.player_role
            );
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Step 3b: Double-deposit guard — prevent overwriting an existing tx_hash
    let existing_tx: Option<String> = match payload.player_role.to_uppercase().as_str() {
        "A" => m.player_a_deposit_tx_hash.clone(),
        "B" => m.player_b_deposit_tx_hash.clone(),
        _ => None,
    };

    if let Some(ref existing) = existing_tx {
        tracing::warn!(
            match_id = %id,
            player_role = %payload.player_role,
            existing_tx = %existing,
            new_tx = %payload.tx_hash,
            "⚠️ Double deposit attempt blocked"
        );
        eprintln!(
            "⚠️ Double deposit blocked: match={}, role={}, existing_tx={}",
            id, payload.player_role, existing
        );
        return Err(StatusCode::CONFLICT);
    }

    // Step 4: Record this player's deposit TX hash (DO NOT mark as confirmed yet - Episode handles this)
    sqlx::query(&format!(
        "UPDATE matches SET {} = $1 WHERE id = $2",
        tx_col
    ))
    .bind(&payload.tx_hash)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        eprintln!("❌ SQL error recording deposit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Step 5: Check whether BOTH players have now deposited
    let updated: Match = sqlx::query_as(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE id = $1"
    )
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("submit_deposit: failed to re-fetch match {}: {:?}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let a_confirmed = updated.player_a_deposit_confirmed.unwrap_or(false);
    let b_confirmed = updated.player_b_deposit_confirmed.unwrap_or(false);

    // We no longer transition to FUNDED here. The MatchEpisode (Blockchain Watcher)
    // is responsible for confirming the actual UTXO and setting the status.
    eprintln!(
        "⏳ Deposit TX recorded for match {}. Waiting for blockchain confirmation.",
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
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let escrow_svc = state.escrow_service.as_ref().ok_or_else(|| {
        eprintln!("❌ EscrowService not available");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Look up the match to get escrow_address and stake_kas
    let row: (Option<String>, i64) =
        sqlx::query_as("SELECT escrow_address, stake_kas FROM matches WHERE id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

    let escrow_address = row.0.unwrap_or_default();
    let stake_kas = row.1 as u64;

    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "NO_ESCROW",
            "message": "No escrow address assigned to this match"
        })));
    }

    match escrow_svc.check_deposits(&escrow_address, stake_kas).await {
        Ok(status) => Ok(Json(serde_json::to_value(status).unwrap())),
        Err(e) => {
            eprintln!("❌ check_deposits failed: {:?}", e);
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
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Fetch match basics
    let row: (Option<String>, i64, Option<bool>, Option<bool>) = sqlx::query_as(
        "SELECT escrow_address, stake_kas, player_a_deposit_confirmed, player_b_deposit_confirmed \
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
    let m = sqlx::query_as::<_, Match>("SELECT * FROM matches WHERE id = $1")
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
    if !webhook_secret.is_empty() {
        let signature = headers
            .get("Faceit-Signature")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !verify_faceit_hmac(&body, &webhook_secret, signature) {
            tracing::warn!("Faceit webhook: HMAC signature mismatch — rejecting request");
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        tracing::warn!("FACEIT_WEBHOOK_SECRET not set — skipping signature validation (insecure!)");
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
    let m = sqlx::query_as::<_, Match>(
        "SELECT id, onchain_match_id, COALESCE(escrow_address, '') as escrow_address, creator_user_id, opponent_user_id, game_id, stake_kas, mode, status, external_match_id, created_at FROM matches WHERE external_match_id = $1"
    )
    .bind(&match_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let m = match m {
        Some(m) => m,
        None => {
            eprintln!("⚠️ No match found for faceit match_id: {}", match_id);
            return Ok(Json(
                serde_json::json!({ "status": "no_match", "match_id": match_id }),
            ));
        }
    };

    let escrow_address = m.escrow_address.clone().unwrap_or_default();
    if escrow_address.is_empty() {
        return Ok(Json(serde_json::json!({ "status": "no_escrow" })));
    }

    // Determine winner address from faceit_id
    // Look up which user has this faceit_id
    let winner_user: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, kaspa_address FROM users WHERE faceit_id = $1")
            .bind(&winner_faceit_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let winner_address = match winner_user {
        Some((_, addr)) => addr,
        None => {
            eprintln!(
                "⚠️ Winner faceit_id {} not found in users",
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
        eprintln!("❌ PayoutService not available");
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
                eprintln!("🔑 Escrow key registered for {}", escrow_address);
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
        // IMPORTANT: stake_kas stores the wager in Sompi (the frontend converts
        // KAS → Sompi before POSTing to /challenges, so no multiplication here).
        // See battle-frontend/src/hooks/useLobby.ts: stake_kas = stakeKas * 100_000_000
        wager_amount_sompi: m.stake_kas as u64,
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
            eprintln!(
                "✅ Payout executed: TX={}, winner={} sompi, treasury={} sompi",
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
            eprintln!("❌ Payout failed: {:?}", e);
            Ok(serde_json::json!({
                "status": "PAYOUT_FAILED",
                "error": format!("{}", e),
            }))
        }
    }
}

// ── v0.2: FaceID Endpoint ─────────────────────────────────────────────────

/// Request body for FaceID hash submission
#[derive(Deserialize)]
pub struct FaceIdReq {
    pub hash: String, // SHA-256 of biometric template (off-chain, anti-fraud only)
}

pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: axum::extract::ws::WebSocket, state: AppState) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = state.tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // axum 0.7 requires Utf8Bytes, which implements From<String>
            if sender.send(axum::extract::ws::Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {
            // keep-alive / ignore incoming messages
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
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
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let m: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Determine player role (A = creator, B = opponent)
    let col = if m.creator_user_id == user_id {
        "player_a_faceid_hash"
    } else if m.opponent_user_id == Some(user_id) {
        "player_b_faceid_hash"
    } else {
        return Err(StatusCode::FORBIDDEN);
    };

    let updated: Match = sqlx::query_as(&format!(
        "UPDATE matches SET {} = $1 WHERE id = $2 RETURNING *",
        col
    ))
    .bind(&payload.hash)
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    eprintln!("🪪  Match {}: {} FaceID hash recorded", id, col);
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
    let user_id = Uuid::parse_str(&user.id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let current_match: Match = sqlx::query_as("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
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
    let updated: Match = sqlx::query_as(
        "UPDATE matches SET status = 'CANCELLED' \
         WHERE id = $1 \
         RETURNING *",
    )
    .bind(id)
    .fetch_one(&state.pool)
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
        eprintln!("❌ PayoutService not available");
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
                eprintln!("🔑 Escrow key registered for refund {}", escrow_address);
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
        wager_amount_sompi: m.stake_kas as u64,
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
            eprintln!("✅ Refund executed: TX A={}, TX B={}", res.0, res.1);
            Ok(res)
        }
        Err(e) => {
            eprintln!("❌ Refund failed: {:?}", e);
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
