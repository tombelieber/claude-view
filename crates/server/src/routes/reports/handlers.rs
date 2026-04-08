// crates/server/src/routes/reports/handlers.rs
//! Route handlers for the reports API.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use claude_view_core::llm::LlmProvider;
use claude_view_core::report::build_report_prompt;
use claude_view_db::{ReportPreview, ReportRow};

use super::digest::build_context_digest;
use super::types::{GenerateRequest, GeneratingGuard, PreviewQuery, GENERATING};

/// GET /api/reports — List all saved reports.
#[utoipa::path(get, path = "/api/reports", tag = "reports",
    responses(
        (status = 200, description = "All saved reports", body = Vec<claude_view_db::ReportRow>),
    )
)]
pub async fn list_reports(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<ReportRow>>> {
    let reports = state.db.list_reports().await?;
    Ok(Json(reports))
}

/// GET /api/reports/preview — Aggregate preview stats for a date range.
#[utoipa::path(get, path = "/api/reports/preview", tag = "reports",
    params(PreviewQuery),
    responses(
        (status = 200, description = "Preview stats for date range", body = claude_view_db::ReportPreview),
    )
)]
pub async fn get_preview(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PreviewQuery>,
) -> ApiResult<Json<ReportPreview>> {
    let preview = state
        .db
        .get_report_preview(params.start_ts, params.end_ts)
        .await?;
    Ok(Json(preview))
}

/// GET /api/reports/:id — Get a single report.
#[utoipa::path(get, path = "/api/reports/{id}", tag = "reports",
    params(("id" = i64, Path, description = "Report ID")),
    responses(
        (status = 200, description = "Single report", body = claude_view_db::ReportRow),
        (status = 404, description = "Report not found"),
    )
)]
pub async fn get_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    match state.db.get_report(id).await? {
        Some(report) => Ok(Json(report)),
        None => Err(ApiError::NotFound(format!("Report {} not found", id))),
    }
}

/// DELETE /api/reports/:id — Delete a report.
#[utoipa::path(delete, path = "/api/reports/{id}", tag = "reports",
    params(("id" = i64, Path, description = "Report ID")),
    responses(
        (status = 204, description = "Report deleted"),
        (status = 404, description = "Report not found"),
    )
)]
pub async fn delete_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    if state.db.delete_report(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Report {} not found", id)))
    }
}

/// POST /api/reports/generate — Stream-generate a report via Claude CLI.
///
/// Returns SSE with events:
/// - `chunk` — text chunk from Claude CLI stdout
/// - `done`  — generation complete, includes `{ reportId }` JSON
/// - `error` — generation failed, includes `{ message }` JSON
#[utoipa::path(post, path = "/api/reports/generate", tag = "reports",
    request_body = GenerateRequest,
    responses(
        (status = 200, description = "SSE stream of report generation", content_type = "text/event-stream"),
        (status = 409, description = "Report generation already in progress"),
    )
)]
pub async fn generate_report(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateRequest>,
) -> ApiResult<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>> {
    // Prevent concurrent generation
    if GENERATING.swap(true, Ordering::SeqCst) {
        return Err(ApiError::Conflict(
            "A report is already being generated".to_string(),
        ));
    }

    // Validate report_type
    if !["daily", "weekly", "custom"].contains(&body.report_type.as_str()) {
        GENERATING.store(false, Ordering::SeqCst);
        return Err(ApiError::BadRequest(format!(
            "Invalid report_type: {}",
            body.report_type
        )));
    }

    // Use timestamps from frontend (computed using local midnight) to avoid timezone mismatch.
    let start_ts = body.start_ts;
    let end_ts = body.end_ts;

    // Build context digest from DB data
    let digest = match build_context_digest(
        &state,
        &body.report_type,
        &body.date_start,
        &body.date_end,
        start_ts,
        end_ts,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            GENERATING.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    if digest.projects.is_empty() {
        GENERATING.store(false, Ordering::SeqCst);
        return Err(ApiError::BadRequest(
            "No sessions found in the specified date range".to_string(),
        ));
    }

    let context_digest_json = serde_json::to_string(&digest).ok();
    let prompt = build_report_prompt(&digest);

    // Preview stats for persisting
    let preview = state
        .db
        .get_report_preview(start_ts, end_ts)
        .await
        .map_err(|e| {
            GENERATING.store(false, Ordering::SeqCst);
            ApiError::Internal(format!("Failed to get preview: {e}"))
        })?;

    // Spawn Claude CLI for streaming
    let provider = super::super::settings::create_llm_provider(&state.db)
        .await
        .inspect_err(|_| {
            GENERATING.store(false, Ordering::SeqCst);
        })?;
    let generation_model = provider.model().to_string();
    let (mut rx, _handle) = match provider.stream_completion(prompt) {
        Ok(pair) => pair,
        Err(e) => {
            GENERATING.store(false, Ordering::SeqCst);
            return Err(ApiError::Internal(format!(
                "Failed to spawn Claude CLI: {e}"
            )));
        }
    };

    // Create RAII guard that resets GENERATING on drop (handles client disconnect).
    let guard = GeneratingGuard;

    // Build SSE stream
    let report_type = body.report_type.clone();
    let date_start = body.date_start.clone();
    let date_end = body.date_end.clone();
    let db = state.db.clone();

    let stream = async_stream::stream! {
        // Move guard into the stream so it drops when the stream drops.
        let _guard = guard;
        let t0 = std::time::Instant::now();
        let mut full_content = String::new();

        while let Some(line) = rx.recv().await {
            if !full_content.is_empty() {
                full_content.push('\n');
            }
            full_content.push_str(&line);

            let chunk_json = serde_json::json!({ "text": line });
            yield Ok(Event::default().event("chunk").data(chunk_json.to_string()));
        }

        let generation_ms = t0.elapsed().as_millis() as i64;

        // Persist the report
        let report_id = db.insert_report(
            &report_type,
            &date_start,
            &date_end,
            &full_content,
            context_digest_json.as_deref(),
            preview.session_count,
            preview.project_count,
            preview.total_duration_secs,
            preview.total_cost_cents,
            Some(generation_ms),
            Some(&generation_model),
            None, // input_tokens — stream_completion doesn't return usage
            None, // output_tokens — stream_completion doesn't return usage
        ).await;

        match report_id {
            Ok(id) => {
                let done_json = serde_json::json!({
                    "reportId": id,
                    "generationMs": generation_ms,
                    "generationModel": &generation_model,
                    "contextDigest": context_digest_json,
                });
                yield Ok(Event::default().event("done").data(done_json.to_string()));
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to persist report");
                let err_json = serde_json::json!({ "message": format!("Failed to save report: {e}") });
                yield Ok(Event::default().event("error").data(err_json.to_string()));
            }
        }

        // Guard drops here, resetting GENERATING to false.
    };

    Ok(Sse::new(stream))
}
