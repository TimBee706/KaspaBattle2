use actix_web::{web, HttpRequest, HttpResponse, Responder};

use crate::routes::AppState;

/// Helper function to extract token and user
async fn extract_user(
    req: &HttpRequest,
    auth_service: &battle_core::auth::AuthService,
) -> Result<battle_core::models::user::User, actix_web::Error> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Authorization Header fehlt"))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Ungültiges Token-Format"))?;

    auth_service
        .validate_session(token)
        .await
        .map_err(|_| actix_web::error::ErrorUnauthorized("Session ungültig oder abgelaufen"))
}

async fn link_faceit(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    match state
        .faceit_oauth
        .generate_auth_url(Some(&user.id), None)
        .await
    {
        Ok((url, _)) => HttpResponse::Found()
            .append_header(("Location", url))
            .finish(),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Generieren der FACEIT Auth URL: {}", e)
        })),
    }
}

async fn login_faceit(state: web::Data<AppState>) -> impl Responder {
    match state.faceit_oauth.generate_auth_url(None, None).await {
        Ok((url, _)) => HttpResponse::Found()
            .append_header(("Location", url))
            .finish(),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Generieren der FACEIT Auth URL: {}", e)
        })),
    }
}

#[derive(serde::Deserialize)]
pub struct FaceitCallbackQuery {
    pub code: String,
    pub state: String,
}

async fn faceit_callback(
    state: web::Data<AppState>,
    query: web::Query<FaceitCallbackQuery>,
) -> impl Responder {
    match state
        .faceit_oauth
        .handle_callback(&query.code, &query.state)
        .await
    {
        Ok((info, tokens, pending_user_id, _return_to)) => {
            let session_token_opt = if let Some(uid) = pending_user_id {
                let _ = state
                    .faceit_oauth
                    .save_faceit_link(&uid, &info, &tokens)
                    .await;
                None
            } else {
                if let Ok((uid, session_token)) = state.auth.handle_faceit_sso(&info).await {
                    let _ = state
                        .faceit_oauth
                        .save_faceit_link(&uid, &info, &tokens)
                        .await;
                    Some(session_token)
                } else {
                    None
                }
            };

            let frontend_url = std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string());
            if let Some(session_token) = session_token_opt {
                HttpResponse::Found()
                    .append_header((
                        "Location",
                        format!("{}/?token={}", frontend_url, session_token),
                    ))
                    .finish()
            } else {
                HttpResponse::Found()
                    .append_header(("Location", format!("{}/?linked=true", frontend_url)))
                    .finish()
            }
        }
        Err(e) => HttpResponse::Found()
            .append_header((
                "Location",
                format!("/?error={}", urlencoding::encode(&e.to_string())),
            ))
            .finish(),
    }
}

async fn get_faceit_status(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    match state.faceit_oauth.get_link_status(&user.id).await {
        Ok(status) => HttpResponse::Ok().json(status),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Abrufen des FACEIT Status: {}", e)
        })),
    }
}

async fn unlink_faceit(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    match state.faceit_oauth.unlink_faceit(&user.id).await {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({ "message": "FACEIT-Verknüpfung entfernt" }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Entfernen der FACEIT Verknüpfung: {}", e)
        })),
    }
}

#[derive(serde::Serialize)]
pub struct FaceitProfileResponse {
    pub faceit_player_id: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub game_id: String,
    pub elo: i32,
    pub skill_level: i32,
    pub is_cached: bool,
}

#[derive(serde::Deserialize)]
pub struct FaceitMatchesQuery {
    pub game: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

async fn get_faceit_profile(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    let link_status = match state.faceit_oauth.get_link_status(&user.id).await {
        Ok(status) => status,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Fehler beim Abrufen des Link Status: {}", e)
            }))
        }
    };

    if !link_status.linked {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Kein FACEIT Account verknüpft"
        }));
    }

    // We need the actual faceit_player_id
    // It's not directly in FaceitLinkStatus. We must query it or add it to FaceitLinkStatus.
    // For now we get the full link:
    let link = match state.auth.get_faceit_link(&user.id).await {
        Ok(Some(l)) => l,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Faceit Link not found"
            }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("DB Error: {}", e)
            }));
        }
    };

    let game_id = "cs2";
    let now = chrono::Utc::now();
    let mut use_cache = false;
    let mut cached_elo = 0;
    let mut cached_level = 0;

    if let Ok(Some(snapshot)) = state
        .auth
        .get_latest_faceit_snapshot(&link.faceit_player_id, game_id)
        .await
    {
        if let Ok(parsed_time) =
            chrono::NaiveDateTime::parse_from_str(&snapshot.snapshot_at, "%Y-%m-%d %H:%M:%S")
        {
            use chrono::TimeZone;
            let dt = chrono::Utc.from_utc_datetime(&parsed_time);
            if now.signed_duration_since(dt).num_minutes() < 15 {
                use_cache = true;
                cached_elo = snapshot.elo;
                cached_level = snapshot.skill_level;
            }
        }
    }

    if use_cache {
        return HttpResponse::Ok().json(FaceitProfileResponse {
            faceit_player_id: link.faceit_player_id.clone(),
            nickname: link.faceit_nickname.clone(),
            avatar_url: link.faceit_avatar_url.clone(),
            game_id: game_id.to_string(),
            elo: cached_elo,
            skill_level: cached_level,
            is_cached: true,
        });
    }

    match state
        .faceit_data
        .get_player_by_id(&link.faceit_player_id)
        .await
    {
        Ok(profile) => {
            let mut elo = 0;
            let mut skill_level = 0;
            if let Some(game_profile) = profile.games.get(game_id) {
                elo = game_profile.faceit_elo;
                skill_level = game_profile.skill_level;
            }

            let _ = state
                .auth
                .save_faceit_snapshot(&user.id, &link.faceit_player_id, game_id, elo, skill_level)
                .await;

            HttpResponse::Ok().json(FaceitProfileResponse {
                faceit_player_id: link.faceit_player_id.clone(),
                nickname: profile.nickname,
                avatar_url: profile.avatar,
                game_id: game_id.to_string(),
                elo,
                skill_level,
                is_cached: false,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Abrufen des FACEIT Profils: {}", e)
        })),
    }
}

async fn get_faceit_matches(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<FaceitMatchesQuery>,
) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    let link = match state.auth.get_faceit_link(&user.id).await {
        Ok(Some(l)) => l,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Kein FACEIT Account verknüpft"
            }))
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("DB Error: {}", e)
            }))
        }
    };

    let game = query.game.as_deref().unwrap_or("cs2");
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);

    match state
        .faceit_data
        .get_player_history(&link.faceit_player_id, game, offset, limit)
        .await
    {
        Ok(history) => HttpResponse::Ok().json(history),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Fehler beim Abrufen der FACEIT Matches: {}", e)
        })),
    }
}

pub fn faceit_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/faceit")
            .route("/link", web::get().to(link_faceit))
            .route("/login", web::get().to(login_faceit))
            .route("/callback", web::get().to(faceit_callback))
            .route("/status", web::get().to(get_faceit_status))
            .route("/link", web::delete().to(unlink_faceit))
            .route("/profile", web::get().to(get_faceit_profile))
            .route("/matches", web::get().to(get_faceit_matches)),
    );
}
