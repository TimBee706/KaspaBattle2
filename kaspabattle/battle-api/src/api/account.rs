//! HTTP handlers for e-mail / password accounts. Logic lives in `crate::account`.
//!
//! | Method | Path (under `/api/v1`)      | Notes                                              |
//! |--------|-----------------------------|----------------------------------------------------|
//! | POST   | `/auth/register`            | 201 identical for new and already-registered mail  |
//! | POST   | `/auth/login`               | sets the HttpOnly session cookie                   |
//! | POST   | `/auth/verify-email`        | single-use token                                   |
//! | POST   | `/auth/resend-verification` | always 202                                         |
//! | POST   | `/auth/forgot-password`     | 202 / 503 when no mail transport is configured     |
//! | POST   | `/auth/reset-password`      | single-use token, revokes all sessions             |
//! | POST   | `/auth/change-password`     | session, revokes all other sessions                |
//! | POST   | `/auth/newsletter`          | consent / revoke (no sending implemented)          |

use crate::account::{self, FieldError, LoginError, RegisterInput, RegisterOutcome};
use crate::api::auth_guard::SessionUserNoWallet;
use crate::api::native::{err, ApiError};
use crate::api::rate_limit::ClientIp;
use crate::api::{build_auth_cookie_with, extract_session_token_from_headers, AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

type Fields = BTreeMap<&'static str, &'static str>;

fn invalid(fields: Fields) -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        serde_json::json!({ "error": "invalid_input", "message": "Please check your input.", "fields": fields }),
    )
}

fn too_many(retry_after: u64) -> Response {
    let mut r = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({ "error": "rate_limited", "message": "Too many attempts. Please wait a moment." })),
    )
        .into_response();
    if let Ok(v) = HeaderValue::from_str(&retry_after.to_string()) {
        r.headers_mut().insert(header::RETRY_AFTER, v);
    }
    r
}

fn hour() -> Duration {
    Duration::from_secs(3600)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegisterBody {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirm: String,
    pub accept_terms: bool,
    #[serde(default)]
    pub newsletter: bool,
    /// Honeypot: invisible to humans, bots fill it.
    #[serde(default)]
    pub website: String,
}

pub async fn register(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<RegisterBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    let ok_body = || {
        (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "status": "registered",
                "emailVerificationRequired": rt.cfg.require_email_verification,
            })),
        )
            .into_response()
    };
    if !b.website.is_empty() {
        return Ok(ok_body()); // bot: pretend success, create nothing
    }
    let mut fields = Fields::new();
    let username = account::validate_username(&b.username)
        .inspect_err(|e| {
            fields.insert("username", e.code());
        })
        .ok();
    let email = account::normalize_email(&b.email)
        .inspect_err(|e| {
            fields.insert("email", e.code());
        })
        .ok();
    if let Err(e) = account::validate_password(&b.password, username.as_deref(), email.as_deref()) {
        fields.insert("password", e.code());
    }
    if b.password != b.password_confirm {
        fields.insert("passwordConfirm", "mismatch");
    }
    if !b.accept_terms {
        fields.insert("acceptTerms", FieldError::Required.code());
    }
    let (Some(username), Some(email)) = (username, email) else {
        return Err(invalid(fields));
    };
    if !fields.is_empty() {
        return Err(invalid(fields));
    }
    // Throttle only requests that would cost real work (Argon2 + DB writes); typos in the form
    // must not eat a legitimate visitor's budget. 5 sign-ups per hour and IP.
    if !rt.throttle.allow(&format!("register:ip:{ip}"), 5, hour()) {
        return Ok(too_many(3600));
    }

    match account::register(
        &state.pool,
        rt,
        RegisterInput {
            username,
            email,
            password: b.password,
            newsletter: b.newsletter,
        },
        &ip,
    )
    .await
    {
        Ok(RegisterOutcome::Created(_)) | Ok(RegisterOutcome::EmailExists) => Ok(ok_body()),
        Ok(RegisterOutcome::UsernameTaken) => Err(err(
            StatusCode::CONFLICT,
            "username_taken",
            "This username is already taken.",
        )),
        Err(e) => {
            tracing::error!(error = %e, "register failed");
            Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Registration failed. Please try again.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LoginBody {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub remember: bool,
}

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<LoginBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    if !rt
        .throttle
        .allow(&format!("login:ip:{ip}"), 30, Duration::from_secs(900))
    {
        return Ok(too_many(900));
    }
    let generic = || {
        err(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "E-Mail-Adresse oder Passwort ist nicht korrekt.",
        )
    };
    if b.password.len() > 512 {
        return Err(generic());
    }
    let Ok(email) = account::normalize_email(&b.email) else {
        return Err(generic());
    };
    match account::login(&state.pool, rt, &email, &b.password, b.remember, &ip).await {
        Ok(ok) => {
            let max_age = ok
                .remember
                .then(|| (ok.expires_at - chrono::Utc::now()).num_seconds().max(0));
            let mut res =
                Json(serde_json::json!({ "userId": ok.user_id, "username": ok.username }))
                    .into_response();
            res.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_str(&build_auth_cookie_with(&ok.token, max_age)).map_err(
                    |_| {
                        err(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "internal_error",
                            "Login failed.",
                        )
                    },
                )?,
            );
            Ok(res)
        }
        Err(LoginError::Invalid) => Err(generic()),
        Err(LoginError::NotVerified) => Err(err(
            StatusCode::FORBIDDEN,
            "email_not_verified",
            "Bitte bestätige zuerst deine E-Mail-Adresse.",
        )),
        Err(LoginError::Throttled { retry_after_secs }) => Ok(too_many(retry_after_secs)),
        Err(LoginError::Internal(e)) => {
            tracing::error!(error = %e, "login failed");
            Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Login failed. Please try again.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenBody {
    pub token: String,
}

pub async fn verify_email(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<TokenBody>,
) -> Result<Response, ApiError> {
    if !state
        .account
        .throttle
        .allow(&format!("verify:ip:{ip}"), 20, hour())
    {
        return Ok(too_many(3600));
    }
    if b.token.len() > 200 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "invalid_or_expired_token",
            "The link is invalid or has expired.",
        ));
    }
    match account::verify_email(&state.pool, &state.account, &b.token, &ip).await {
        Ok(true) => Ok(Json(serde_json::json!({ "verified": true })).into_response()),
        Ok(false) => Err(err(
            StatusCode::BAD_REQUEST,
            "invalid_or_expired_token",
            "The link is invalid or has expired.",
        )),
        Err(e) => {
            tracing::error!(error = %e, "verify_email failed");
            Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Verification failed.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailBody {
    pub email: String,
}

pub async fn resend_verification(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<EmailBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    let neutral = || {
        (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "status": "ok" })),
        )
            .into_response()
    };
    if !rt.throttle.allow(&format!("resend:ip:{ip}"), 5, hour()) {
        return Ok(too_many(3600));
    }
    if let Ok(email) = account::normalize_email(&b.email) {
        if rt.throttle.allow(
            &format!("resend:acct:{}", account::email_key(&email)),
            3,
            hour(),
        ) {
            account::resend_verification(&state.pool, rt, &email).await;
        }
    }
    Ok(neutral())
}

pub async fn forgot_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<EmailBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    if !rt.mailer.is_configured() {
        // Never pretend a reset mail was delivered when there is no mail transport.
        return Err(err(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_delivery_unavailable",
            "Password reset by e-mail is temporarily unavailable.",
        ));
    }
    if !rt.throttle.allow(&format!("forgot:ip:{ip}"), 5, hour()) {
        return Ok(too_many(3600));
    }
    if let Ok(email) = account::normalize_email(&b.email) {
        if rt.throttle.allow(
            &format!("forgot:acct:{}", account::email_key(&email)),
            3,
            hour(),
        ) {
            account::forgot_password(&state.pool, rt, &email, &ip).await;
        }
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResetBody {
    pub token: String,
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<ResetBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    if !rt.throttle.allow(&format!("reset:ip:{ip}"), 20, hour()) {
        return Ok(too_many(3600));
    }
    if b.token.len() > 200 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "invalid_or_expired_token",
            "The link is invalid or has expired.",
        ));
    }
    if let Err(e) = account::validate_password(&b.new_password, None, None) {
        return Err(invalid(Fields::from([("newPassword", e.code())])));
    }
    match account::reset_password(&state.pool, rt, &b.token, &b.new_password, &ip).await {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "ok" })).into_response()),
        Ok(false) => Err(err(
            StatusCode::BAD_REQUEST,
            "invalid_or_expired_token",
            "The link is invalid or has expired.",
        )),
        Err(e) => {
            tracing::error!(error = %e, "reset_password failed");
            Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password reset failed.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(b): Json<ChangePasswordBody>,
) -> Result<Response, ApiError> {
    let rt = &state.account;
    if !rt
        .throttle
        .allow(&format!("chpw:user:{}", user.id), 10, hour())
    {
        return Ok(too_many(3600));
    }
    let name = sqlx::query_scalar::<_, Option<String>>("SELECT username FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&state.pool)
        .await
        .ok()
        .flatten();
    if let Err(e) = account::validate_password(&b.new_password, name.as_deref(), Some(&user.email))
    {
        return Err(invalid(Fields::from([("newPassword", e.code())])));
    }
    let keep = extract_session_token_from_headers(&headers).unwrap_or_default();
    match account::change_password(
        &state.pool,
        rt,
        user.id,
        &b.current_password,
        &b.new_password,
        &keep,
        &ip,
    )
    .await
    {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "ok" })).into_response()),
        Ok(false) => Err(err(
            StatusCode::FORBIDDEN,
            "invalid_credentials",
            "Das aktuelle Passwort ist nicht korrekt.",
        )),
        Err(e) => {
            tracing::error!(error = %e, "change_password failed");
            Err(err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password change failed.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewsletterBody {
    pub subscribe: bool,
}

pub async fn newsletter(
    State(state): State<AppState>,
    SessionUserNoWallet(user): SessionUserNoWallet,
    Json(b): Json<NewsletterBody>,
) -> Result<Response, ApiError> {
    account::set_newsletter(&state.pool, user.id, b.subscribe)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "newsletter update failed");
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Could not update the setting.",
            )
        })?;
    Ok(Json(serde_json::json!({ "subscribed": b.subscribe })).into_response())
}
