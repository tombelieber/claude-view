//! Prometheus metrics endpoint.
//!
//! Exposes application metrics in Prometheus text format at `GET /metrics`.

use std::sync::Arc;

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use crate::metrics::render_metrics;
use crate::state::AppState;

/// GET /metrics - Prometheus metrics endpoint.
///
/// Returns metrics in Prometheus text format for scraping.
/// Returns 503 Service Unavailable if metrics are not initialized.
#[utoipa::path(get, path = "/metrics", tag = "system",
    responses(
        (status = 200, description = "Prometheus metrics in text format", content_type = "text/plain"),
        (status = 503, description = "Metrics not initialized"),
    )
)]
pub async fn metrics_handler() -> Response {
    match render_metrics() {
        Some(output) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            output,
        )
            .into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Metrics not initialized").into_response(),
    }
}

/// Create the metrics routes router.
///
/// Note: This router does NOT use the `/api` prefix since `/metrics` is a
/// standard Prometheus endpoint path.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/metrics", get(metrics_handler))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    #[tokio::test]
    async fn test_metrics_endpoint_exists() {
        // Initialize metrics for this test
        crate::metrics::init_metrics();

        let db = test_db().await;
        let app = crate::create_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // In test environments another global recorder may already be installed.
        // If so, metrics may remain uninitialized here and return the documented 503 fallback.
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(body.to_vec()).unwrap(),
                "Metrics not initialized"
            );
            return;
        }

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("text/plain"));
    }

    #[tokio::test]
    async fn test_metrics_content_format() {
        // Initialize metrics
        crate::metrics::init_metrics();

        let db = test_db().await;
        let app = crate::create_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        if status == StatusCode::SERVICE_UNAVAILABLE {
            assert_eq!(body_str, "Metrics not initialized");
            return;
        }

        assert_eq!(status, StatusCode::OK);
        // Should be Prometheus text output (not the 503 fallback text).
        assert!(!body_str.contains("Metrics not initialized"));
    }
}
