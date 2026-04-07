//! Route handlers for the classification API.
//!
//! - POST /api/classify              — start a classification job
//! - GET  /api/classify/status       — get classification status
//! - POST /api/classify/cancel       — cancel running classification
//! - POST /api/classify/single/:id   — classify a single session

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

use crate::classify_state::ClassifyStatus;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use claude_view_core::llm::{ClassificationRequest, LlmProvider};

use super::job::run_classification;
use super::types::{
    CancelResponse, ClassifyErrorInfo, ClassifyLastRun, ClassifyProgressInfo, ClassifyRequest,
    ClassifyResponse, ClassifySingleResponse, ClassifyStatusResponse,
};

/// POST /api/classify — Trigger a classification job.
#[utoipa::path(post, path = "/api/classify", tag = "classify",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Classification job started or dry-run result", body = crate::routes::classify::ClassifyResponse),
        (status = 202, description = "Classification job accepted"),
        (status = 409, description = "Job already running"),
    )
)]
pub async fn start_classification(
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
        return Err(ApiError::BadRequest("No sessions to classify".to_string()));
    }

    // Dry run: strict mode returns scope only (no synthetic pre-run estimates).
    if body.dry_run {
        return Ok((
            StatusCode::OK,
            Json(ClassifyResponse {
                job_id: 0,
                total_sessions: session_count,
                status: "dry_run".to_string(),
            }),
        ));
    }

    // Create the job in the database
    let settings = state
        .db
        .get_app_settings()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read LLM settings: {e}")))?;
    let provider_name = "claude-cli";
    let model_name = settings.llm_model.clone();
    let db_job_id = state
        .db
        .create_classification_job(session_count, provider_name, &model_name)
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
            status: "running".to_string(),
        }),
    ))
}

/// GET /api/classify/status — Get classification status.
#[utoipa::path(get, path = "/api/classify/status", tag = "classify",
    responses(
        (status = 200, description = "Classification job status and progress", body = crate::routes::classify::ClassifyStatusResponse),
    )
)]
pub async fn get_classification_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ClassifyStatusResponse>> {
    let classify_state = &state.classify;
    let current_status = classify_state.status();

    let total_sessions = state
        .db
        .count_all_sessions()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count sessions: {e}")))?;
    let classified_sessions = state
        .db
        .count_classified_sessions()
        .await
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
        if id > 0 {
            Some(id)
        } else {
            None
        }
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

/// POST /api/classify/cancel — Cancel a running classification job.
#[utoipa::path(post, path = "/api/classify/cancel", tag = "classify",
    responses(
        (status = 200, description = "Classification job cancelled", body = crate::routes::classify::CancelResponse),
        (status = 400, description = "No job running"),
    )
)]
pub async fn cancel_classification(
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

/// POST /api/classify/single/:session_id — Classify a single session synchronously.
///
/// Bypasses ClassifyState entirely — no job record, no SSE.
/// Returns the classification result directly.
/// Uses dedicated O(1) DB queries — NOT the bulk session list.
#[utoipa::path(post, path = "/api/classify/single/{session_id}", tag = "classify",
    params(("session_id" = String, Path, description = "Session ID to classify")),
    responses(
        (status = 200, description = "Classification result (live or cached)", body = crate::routes::classify::ClassifySingleResponse),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn classify_single_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let t0 = std::time::Instant::now();
    tracing::info!(session_id = %session_id, "classify/single: request received");

    // 1. Check if already classified (O(1) query)
    if let Some((l1, l2, l3, conf)) = state.db.get_session_classification(&session_id).await? {
        tracing::info!(session_id = %session_id, l2 = %l2, "classify/single: cache hit");
        return Ok((
            StatusCode::OK,
            Json(ClassifySingleResponse {
                session_id,
                category_l1: l1,
                category_l2: l2,
                category_l3: l3,
                confidence: conf,
                was_cached: true,
            }),
        ));
    }

    // 2. Fetch session data for classification (O(1) query)
    let (_, preview, skills_json) = state
        .db
        .get_session_for_classification(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let preview_short = &preview[..preview.len().min(80)];
    tracing::info!(session_id = %session_id, preview = %preview_short, "classify/single: calling Claude CLI");

    // 3. Parse skills
    let skills: Vec<String> = match serde_json::from_str(&skills_json) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "corrupt skills_json in DB, using empty default");
            Vec::new()
        }
    };

    // 4. Classify via Claude CLI
    let provider = crate::routes::settings::create_llm_provider(&state.db).await?;
    let request = ClassificationRequest {
        session_id: session_id.clone(),
        first_prompt: preview,
        files_touched: vec![],
        skills_used: skills,
    };

    let resp = provider.classify(request).await.map_err(|e| {
        tracing::error!(session_id = %session_id, elapsed_ms = t0.elapsed().as_millis() as u64, error = %e, "classify/single: failed");
        ApiError::Internal(format!("Classification failed: {e}"))
    })?;

    tracing::info!(
        session_id = %session_id,
        l1 = %resp.category_l1,
        l2 = %resp.category_l2,
        l3 = %resp.category_l3,
        confidence = resp.confidence,
        elapsed_ms = t0.elapsed().as_millis() as u64,
        "classify/single: success"
    );

    // 5. Persist to DB
    state
        .db
        .update_session_classification(
            &session_id,
            &resp.category_l1,
            &resp.category_l2,
            &resp.category_l3,
            resp.confidence,
            "claude-cli",
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ClassifySingleResponse {
            session_id,
            category_l1: resp.category_l1,
            category_l2: resp.category_l2,
            category_l3: resp.category_l3,
            confidence: resp.confidence,
            was_cached: false,
        }),
    ))
}
