//! Route handlers for CLI session CRUD operations.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

use super::types::{CliSession, CliSessionStatus, CreateRequest, CreateResponse, ListResponse};

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/cli-sessions -- Create a new tmux-backed CLI session.
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> ApiResult<Json<CreateResponse>> {
    // Check tmux availability.
    if !state.tmux.is_available() {
        return Err(ApiError::ServiceUnavailable(
            "tmux is not installed or not available".to_string(),
        ));
    }

    // Generate a short unique ID.
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let session_id = format!("cv-{short_id}");

    // Create the tmux session.
    state
        .tmux
        .new_session(&session_id, req.project_dir.as_deref(), &req.args)
        .map_err(|e| ApiError::Internal(format!("Failed to create tmux session: {e}")))?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let session = CliSession {
        id: session_id,
        created_at: now_ms,
        status: CliSessionStatus::Running,
        project_dir: req.project_dir,
        args: req.args,
    };

    // Store the session.
    state.cli_sessions.insert(session.clone()).await;

    tracing::info!(id = %session.id, "CLI session created");

    Ok(Json(CreateResponse { session }))
}

/// GET /api/cli-sessions -- List all CLI sessions, with health check.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> ApiResult<Json<ListResponse>> {
    let sessions = state.cli_sessions.list().await;

    // Run a quick health check: mark sessions that no longer exist in tmux.
    let mut result = Vec::with_capacity(sessions.len());
    for mut session in sessions {
        if session.status != CliSessionStatus::Exited && !state.tmux.has_session(&session.id) {
            session.status = CliSessionStatus::Exited;
            state
                .cli_sessions
                .update_status(&session.id, CliSessionStatus::Exited)
                .await;
        }
        result.push(session);
    }

    Ok(Json(ListResponse { sessions: result }))
}

/// DELETE /api/cli-sessions/{id} -- Kill a CLI session.
pub async fn kill_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // Check if the session exists in our store.
    let session = state
        .cli_sessions
        .get(&id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("CLI session not found: {id}")))?;

    // Kill the tmux session (ignore errors if already dead).
    if state.tmux.has_session(&session.id) {
        let _ = state.tmux.kill_session(&session.id);
    }

    // Remove from store.
    state.cli_sessions.remove(&id).await;

    tracing::info!(id = %id, "CLI session killed");

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
