use axum::{
    extract::{Query, State},
    response::Redirect,
    routing::get,
    Router,
};
use serde::Deserialize;

use crate::api::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_faceit))
        .route("/callback", get(faceit_callback))
}

async fn login_faceit(State(state): State<AppState>) -> Result<Redirect, axum::http::StatusCode> {
    match state.faceit_service.generate_auth_url(None).await {
        Ok((url, _)) => Ok(Redirect::temporary(&url)),
        Err(e) => {
            eprintln!("Fehler beim Generieren der Faceit Auth URL: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

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
    let (info, tokens, _) = state
        .faceit_service
        .handle_callback(&query.code, &query.state)
        .await
        .map_err(|e| {
            eprintln!("Faceit Callback Error: {}", e);
            axum::http::StatusCode::BAD_REQUEST
        })?;

    // 2. Auth Session Generieren / Faceit Link speichern
    let (uid, session_token) = state
        .auth_service
        .handle_faceit_sso(&info)
        .await
        .map_err(|e| {
            eprintln!("Auth SSO Error: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let _ = state
        .faceit_service
        .save_faceit_link(&uid, &info, &tokens)
        .await;

    // 3. Zurück ins Frontend mit Session Token (HttpOnly Cookie)
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let cookie_str = format!(
        "kaspabattle-auth={}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800",
        session_token
    );

    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, frontend_url)
        .header(axum::http::header::SET_COOKIE, cookie_str)
        .body(axum::body::Body::empty())
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}
