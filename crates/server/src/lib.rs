// crates/server/src/lib.rs
//! Vibe-recall server library.
//!
//! This crate provides the Axum-based HTTP server for the vibe-recall application.
//! It serves a REST API for listing Claude Code projects and retrieving session data.

pub mod error;
pub mod routes;
pub mod state;

pub use error::*;
pub use routes::api_routes;
pub use state::AppState;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Create the Axum application with all routes and middleware.
///
/// This sets up:
/// - API routes (health, projects, sessions)
/// - CORS for development (allows any origin)
/// - Request tracing
pub fn create_app() -> Router {
    let state = AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(api_routes(state))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

// ============================================================================
// Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// Helper to make a GET request to the app.
    async fn get(app: Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        (status, body_str)
    }

    // ========================================================================
    // Health Endpoint Tests
    // ========================================================================

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_app();
        let (status, body) = get(app, "/api/health").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"status\":\"ok\""));
        assert!(body.contains("\"version\""));
        assert!(body.contains("\"uptime_secs\""));
    }

    #[tokio::test]
    async fn test_health_endpoint_response_structure() {
        let app = create_app();
        let (status, body) = get(app, "/api/health").await;

        assert_eq!(status, StatusCode::OK);

        // Parse the JSON to verify structure
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json["version"].is_string());
        assert!(json["uptime_secs"].is_number());
    }

    // ========================================================================
    // Projects Endpoint Tests
    // ========================================================================

    #[tokio::test]
    async fn test_projects_endpoint() {
        let app = create_app();
        let (status, body) = get(app, "/api/projects").await;

        // Should return 200 with an array (may be empty or have error depending on system)
        // On systems without Claude projects dir, it will return an error
        // On systems with Claude projects dir, it returns an array
        // Either way, it should be a valid response
        assert!(
            status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
            "Expected 200 or 500, got {}",
            status
        );

        if status == StatusCode::OK {
            // Verify it's an array
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert!(json.is_array(), "Expected array, got: {}", body);
        }
    }

    // ========================================================================
    // Session Endpoint Tests
    // ========================================================================

    #[tokio::test]
    async fn test_session_not_found() {
        let app = create_app();
        let (status, body) = get(app, "/api/session/nonexistent-project/nonexistent-session").await;

        // Should return 404 or 500 (depending on whether projects dir exists)
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR,
            "Expected 404 or 500, got {}",
            status
        );

        // Should have an error response
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.get("error").is_some(), "Expected error field in response");
    }

    #[tokio::test]
    async fn test_session_invalid_project() {
        let app = create_app();
        let (status, body) = get(app, "/api/session/invalid%2Fpath/abc123").await;

        // Should return an error (404 or 500)
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR,
            "Expected 404 or 500, got {}",
            status
        );

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.get("error").is_some());
    }

    // ========================================================================
    // CORS Tests
    // ========================================================================

    #[tokio::test]
    async fn test_cors_headers() {
        let app = create_app();

        // Make an OPTIONS preflight request
        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/health")
                    .header("Origin", "http://localhost:3000")
                    .header("Access-Control-Request-Method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Check for CORS headers
        let headers = response.headers();
        assert!(
            headers.contains_key("access-control-allow-origin"),
            "Expected access-control-allow-origin header"
        );
    }

    #[tokio::test]
    async fn test_cors_allows_any_origin() {
        let app = create_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header("Origin", "http://example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let headers = response.headers();
        let allow_origin = headers.get("access-control-allow-origin");
        assert!(allow_origin.is_some());
        assert_eq!(allow_origin.unwrap(), "*");
    }

    // ========================================================================
    // 404 Tests
    // ========================================================================

    #[tokio::test]
    async fn test_404_for_unknown_route() {
        let app = create_app();
        let (status, _body) = get(app, "/api/nonexistent").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_404_for_root_path() {
        let app = create_app();
        let (status, _body) = get(app, "/").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_404_for_non_api_path() {
        let app = create_app();
        let (status, _body) = get(app, "/health").await;

        // Without /api prefix, should be 404
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // ========================================================================
    // App Creation Tests
    // ========================================================================

    #[test]
    fn test_create_app() {
        // Should not panic
        let _app = create_app();
    }

    #[tokio::test]
    async fn test_multiple_requests() {
        // Verify the app can handle multiple requests
        let app = create_app();

        // First request
        let (status1, _) = get(app.clone(), "/api/health").await;
        assert_eq!(status1, StatusCode::OK);

        // Second request
        let (status2, _) = get(app, "/api/health").await;
        assert_eq!(status2, StatusCode::OK);
    }
}
