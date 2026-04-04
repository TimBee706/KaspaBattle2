/// CSRF Protection Middleware — Origin/Referer Validation
///
/// Validates that mutating requests (POST, PUT, PATCH, DELETE) originate from
/// one of the configured allowed origins. Supports multiple origins.
///
/// Strategy: Check `Origin` header first, fall back to `Referer`. Reject if
/// neither matches any of the configured origins.
use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Extracts the origin from either the Origin or Referer header.
fn extract_origin(headers: &HeaderMap) -> Option<String> {
    // Prefer Origin header (set by browsers for cross-origin requests)
    if let Some(origin) = headers.get("origin") {
        return origin.to_str().ok().map(|s| s.to_string());
    }

    // Fallback to Referer header (strip path component)
    if let Some(referer) = headers.get("referer") {
        if let Ok(referer_str) = referer.to_str() {
            // Extract scheme://host[:port] from the referer URL
            // e.g. "https://app.kaspabattle.com/dashboard?foo=bar" -> "https://app.kaspabattle.com"
            if let Some(scheme_end) = referer_str.find("://") {
                let after_scheme = &referer_str[scheme_end + 3..];
                let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
                let origin = format!("{}{}", &referer_str[..scheme_end + 3], &after_scheme[..authority_end]);
                return Some(origin);
            }
        }
    }

    None
}

/// Checks if the given origin matches any of the allowed origins.
fn is_allowed_origin_multi(origin: &str, allowed_origins: &[String]) -> bool {
    let origin = origin.trim_end_matches('/');
    allowed_origins
        .iter()
        .any(|allowed| origin.eq_ignore_ascii_case(allowed.trim_end_matches('/')))
}

/// CSRF middleware that validates Origin/Referer against a list of allowed origins.
///
/// This variant accepts a `Vec<String>` of allowed origins instead of a single one,
/// enabling multi-origin deployments (e.g. apex + www, multiple environments).
pub async fn csrf_protection_layer_multi(
    allowed_origins: Vec<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();

    // Only check mutating methods
    if matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(request).await;
    }

    // Skip CSRF check for webhook endpoints (they use HMAC/API key auth instead)
    let path = request.uri().path().to_string();
    if path.starts_with("/api/v1/webhooks/") || path.starts_with("/api/v1/oracle/") {
        return next.run(request).await;
    }

    let headers = request.headers().clone();
    match extract_origin(&headers) {
        Some(origin) => {
            if is_allowed_origin_multi(&origin, &allowed_origins) {
                next.run(request).await
            } else {
                tracing::warn!(
                    origin = %origin,
                    allowed = ?allowed_origins,
                    path = %path,
                    "CSRF: origin mismatch — request blocked"
                );
                (StatusCode::FORBIDDEN, "CSRF validation failed: origin mismatch").into_response()
            }
        }
        None => {
            // No Origin or Referer header — could be server-to-server or curl.
            // For maximum safety, reject. API clients must set Origin header.
            tracing::warn!(
                path = %path,
                method = %method,
                "CSRF: missing Origin/Referer header — request blocked"
            );
            (StatusCode::FORBIDDEN, "CSRF validation failed: missing Origin header").into_response()
        }
    }
}

/// Backwards-compatible single-origin variant. Kept for potential future use.
#[allow(dead_code)]
pub async fn csrf_protection_layer(
    axum::extract::State(frontend_url): axum::extract::State<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    csrf_protection_layer_multi(vec![frontend_url], request, next).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_origin_multi() {
        let origins = vec![
            "http://localhost:5173".to_string(),
            "https://app.kaspabattle.com".to_string(),
        ];

        assert!(is_allowed_origin_multi("http://localhost:5173", &origins));
        assert!(is_allowed_origin_multi("http://localhost:5173/", &origins));
        assert!(is_allowed_origin_multi("https://app.kaspabattle.com", &origins));
        assert!(is_allowed_origin_multi("https://app.kaspabattle.com/", &origins));
        assert!(!is_allowed_origin_multi("https://evil.com", &origins));
        assert!(!is_allowed_origin_multi("http://localhost:3000", &origins));
    }

    #[test]
    fn test_extract_origin_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("origin", "https://app.kaspabattle.com".parse().unwrap());
        assert_eq!(extract_origin(&headers), Some("https://app.kaspabattle.com".to_string()));

        let mut headers = HeaderMap::new();
        headers.insert("referer", "https://app.kaspabattle.com/dashboard?foo=bar".parse().unwrap());
        assert_eq!(extract_origin(&headers), Some("https://app.kaspabattle.com".to_string()));
    }

    #[test]
    fn test_extract_origin_no_headers() {
        let headers = HeaderMap::new();
        assert_eq!(extract_origin(&headers), None);
    }
}
