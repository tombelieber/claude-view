// crates/server/src/routes/reports.rs
//! Report API routes.
//!
//! - POST /reports/generate — Stream-generate a report via Claude CLI (SSE)
//! - GET  /reports           — List all saved reports
//! - GET  /reports/:id       — Get a single report
//! - DELETE /reports/:id     — Delete a report
//! - GET  /reports/preview   — Aggregate preview stats for a date range

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use claude_view_core::report::{
    BranchDigest, ContextDigest, ProjectDigest, SessionDigest, build_report_prompt,
};
use claude_view_db::{ReportPreview, ReportRow};

/// Guard to prevent concurrent report generation.
static GENERATING: AtomicBool = AtomicBool::new(false);

/// RAII guard that resets GENERATING to false on drop.
/// Ensures the lock is released even if the SSE stream is dropped (client disconnect).
struct GeneratingGuard;

impl Drop for GeneratingGuard {
    fn drop(&mut self) {
        GENERATING.store(false, Ordering::SeqCst);
    }
}

// ============================================================================
// Request / Response Types
// ============================================================================

/// Request body for POST /api/reports/generate.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub report_type: String,
    pub date_start: String,
    pub date_end: String,
    /// Unix timestamp for range start (from frontend, uses local midnight).
    pub start_ts: i64,
    /// Unix timestamp for range end (from frontend, uses local midnight + 86399).
    pub end_ts: i64,
}

/// Query params for GET /api/reports/preview.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewQuery {
    pub start_ts: i64,
    pub end_ts: i64,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET /api/reports — List all saved reports.
async fn list_reports(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ReportRow>>> {
    let reports = state.db.list_reports().await?;
    Ok(Json(reports))
}

/// GET /api/reports/preview — Aggregate preview stats for a date range.
async fn get_preview(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PreviewQuery>,
) -> ApiResult<Json<ReportPreview>> {
    let preview = state.db.get_report_preview(params.start_ts, params.end_ts).await?;
    Ok(Json(preview))
}

/// GET /api/reports/:id — Get a single report.
async fn get_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    match state.db.get_report(id).await? {
        Some(report) => Ok(Json(report)),
        None => Err(ApiError::NotFound(format!("Report {} not found", id))),
    }
}

/// DELETE /api/reports/:id — Delete a report.
async fn delete_report(
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
async fn generate_report(
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
    let digest = match build_context_digest(&state, &body.report_type, &body.date_start, &body.date_end, start_ts, end_ts).await {
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
    let preview = state.db.get_report_preview(start_ts, end_ts).await.map_err(|e| {
        GENERATING.store(false, Ordering::SeqCst);
        ApiError::Internal(format!("Failed to get preview: {e}"))
    })?;

    // Spawn Claude CLI for streaming
    let provider = claude_view_core::llm::ClaudeCliProvider::new("haiku").with_timeout(120);
    let (mut rx, _handle) = match provider.stream_completion(prompt) {
        Ok(pair) => pair,
        Err(e) => {
            GENERATING.store(false, Ordering::SeqCst);
            return Err(ApiError::Internal(format!("Failed to spawn Claude CLI: {e}")));
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
        ).await;

        match report_id {
            Ok(id) => {
                let done_json = serde_json::json!({
                    "reportId": id,
                    "generationMs": generation_ms,
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

/// Build a context digest from DB data for the given date range.
async fn build_context_digest(
    state: &AppState,
    report_type: &str,
    date_start: &str,
    date_end: &str,
    start_ts: i64,
    end_ts: i64,
) -> Result<ContextDigest, ApiError> {
    // Query sessions in range via Database method
    let sessions = state.db.get_sessions_in_range(start_ts, end_ts).await
        .map_err(|e| ApiError::Internal(format!("Failed to query sessions: {e}")))?;

    if sessions.is_empty() {
        return Ok(ContextDigest::default());
    }

    // Group by project -> branch
    let mut project_map: HashMap<String, HashMap<String, Vec<SessionDigest>>> = HashMap::new();
    let mut project_durations: HashMap<String, i64> = HashMap::new();
    let mut project_session_counts: HashMap<String, usize> = HashMap::new();

    for (_, project, preview, category, duration, branch) in &sessions {
        let branch_name = branch.as_deref().unwrap_or("(no branch)").to_string();

        project_map
            .entry(project.clone())
            .or_default()
            .entry(branch_name)
            .or_default()
            .push(SessionDigest {
                first_prompt: preview.clone(),
                category: category.clone(),
                duration_secs: *duration,
            });

        *project_durations.entry(project.clone()).or_default() += duration;
        *project_session_counts.entry(project.clone()).or_default() += 1;
    }

    // Query commit counts per project via Database method
    let commit_rows = state.db.get_commit_counts_in_range(start_ts, end_ts).await
        .unwrap_or_default();
    let commit_counts: HashMap<String, i64> = commit_rows.into_iter().collect();

    // Query top tools and skills via Database methods
    let top_tools = state.db.get_top_tools_in_range(start_ts, end_ts, 5).await
        .unwrap_or_default();
    let top_skills = state.db.get_top_skills_in_range(start_ts, end_ts, 5).await
        .unwrap_or_default();

    // Build project digests
    let mut projects: Vec<ProjectDigest> = project_map
        .into_iter()
        .map(|(name, branches)| {
            let branch_digests: Vec<BranchDigest> = branches
                .into_iter()
                .map(|(branch_name, sessions)| BranchDigest {
                    name: branch_name,
                    sessions,
                })
                .collect();

            ProjectDigest {
                session_count: *project_session_counts.get(&name).unwrap_or(&0),
                commit_count: *commit_counts.get(&name).unwrap_or(&0) as usize,
                total_duration_secs: *project_durations.get(&name).unwrap_or(&0),
                branches: branch_digests,
                name,
            }
        })
        .collect();

    // Sort projects by session count descending
    projects.sort_by(|a, b| b.session_count.cmp(&a.session_count));

    let total_sessions = sessions.len();
    let total_projects = projects.len();
    let date_range = if date_start == date_end {
        date_start.to_string()
    } else {
        format!("{date_start} to {date_end}")
    };

    Ok(ContextDigest {
        report_type: report_type.to_string(),
        date_range,
        projects,
        top_tools,
        top_skills,
        summary_line: format!("{total_sessions} sessions across {total_projects} projects"),
    })
}

// ============================================================================
// Router
// ============================================================================

/// Build the reports router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/reports", get(list_reports))
        .route("/reports/preview", get(get_preview))
        .route("/reports/generate", post(generate_report))
        .route("/reports/{id}", get(get_report).delete(delete_report))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use claude_view_db::Database;
    use tower::ServiceExt;

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new().nest("/api", router()).with_state(state)
    }

    /// Parse an ISO date string (YYYY-MM-DD) to a unix timestamp.
    /// If `end_of_day` is true, returns 23:59:59 of that day.
    fn parse_date_to_ts(date_str: &str, end_of_day: bool) -> Result<i64, String> {
        let parts: Vec<&str> = date_str.split('-').collect();
        if parts.len() != 3 {
            return Err(format!("expected YYYY-MM-DD, got {date_str}"));
        }
        let y: i32 = parts[0].parse().map_err(|_| "invalid year")?;
        let m: u32 = parts[1].parse().map_err(|_| "invalid month")?;
        let d: u32 = parts[2].parse().map_err(|_| "invalid day")?;

        let days = days_from_civil(y, m, d);
        let base = days as i64 * 86400;

        if end_of_day {
            Ok(base + 86399)
        } else {
            Ok(base)
        }
    }

    /// Civil date to days since Unix epoch (1970-01-01).
    fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = (y - era * 400) as u32;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe as i32 - 719468
    }

    #[tokio::test]
    async fn test_router_creation() {
        let _router = router();
    }

    #[tokio::test]
    async fn test_list_reports_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/reports")
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

    #[tokio::test]
    async fn test_list_reports_with_data() {
        let db = Database::new_in_memory().await.unwrap();
        db.insert_report("daily", "2026-02-21", "2026-02-21", "- Did stuff", None, 5, 2, 3600, 100, None)
            .await
            .unwrap();

        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/reports")
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
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["reportType"], "daily");
        assert_eq!(json[0]["contentMd"], "- Did stuff");
    }

    #[tokio::test]
    async fn test_get_report_by_id() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db
            .insert_report("weekly", "2026-02-17", "2026-02-21", "week summary", None, 32, 5, 64800, 2450, None)
            .await
            .unwrap();

        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/api/reports/{id}"))
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
        assert_eq!(json["reportType"], "weekly");
        assert_eq!(json["sessionCount"], 32);
    }

    #[tokio::test]
    async fn test_get_report_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/reports/99999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_delete_report() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db
            .insert_report("daily", "2026-02-21", "2026-02-21", "test", None, 1, 1, 100, 10, None)
            .await
            .unwrap();

        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/api/reports/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_report_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/reports/99999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_preview_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/reports/preview?startTs=0&endTs=9999999999")
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
        assert_eq!(json["sessionCount"], 0);
    }

    #[test]
    fn test_parse_date_to_ts() {
        // 2026-02-21 is a known date
        let ts = parse_date_to_ts("2026-02-21", false).unwrap();
        assert!(ts > 0);

        let ts_end = parse_date_to_ts("2026-02-21", true).unwrap();
        assert_eq!(ts_end - ts, 86399);
    }

    #[test]
    fn test_parse_date_to_ts_invalid() {
        assert!(parse_date_to_ts("not-a-date", false).is_err());
        assert!(parse_date_to_ts("2026-13-01", false).is_ok()); // month validation is lax (ok for our purposes)
    }

    #[test]
    fn test_days_from_civil() {
        // 1970-01-01 should be day 0
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        // 1970-01-02 should be day 1
        assert_eq!(days_from_civil(1970, 1, 2), 1);
    }
}
