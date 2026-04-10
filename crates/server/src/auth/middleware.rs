//! Tower auth middleware for API key and proxy auth.
//!
//! Applied to all `/api/*` routes. Localhost requests bypass auth.
//! Remote requests require `Authorization: Bearer cv_live_...`.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::api_key::{validate_key, ApiKeyStore};

/// Shared auth state passed to the middleware function.
#[derive(Clone)]
pub struct AuthState {
    pub key_store: Arc<RwLock<ApiKeyStore>>,
}

/// Axum middleware function for API key auth.
pub async fn auth_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(auth): axum::extract::State<AuthState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Localhost bypass — backward compat
    if is_localhost(&addr) {
        return Ok(next.run(req).await);
    }

    // Remote: require valid API key
    let token = extract_bearer(req.headers()).ok_or(StatusCode::UNAUTHORIZED)?;
    let store = auth.key_store.read().await;
    validate_key(token, &store).ok_or(StatusCode::UNAUTHORIZED)?;
    drop(store);

    Ok(next.run(req).await)
}

fn is_localhost(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_localhost_ipv4() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(is_localhost(&addr));
    }

    #[test]
    fn test_is_localhost_ipv6() {
        let addr: SocketAddr = "[::1]:8080".parse().unwrap();
        assert!(is_localhost(&addr));
    }

    #[test]
    fn test_is_not_localhost() {
        let addr: SocketAddr = "192.168.1.1:8080".parse().unwrap();
        assert!(!is_localhost(&addr));
    }

    #[test]
    fn test_extract_bearer_valid() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("authorization", "Bearer cv_live_abc123".parse().unwrap());
        assert_eq!(extract_bearer(&headers), Some("cv_live_abc123"));
    }

    #[test]
    fn test_extract_bearer_missing() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(extract_bearer(&headers), None);
    }

    #[test]
    fn test_extract_bearer_not_bearer() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("authorization", "Basic abc123".parse().unwrap());
        assert_eq!(extract_bearer(&headers), None);
    }
}
