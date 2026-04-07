//! Route handlers for indexing progress endpoints.
//!
//! - `GET /api/indexing/progress` -- SSE stream (kept for direct-connect production use)
//! - `GET /api/indexing/status`   -- JSON snapshot (reliable through any proxy)

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::{Json, Router};

use crate::indexing_state::IndexingStatus;
use crate::state::AppState;

use super::types::IndexingStatusResponse;

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

/// GET /api/indexing/status -- lightweight JSON snapshot of indexing progress.
///
/// Returns the current phase, indexed count, and total count.
/// Designed for polling (every 200-300 ms) from the frontend during rebuilds.
#[utoipa::path(get, path = "/api/indexing/status", tag = "sync",
    responses(
        (status = 200, description = "Current indexing phase and progress", body = crate::routes::indexing::IndexingStatusResponse),
    )
)]
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
#[utoipa::path(get, path = "/api/indexing/progress", tag = "sync",
    responses(
        (status = 200, description = "SSE stream of indexing progress events", content_type = "text/event-stream"),
    )
)]
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
