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

    match state.faceit_oauth.generate_auth_url(&user.id).await {
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
        Ok(_) => HttpResponse::Found()
            .append_header(("Location", "/?linked=true"))
            .finish(),
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

pub fn faceit_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/faceit")
            .route("/link", web::get().to(link_faceit))
            .route("/callback", web::get().to(faceit_callback))
            .route("/status", web::get().to(get_faceit_status))
            .route("/link", web::delete().to(unlink_faceit)),
    );
}
