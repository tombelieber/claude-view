// crates/server/src/routes/classify.rs
//! Classification API routes.
//!
//! - POST /classify       — Trigger classification job
//! - GET  /classify/status — Get classification status
//! - GET  /classify/stream — SSE stream of classification progress
//! - POST /classify/cancel — Cancel running classification

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use ts_rs::TS;

use crate::classify_state::ClassifyStatus;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use vibe_recall_core::classification::{
    self, ClassificationInput, BATCH_SIZE,
};
use vibe_recall_core::llm::{ClassificationRequest, LlmProvider};

// ============================================================================
// Request / Response Types
// ============================================================================

/// Request body for POST /api/classify.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyRequest {
    /// Which sessions to classify: "unclassified" or "all"
    pub mode: String,
    /// Dry run: calculate cost without executing
    #[serde(default)]
    pub dry_run: bool,
}

/// Response for POST /api/classify (202 Accepted).
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifyResponse {
    #[ts(type = "number")]
    pub job_id: i64,
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub estimated_cost_cents: i64,
    #[ts(type = "number")]
    pub estimated_duration_secs: i64,
    pub status: String,
}

/// Response for POST /api/classify/cancel.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CancelResponse {
    #[ts(type = "number")]
    pub job_id: i64,
    #[ts(type = "number")]
    pub classified: u64,
    pub status: String,
}

/// Response for GET /api/classify/status.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifyStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<ClassifyProgressInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<ClassifyLastRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ClassifyErrorInfo>,
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub classified_sessions: i64,
    #[ts(type = "number")]
    pub unclassified_sessions: i64,
}

/// Progress information for a running classification.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifyProgressInfo {
    #[ts(type = "number")]
    pub classified: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub percentage: f64,
    pub eta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_batch: Option<String>,
}

/// Information about the last completed classification run.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifyLastRun {
    #[ts(type = "number")]
    pub job_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[ts(type = "number")]
    pub sessions_classified: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub cost_cents: Option<i64>,
    #[ts(type = "number")]
    pub error_count: i64,
    pub status: String,
}

/// Error information for failed classification.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifyErrorInfo {
    pub message: String,
    pub retryable: bool,
}

/// SSE progress event data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SseProgressData {
    classified: u64,
    total: u64,
    percentage: f64,
    eta: String,
}

/// SSE complete event data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SseCompleteData {
    job_id: i64,
    classified: u64,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// POST /api/classify — Trigger a classification job.
async fn start_classification(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClassifyRequest>,
) -> ApiResult<impl IntoResponse> {
    // Check if a classification is already running
    let classify_state = &state.classify;
    if classify_state.status() == ClassifyStatus::Running {
        return Err(ApiError::Conflict(
            "A classification job is already running".to_string(),
        ));
    }

    // Count sessions to classify
    let session_count = match body.mode.as_str() {
        "unclassified" => state.db.count_unclassified_sessions().await?,
        "all" => state.db.count_all_sessions().await?,
        _ => {
            return Err(ApiError::BadRequest(
                "mode must be 'unclassified' or 'all'".to_string(),
            ));
        }
    };

    if session_count == 0 {
        return Err(ApiError::BadRequest(
            "No sessions to classify".to_string(),
        ));
    }

    let batch_size = BATCH_SIZE as i64;
    let estimated_cost = classification::estimate_cost_cents(session_count, batch_size);
    let estimated_duration = classification::estimate_duration_secs(session_count, batch_size);

    // Dry run: just return estimates without starting
    if body.dry_run {
        return Ok((
            StatusCode::OK,
            Json(ClassifyResponse {
                job_id: 0,
                total_sessions: session_count,
                estimated_cost_cents: estimated_cost,
                estimated_duration_secs: estimated_duration,
                status: "dry_run".to_string(),
            }),
        ));
    }

    // Create the job in the database
    let provider_name = "claude-cli";
    let model_name = "haiku";
    let db_job_id = state
        .db
        .create_classification_job(session_count, provider_name, model_name, Some(estimated_cost))
        .await?;

    let job_id_str = format!("cls_{}", db_job_id);

    // Set the classify state to running
    classify_state.set_running(job_id_str.clone(), db_job_id, session_count as u64);

    // Spawn the background classification task
    let task_state = Arc::clone(&state);
    let task_mode = body.mode.clone();
    tokio::spawn(async move {
        run_classification(task_state, db_job_id, &task_mode).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(ClassifyResponse {
            job_id: db_job_id,
            total_sessions: session_count,
            estimated_cost_cents: estimated_cost,
            estimated_duration_secs: estimated_duration,
            status: "running".to_string(),
        }),
    ))
}

/// GET /api/classify/status — Get classification status.
async fn get_classification_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ClassifyStatusResponse>> {
    let classify_state = &state.classify;
    let current_status = classify_state.status();

    let total_sessions = state.db.count_all_sessions().await
        .map_err(|e| ApiError::Internal(format!("Failed to count sessions: {e}")))?;
    let classified_sessions = state.db.count_classified_sessions().await
        .map_err(|e| ApiError::Internal(format!("Failed to count classified sessions: {e}")))?;
    let unclassified_sessions = total_sessions - classified_sessions;

    let progress = if current_status == ClassifyStatus::Running {
        Some(ClassifyProgressInfo {
            classified: classify_state.classified(),
            total: classify_state.total(),
            percentage: classify_state.percentage(),
            eta: classify_state.eta_string(),
            current_batch: classify_state.current_batch(),
        })
    } else {
        None
    };

    let error = if current_status == ClassifyStatus::Failed {
        Some(ClassifyErrorInfo {
            message: classify_state
                .error_message()
                .unwrap_or_else(|| "Unknown error".to_string()),
            retryable: true,
        })
    } else {
        None
    };

    // Get last completed job info from database
    let last_run = match state.db.get_last_completed_classification_job().await {
        Ok(job) => job.map(|job| ClassifyLastRun {
            job_id: job.id,
            completed_at: job.completed_at,
            sessions_classified: job.classified_count,
            cost_cents: job.actual_cost_cents,
            error_count: job.failed_count,
            status: job.status.as_db_str().to_string(),
        }),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch last classification job");
            None
        }
    };

    let job_id = if current_status == ClassifyStatus::Running {
        let id = classify_state.db_job_id();
        if id > 0 { Some(id) } else { None }
    } else {
        None
    };

    Ok(Json(ClassifyStatusResponse {
        status: current_status.as_str().to_string(),
        job_id,
        progress,
        last_run,
        error,
        total_sessions,
        classified_sessions,
        unclassified_sessions,
    }))
}

/// GET /api/classify/stream — SSE stream of classification progress.
async fn stream_classification(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let classify_state = Arc::clone(&state.classify);

    let stream = async_stream::stream! {
        let mut last_classified = 0u64;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));

        loop {
            interval.tick().await;

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

/// POST /api/classify/cancel — Cancel a running classification job.
async fn cancel_classification(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<CancelResponse>> {
    let classify_state = &state.classify;

    if classify_state.status() != ClassifyStatus::Running {
        return Err(ApiError::BadRequest(
            "No classification job is currently running".to_string(),
        ));
    }

    let db_job_id = classify_state.db_job_id();
    let classified = classify_state.classified();

    // Request cancellation
    classify_state.request_cancel();

    Ok(Json(CancelResponse {
        job_id: db_job_id,
        classified,
        status: "cancelled".to_string(),
    }))
}

// ============================================================================
// Background Classification Task
// ============================================================================

/// Run the classification loop in the background.
///
/// Fetches sessions in batches, calls the LLM provider, and persists results.
async fn run_classification(state: Arc<AppState>, db_job_id: i64, mode: &str) {
    let classify_state = &state.classify;
    let db = &state.db;

    // Fetch all sessions to classify
    let sessions = match mode {
        "all" => db.get_all_sessions_for_classification(100_000).await,
        _ => db.get_unclassified_sessions(100_000).await,
    };

    let sessions = match sessions {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Failed to fetch sessions: {}", e);
            tracing::error!("{}", msg);
            classify_state.set_failed(msg.clone());
            if let Err(e) = db.fail_classification_job(db_job_id, &msg).await {
                tracing::error!(error = %e, "Failed to record classification job failure");
            }
            return;
        }
    };

    let total = sessions.len();
    if total == 0 {
        classify_state.set_completed();
        if let Err(e) = db.complete_classification_job(db_job_id, Some(0)).await {
            tracing::error!(error = %e, "Failed to complete classification job with 0 sessions");
        }
        return;
    }

    // Build classification inputs
    let inputs: Vec<ClassificationInput> = sessions
        .iter()
        .map(|(id, preview, skills_json)| {
            let skills: Vec<String> = serde_json::from_str(skills_json).unwrap_or_default();
            ClassificationInput {
                session_id: id.clone(),
                preview: preview.clone(),
                skills_used: skills,
            }
        })
        .collect();

    // Process in batches
    let mut classified_total = 0u64;
    let mut failed_total = 0u64;
    let mut batch_num = 0usize;

    for batch in inputs.chunks(BATCH_SIZE) {
        // Check for cancellation
        if classify_state.is_cancel_requested() {
            classify_state.set_cancelled();
            if let Err(e) = db.cancel_classification_job(db_job_id).await {
                tracing::error!(error = %e, "Failed to cancel classification job");
            }
            if let Err(e) = db
                .update_classification_job_progress(
                    db_job_id,
                    classified_total as i64,
                    0,
                    failed_total as i64,
                    None,
                )
                .await
            {
                tracing::error!(error = %e, "Failed to update cancelled job progress");
            }
            return;
        }

        batch_num += 1;
        classify_state.set_current_batch(format!("Batch {} ({} sessions)", batch_num, batch.len()));

        tracing::debug!(batch_num, batch_size = batch.len(), "Processing batch");

        // For the MVP, classify each session individually using the existing provider
        let mut batch_updates: Vec<(String, String, String, String, f64, String)> = Vec::new();

        for input in batch {
            if classify_state.is_cancel_requested() {
                break;
            }

            let single_provider =
                vibe_recall_core::llm::ClaudeCliProvider::new("haiku").with_timeout(60);
            let single_request = ClassificationRequest {
                session_id: input.session_id.clone(),
                first_prompt: input.preview.clone(),
                files_touched: vec![],
                skills_used: input.skills_used.clone(),
            };

            match single_provider.classify(single_request).await {
                Ok(resp) => {
                    batch_updates.push((
                        input.session_id.clone(),
                        resp.category_l1.clone(),
                        resp.category_l2.clone(),
                        resp.category_l3.clone(),
                        resp.confidence,
                        "claude-cli".to_string(),
                    ));
                    classified_total += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %input.session_id,
                        error = %e,
                        "Single session classification failed"
                    );
                    failed_total += 1;
                    classify_state.increment_errors();
                }
            }
        }

        // Batch write to database (single transaction)
        if !batch_updates.is_empty() {
            if let Err(e) = db.batch_update_session_classifications(&batch_updates).await {
                tracing::error!(error = %e, "Failed to persist batch classifications");
                // Count the batch as failed since results weren't persisted
                failed_total += batch_updates.len() as u64;
                classified_total -= batch_updates.len() as u64;
            }
        }

        classify_state.increment_classified(batch_updates.len() as u64);

        // Update job progress in database
        if let Err(e) = db
            .update_classification_job_progress(
                db_job_id,
                classified_total as i64,
                0,
                failed_total as i64,
                None,
            )
            .await
        {
            tracing::error!(error = %e, "Failed to update classification progress");
        }
    }

    // Job completed
    classify_state.set_completed();
    if let Err(e) = db.complete_classification_job(db_job_id, Some(0)).await {
        tracing::error!(error = %e, "Failed to complete classification job");
    }
    if let Err(e) = db
        .update_classification_job_progress(
            db_job_id,
            classified_total as i64,
            0,
            failed_total as i64,
            None,
        )
        .await
    {
        tracing::error!(error = %e, "Failed to update final job progress");
    }

    tracing::info!(
        classified = classified_total,
        failed = failed_total,
        "Classification job completed"
    );
}

// ============================================================================
// Router
// ============================================================================

/// Build the classify router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/classify", post(start_classification))
        .route("/classify/status", get(get_classification_status))
        .route("/classify/stream", get(stream_classification))
        .route("/classify/cancel", post(cancel_classification))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let _router = router();
    }

    #[test]
    fn test_classify_request_deserialize() {
        let json = r#"{"mode": "unclassified"}"#;
        let req: ClassifyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, "unclassified");
        assert!(!req.dry_run);
    }

    #[test]
    fn test_classify_request_dry_run() {
        let json = r#"{"mode": "all", "dryRun": true}"#;
        let req: ClassifyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, "all");
        assert!(req.dry_run);
    }

    #[test]
    fn test_classify_response_serialize() {
        let resp = ClassifyResponse {
            job_id: 42,
            total_sessions: 100,
            estimated_cost_cents: 5,
            estimated_duration_secs: 40,
            status: "running".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jobId\":42"));
        assert!(json.contains("\"totalSessions\":100"));
    }

    #[test]
    fn test_cancel_response_serialize() {
        let resp = CancelResponse {
            job_id: 1,
            classified: 50,
            status: "cancelled".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jobId\":1"));
        assert!(json.contains("\"classified\":50"));
    }

    #[test]
    fn test_classify_status_response_serialize() {
        let resp = ClassifyStatusResponse {
            status: "idle".to_string(),
            job_id: None,
            progress: None,
            last_run: None,
            error: None,
            total_sessions: 500,
            classified_sessions: 400,
            unclassified_sessions: 100,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"idle\""));
        assert!(json.contains("\"totalSessions\":500"));
        assert!(!json.contains("\"jobId\"")); // Should be skipped when None
    }

    #[tokio::test]
    async fn test_start_classification_empty_db() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/classify")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"mode":"unclassified"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 400 because no sessions exist
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_status_idle() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/classify/status")
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
        assert_eq!(json["status"], "idle");
        assert_eq!(json["totalSessions"], 0);
    }

    #[tokio::test]
    async fn test_cancel_when_not_running() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        use vibe_recall_db::Database;

        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);

        let app = Router::new()
            .nest("/api", router())
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/classify/cancel")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 400 because no job is running
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
