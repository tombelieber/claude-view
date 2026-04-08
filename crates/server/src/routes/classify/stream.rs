//! SSE streaming endpoint for classification progress.

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use std::convert::Infallible;
use std::sync::Arc;

use crate::classify_state::ClassifyStatus;
use crate::state::AppState;

use super::types::{SseCompleteData, SseProgressData};

/// GET /api/classify/stream — SSE stream of classification progress.
#[utoipa::path(get, path = "/api/classify/stream", tag = "classify",
    responses(
        (status = 200, description = "SSE stream of classification progress events", content_type = "text/event-stream"),
    )
)]
pub async fn stream_classification(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let classify_state = Arc::clone(&state.classify);
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        let mut last_classified = 0u64;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(1000));

        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
            }

            let status = classify_state.status();
            let classified = classify_state.classified();

            // Emit progress if it changed
            if classified != last_classified && status == ClassifyStatus::Running {
                last_classified = classified;
                let data = SseProgressData {
                    classified,
                    total: classify_state.total(),
                    percentage: classify_state.percentage(),
                    eta: classify_state.eta_string(),
                };
                let json = match serde_json::to_string(&data) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to serialize SSE progress data");
                        continue;
                    }
                };
                yield Ok(Event::default().event("progress").data(json));
            }

            // Emit terminal events
            match status {
                ClassifyStatus::Completed => {
                    let data = SseCompleteData {
                        job_id: classify_state.db_job_id(),
                        classified: classify_state.classified(),
                    };
                    let json = match serde_json::to_string(&data) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to serialize SSE progress data");
                            continue;
                        }
                    };
                    yield Ok(Event::default().event("complete").data(json));
                    break;
                }
                ClassifyStatus::Failed => {
                    let msg = classify_state.error_message().unwrap_or_else(|| "Unknown error".to_string());
                    let data = serde_json::json!({
                        "message": msg,
                        "retrying": false,
                    });
                    yield Ok(Event::default().event("error").data(data.to_string()));
                    break;
                }
                ClassifyStatus::Cancelled => {
                    let data = serde_json::json!({
                        "jobId": classify_state.db_job_id(),
                        "classified": classify_state.classified(),
                    });
                    yield Ok(Event::default().event("cancelled").data(data.to_string()));
                    break;
                }
                ClassifyStatus::Idle => {
                    // Nothing running — emit idle and stop
                    yield Ok(Event::default().event("idle").data("{}"));
                    break;
                }
                ClassifyStatus::Running => {
                    // Continue polling
                }
            }
        }
    };

    Sse::new(stream)
}
