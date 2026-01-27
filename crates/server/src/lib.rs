// crates/server/src/lib.rs
//! Vibe-recall server library.
//!
//! This crate provides the Axum-based HTTP server for the vibe-recall application.
//! It serves a REST API for listing Claude Code projects and retrieving session data.

pub mod error;
pub mod indexing_state;
pub mod routes;
pub mod state;

pub use error::*;
pub use indexing_state::{IndexingState, IndexingStatus};
pub use routes::api_routes;
pub use state::AppState;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use vibe_recall_db::Database;

/// Create the Axum application with all routes and middleware (API-only mode).
///
/// This sets up:
/// - API routes (health, projects, sessions)
/// - CORS for development (allows any origin)
/// - Request tracing
pub fn create_app(db: Database) -> Router {
    create_app_with_static(db, None)
}

/// Create the Axum application with optional static file serving.
///
/// Uses a default (idle) `IndexingState`. For server-first startup where the
/// caller owns the indexing handle, use [`create_app_with_indexing_and_static`].
///
/// # Arguments
///
/// * `db` - Database handle for session/project queries.
/// * `static_dir` - Optional path to static files directory.
pub fn create_app_with_static(db: Database, static_dir: Option<PathBuf>) -> Router {
    create_app_with_indexing_and_static(db, Arc::new(IndexingState::new()), static_dir)
}

/// Create app with an external `IndexingState` (API-only mode).
///
/// This is the primary entry point for server-first startup, where the caller
/// creates an `IndexingState`, passes it here, and also hands it to the
/// background indexing task.
pub fn create_app_with_indexing(db: Database, indexing: Arc<IndexingState>) -> Router {
    create_app_with_indexing_and_static(db, indexing, None)
}

/// Create the Axum application with an external `IndexingState` and optional
/// static file serving.
///
/// This is the most flexible constructor — all other `create_app*` functions
/// delegate to this one.
///
/// # Arguments
///
/// * `db` - Database handle for session/project queries.
/// * `indexing` - Shared indexing progress state.
/// * `static_dir` - Optional path to the directory containing static files
///   (e.g., React build output). If provided, the server will serve static
///   files and fall back to `index.html` for client-side routing (SPA mode).
pub fn create_app_with_indexing_and_static(
    db: Database,
    indexing: Arc<IndexingState>,
    static_dir: Option<PathBuf>,
) -> Router {
    let state = AppState::new_with_indexing(db, indexing);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut app = Router::new()
        .merge(api_routes(state))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Serve static files with SPA fallback
    // Use .fallback() instead of .not_found_service() to return 200 for SPA routing
    // (not_found_service returns 404, which is incorrect for client-side routing)
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(&index)));
    }

    app
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

    /// Helper: create an in-memory database for tests.
    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB for tests")
    }

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
        let app = create_app(test_db().await);
        let (status, body) = get(app, "/api/health").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"status\":\"ok\""));
        assert!(body.contains("\"version\""));
        assert!(body.contains("\"uptime_secs\""));
    }

    #[tokio::test]
    async fn test_health_endpoint_response_structure() {
        let app = create_app(test_db().await);
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
        let app = create_app(test_db().await);
        let (status, body) = get(app, "/api/projects").await;

        // With an empty in-memory DB, should always return 200 with an empty array
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.is_array(), "Expected array, got: {}", body);
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    // ========================================================================
    // Session Endpoint Tests
    // ========================================================================

    #[tokio::test]
    async fn test_session_not_found() {
        let app = create_app(test_db().await);
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
        let app = create_app(test_db().await);
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
        let app = create_app(test_db().await);

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
        let app = create_app(test_db().await);

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
        let app = create_app(test_db().await);
        let (status, _body) = get(app, "/api/nonexistent").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_404_for_root_path() {
        let app = create_app(test_db().await);
        let (status, _body) = get(app, "/").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_404_for_non_api_path() {
        let app = create_app(test_db().await);
        let (status, _body) = get(app, "/health").await;

        // Without /api prefix, should be 404
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // ========================================================================
    // App Creation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_create_app() {
        // Should not panic
        let _app = create_app(test_db().await);
    }

    #[tokio::test]
    async fn test_create_app_with_static_none() {
        // Should not panic when no static dir is provided
        let _app = create_app_with_static(test_db().await, None);
    }

    #[tokio::test]
    async fn test_create_app_with_static_some() {
        // Should not panic when a static dir path is provided
        // (even if the path doesn't exist - ServeDir handles this gracefully)
        let _app = create_app_with_static(test_db().await, Some(std::path::PathBuf::from("/nonexistent")));
    }

    #[tokio::test]
    async fn test_multiple_requests() {
        // Verify the app can handle multiple requests
        let app = create_app(test_db().await);

        // First request
        let (status1, _) = get(app.clone(), "/api/health").await;
        assert_eq!(status1, StatusCode::OK);

        // Second request
        let (status2, _) = get(app, "/api/health").await;
        assert_eq!(status2, StatusCode::OK);
    }

    // ========================================================================
    // Static File Serving Tests
    // ========================================================================

    #[tokio::test]
    async fn test_static_serving_with_temp_dir() {
        use std::io::Write;
        use tempfile::TempDir;

        // Create a temporary directory with an index.html
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.html");
        let mut index_file = std::fs::File::create(&index_path).unwrap();
        writeln!(index_file, "<!DOCTYPE html><html><body>Test</body></html>").unwrap();

        // Create app with static serving
        let app = create_app_with_static(test_db().await, Some(temp_dir.path().to_path_buf()));

        // Root path should serve index.html
        let (status, body) = get(app.clone(), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("<!DOCTYPE html>"));

        // API endpoints should still work
        let (status, _) = get(app, "/api/health").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_static_serving_spa_fallback() {
        use std::io::Write;
        use tempfile::TempDir;

        // Create a temporary directory with an index.html
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.html");
        let mut index_file = std::fs::File::create(&index_path).unwrap();
        writeln!(index_file, "<!DOCTYPE html><html><body>SPA</body></html>").unwrap();

        // Create app with static serving
        let app = create_app_with_static(test_db().await, Some(temp_dir.path().to_path_buf()));

        // Unknown paths should fall back to index.html (SPA routing)
        let (status, body) = get(app, "/some/client/route").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("SPA"));
    }

    #[tokio::test]
    async fn test_static_serving_serves_actual_files() {
        use std::io::Write;
        use tempfile::TempDir;

        // Create a temporary directory with multiple files
        let temp_dir = TempDir::new().unwrap();

        // Create index.html
        let index_path = temp_dir.path().join("index.html");
        let mut index_file = std::fs::File::create(&index_path).unwrap();
        writeln!(index_file, "<!DOCTYPE html><html><body>Index</body></html>").unwrap();

        // Create a JS file in assets/
        let assets_dir = temp_dir.path().join("assets");
        std::fs::create_dir(&assets_dir).unwrap();
        let js_path = assets_dir.join("app.js");
        let mut js_file = std::fs::File::create(&js_path).unwrap();
        writeln!(js_file, "console.log('Hello');").unwrap();

        // Create app with static serving
        let app = create_app_with_static(test_db().await, Some(temp_dir.path().to_path_buf()));

        // JS file should be served directly
        let (status, body) = get(app, "/assets/app.js").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("console.log"));
    }
}
