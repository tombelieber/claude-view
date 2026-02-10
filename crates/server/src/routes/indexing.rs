//! Indexing progress endpoints.
//!
//! - `GET /api/indexing/progress` — SSE stream (kept for direct-connect production use)
//! - `GET /api/indexing/status`   — JSON snapshot (reliable through any proxy)

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use ts_rs::TS;

use crate::indexing_state::IndexingStatus;
use crate::state::AppState;

/// JSON snapshot of current indexing progress (for polling).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatusResponse {
    pub phase: String,
    pub indexed: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Build the indexing sub-router.
///
/// Routes:
/// - `GET /indexing/progress` - SSE stream of indexing progress events
/// - `GET /indexing/status`   - JSON snapshot for polling
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/indexing/progress", get(indexing_progress))
        .route("/indexing/status", get(indexing_status))
}

/// GET /api/indexing/status — lightweight JSON snapshot of indexing progress.
///
/// Returns the current phase, indexed count, and total count.
/// Designed for polling (every 200–300 ms) from the frontend during rebuilds.
pub async fn indexing_status(
    State(state): State<Arc<AppState>>,
) -> Json<IndexingStatusResponse> {
    let indexing = &state.indexing;
    let status = indexing.status();

    let phase = match status {
        IndexingStatus::Idle => "idle",
        IndexingStatus::ReadingIndexes => "reading-indexes",
        IndexingStatus::DeepIndexing => "deep-indexing",
        IndexingStatus::Done => "done",
        IndexingStatus::Error => "error",
    };

    Json(IndexingStatusResponse {
        phase: phase.to_string(),
        indexed: indexing.indexed(),
        total: indexing.total(),
        error_message: if status == IndexingStatus::Error {
            indexing.error()
        } else {
            None
        },
    })
}

/// SSE handler that streams indexing progress events.
///
/// # Events
///
/// | Event name       | When emitted                              |
/// |------------------|-------------------------------------------|
/// | `status`         | Indexer enters `ReadingIndexes` phase      |
/// | `ready`          | Projects/sessions discovered, UI can load  |
/// | `deep-progress`  | Each batch of deep-indexed sessions        |
/// | `done`           | Indexing complete                          |
/// | `error`          | Indexing failed                            |
///
/// The stream terminates after `done` or `error`.
pub async fn indexing_progress(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let indexing = state.indexing.clone();

    let stream = async_stream::stream! {
        let mut last_status = IndexingStatus::Idle;
        let mut last_indexed = 0usize;
        let started = std::time::Instant::now();
        // Safety: timeout after 10 minutes to prevent infinite loops if background task panics.
        // Indexing can take longer than git sync, so use a 10-minute timeout (vs 5 for git sync).
        let max_duration = std::time::Duration::from_secs(600);

        loop {
            let status = indexing.status();
            let indexed = indexing.indexed();
            let total = indexing.total();
            let projects = indexing.projects_found();
            let sessions = indexing.sessions_found();

            match status {
                IndexingStatus::Idle => {
                    // Not started yet, wait
                }
                IndexingStatus::ReadingIndexes => {
                    if last_status != IndexingStatus::ReadingIndexes {
                        let data = serde_json::json!({
                            "status": "reading-indexes"
                        });
                        yield Ok(Event::default().event("status").data(data.to_string()));
                        last_status = status;
                    }
                }
                IndexingStatus::Done if sessions > 0 && last_status != IndexingStatus::Done => {
                    // Emit ready first if we haven't yet
                    if last_status != IndexingStatus::DeepIndexing {
                        let data = serde_json::json!({
                            "status": "ready",
                            "projects": projects,
                            "sessions": sessions,
                        });
                        yield Ok(Event::default().event("ready").data(data.to_string()));
                    }
                    let data = serde_json::json!({
                        "status": "done",
                        "indexed": indexed,
                        "total": total,
                    });
                    yield Ok(Event::default().event("done").data(data.to_string()));
                    break; // Stream complete
                }
                IndexingStatus::DeepIndexing => {
                    if last_status != IndexingStatus::DeepIndexing {
                        // First deep-indexing event - emit ready
                        let data = serde_json::json!({
                            "status": "ready",
                            "projects": projects,
                            "sessions": sessions,
                        });
                        yield Ok(Event::default().event("ready").data(data.to_string()));
                        last_status = IndexingStatus::DeepIndexing;
                    }

                    if indexed != last_indexed {
                        let data = serde_json::json!({
                            "status": "deep-indexing",
                            "indexed": indexed,
                            "total": total,
                        });
                        yield Ok(Event::default().event("deep-progress").data(data.to_string()));
                        last_indexed = indexed;
                    }
                }
                IndexingStatus::Error => {
                    let error_msg = indexing.error().unwrap_or_default();
                    let data = serde_json::json!({
                        "status": "error",
                        "message": error_msg,
                    });
                    yield Ok(Event::default().event("error").data(data.to_string()));
                    break;
                }
                _ => {}
            }

            // Safety: timeout to prevent infinite loops if background task panics
            if started.elapsed() > max_duration {
                let data = serde_json::json!({
                    "status": "error",
                    "message": "Indexing timed out after 10 minutes",
                });
                yield Ok(Event::default().event("error").data(data.to_string()));
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    };

    Sse::new(stream)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::indexing_state::IndexingState;
    use crate::create_app_with_indexing;
    use vibe_recall_db::Database;

    #[tokio::test]
    async fn test_sse_endpoint_returns_event_stream() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        // Set to Done so the stream completes quickly
        state.set_status(IndexingStatus::Done);
        state.set_sessions_found(10);
        state.set_projects_found(2);

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("text/event-stream"),
            "Expected text/event-stream, got: {}",
            content_type
        );
    }

    #[tokio::test]
    async fn test_sse_done_emits_ready_and_done_events() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_status(IndexingStatus::Done);
        state.set_sessions_found(42);
        state.set_projects_found(3);
        state.set_total(42);

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        // Should contain both ready and done events
        assert!(
            body_str.contains("event: ready"),
            "Expected 'event: ready' in body: {}",
            body_str
        );
        assert!(
            body_str.contains("event: done"),
            "Expected 'event: done' in body: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_polling_status_idle() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["phase"], "idle");
        assert_eq!(json["indexed"], 0);
        assert_eq!(json["total"], 0);
        assert!(json.get("errorMessage").is_none());
    }

    #[tokio::test]
    async fn test_polling_status_deep_indexing() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_status(IndexingStatus::DeepIndexing);
        state.set_indexed(50);
        state.set_total(100);

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["phase"], "deep-indexing");
        assert_eq!(json["indexed"], 50);
        assert_eq!(json["total"], 100);
    }

    #[tokio::test]
    async fn test_polling_status_error() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_error("disk full".to_string());

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["phase"], "error");
        assert_eq!(json["errorMessage"], "disk full");
    }

    #[tokio::test]
    async fn test_sse_error_emits_error_event() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_error("disk full".to_string());

        let app = create_app_with_indexing(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: error"),
            "Expected 'event: error' in body: {}",
            body_str
        );
        assert!(
            body_str.contains("disk full"),
            "Expected error message in body: {}",
            body_str
        );
    }
}
