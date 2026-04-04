use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue},
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::api::AppState;

/// Maskiert eine FACEIT Player-ID für pseudonymisiertes Logging (zeigt nur erste 6 Zeichen).
fn mask_faceit_id(id: &str) -> String {
    if id.len() > 6 {
        format!("{}...", &id[..6])
    } else {
        id.to_string()
    }
}

/// Cache TTL in Sekunden (5 Minuten).
const FACEIT_CACHE_TTL_SECS: i64 = 300;

/// Erlaubte game_id-Werte für FACEIT Data API Calls (Whitelist, verhindert Path-Injection).
const ALLOWED_GAME_IDS: &[&str] = &["cs2", "csgo", "dota2", "valorant", "lol", "rocket_league"];

/// Prüft ob eine game_id erlaubt ist. Gibt bereinigten String zurück oder Fehler.
fn validate_game_id(game_id: &str) -> Result<&str, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    if ALLOWED_GAME_IDS.contains(&game_id) {
        Ok(game_id)
    } else {
        Err((
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "invalid_game_id",
                "message": format!(
                    "'{}' is not a supported game_id. Allowed: {}",
                    game_id,
                    ALLOWED_GAME_IDS.join(", ")
                )
            })),
        ))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_faceit))
        .route("/auth-url", get(auth_url_faceit))
        .route("/link", get(link_faceit))
        .route("/callback", get(faceit_callback))
        .route("/session-bounce", get(session_bounce))
        .route("/status", get(faceit_status))
        .route("/profile", get(faceit_profile))
        .route("/stats", get(faceit_stats))
        .route("/matches", get(faceit_matches))
        .route("/disconnect", post(faceit_disconnect))
}

// ── /faceit/login ──────────────────────────────────────────────────────────

fn default_frontend_url() -> String {
    std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string())
}

#[allow(dead_code)]
fn sanitize_return_to(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return None;
    }

    let without_fragment = trimmed.split('#').next()?.trim_end_matches('/');

    Some(without_fragment.to_string())
}

#[allow(dead_code)]
fn infer_return_to(headers: &HeaderMap) -> String {
    let header_candidates = [
        headers.get("origin"),
        headers.get("referer"),
        headers.get("x-forwarded-origin"),
    ];

    for value in header_candidates.into_iter().flatten() {
        if let Ok(raw) = value.to_str() {
            if let Some(url) = sanitize_return_to(raw) {
                return url;
            }
        }
    }

    if let (Some(proto), Some(host)) = (
        headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok()),
        headers
            .get("x-forwarded-host")
            .and_then(|v| v.to_str().ok()),
    ) {
        let forwarded = format!("{}://{}", proto, host);
        if let Some(url) = sanitize_return_to(&forwarded) {
            return url;
        }
    }

    default_frontend_url()
}

fn append_query_param(base: &str, key: &str, value: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{}{}{}={}", base, separator, key, value)
}

// ── /faceit/auth-url (JSON — für Frontend-navigierten OAuth-Flow) ───────────

#[derive(Serialize)]
struct AuthUrlResponse {
    url: String,
}

async fn auth_url_faceit(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthUrlResponse>, axum::http::StatusCode> {
    let return_to = infer_return_to(&headers);
    tracing::info!("🔐 FACEIT auth-url: return_to={}", return_to);

    match state
        .faceit_service
        .generate_auth_url(None, Some(return_to))
        .await
    {
        Ok((url, _)) => {
            tracing::info!("🔐 FACEIT auth URL (JSON): {}", url);
            Ok(Json(AuthUrlResponse { url }))
        }
        Err(e) => {
            tracing::error!("❌ FACEIT auth-url generation failed: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn login_faceit(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Redirect, axum::http::StatusCode> {
    let return_to = infer_return_to(&headers);
    tracing::info!("🔐 FACEIT login: return_to={}", return_to);

    match state
        .faceit_service
        .generate_auth_url(None, Some(return_to))
        .await
    {
        Ok((url, _)) => {
            tracing::info!("🔐 FACEIT auth URL: {}", url);
            Ok(Redirect::temporary(&url))
        }
        Err(e) => {
            tracing::error!("❌ FACEIT auth URL generation failed");
            tracing::info!("  Detail: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn link_faceit(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
    headers: HeaderMap,
) -> Result<Redirect, axum::http::StatusCode> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;
    let return_to = infer_return_to(&headers);

    match state
        .faceit_service
        .generate_auth_url(Some(&u.id.to_string()), Some(return_to))
        .await
    {
        Ok((url, _)) => Ok(Redirect::temporary(&url)),
        Err(e) => {
            tracing::error!(
                "❌ FACEIT link URL generation failed for user {}",
                mask_faceit_id(&u.id.to_string())
            );
            tracing::info!("  Detail: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ── /faceit/callback ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FaceitCallbackQuery {
    pub code: String,
    pub state: String,
}

async fn faceit_callback(
    State(state): State<AppState>,
    Query(query): Query<FaceitCallbackQuery>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    // 1. Token Exchange -> User Info & Tokens holen
    let (info, tokens, existing_user_id, return_to) = state
        .faceit_service
        .handle_callback(&query.code, &query.state)
        .await
        .map_err(|e| {
            tracing::error!("❌ FACEIT callback error: {}", e);
            axum::http::StatusCode::BAD_REQUEST
        })?;

    tracing::info!(
        "✅ FACEIT callback: user {} linked",
        mask_faceit_id(&info.guid)
    );

    // 2. Auth Session Generieren / Faceit Link speichern
    let (uid, session_token) = state
        .auth_service
        .handle_faceit_sso(&info, existing_user_id.as_deref())
        .await
        .map_err(|e| {
            tracing::error!(
                "❌ Auth SSO Error for FACEIT user {}",
                mask_faceit_id(&info.guid)
            );
            tracing::info!("  Detail: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let _ = state
        .faceit_service
        .save_faceit_link(&uid, &info, &tokens)
        .await;

    // 3. Redirect through session-bounce to set the cookie on the SAME origin the
    //    frontend uses for API requests.  When the frontend talks through the Vite
    //    proxy (localhost:5173/api/v1/…) the cookie must be set on that origin — not
    //    on localhost:8080 which the browser treats as a different origin.
    let frontend_url = return_to.unwrap_or_else(default_frontend_url);

    // Derive bounce base from the frontend URL so the browser navigates through
    // the same origin (Vite proxy or production reverse-proxy).
    let bounce_base = format!(
        "{}/api/v1/faceit/session-bounce",
        frontend_url.trim_end_matches('/')
    );
    let bounce_url = format!(
        "{}?token={}&next={}",
        bounce_base,
        urlencoding::encode(&session_token),
        urlencoding::encode(&append_query_param(&frontend_url, "linked", "1")),
    );

    Ok(Redirect::temporary(&bounce_url))
}

// ── /faceit/session-bounce ─────────────────────────────────────────────────
// Sets the auth cookie on localhost domain, then redirects to the frontend.
// This solves the cookie-domain mismatch when OAuth callback goes through ngrok.

#[derive(Deserialize)]
struct SessionBounceQuery {
    token: String,
    next: String,
}

async fn session_bounce(
    Query(query): Query<SessionBounceQuery>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    // Use the shared cookie builder to ensure consistent SameSite/Secure attributes.
    // The target URL determines whether Secure flag is needed.
    let cookie_str = crate::api::build_auth_cookie_for_target(&query.token, &query.next);

    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, &query.next)
        .header(
            axum::http::header::SET_COOKIE,
            HeaderValue::from_str(&cookie_str)
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .body(axum::body::Body::empty())
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("🔐 Session bounce: cookie set, redirecting to {}", query.next);
    Ok(response)
}

// ── /faceit/status ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct FaceitStatusResponse {
    connected: bool,
    faceit_nickname: Option<String>,
    faceit_avatar_url: Option<String>,
    faceit_elo: Option<i32>,
    faceit_skill_level: Option<i32>,
    linked_at: Option<String>,
}

async fn faceit_status(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
) -> Result<Json<FaceitStatusResponse>, axum::http::StatusCode> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    match state
        .faceit_service
        .get_link_status(&u.id.to_string())
        .await
    {
        Ok(status) => Ok(Json(FaceitStatusResponse {
            connected: status.linked,
            faceit_nickname: status.faceit_nickname,
            faceit_avatar_url: status.faceit_avatar_url,
            faceit_elo: status.faceit_elo,
            faceit_skill_level: status.faceit_skill_level,
            linked_at: status.linked_at,
        })),
        Err(e) => {
            tracing::error!(
                "❌ FACEIT status check failed for user {}",
                mask_faceit_id(&u.id.to_string())
            );
            tracing::info!("  Detail: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ── /faceit/profile ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct FaceitProfileApiResponse {
    faceit_player_id: String,
    nickname: String,
    avatar_url: Option<String>,
    country: String,
    elo: i32,
    skill_level: i32,
    games: Vec<String>,
    faceit_url: String,
    is_cached: bool,
}

async fn faceit_profile(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
) -> Result<Json<FaceitProfileApiResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    let faceit_data_svc = state.faceit_data_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ FaceitDataService not available (missing FACEIT_DATA_API_KEY)");
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "faceit_data_unavailable",
                "message": "FACEIT Data API is not configured on this server. Please contact the administrator."
            })),
        )
    })?;

    // Get faceit_player_id + cached data from DB (including cache timestamp)
    let row = sqlx::query(
        "SELECT faceit_player_id, faceit_nickname, faceit_avatar_url, faceit_elo, faceit_skill_level, faceit_cache_updated_at FROM faceit_links WHERE user_id = $1::uuid"
    )
    .bind(&u.id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "db_error",
            "message": "Failed to retrieve FACEIT link from database."
        })),
    ))?;

    let row = row.ok_or_else(|| (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "faceit_not_linked",
            "message": "No FACEIT account linked to this user."
        })),
    ))?;

    let player_id: String = row.try_get("faceit_player_id").unwrap_or_default();
    let cached_nick: String = row.try_get("faceit_nickname").unwrap_or_default();
    let cached_avatar: Option<String> = row.try_get("faceit_avatar_url").unwrap_or(None);
    let cached_elo: Option<i32> = row.try_get("faceit_elo").unwrap_or(None);
    let cached_level: Option<i32> = row.try_get("faceit_skill_level").unwrap_or(None);
    let cache_updated_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("faceit_cache_updated_at").unwrap_or(None);

    // TTL check: if cache is <5 min old, return cached data directly
    let cache_fresh = cache_updated_at
        .map(|ts| (chrono::Utc::now() - ts).num_seconds() < FACEIT_CACHE_TTL_SECS)
        .unwrap_or(false);

    if cache_fresh && cached_elo.is_some() && cached_level.is_some() {
        tracing::info!(
            "📦 FACEIT profile [{}] served from cache",
            mask_faceit_id(&player_id)
        );
        return Ok(Json(FaceitProfileApiResponse {
            faceit_player_id: player_id.clone(),
            nickname: cached_nick.clone(),
            avatar_url: cached_avatar,
            country: String::new(),
            elo: cached_elo.unwrap_or(0),
            skill_level: cached_level.unwrap_or(0),
            games: vec!["cs2".to_string()],
            faceit_url: format!("https://www.faceit.com/en/players/{}", cached_nick),
            is_cached: true,
        }));
    }

    // Cache stale or missing → fetch live from FACEIT Data API
    tracing::info!(
        "🌐 FACEIT profile [{}] fetching from API",
        mask_faceit_id(&player_id)
    );

    match faceit_data_svc.get_player_by_id(&player_id).await {
        Ok(profile) => {
            // Extract game-specific ELO (default to cs2)
            let (elo, skill_level) = profile
                .games
                .get("cs2")
                .map(|g| (g.faceit_elo, g.skill_level))
                .unwrap_or((cached_elo.unwrap_or(0), cached_level.unwrap_or(0)));

            // Update cache in DB (including timestamp)
            let _ = sqlx::query(
                "UPDATE faceit_links SET faceit_elo = $1, faceit_skill_level = $2, faceit_cache_updated_at = NOW() WHERE user_id = $3::uuid"
            )
            .bind(elo)
            .bind(skill_level)
            .bind(&u.id.to_string())
            .execute(&state.pool)
            .await;

            let games: Vec<String> = profile.games.keys().cloned().collect();

            Ok(Json(FaceitProfileApiResponse {
                faceit_player_id: player_id,
                nickname: profile.nickname,
                avatar_url: profile.avatar,
                country: profile.country.unwrap_or_default(),
                elo,
                skill_level,
                games,
                faceit_url: format!("https://www.faceit.com/en/players/{}", cached_nick),
                is_cached: false,
            }))
        }
        Err(e) => {
            tracing::warn!(
                "⚠️ FACEIT Data API Error for player [{}], falling back to cached data: {}",
                mask_faceit_id(&player_id),
                e
            );
            // Graceful degradation: return cached data with is_cached=true
            Ok(Json(FaceitProfileApiResponse {
                faceit_player_id: player_id,
                nickname: cached_nick.clone(),
                avatar_url: cached_avatar,
                country: String::new(),
                elo: cached_elo.unwrap_or(0),
                skill_level: cached_level.unwrap_or(0),
                games: vec!["cs2".to_string()],
                faceit_url: format!("https://www.faceit.com/en/players/{}", cached_nick),
                is_cached: true,
            }))
        }
    }
}

// ── /faceit/stats ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FaceitStatsQuery {
    game: Option<String>,
}

#[derive(Serialize)]
struct FaceitStatsApiResponse {
    game_id: String,
    lifetime: serde_json::Value,
    is_cached: bool,
}

async fn faceit_stats(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
    Query(query): Query<FaceitStatsQuery>,
) -> Result<Json<FaceitStatsApiResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    let faceit_data_svc = state.faceit_data_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ FaceitDataService not available (missing FACEIT_DATA_API_KEY)");
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "faceit_data_unavailable",
                "message": "FACEIT Data API is not configured on this server. Please contact the administrator."
            })),
        )
    })?;

    let game_id_raw = query.game.as_deref().unwrap_or("cs2");
    let game_id = validate_game_id(game_id_raw)?;

    // Load player_id + existing stats cache from DB
    let row = sqlx::query(
        "SELECT faceit_player_id, stats_cache_json, stats_cache_game_id, stats_cache_updated_at \
         FROM faceit_links WHERE user_id = $1::uuid"
    )
    .bind(&u.id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "db_error",
            "message": "Failed to retrieve FACEIT link from database."
        })),
    ))?
    .ok_or_else(|| (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "faceit_not_linked",
            "message": "No FACEIT account linked to this user."
        })),
    ))?;

    let player_id: String = row.try_get("faceit_player_id").unwrap_or_default();
    let cached_stats: Option<serde_json::Value> = row.try_get("stats_cache_json").unwrap_or(None);
    let cached_game_id: Option<String> = row.try_get("stats_cache_game_id").unwrap_or(None);
    let cache_updated_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("stats_cache_updated_at").unwrap_or(None);

    // TTL check: if cache for this game_id is < 5 min old, return cached data
    let cache_hit = cached_stats.is_some()
        && cached_game_id.as_deref() == Some(game_id)
        && cache_updated_at
            .map(|ts| (chrono::Utc::now() - ts).num_seconds() < FACEIT_CACHE_TTL_SECS)
            .unwrap_or(false);

    if cache_hit {
        tracing::info!(
            "📦 FACEIT stats [{}] game={} served from cache",
            mask_faceit_id(&player_id),
            game_id
        );
        return Ok(Json(FaceitStatsApiResponse {
            game_id: game_id.to_string(),
            lifetime: cached_stats.unwrap(),
            is_cached: true,
        }));
    }

    tracing::info!(
        "🌐 FACEIT stats [{}] game={} fetching from API",
        mask_faceit_id(&player_id),
        game_id
    );

    match faceit_data_svc.get_player_stats(&player_id, game_id).await {
        Ok(stats) => {
            let lifetime = serde_json::to_value(&stats).unwrap_or(serde_json::json!({}));

            // Persist stats cache (non-fatal if it fails)
            let _ = sqlx::query(
                "UPDATE faceit_links \
                 SET stats_cache_json = $1, stats_cache_game_id = $2, stats_cache_updated_at = NOW() \
                 WHERE user_id = $3::uuid"
            )
            .bind(&lifetime)
            .bind(game_id)
            .bind(&u.id.to_string())
            .execute(&state.pool)
            .await;

            Ok(Json(FaceitStatsApiResponse {
                game_id: game_id.to_string(),
                lifetime,
                is_cached: false,
            }))
        }
        Err(e) => {
            tracing::warn!(
                "⚠️ FACEIT Stats API Error for player [{}] game={}: {}",
                mask_faceit_id(&player_id),
                game_id,
                e
            );

            // Graceful degradation: return cached stats if available, even if expired
            if let Some(cached) = cached_stats {
                tracing::info!(
                    "📦 FACEIT stats [{}] game={} serving stale cache after upstream error",
                    mask_faceit_id(&player_id),
                    game_id
                );
                return Ok(Json(FaceitStatsApiResponse {
                    game_id: game_id.to_string(),
                    lifetime: cached,
                    is_cached: true,
                }));
            }

            Err((
                axum::http::StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "faceit_upstream_error",
                    "message": format!("FACEIT Data API request failed: {}", e)
                })),
            ))
        }
    }
}

// ── /faceit/matches ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FaceitMatchesQuery {
    game: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct FaceitMatchesApiResponse {
    game_id: String,
    items: Vec<serde_json::Value>,
    start: i32,
    end: i32,
    is_cached: bool,
}

async fn faceit_matches(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
    Query(query): Query<FaceitMatchesQuery>,
) -> Result<Json<FaceitMatchesApiResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    let faceit_data_svc = state.faceit_data_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ FaceitDataService not available (missing FACEIT_DATA_API_KEY)");
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "faceit_data_unavailable",
                "message": "FACEIT Data API is not configured on this server."
            })),
        )
    })?;

    let game_id_raw = query.game.as_deref().unwrap_or("cs2");
    let game_id = validate_game_id(game_id_raw)?;
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(20).min(100); // max 100 per request

    // Get faceit_player_id from DB
    let player_id: String =
        sqlx::query_scalar("SELECT faceit_player_id FROM faceit_links WHERE user_id = $1::uuid")
            .bind(&u.id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "db_error",
                    "message": "Failed to retrieve FACEIT link from database."
                })),
            ))?
            .ok_or_else(|| (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "faceit_not_linked",
                    "message": "No FACEIT account linked to this user."
                })),
            ))?;

    tracing::info!(
        "🌐 FACEIT matches [{}] game={} offset={} limit={}",
        mask_faceit_id(&player_id),
        game_id,
        offset,
        limit
    );

    match faceit_data_svc.get_player_history(&player_id, game_id, offset, limit).await {
        Ok(history) => {
            let items = history
                .items
                .iter()
                .map(|item| serde_json::to_value(item).unwrap_or(serde_json::json!({})))
                .collect();
            Ok(Json(FaceitMatchesApiResponse {
                game_id: game_id.to_string(),
                items,
                start: history.start,
                end: history.end,
                is_cached: false,
            }))
        }
        Err(e) => {
            tracing::error!(
                "⚠️ FACEIT History API Error for player [{}] game={}: {}",
                mask_faceit_id(&player_id),
                game_id,
                e
            );
            Err((
                axum::http::StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "faceit_upstream_error",
                    "message": format!("FACEIT Data API request failed: {}", e)
                })),
            ))
        }
    }
}

// ── /faceit/disconnect ─────────────────────────────────────────────────────

async fn faceit_disconnect(
    State(state): State<AppState>,
    user: crate::api::auth_guard::SessionUserNoWallet,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    match state.faceit_service.unlink_faceit(&u.id.to_string()).await {
        Ok(()) => Ok(Json(serde_json::json!({
            "success": true,
            "message": "FACEIT-Verbindung wurde getrennt"
        }))),
        Err(e) => {
            tracing::error!("Faceit Disconnect Error: {}", e);
            // Idempotent: return success even if no link found
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "FACEIT-Verbindung wurde getrennt"
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{infer_return_to, sanitize_return_to};
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn sanitize_return_to_keeps_http_urls_and_removes_fragments() {
        assert_eq!(
            sanitize_return_to("https://example.com/lobby?foo=bar#frag").as_deref(),
            Some("https://example.com/lobby?foo=bar")
        );
        assert_eq!(
            sanitize_return_to("http://localhost:5173/").as_deref(),
            Some("http://localhost:5173")
        );
        assert_eq!(sanitize_return_to("javascript:alert(1)"), None);
    }

    #[test]
    fn infer_return_to_prefers_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "origin",
            HeaderValue::from_static("https://demo.ngrok-free.dev"),
        );
        headers.insert(
            "referer",
            HeaderValue::from_static("https://localhost:5173/lobby"),
        );

        assert_eq!(infer_return_to(&headers), "https://demo.ngrok-free.dev");
    }

    #[test]
    fn infer_return_to_uses_forwarded_host_when_needed() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("demo.ngrok-free.dev"),
        );

        assert_eq!(infer_return_to(&headers), "https://demo.ngrok-free.dev");
    }
}
