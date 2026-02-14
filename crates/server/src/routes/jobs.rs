// crates/server/src/routes/jobs.rs
//! API routes for background job management.
//!
//! - GET /jobs — List all active background jobs
//! - GET /jobs/stream — SSE stream of job progress updates

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::Router;
use std::convert::Infallible;
use std::sync::Arc;

use crate::jobs::JobProgress;
use crate::state::AppState;

/// GET /api/jobs — List all active jobs.
async fn list_jobs(State(state): State<Arc<AppState>>) -> axum::Json<Vec<JobProgress>> {
    axum::Json(state.jobs.active_jobs())
}

/// GET /api/jobs/stream — SSE stream of all job progress updates.
async fn stream_jobs(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.jobs.subscribe();

    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Ok(progress) = rx.recv().await {
            let json = serde_json::to_string(&progress).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }
    };

    Sse::new(stream)
}

/// Build the jobs router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/jobs", get(list_jobs))
        .route("/jobs/stream", get(stream_jobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobRunner;

    #[test]
    fn test_router_creation() {
        // Smoke test: router should be constructable
        let _router = router();
    }

    #[tokio::test]
    async fn test_list_jobs_empty() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(AppState {
            start_time: std::time::Instant::now(),
            db,
            indexing: Arc::new(crate::IndexingState::new()),
            registry: Arc::new(std::sync::RwLock::new(None)),
            jobs: Arc::new(JobRunner::new()),
            classify: Arc::new(crate::classify_state::ClassifyState::new()),
            facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
            git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
            pricing: std::collections::HashMap::new(),
            live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            live_tx: tokio::sync::broadcast::channel(256).0,
            rules_dir: std::env::temp_dir().join("claude-rules-test"),
        });

        let app = Router::new()
            .route("/api/jobs", get(list_jobs))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(json.is_empty());
    }
}
