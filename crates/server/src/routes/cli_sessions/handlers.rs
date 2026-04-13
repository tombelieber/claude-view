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

    use super::lifecycle_debug::log_lifecycle;
    log_lifecycle(
        "API_CREATE_START",
        None,
        None,
        Some(&tmux_name),
        &format!("POST /api/cli-sessions — project_dir={:?}", req.project_dir),
    );

    // Register in tmux index BEFORE creating the session so the Born handler
    // can match it immediately when Claude writes pid.json.
    state.tmux_index.insert(tmux_name.clone()).await;

    log_lifecycle(
        "TMUX_INDEX_REGISTERED",
        None,
        None,
        Some(&tmux_name),
        "tmux index entry created before tmux spawn",
    );

    // Create the tmux session.
    if let Err(e) = state
        .tmux
        .new_session(&tmux_name, req.project_dir.as_deref(), &req.args)
    {
        log_lifecycle(
            "TMUX_SPAWN_FAILED",
            None,
            None,
            Some(&tmux_name),
            &format!("tmux new-session failed: {e}"),
        );
        // Rollback tmux index on failure.
        state.tmux_index.remove(&tmux_name).await;
        return Err(ApiError::Internal(format!(
            "Failed to create tmux session: {e}"
        )));
    }

    log_lifecycle(
        "TMUX_SESSION_SPAWNED",
        None,
        None,
        Some(&tmux_name),
        "tmux new-session -d succeeded",
    );

    // Get the pane PID — after exec, this is the Claude process PID.
    let pane_pid = match state.tmux.pane_pid(&tmux_name) {
        Some(pid) => {
            log_lifecycle(
                "TMUX_PANE_PID_RESOLVED",
                Some(pid),
                None,
                Some(&tmux_name),
                &format!("pane_pid={pid} (Claude CLI process)"),
            );
            pid
        }
        None => {
            log_lifecycle(
                "TMUX_PANE_PID_FAILED",
                None,
                None,
                Some(&tmux_name),
                "could not read pane PID",
            );
            // Cleanup: kill tmux session, remove from index.
            let _ = state.tmux.kill_session(&tmux_name);
            state.tmux_index.remove(&tmux_name).await;
            return Err(ApiError::Internal(
                "Failed to read tmux pane PID".to_string(),
            ));
        }
    };

    // Fast pre-check: if pid.json already exists (rare — Claude boots in ~300ms,
    // we arrive here at ~63ms), skip the waiter/poll entirely.
    let session_id_precheck = resolve_claude_session_id(pane_pid);
    if let Some(ref id) = session_id_precheck {
        log_lifecycle(
            "PID_JSON_PRECHECK_HIT",
            Some(pane_pid),
            Some(id),
            Some(&tmux_name),
            "pid.json already existed before waiter registered",
        );
    }

    // Race: Born waiter (FSEvents, sub-ms) vs fallback poll (50ms intervals).
    // In production, Born wins. In tests (no watcher), poll wins.
    // Pre-check covers the ultra-fast boot race condition.
    //
    // Drop guard ensures the waiter entry is cleaned from the map even if
    // the Axum handler task is cancelled (client disconnect).
    struct WaiterGuard {
        pid: u32,
        waiters: super::BornWaiters,
        disarmed: bool,
    }
    impl Drop for WaiterGuard {
        fn drop(&mut self) {
            if !self.disarmed {
                self.waiters.lock().unwrap().remove(&self.pid);
            }
        }
    }

    let session_id = if let Some(id) = session_id_precheck {
        Some(id)
    } else {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut waiters = state.born_waiters.lock().unwrap();
            waiters.insert(pane_pid, tx);
        }
        let mut guard = WaiterGuard {
            pid: pane_pid,
            waiters: state.born_waiters.clone(),
            disarmed: false,
        };

        log_lifecycle(
            "BORN_WAITER_REGISTERED",
            Some(pane_pid),
            None,
            Some(&tmux_name),
            "racing Born event vs fallback poll",
        );

        let result = tokio::select! {
            result = rx => {
                // Born handler consumed the waiter entry — disarm the guard.
                guard.disarmed = true;
                match result {
                    Ok(id) => {
                        log_lifecycle(
                            "BORN_EVENT_RECEIVED",
                            Some(pane_pid),
                            Some(&id),
                            Some(&tmux_name),
                            "sessionId from Born handler (zero-poll)",
                        );
                        Some(id)
                    }
                    Err(_) => None,
                }
            }
            result = poll_for_session_id(pane_pid) => {
                // Poll won — guard will clean up the waiter on drop.
                if let Some(ref id) = result {
                    log_lifecycle(
                        "POLL_RESOLVED",
                        Some(pane_pid),
                        Some(id),
                        Some(&tmux_name),
                        "sessionId from fallback poll",
                    );
                }
                result
            }
        };
        drop(guard);
        result
    };

    let session_id = match session_id {
        Some(id) => id,
        None => {
            log_lifecycle(
                "SESSION_START_TIMEOUT",
                Some(pane_pid),
                None,
                Some(&tmux_name),
                "neither Born event nor poll found pid.json within timeout",
            );
            let _ = state.tmux.kill_session(&tmux_name);
            state.tmux_index.remove(&tmux_name).await;
            return Err(ApiError::ServiceUnavailable(
                "Claude CLI failed to start within timeout".to_string(),
            ));
        }
    };

    log_lifecycle(
        "API_CREATE_COMPLETE",
        Some(pane_pid),
        Some(&session_id),
        Some(&tmux_name),
        "full identity resolved, returning to frontend",
    );

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

/// Fallback: poll ~/.claude/sessions/{pid}.json until it appears.
/// Used when Born waiter doesn't fire (tests, watcher down).
/// Short poll at 50ms intervals, 5s timeout (100 attempts).
async fn poll_for_session_id(pid: u32) -> Option<String> {
    let home = dirs::home_dir()?;
    let path = home.join(format!(".claude/sessions/{pid}.json"));

    for _ in 0..100 {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(id) = parsed.get("sessionId").and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        return Some(id.to_string());
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    tracing::warn!(pid, "Fallback poll: timed out waiting for pid.json");
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
    use super::lifecycle_debug::log_lifecycle;

    // Resolve full identity BEFORE killing anything — capture pid + sessionId.
    let (resolved_pid, resolved_session_id) = {
        let map = state.live_sessions.read().await;
        let found = map.iter().find_map(|(key, s)| {
            if s.ownership
                .as_ref()
                .and_then(|o| o.tmux.as_ref())
                .is_some_and(|t| t.cli_session_id == id)
            {
                Some((s.hook.pid, key.clone()))
            } else {
                None
            }
        });
        match found {
            Some((pid, session_id)) => (pid, Some(session_id)),
            None => (None, None),
        }
    };

    log_lifecycle(
        "API_KILL_START",
        resolved_pid,
        resolved_session_id.as_deref(),
        Some(&id),
        &format!("DELETE /api/cli-sessions/{id} — tab close initiated"),
    );

    // Check tmux index.
    if !state.tmux_index.contains(&id).await {
        log_lifecycle(
            "API_KILL_NOT_FOUND",
            resolved_pid,
            resolved_session_id.as_deref(),
            Some(&id),
            "tmux name not in index",
        );
        return Err(ApiError::NotFound(format!("CLI session not found: {id}")));
    }

    // Kill the tmux session (ignore errors if already dead).
    let tmux_existed = state.tmux.has_session(&id);
    if tmux_existed {
        let kill_result = state.tmux.kill_session(&id);
        log_lifecycle(
            "TMUX_KILL_EXECUTED",
            resolved_pid,
            resolved_session_id.as_deref(),
            Some(&id),
            &format!(
                "tmux kill-session -t {} — result={:?}",
                id,
                kill_result.as_ref().map(|_| "ok").unwrap_or("err")
            ),
        );
    } else {
        log_lifecycle(
            "TMUX_ALREADY_DEAD",
            resolved_pid,
            resolved_session_id.as_deref(),
            Some(&id),
            "tmux session already gone before kill",
        );
    }

    // Check state + SIGTERM the Claude process directly.
    if let Some(pid) = resolved_pid {
        let pid_json_exists = dirs::home_dir()
            .map(|h| h.join(format!(".claude/sessions/{pid}.json")).exists())
            .unwrap_or(false);
        let process_alive = unsafe { libc::kill(pid as i32, 0) } == 0;
        log_lifecycle(
            "KILL_STATE_SNAPSHOT",
            Some(pid),
            resolved_session_id.as_deref(),
            Some(&id),
            &format!(
                "post-tmux-kill snapshot: process_alive={process_alive}, pid_json_exists={pid_json_exists}"
            ),
        );

        // SIGTERM the Claude CLI process directly — tmux kill only sends SIGHUP
        // which Claude CLI ignores (keeps running its 30s timer).
        if process_alive {
            let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            log_lifecycle(
                "SIGTERM_SENT",
                Some(pid),
                resolved_session_id.as_deref(),
                Some(&id),
                &format!(
                    "kill({pid}, SIGTERM) → {}",
                    if result == 0 { "delivered" } else { "failed" }
                ),
            );
        }
    }

    // Remove from tmux index.
    state.tmux_index.remove(&id).await;

    log_lifecycle(
        "TMUX_INDEX_REMOVED",
        resolved_pid,
        resolved_session_id.as_deref(),
        Some(&id),
        "tmux index entry removed",
    );

    // Reap through the canonical path — reap_session handles ALL secondary maps
    // (transcript_to_session, hook_event_channels, accumulator, closed_ring, SSE).
    // Wait briefly for SIGTERM to take effect (reaper refuses to reap alive PIDs).
    if let Some(ref key) = resolved_session_id {
        if let Some(ref manager) = state.live_manager {
            // SIGTERM was sent above; Claude CLI exits in ~200ms.
            // Poll up to 500ms for process death so reaper accepts the reap.
            if let Some(pid) = resolved_pid {
                for _ in 0..10 {
                    if !crate::live::process::is_pid_alive(pid) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
            let result = manager.reap_session(key).await;
            log_lifecycle(
                "REAP_SESSION_RESULT",
                resolved_pid,
                Some(key),
                Some(&id),
                &format!("reap_session() → {:?}", result),
            );
        }
    }

    log_lifecycle(
        "API_KILL_COMPLETE",
        resolved_pid,
        resolved_session_id.as_deref(),
        Some(&id),
        "kill handler finished, returning 200",
    );

    tracing::info!(id = %id, "CLI session killed");

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
