//! Route handlers for CLI session CRUD operations.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    error::{ApiError, ApiResult},
    live::state::{CliSessionInfo, SessionEvent},
    state::AppState,
};

use super::types::{CliSession, CliSessionStatus, CreateRequest, CreateResponse, ListResponse};

fn to_cli_info(s: &CliSession) -> CliSessionInfo {
    CliSessionInfo {
        id: s.id.clone(),
        created_at: s.created_at,
        status: match s.status {
            CliSessionStatus::Running => "running".to_string(),
            CliSessionStatus::Exited => "exited".to_string(),
        },
        project_dir: s.project_dir.clone(),
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/cli-sessions -- Create a new tmux-backed CLI session.
#[utoipa::path(post, path = "/api/cli-sessions", tag = "cli",
    request_body = CreateRequest,
    responses(
        (status = 200, description = "CLI session created", body = CreateResponse),
        (status = 400, description = "Invalid request (e.g. bad project_dir)"),
        (status = 409, description = "Maximum concurrent sessions reached"),
        (status = 503, description = "tmux unavailable"),
    )
)]
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> ApiResult<Json<CreateResponse>> {
    // Limit concurrent sessions to prevent resource exhaustion.
    const MAX_CLI_SESSIONS: usize = 10;
    if state.cli_sessions.list().await.len() >= MAX_CLI_SESSIONS {
        return Err(ApiError::Conflict(format!(
            "Maximum {MAX_CLI_SESSIONS} concurrent CLI sessions reached"
        )));
    }

    // Check tmux availability.
    if !state.tmux.is_available() {
        return Err(ApiError::ServiceUnavailable(
            "tmux is not installed or not available".to_string(),
        ));
    }

    // Validate project_dir if provided — must be an absolute path to an existing directory.
    if let Some(ref dir) = req.project_dir {
        let path = std::path::Path::new(dir);
        if !path.is_absolute() {
            return Err(ApiError::BadRequest(
                "project_dir must be an absolute path".to_string(),
            ));
        }
        if !path.is_dir() {
            return Err(ApiError::BadRequest(format!(
                "project_dir does not exist or is not a directory: {dir}"
            )));
        }
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

    // Broadcast SSE event so connected frontends update immediately.
    let _ = state.live_tx.send(SessionEvent::CliSessionCreated {
        cli_session: to_cli_info(&session),
    });

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
            // Broadcast status change so frontends update immediately.
            let _ = state.live_tx.send(SessionEvent::CliSessionUpdated {
                cli_session: to_cli_info(&session),
            });
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

    // Broadcast removal so frontends update immediately.
    let _ = state.live_tx.send(SessionEvent::CliSessionRemoved {
        cli_session_id: id.clone(),
    });

    tracing::info!(id = %id, "CLI session killed");

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
