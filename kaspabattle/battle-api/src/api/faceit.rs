use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect},
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
fn validate_game_id(
    game_id: &str,
) -> Result<&str, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
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

/// Derives the post-login route from a `return_to` hint. Only relative in-app routes are
/// honoured (see `sanitize_return_path`); the origin always comes from `FRONTEND_URL`, never
/// from request headers, so a forged Origin/Referer cannot redirect users elsewhere.
fn requested_return_path(headers: &HeaderMap) -> String {
    headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .and_then(referer_path)
        .and_then(|p| battle_core::faceit_oauth::sanitize_return_path(&p))
        .unwrap_or_else(|| "/lobby".to_string())
}

/// Path + query of an absolute http(s) referer URL.
fn referer_path(referer: &str) -> Option<String> {
    let rest = referer
        .strip_prefix("https://")
        .or_else(|| referer.strip_prefix("http://"))?;
    Some(match rest.find('/') {
        Some(i) => rest[i..].to_string(),
        None => "/".to_string(),
    })
}

/// Final redirect target after a successful login: canonical origin + safe relative path
/// + `linked=1` so the SPA reloads `/auth/me` and `/faceit/status`.
fn post_login_url(return_path: Option<&str>) -> String {
    post_login_url_for(&default_frontend_url(), return_path)
}

fn post_login_url_for(origin: &str, return_path: Option<&str>) -> String {
    let origin = origin.trim_end_matches('/');
    let mut path = return_path
        .and_then(battle_core::faceit_oauth::sanitize_return_path)
        .unwrap_or_else(|| "/lobby".to_string());
    // Landing on the login page or the callback itself would loop; go to the lobby instead.
    if path == "/" || path.starts_with("/auth/") {
        path = "/lobby".to_string();
    }
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{}{}{}linked=1", origin, path, sep)
}

fn error_redirect(code: &str) -> axum::response::Response {
    let url = format!(
        "{}/?error={}",
        default_frontend_url().trim_end_matches('/'),
        code
    );
    Redirect::to(&url).into_response()
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
    let return_to = requested_return_path(&headers);

    match state
        .faceit_service
        .generate_auth_url(None, Some(return_to))
        .await
    {
        Ok((url, _)) => {
            tracing::info!("🔐 FACEIT auth URL generated");
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
    let return_to = requested_return_path(&headers);

    match state
        .faceit_service
        .generate_auth_url(None, Some(return_to))
        .await
    {
        Ok((url, _)) => {
            tracing::info!("🔐 FACEIT auth URL generated");
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
    let return_to = requested_return_path(&headers);

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
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Server-side OAuth callback on the canonical origin. On success it sets the session cookie
/// and answers `303 See Other` to `/lobby?linked=1`; the session token never appears in a URL.
async fn faceit_callback(
    State(state): State<AppState>,
    Query(query): Query<FaceitCallbackQuery>,
) -> axum::response::Response {
    if let Some(err) = query.error.as_deref() {
        tracing::warn!("FACEIT callback returned an OAuth error");
        let _ = err;
        return error_redirect("faceit_denied");
    }
    let (Some(code), Some(oauth_state)) = (query.code.as_deref(), query.state.as_deref()) else {
        return error_redirect("faceit_invalid_callback");
    };

    // 1. Redeem state (single use) + token exchange + userinfo
    let (info, tokens, existing_user_id, return_to) = match state
        .faceit_service
        .handle_callback(code, oauth_state)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("❌ FACEIT callback failed: {}", e);
            return error_redirect("faceit_login_failed");
        }
    };

    let masked = mask_faceit_id(&info.guid);

    // 2. Resolve / create the local user and issue a session
    let (uid, session_token) = match state
        .auth_service
        .handle_faceit_sso(&info, existing_user_id.as_deref())
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("❌ Auth SSO error for FACEIT user {}: {}", masked, e);
            return error_redirect("faceit_login_failed");
        }
    };

    // 3. Persist the link. A failure must NOT look like "logged in and linked".
    if let Err(e) = state
        .faceit_service
        .save_faceit_link(&uid, &info, &tokens)
        .await
    {
        tracing::error!("❌ Saving FACEIT link failed for user {}: {}", masked, e);
        return error_redirect("faceit_link_failed");
    }

    tracing::info!("✅ FACEIT callback: user {} linked", masked);

    // 4. Set the cookie on this (canonical) origin and redirect with 303.
    let target = post_login_url(return_to.as_deref());
    let cookie = crate::api::build_auth_cookie(&session_token);
    let Ok(cookie_value) = HeaderValue::from_str(&cookie) else {
        return error_redirect("faceit_login_failed");
    };
    let mut response = Redirect::to(&target).into_response(); // 303 See Other
    response
        .headers_mut()
        .insert(axum::http::header::SET_COOKIE, cookie_value);
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response
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

#[derive(Deserialize)]
struct FaceitProfileQuery {
    game: Option<String>,
}

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
    Query(query): Query<FaceitProfileQuery>,
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
    .bind(u.id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "db_error",
            "message": "Failed to retrieve FACEIT link from database."
        })),
    ))?;

    let row = row.ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "faceit_not_linked",
                "message": "No FACEIT account linked to this user."
            })),
        )
    })?;

    let player_id: String = row.try_get("faceit_player_id").unwrap_or_default();
    let cached_nick: String = row.try_get("faceit_nickname").unwrap_or_default();
    let cached_avatar: Option<String> = row.try_get("faceit_avatar_url").unwrap_or(None);
    let cached_elo: Option<i32> = row.try_get("faceit_elo").unwrap_or(None);
    let cached_level: Option<i32> = row.try_get("faceit_skill_level").unwrap_or(None);
    let cache_updated_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("faceit_cache_updated_at").unwrap_or(None);

    let requested_game_id_raw = query.game.as_deref().unwrap_or("cs2");
    let requested_game_id = validate_game_id(requested_game_id_raw)?;

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
            // TODO(R-01): Persist games list in DB (e.g. faceit_games_cache column)
            // and return it here. Empty vec hides the frontend game selector on cache hits.
            games: vec![],
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
            // Extract game-specific ELO by requested game_id, fallback to cs2 or any available game.
            let (elo, skill_level) = profile
                .games
                .get(requested_game_id)
                .map(|g| (g.faceit_elo, g.skill_level))
                .or_else(|| {
                    profile
                        .games
                        .get("cs2")
                        .map(|g| (g.faceit_elo, g.skill_level))
                })
                .or_else(|| {
                    profile
                        .games
                        .values()
                        .next()
                        .map(|g| (g.faceit_elo, g.skill_level))
                })
                .unwrap_or((cached_elo.unwrap_or(0), cached_level.unwrap_or(0)));

            // Update cache in DB (including timestamp)
            let _ = sqlx::query(
                "UPDATE faceit_links SET faceit_elo = $1, faceit_skill_level = $2, faceit_cache_updated_at = NOW() WHERE user_id = $3::uuid"
            )
            .bind(elo)
            .bind(skill_level)
            .bind(u.id.to_string())
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
                // TODO(R-01): Return cached games list once persisted in DB.
                games: vec![],
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
         FROM faceit_links WHERE user_id = $1::uuid",
    )
    .bind(u.id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "db_error",
                "message": "Failed to retrieve FACEIT link from database."
            })),
        )
    })?
    .ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "faceit_not_linked",
                "message": "No FACEIT account linked to this user."
            })),
        )
    })?;

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
            .bind(u.id.to_string())
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
            .bind(u.id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "db_error",
                        "message": "Failed to retrieve FACEIT link from database."
                    })),
                )
            })?
            .ok_or_else(|| {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": "faceit_not_linked",
                        "message": "No FACEIT account linked to this user."
                    })),
                )
            })?;

    tracing::info!(
        "🌐 FACEIT matches [{}] game={} offset={} limit={}",
        mask_faceit_id(&player_id),
        game_id,
        offset,
        limit
    );

    match faceit_data_svc
        .get_player_history(&player_id, game_id, offset, limit)
        .await
    {
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
    use super::{post_login_url_for, referer_path, validate_game_id};
    use axum::http::StatusCode;

    const ORIGIN: &str = "https://www.example.test";

    #[test]
    fn post_login_redirect_targets_lobby_with_linked_flag() {
        assert_eq!(
            post_login_url_for(ORIGIN, None),
            "https://www.example.test/lobby?linked=1"
        );
        assert_eq!(
            post_login_url_for(ORIGIN, Some("/lobby")),
            "https://www.example.test/lobby?linked=1"
        );
        assert_eq!(
            post_login_url_for(ORIGIN, Some("/lobby?tab=2")),
            "https://www.example.test/lobby?tab=2&linked=1"
        );
        // landing/callback routes would loop -> lobby
        assert_eq!(
            post_login_url_for(ORIGIN, Some("/")),
            "https://www.example.test/lobby?linked=1"
        );
        assert_eq!(
            post_login_url_for(ORIGIN, Some("/auth/faceit/callback")),
            "https://www.example.test/lobby?linked=1"
        );
    }

    #[test]
    fn post_login_redirect_never_leaves_canonical_origin() {
        for evil in [
            "https://evil.example/x",
            "//evil.example",
            "/\\evil.example",
            "javascript:1",
        ] {
            let url = post_login_url_for(ORIGIN, Some(evil));
            assert!(
                url.starts_with("https://www.example.test/lobby"),
                "{evil}: {url}"
            );
        }
    }

    #[test]
    fn referer_path_extracts_only_the_path() {
        assert_eq!(
            referer_path("https://x.test/lobby?a=1").as_deref(),
            Some("/lobby?a=1")
        );
        assert_eq!(referer_path("https://x.test").as_deref(), Some("/"));
        assert_eq!(referer_path("ftp://x.test/a"), None);
    }

    #[test]
    fn validate_game_id_allows_valorant() {
        assert_eq!(validate_game_id("valorant").unwrap(), "valorant");
    }

    #[test]
    fn validate_game_id_allows_rocket_league() {
        assert_eq!(validate_game_id("rocket_league").unwrap(), "rocket_league");
    }

    #[test]
    fn validate_game_id_allows_dota2() {
        assert_eq!(validate_game_id("dota2").unwrap(), "dota2");
    }

    #[test]
    fn validate_game_id_allows_lol() {
        assert_eq!(validate_game_id("lol").unwrap(), "lol");
    }

    #[test]
    fn validate_game_id_rejects_unknown_game_id() {
        let err = validate_game_id("unknown").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
}
