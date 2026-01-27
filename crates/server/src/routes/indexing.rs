//! SSE endpoint for real-time indexing progress.
//!
//! `GET /api/indexing/progress` streams Server-Sent Events that reflect the
//! current [`IndexingStatus`] of the background indexer.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::Router;

use crate::indexing_state::IndexingStatus;
use crate::state::AppState;

/// Build the indexing sub-router.
///
/// Routes:
/// - `GET /indexing/progress` - SSE stream of indexing progress events
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/indexing/progress", get(indexing_progress))
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
