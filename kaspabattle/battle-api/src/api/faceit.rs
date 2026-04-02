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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_faceit))
        .route("/auth-url", get(auth_url_faceit))
        .route("/link", get(link_faceit))
        .route("/callback", get(faceit_callback))
        .route("/status", get(faceit_status))
        .route("/profile", get(faceit_profile))
        .route("/stats", get(faceit_stats))
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

    // 3. Zurück ins Frontend mit Session-Cookie
    let frontend_url = return_to.unwrap_or_else(default_frontend_url);

    let cookie_str = super::build_auth_cookie_for_target(&session_token, &frontend_url);
    let redirect_url = append_query_param(&frontend_url, "linked", "1");

    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, redirect_url)
        .header(
            axum::http::header::SET_COOKIE,
            HeaderValue::from_str(&cookie_str)
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .body(axum::body::Body::empty())
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<Json<FaceitProfileApiResponse>, axum::http::StatusCode> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    let faceit_data_svc = state.faceit_data_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ FaceitDataService not available (missing FACEIT_DATA_API_KEY)");
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Get faceit_player_id + cached data from DB (including cache timestamp)
    let row = sqlx::query(
        "SELECT faceit_player_id, faceit_nickname, faceit_avatar_url, faceit_elo, faceit_skill_level, faceit_cache_updated_at FROM faceit_links WHERE user_id = $1::uuid"
    )
    .bind(&u.id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let row = row.ok_or(axum::http::StatusCode::NOT_FOUND)?;

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
            tracing::error!("⚠️ FACEIT Data API Error, using cached data: {}", e);
            // Fallback to cached data
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
) -> Result<Json<FaceitStatsApiResponse>, axum::http::StatusCode> {
    let crate::api::auth_guard::SessionUserNoWallet(u) = user;

    let faceit_data_svc = state.faceit_data_service.as_ref().ok_or_else(|| {
        tracing::error!("❌ FaceitDataService not available (missing FACEIT_DATA_API_KEY)");
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    })?;

    let game_id = query.game.as_deref().unwrap_or("cs2");

    // Get faceit_player_id from DB
    let player_id: String =
        sqlx::query_scalar("SELECT faceit_player_id FROM faceit_links WHERE user_id = $1::uuid")
            .bind(&u.id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    match faceit_data_svc.get_player_stats(&player_id, game_id).await {
        Ok(stats) => {
            let lifetime = serde_json::to_value(&stats).unwrap_or(serde_json::json!({}));
            Ok(Json(FaceitStatsApiResponse {
                game_id: game_id.to_string(),
                lifetime,
                is_cached: false,
            }))
        }
        Err(e) => {
            tracing::error!("⚠️ FACEIT Stats API Error: {}", e);
            Err(axum::http::StatusCode::BAD_GATEWAY)
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
