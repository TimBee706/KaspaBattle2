//! Rate-Limiting middleware for KaspaBattle API.
//!
//! Implements per-IP rate limiting using the `governor` crate (GCRA algorithm).
//! Returns HTTP 429 with a `Retry-After` header when the limit is exceeded.
//!
//! ## Tiers
//!
//! | Tier | Endpoints | Limit |
//! |---|---|---|
//! | `auth` | wallet-challenge, wallet-verify, logout | 5 req/min per IP |
//! | `match_create` | POST /challenges | 3 req/min per IP |
//! | `global` | All other API routes | 120 req/min per IP |
//!
//! ## Configuration (env vars)
//!
//! - `RATE_LIMIT_AUTH_RPM` — Auth requests per minute per IP (default: 5)
//! - `RATE_LIMIT_GLOBAL_RPM` — Global requests per minute per IP (default: 120)
//! - `RATE_LIMIT_MATCH_RPM` — Match-create requests per minute per IP (default: 3)

use std::{
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    Quota, RateLimiter,
};

/// Type alias for a per-IP rate limiter.
pub type IpRateLimiter = Arc<
    RateLimiter<
        std::net::IpAddr,
        governor::state::keyed::DefaultKeyedStateStore<IpAddr>,
        DefaultClock,
        NoOpMiddleware,
    >,
>;

/// Build an IP-keyed rate limiter from a requests-per-minute value.
pub fn build_ip_limiter(rpm: u32) -> IpRateLimiter {
    let quota = Quota::per_minute(
        NonZeroU32::new(rpm.max(1)).expect("rpm must be > 0"),
    );
    Arc::new(RateLimiter::keyed(quota))
}

/// Read `RATE_LIMIT_AUTH_RPM` from env (default: 5).
pub fn auth_rpm() -> u32 {
    std::env::var("RATE_LIMIT_AUTH_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

/// Read `RATE_LIMIT_GLOBAL_RPM` from env (default: 120).
pub fn global_rpm() -> u32 {
    std::env::var("RATE_LIMIT_GLOBAL_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120)
}

/// Read `RATE_LIMIT_MATCH_RPM` from env (default: 3).
pub fn match_rpm() -> u32 {
    std::env::var("RATE_LIMIT_MATCH_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
}

/// Extract the client IP from the request.
///
/// Checks `X-Forwarded-For` first (for reverse-proxy setups), then falls back
/// to the TCP peer address from `ConnectInfo<SocketAddr>`.
fn extract_ip(req: &Request<Body>) -> Option<IpAddr> {
    // Try X-Forwarded-For first (first IP in comma-separated list)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(first) = val.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // Fallback to ConnectInfo
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
}

/// Build a 429 Too Many Requests response with Retry-After header.
fn too_many_requests() -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        "Rate limit exceeded. Please slow down.",
    )
        .into_response();
    response
        .headers_mut()
        .insert("Retry-After", HeaderValue::from_static("60"));
    response
}

/// Axum middleware: per-IP rate limiter.
///
/// Pass the limiter via `move |req, next| { let lim = limiter.clone(); ... }`.
pub async fn rate_limit_middleware(
    limiter: IpRateLimiter,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = match extract_ip(&req) {
        Some(ip) => ip,
        None => {
            // Cannot determine IP — allow by default to avoid DDoSing legitimate users
            tracing::warn!("rate_limit: could not extract client IP, allowing request");
            return next.run(req).await;
        }
    };

    match limiter.check_key(&ip) {
        Ok(_) => next.run(req).await,
        Err(_) => {
            tracing::warn!(
                client_ip = %ip,
                "Rate limit exceeded"
            );
            too_many_requests()
        }
    }
}
