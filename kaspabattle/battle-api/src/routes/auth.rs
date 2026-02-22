use actix_web::{web, HttpRequest, HttpResponse, Responder};
use battle_core::auth::AuthService;
use battle_core::models::user::{LoginRequest, RegisterRequest, SetKaspaAddressRequest, User};

use crate::routes::AppState;

/// Extrahiert den authentifizierten User aus dem Request.
/// Liest den "Authorization: Bearer <token>" Header aus.
/// Gibt den User zurück oder 401 Unauthorized.
async fn extract_user(
    req: &HttpRequest,
    auth_service: &AuthService,
) -> Result<User, actix_web::Error> {
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

async fn register(state: web::Data<AppState>, body: web::Json<RegisterRequest>) -> impl Responder {
    match state.auth.register(body.into_inner()).await {
        Ok(auth_res) => HttpResponse::Created().json(auth_res),
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("existiert bereits") {
                HttpResponse::Conflict().json(serde_json::json!({ "error": err_msg }))
            } else {
                HttpResponse::BadRequest().json(serde_json::json!({ "error": err_msg }))
            }
        }
    }
}

async fn login(state: web::Data<AppState>, body: web::Json<LoginRequest>) -> impl Responder {
    match state.auth.login(body.into_inner()).await {
        Ok(auth_res) => HttpResponse::Ok().json(auth_res),
        Err(_) => HttpResponse::Unauthorized()
            .json(serde_json::json!({ "error": "Ungültige Anmeldedaten" })),
    }
}

async fn logout(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    // Only check token format, don't strict-validate since they want to logout
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header["Bearer ".len()..];
            let _ = state.auth.logout(token).await; // ignore strictly failing
            HttpResponse::Ok().json(serde_json::json!({ "message": "Erfolgreich abgemeldet" }))
        }
        _ => HttpResponse::Unauthorized().finish(),
    }
}

async fn get_me(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    match extract_user(&req, &state.auth).await {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(e) => HttpResponse::from_error(e),
    }
}

async fn set_kaspa_address(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<SetKaspaAddressRequest>,
) -> impl Responder {
    let user = match extract_user(&req, &state.auth).await {
        Ok(u) => u,
        Err(e) => return HttpResponse::from_error(e),
    };

    match state
        .auth
        .set_kaspa_address(&user.id, &body.kaspa_address)
        .await
    {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({ "kaspa_address": body.kaspa_address }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub fn auth_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/me", web::get().to(get_me))
            .route("/me/kaspa-address", web::put().to(set_kaspa_address)),
    );
}
