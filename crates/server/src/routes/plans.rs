//! Plan document endpoints for session implementation plans.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use claude_view_core::plan_files::{self, PlanDocument};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/sessions/:id/plans -- returns plan documents for the session's slug.
pub async fn get_session_plans(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<Vec<PlanDocument>>> {
    // Look up the session to get its slug
    let session = state
        .db
        .get_session_by_id(&session_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id} not found")))?;

    let slug = session
        .slug
        .ok_or_else(|| ApiError::NotFound("Session has no associated plan slug".to_string()))?;

    let plans_dir = plan_files::claude_plans_dir()
        .ok_or_else(|| ApiError::NotFound("Cannot resolve home directory for plans".to_string()))?;

    // Blocking filesystem I/O -- matches file_history.rs and grep.rs patterns
    let plans = tokio::task::spawn_blocking(move || plan_files::find_plan_files(&plans_dir, &slug))
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(plans))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sessions/{id}/plans", get(get_session_plans))
}
