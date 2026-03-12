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
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatusResponse {
    pub phase: String,
    pub indexed: usize,
    pub total: usize,
    #[ts(type = "number")]
    pub bytes_processed: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
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
pub async fn indexing_status(State(state): State<Arc<AppState>>) -> Json<IndexingStatusResponse> {
    let indexing = &state.indexing;
    let status = indexing.status();

    let phase = match status {
        IndexingStatus::Idle => "idle",
        IndexingStatus::ReadingIndexes => "reading-indexes",
        IndexingStatus::DeepIndexing => "deep-indexing",
        IndexingStatus::Finalizing => "finalizing",
        IndexingStatus::Done => "done",
        IndexingStatus::Error => "error",
    };

    Json(IndexingStatusResponse {
        phase: phase.to_string(),
        indexed: indexing.indexed(),
        total: indexing.total(),
        bytes_processed: indexing.bytes_processed(),
        bytes_total: indexing.bytes_total(),
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
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        let mut last_status = IndexingStatus::Idle;
        let mut last_indexed = 0usize;
        // Progress-aware stall detection: only timeout when the indexer
        // has made zero progress (no change in status, indexed, or bytes)
        // for stall_timeout. Active indexing never triggers timeout.
        let mut last_progress_at = std::time::Instant::now();
        let mut prev_indexed = 0usize;
        let mut prev_bytes = 0u64;
        let mut prev_status = IndexingStatus::Idle;
        let stall_timeout = indexing.stall_timeout();

        loop {
            let status = indexing.status();
            let indexed = indexing.indexed();
            let total = indexing.total();
            let projects = indexing.projects_found();
            let sessions = indexing.sessions_found();
            let bytes_processed = indexing.bytes_processed();
            let bytes_total = indexing.bytes_total();

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
                IndexingStatus::Done if last_status != IndexingStatus::Done => {
                    // Emit ready first if we haven't yet
                    if !matches!(last_status, IndexingStatus::DeepIndexing | IndexingStatus::Finalizing) {
                        // Use the larger of hint-based sessions and scan-based total.
                        // sessions_found comes from filesystem discovery (Task 1 fix)
                        // but total may be larger if set after sessions_found.
                        let effective_sessions = std::cmp::max(sessions, total);
                        let data = serde_json::json!({
                            "status": "ready",
                            "projects": projects,
                            "sessions": effective_sessions,
                        });
                        yield Ok(Event::default().event("ready").data(data.to_string()));
                    }
                    let data = serde_json::json!({
                        "status": "done",
                        "indexed": indexed,
                        "total": total,
                        "bytes_processed": bytes_processed,
                        "bytes_total": bytes_total,
                    });
                    yield Ok(Event::default().event("done").data(data.to_string()));
                    break; // Stream complete
                }
                IndexingStatus::DeepIndexing => {
                    if last_status != IndexingStatus::DeepIndexing {
                        // Wait for on_total_known to fire before emitting ready.
                        // Without this, the ready event shows 0 sessions when
                        // no sessions-index.json hint files exist.
                        if sessions > 0 || total > 0 {
                            let effective_sessions = std::cmp::max(sessions, total);
                            let data = serde_json::json!({
                                "status": "ready",
                                "projects": projects,
                                "sessions": effective_sessions,
                            });
                            yield Ok(Event::default().event("ready").data(data.to_string()));
                            last_status = IndexingStatus::DeepIndexing;
                        }
                        // If sessions==0 && total==0, don't set last_status.
                        // Next poll will re-enter this branch after on_total_known fires.
                    }

                    if indexed != last_indexed {
                        let data = serde_json::json!({
                            "status": "deep-indexing",
                            "indexed": indexed,
                            "total": total,
                            "bytes_processed": bytes_processed,
                            "bytes_total": bytes_total,
                        });
                        yield Ok(Event::default().event("deep-progress").data(data.to_string()));
                        last_indexed = indexed;
                    }
                }
                IndexingStatus::Finalizing => {
                    if last_status != IndexingStatus::Finalizing {
                        let data = serde_json::json!({
                            "status": "finalizing",
                            "indexed": indexed,
                            "total": total,
                        });
                        yield Ok(Event::default().event("finalizing").data(data.to_string()));
                        last_status = IndexingStatus::Finalizing;
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

            // Progress-aware stall detection: reset timer on any progress,
            // timeout only when genuinely stuck (no progress for stall_timeout).
            if indexed != prev_indexed || bytes_processed != prev_bytes || status != prev_status {
                last_progress_at = std::time::Instant::now();
                prev_indexed = indexed;
                prev_bytes = bytes_processed;
                prev_status = status;
            }
            if last_progress_at.elapsed() > stall_timeout {
                let threshold_secs = stall_timeout.as_secs();
                let stall_msg = if threshold_secs >= 60 {
                    format!("Indexing stalled — no progress for {} minutes", threshold_secs / 60)
                } else {
                    format!("Indexing stalled — no progress for {} seconds", threshold_secs)
                };
                let data = serde_json::json!({
                    "status": "error",
                    "message": stall_msg,
                });
                yield Ok(Event::default().event("error").data(data.to_string()));
                break;
            }

            // Wait for next poll interval OR shutdown signal (clean Ctrl+C exit).
            // Only break if the value became true (real shutdown). Ignore Err from
            // dropped sender (test constructors drop the Sender immediately).
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
            }
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

    use crate::create_app_with_indexing;
    use crate::indexing_state::IndexingState;
    use claude_view_db::Database;

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

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 3s — Done with sessions > 0 should terminate immediately")
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

    // ---- Task 2 TDD: write failing tests before fixes ----

    /// Regression: SSE must complete when Done with sessions_found == 0.
    /// This happens when no sessions-index.json files exist but
    /// scan_and_index_all discovers .jsonl files from the filesystem.
    #[tokio::test]
    async fn test_sse_done_with_zero_sessions_found_still_completes() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_stall_timeout(std::time::Duration::from_secs(2));
        state.set_status(IndexingStatus::Done);
        state.set_sessions_found(0);
        state.set_projects_found(0);
        state.set_total(15);
        state.set_indexed(15);

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

        // Wrap in timeout to fail fast in TDD red phase.
        // Without fix: SSE spins until stall_timeout → this timeout fires first.
        let body = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 3s — if this times out, Done with 0 sessions is stuck")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: done"),
            "Expected 'event: done' but got: {}",
            body_str
        );
        assert!(
            !body_str.contains("timed out"),
            "Must NOT contain timeout error: {}",
            body_str
        );
        assert!(
            body_str.contains("event: ready"),
            "Expected 'event: ready' in body: {}",
            body_str
        );
    }

    /// Edge case: brand new Claude Code install with zero sessions.
    /// Done with 0 sessions and 0 total must still complete the SSE stream.
    #[tokio::test]
    async fn test_sse_done_with_zero_sessions_and_zero_total_completes() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_stall_timeout(std::time::Duration::from_secs(2));
        state.set_status(IndexingStatus::Done);
        state.set_sessions_found(0);
        state.set_projects_found(0);
        state.set_total(0);
        state.set_indexed(0);

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

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 3s for empty install")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: done"),
            "Expected 'event: done' for empty install: {}",
            body_str
        );
        assert!(
            !body_str.contains("timed out"),
            "Must NOT timeout for empty install: {}",
            body_str
        );
    }

    /// Stall detection: SSE must timeout when indexer makes zero progress.
    /// Simulates a stuck indexer (DeepIndexing status, counters frozen).
    /// Uses 1-second stall timeout for fast test execution.
    #[tokio::test]
    async fn test_sse_stall_detection_triggers_timeout() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        // Use short stall timeout for test speed
        state.set_stall_timeout(std::time::Duration::from_secs(1));
        // Stuck in DeepIndexing — counters frozen at 5/100
        state.set_status(IndexingStatus::DeepIndexing);
        state.set_sessions_found(100);
        state.set_total(100);
        state.set_indexed(5);

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

        // Stall test: stream should end within stall_timeout + margin.
        // With 1s stall timeout + 100ms poll interval, expect ~1.5s.
        let body = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 5s (1s stall timeout + margin)")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        // Should eventually emit an error about stalled progress
        assert!(
            body_str.contains("event: error"),
            "Expected stall timeout error: {}",
            body_str
        );
        assert!(
            body_str.contains("no progress"),
            "Expected 'no progress' in error message: {}",
            body_str
        );
    }

    /// Active indexing must never trigger timeout regardless of total duration.
    /// Simulates: SSE connects, sees DeepIndexing with progress updates.
    /// Uses a short stall_timeout but the state changes between polls.
    #[tokio::test]
    async fn test_sse_active_progress_does_not_timeout() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        // Stall timeout longer than the progress interval (200ms) with generous margin.
        // 2s timeout vs 200ms progress = 1.8s margin — resilient under CI load.
        state.set_stall_timeout(std::time::Duration::from_secs(2));
        state.set_status(IndexingStatus::DeepIndexing);
        state.set_sessions_found(3);
        state.set_total(3);
        state.set_indexed(0);

        let app = create_app_with_indexing(db, state.clone());

        // Spawn a task that simulates progress then completion
        let progress_state = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            progress_state.set_indexed(1);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            progress_state.set_indexed(2);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            progress_state.set_indexed(3);
            progress_state.set_status(IndexingStatus::Done);
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 5s — active indexing must not hang")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: done"),
            "Expected 'event: done' — active indexing must complete: {}",
            body_str
        );
        assert!(
            !body_str.contains("timed out") && !body_str.contains("no progress"),
            "Must NOT contain any timeout error: {}",
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

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("SSE must complete within 3s — Error status should terminate immediately")
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

    // ---- Task 3: Integration + E2E regression tests ----

    /// Integration: full indexing lifecycle with realistic state transitions.
    /// Tests all three layers working together:
    /// - Layer 1: sessions_found updated during DeepIndexing (simulated on_total_known)
    /// - Layer 2: Done with initially-zero sessions_found completes SSE
    /// - Layer 3: Active progress resets stall timer, no false timeout
    #[tokio::test]
    async fn test_sse_full_lifecycle_integration() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_stall_timeout(std::time::Duration::from_secs(2));

        let app = create_app_with_indexing(db, state.clone());

        // Simulate realistic indexing lifecycle in background
        let bg = state.clone();
        tokio::spawn(async move {
            // Phase 1: ReadingIndexes (reading hints)
            bg.set_status(IndexingStatus::ReadingIndexes);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Phase 2: Hints read — sessions_found from hints = 0
            // (simulating no sessions-index.json files exist)
            bg.set_sessions_found(0);
            bg.set_projects_found(0);

            // Phase 3: DeepIndexing — on_total_known fires with filesystem count
            bg.set_status(IndexingStatus::DeepIndexing);
            bg.set_total(5);
            bg.set_sessions_found(5); // Layer 1 fix: update sessions_found
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Phase 4: Progress updates
            for i in 1..=5 {
                bg.set_indexed(i);
                bg.add_bytes_processed(1000);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            // Phase 5: Finalizing
            bg.set_status(IndexingStatus::Finalizing);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Phase 6: Done
            bg.set_status(IndexingStatus::Done);
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/indexing/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("Full lifecycle SSE must complete within 5s")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        // Verify full lifecycle events, no errors
        assert!(
            body_str.contains("event: status"),
            "Expected reading-indexes status event: {}",
            body_str
        );
        assert!(
            body_str.contains("event: ready"),
            "Expected ready event: {}",
            body_str
        );
        assert!(
            body_str.contains("event: deep-progress"),
            "Expected deep-progress event: {}",
            body_str
        );
        assert!(
            body_str.contains("event: done"),
            "Expected done event: {}",
            body_str
        );
        assert!(
            !body_str.contains("timed out") && !body_str.contains("no progress"),
            "Must NOT contain any timeout/stall error: {}",
            body_str
        );
    }

    /// E2E regression: reproduces the exact original bug conditions.
    /// sessions_found=0 (no sessions-index.json) + status=Done (scan already complete)
    /// = the SSE stream MUST complete with correct session count, NOT timeout.
    ///
    /// Before fix: SSE spun forever → stall timeout → false error.
    /// After fix: SSE emits ready (with max(0, total)=15 sessions) + done immediately.
    #[tokio::test]
    async fn test_e2e_first_launch_no_false_timeout() {
        let db = Database::new_in_memory().await.unwrap();
        let state = Arc::new(IndexingState::new());
        state.set_stall_timeout(std::time::Duration::from_secs(2));

        // Exact original-bug conditions:
        // - sessions_found=0 (no sessions-index.json files existed)
        // - status=Done (scan_and_index_all completed before SSE connected)
        // - total=15 (filesystem discovered 15 .jsonl files)
        // - indexed=15 (all files were indexed)
        state.set_sessions_found(0);
        state.set_projects_found(0);
        state.set_total(15);
        state.set_indexed(15);
        state.set_status(IndexingStatus::Done);

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

        let body = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            axum::body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("First-launch SSE must complete within 3s — must not hang")
        .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: done"),
            "First launch must emit done: {}",
            body_str
        );
        assert!(
            body_str.contains("event: ready"),
            "First launch must emit ready: {}",
            body_str
        );
        assert!(
            !body_str.contains("timed out"),
            "First launch must NOT show 'timed out': {}",
            body_str
        );
        assert!(
            !body_str.contains("no progress"),
            "First launch must NOT show 'no progress': {}",
            body_str
        );
        // Verify ready event uses max(sessions=0, total=15) = 15
        assert!(
            body_str.contains("\"sessions\":15"),
            "Ready should show 15 sessions (from max(0, 15)): {}",
            body_str
        );
    }
}
