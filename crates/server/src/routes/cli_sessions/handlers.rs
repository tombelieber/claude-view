//! Route handlers for CLI session CRUD operations.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use super::types::{CreateRequest, CreateResponse};
use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

/// Read the Claude session ID from ~/.claude/sessions/{pid}.json.
/// Retained for startup reconciliation (reconcile.rs).
pub(super) fn resolve_claude_session_id(pid: u32) -> Option<String> {
    let home = dirs::home_dir()?;
    let path = home.join(format!(".claude/sessions/{pid}.json"));
    let data = std::fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
    parsed.get("sessionId")?.as_str().map(String::from)
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/cli-sessions -- Create a new tmux-backed CLI session.
///
/// Blocks until the Claude process inside tmux writes its pid.json, then
/// returns the real Claude session UUID. The Born handler (sessions_lifecycle)
/// creates the LiveSession and sets tmux ownership naturally — no intermediate
/// "Spawning" entry needed.
#[utoipa::path(post, path = "/api/cli-sessions", tag = "cli",
    request_body = CreateRequest,
    responses(
        (status = 200, description = "CLI session created", body = CreateResponse),
        (status = 400, description = "Invalid request (e.g. bad project_dir)"),
        (status = 409, description = "Maximum concurrent sessions reached"),
        (status = 503, description = "tmux unavailable or Claude failed to start"),
    )
)]
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> ApiResult<Json<CreateResponse>> {
    // Limit concurrent sessions to prevent resource exhaustion.
    const MAX_CLI_SESSIONS: usize = 10;
    if state.tmux_index.len().await >= MAX_CLI_SESSIONS {
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

    // Generate a short unique tmux session name.
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let tmux_name = format!("cv-{short_id}");

    // Register in tmux index BEFORE creating the session so the Born handler
    // can match it immediately when Claude writes pid.json.
    state.tmux_index.insert(tmux_name.clone()).await;

    // Create the tmux session.
    if let Err(e) = state
        .tmux
        .new_session(&tmux_name, req.project_dir.as_deref(), &req.args)
    {
        // Rollback tmux index on failure.
        state.tmux_index.remove(&tmux_name).await;
        return Err(ApiError::Internal(format!(
            "Failed to create tmux session: {e}"
        )));
    }

    // Get the pane PID — after exec, this is the Claude process PID.
    let pane_pid = match state.tmux.pane_pid(&tmux_name) {
        Some(pid) => pid,
        None => {
            // Cleanup: kill tmux session, remove from index.
            let _ = state.tmux.kill_session(&tmux_name);
            state.tmux_index.remove(&tmux_name).await;
            return Err(ApiError::Internal(
                "Failed to read tmux pane PID".to_string(),
            ));
        }
    };

    // Poll for ~/.claude/sessions/{pane_pid}.json until Claude writes it.
    let session_id = match poll_for_session_id(pane_pid).await {
        Some(id) => id,
        None => {
            // Cleanup: kill tmux session, remove from index.
            let _ = state.tmux.kill_session(&tmux_name);
            state.tmux_index.remove(&tmux_name).await;
            return Err(ApiError::ServiceUnavailable(
                "Claude CLI failed to start within timeout".to_string(),
            ));
        }
    };

    tracing::info!(
        tmux = %tmux_name,
        session_id = %session_id,
        pane_pid = pane_pid,
        "CLI session created — Claude session resolved"
    );

    Ok(Json(CreateResponse {
        session_id,
        tmux_session_name: tmux_name,
    }))
}

/// Poll ~/.claude/sessions/{pid}.json until it appears and contains a sessionId.
/// Returns None after timeout (~15s).
async fn poll_for_session_id(pid: u32) -> Option<String> {
    let home = dirs::home_dir()?;
    let path = home.join(format!(".claude/sessions/{pid}.json"));

    // Poll every 200ms for up to 15 seconds (75 attempts).
    for _ in 0..75 {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(id) = parsed.get("sessionId").and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        return Some(id.to_string());
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    tracing::warn!(pid, "Timed out waiting for Claude session pid.json");
    None
}

/// DELETE /api/cli-sessions/{id} -- Kill a CLI session.
///
/// `id` is the tmux session name (e.g. "cv-abc123"). Finds the corresponding
/// LiveSession by tmux ownership and reaps it.
#[utoipa::path(delete, path = "/api/cli-sessions/{id}", tag = "cli",
    params(("id" = String, Path, description = "Tmux session name")),
    responses(
        (status = 200, description = "Session killed and removed"),
        (status = 404, description = "CLI session not found"),
    )
)]
pub async fn kill_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // Check tmux index.
    if !state.tmux_index.contains(&id).await {
        return Err(ApiError::NotFound(format!("CLI session not found: {id}")));
    }

    // Kill the tmux session (ignore errors if already dead).
    if state.tmux.has_session(&id) {
        let _ = state.tmux.kill_session(&id);
    }

    // Remove from tmux index.
    state.tmux_index.remove(&id).await;

    // Find the LiveSession by tmux ownership and remove if still in early state.
    // For Born+ sessions, death detection (kqueue/reconciler) handles cleanup.
    {
        let session_key = {
            let map = state.live_sessions.read().await;
            map.iter().find_map(|(key, s)| {
                if s.ownership
                    .as_ref()
                    .and_then(|o| o.tmux.as_ref())
                    .is_some_and(|t| t.cli_session_id == id)
                {
                    Some(key.clone())
                } else {
                    None
                }
            })
        };

        if let Some(key) = session_key {
            let map = state.live_sessions.read().await;
            if let Some(session) = map.get(&key) {
                if session.status == crate::live::state::SessionStatus::Spawning {
                    drop(map);
                    let mut map = state.live_sessions.write().await;
                    let removed = map.remove(&key);
                    drop(map);
                    if let Some(session) = removed {
                        let _ =
                            state
                                .live_tx
                                .send(crate::live::state::SessionEvent::SessionRemove {
                                    session_id: key,
                                    session,
                                });
                    }
                }
            }
        }
    }

    tracing::info!(id = %id, "CLI session killed");

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
