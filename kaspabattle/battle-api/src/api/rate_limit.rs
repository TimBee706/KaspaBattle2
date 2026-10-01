//! Rate-Limiting middleware for KaspaBattle API.
//!
//! Implements per-IP rate limiting using the `governor` crate (GCRA algorithm).
//! Returns HTTP 429 with a `Retry-After` header when the limit is exceeded.
//!
//! ## Tiers
//!
//! | Tier | Endpoints | Limit |
//! |---|---|---|
//! | `auth` | wallet-challenge, wallet-verify, logout | 30 req/min per IP |
//! | `match_create` | POST /challenges | 3 req/min per IP |
//! | `global` | All other API routes | 120 req/min per IP |
//!
//! ## Configuration (env vars)
//!
//! - `RATE_LIMIT_AUTH_RPM` — Auth requests per minute per IP (default: 30)
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

/// Read `RATE_LIMIT_AUTH_RPM` from env (default: 30).
pub fn auth_rpm() -> u32 {
    std::env::var("RATE_LIMIT_AUTH_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
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

/// Picks the client IP out of an `X-Forwarded-For` value: the **last** entry.
///
/// AUDIT F-16: the leftmost entries are whatever the client sent, so trusting the first one
/// lets anybody dodge (or poison) the per-IP limits by prepending a fake address. The last
/// entry is the one appended by our own trusted reverse proxy (Caddy) for the peer it saw.
/// Only meaningful with `TRUST_X_FORWARDED_FOR=true` and the backend reachable exclusively
/// through that proxy.
fn client_ip_from_xff(value: &str) -> Option<IpAddr> {
    value.rsplit(',').next()?.trim().parse::<IpAddr>().ok()
}

/// Extract the client IP from the request.
///
/// With `TRUST_X_FORWARDED_FOR=true` (required behind Caddy, otherwise every user shares the
/// proxy's IP and therefore one rate-limit bucket) the proxy-appended `X-Forwarded-For` entry
/// is used; otherwise the TCP peer address from `ConnectInfo<SocketAddr>`.
fn extract_ip(req: &Request<Body>) -> Option<IpAddr> {
    // Check if TRUST_X_FORWARDED_FOR is set
    let trust_proxy = std::env::var("TRUST_X_FORWARDED_FOR")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if trust_proxy {
        if let Some(ip) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(client_ip_from_xff)
        {
            return Some(ip);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xff_uses_the_proxy_appended_last_entry() {
        // A client-supplied fake address in front must not win.
        assert_eq!(
            client_ip_from_xff("1.2.3.4, 203.0.113.7"),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            client_ip_from_xff("203.0.113.7"),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            client_ip_from_xff("evil, 2001:db8::1"),
            Some("2001:db8::1".parse().unwrap())
        );
    }

    #[test]
    fn xff_garbage_yields_none_so_the_peer_address_is_used() {
        assert_eq!(client_ip_from_xff(""), None);
        assert_eq!(client_ip_from_xff("1.2.3.4, not-an-ip"), None);
        assert_eq!(client_ip_from_xff("unknown"), None);
    }

    #[test]
    fn distinct_ips_get_distinct_buckets() {
        let limiter = build_ip_limiter(1);
        let a: IpAddr = "203.0.113.1".parse().unwrap();
        let b: IpAddr = "203.0.113.2".parse().unwrap();
        assert!(limiter.check_key(&a).is_ok());
        assert!(limiter.check_key(&a).is_err(), "second hit from the same IP is limited");
        assert!(limiter.check_key(&b).is_ok(), "another IP is unaffected");
    }
}
