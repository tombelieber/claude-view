//! Route handlers for CLI session CRUD operations.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{CliSession, CliSessionStatus, CreateRequest, CreateResponse};
use crate::{
    error::{ApiError, ApiResult},
    live::state::{
        AgentState, AgentStateGroup, HookFields, JsonlFields, LiveSession, SessionEvent,
        SessionStatus, StatuslineFields,
    },
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

    // Generate a short unique ID.
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let session_id = format!("cv-{short_id}");

    // Create the tmux session.
    state
        .tmux
        .new_session(&session_id, req.project_dir.as_deref(), &req.args)
        .map_err(|e| ApiError::Internal(format!("Failed to create tmux session: {e}")))?;

    // Register in tmux index.
    state.tmux_index.insert(session_id.clone()).await;

    // Insert minimal LiveSession into the unified store.
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let live_session = LiveSession {
        id: session_id.clone(),
        status: SessionStatus::Spawning,
        started_at: Some(now_ms),
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "spawning".into(),
                label: "Starting...".into(),
                context: None,
            },
            pid: None,
            title: String::new(),
            last_user_message: String::new(),
            current_activity: "Starting...".into(),
            turn_count: 0,
            last_activity_at: now_ms,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: JsonlFields {
            project: String::new(),
            project_display_name: req
                .project_dir
                .as_deref()
                .and_then(|d| {
                    std::path::Path::new(d)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(String::from)
                })
                .unwrap_or_default(),
            project_path: req.project_dir.clone().unwrap_or_default(),
            file_path: String::new(),
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            tokens: Default::default(),
            cost: Default::default(),
            cache_status: claude_view_core::pricing::CacheStatus::Unknown,
            last_turn_task_seconds: None,
            last_cache_hit_at: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            slug: None,
            user_files: None,
            source: None,
            phase: Default::default(),
            ai_title: None,
        },
        session_kind: None,
        entrypoint: None,
        ownership: Some(claude_view_types::SessionOwnership {
            tmux: Some(claude_view_types::TmuxBinding {
                cli_session_id: session_id.clone(),
            }),
            ..Default::default()
        }),
        pending_interaction: None,
    };

    // Insert into LiveSessionMap and broadcast.
    {
        let mut map = state.live_sessions.write().await;
        map.insert(session_id.clone(), live_session.clone());
    }
    let _ = state.live_tx.send(SessionEvent::SessionUpsert {
        session: live_session,
    });

    tracing::info!(id = %session_id, "CLI session created (Spawning)");

    // Return CliSession shape for backward compat (frontend still expects this during migration).
    let session = CliSession {
        id: session_id,
        created_at: now_ms as u64,
        status: CliSessionStatus::Running,
        project_dir: req.project_dir,
        args: req.args,
        claude_session_id: None,
    };

    Ok(Json(CreateResponse { session }))
}

/// DELETE /api/cli-sessions/{id} -- Kill a CLI session.
#[utoipa::path(delete, path = "/api/cli-sessions/{id}", tag = "cli",
    params(("id" = String, Path, description = "CLI session ID")),
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

    // If session is still Spawning (no PID yet), remove from LiveSessionMap directly.
    // For Born+ sessions, death detection (kqueue/reconciler) handles cleanup naturally.
    {
        let mut map = state.live_sessions.write().await;
        if let Some(session) = map.get(&id) {
            if session.status == SessionStatus::Spawning {
                let removed = map.remove(&id);
                drop(map); // Release write lock before sending event.
                if let Some(session) = removed {
                    let _ = state.live_tx.send(SessionEvent::SessionRemove {
                        session_id: id.clone(),
                        session,
                    });
                }
            }
        }
    }

    tracing::info!(id = %id, "CLI session killed");

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
