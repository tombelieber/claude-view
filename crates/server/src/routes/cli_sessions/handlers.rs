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

    tracing::info!(
        tmux_session = %tmux_name,
        project_dir = ?req.project_dir,
        "cli.create.start"
    );

    // Register in tmux index BEFORE creating the session so the Born handler
    // can match it immediately when Claude writes pid.json.
    state.tmux_index.insert(tmux_name.clone()).await;

    tracing::debug!(
        tmux_session = %tmux_name,
        "cli.tmux_index.registered"
    );

    // Create the tmux session.
    if let Err(e) = state
        .tmux
        .new_session(&tmux_name, req.project_dir.as_deref(), &req.args)
    {
        tracing::error!(
            tmux_session = %tmux_name,
            error = %e,
            "cli.tmux.spawn.failed"
        );
        // Rollback tmux index on failure.
        state.tmux_index.remove(&tmux_name).await;
        return Err(ApiError::Internal(format!(
            "Failed to create tmux session: {e}"
        )));
    }

    tracing::info!(
        tmux_session = %tmux_name,
        "cli.tmux.spawned"
    );

    // Auto-accept workspace trust dialog. Claude CLI shows an interactive
    // "trust this folder?" prompt for untrusted directories that blocks
    // pid.json creation. The dialog needs ~1s to render before it can
    // accept input. We fire Enter at 1s and 2s to cover the window.
    // For already-trusted directories the poll resolves in <300ms and
    // the spawned task's Enter hits an empty prompt line (harmless no-op).
    {
        let tmux = state.tmux.clone();
        let name = tmux_name.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let _ = tmux.send_keys(&name, "Enter");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let _ = tmux.send_keys(&name, "Enter");
        });
    }

    // Get the pane PID — after exec, this is the Claude process PID.
    let pane_pid = match state.tmux.pane_pid(&tmux_name) {
        Some(pid) => {
            tracing::debug!(
                tmux_session = %tmux_name,
                pane_pid = pid,
                "cli.tmux.pid_resolved"
            );
            pid
        }
        None => {
            tracing::error!(
                tmux_session = %tmux_name,
                "cli.tmux.pid.failed"
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
        tracing::debug!(
            tmux_session = %tmux_name,
            pane_pid = pane_pid,
            session_id = %id,
            "cli.born.precheck_hit"
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

        tracing::debug!(
            tmux_session = %tmux_name,
            pane_pid = pane_pid,
            "cli.born.waiting"
        );

        let result = tokio::select! {
            result = rx => {
                // Born handler consumed the waiter entry — disarm the guard.
                guard.disarmed = true;
                match result {
                    Ok(id) => {
                        tracing::info!(
                            tmux_session = %tmux_name,
                            pane_pid = pane_pid,
                            session_id = %id,
                            "cli.born.pid_resolved"
                        );
                        Some(id)
                    }
                    Err(_) => None,
                }
            }
            result = poll_for_session_id(pane_pid) => {
                // Poll won — guard will clean up the waiter on drop.
                if let Some(ref id) = result {
                    tracing::debug!(
                        tmux_session = %tmux_name,
                        pane_pid = pane_pid,
                        session_id = %id,
                        "cli.poll.fallback"
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
            tracing::error!(
                tmux_session = %tmux_name,
                pane_pid = pane_pid,
                "cli.session.start.timeout"
            );
            let _ = state.tmux.kill_session(&tmux_name);
            state.tmux_index.remove(&tmux_name).await;
            return Err(ApiError::ServiceUnavailable(
                "Claude CLI failed to start within timeout".to_string(),
            ));
        }
    };

    tracing::info!(
        tmux_session = %tmux_name,
        session_id = %session_id,
        pane_pid = pane_pid,
        "cli.create.complete"
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

    tracing::info!(
        tmux_session = %id,
        pane_pid = ?resolved_pid,
        session_id = ?resolved_session_id,
        "cli.kill.start"
    );

    // Check tmux index.
    if !state.tmux_index.contains(&id).await {
        tracing::debug!(
            tmux_session = %id,
            "cli.kill.not_found"
        );
        return Err(ApiError::NotFound(format!("CLI session not found: {id}")));
    }

    // Kill the tmux session (ignore errors if already dead).
    let tmux_existed = state.tmux.has_session(&id);
    if tmux_existed {
        let kill_result = state.tmux.kill_session(&id);
        tracing::debug!(
            tmux_session = %id,
            pane_pid = ?resolved_pid,
            success = kill_result.is_ok(),
            "cli.kill.tmux_executed"
        );
    } else {
        tracing::debug!(
            tmux_session = %id,
            pane_pid = ?resolved_pid,
            "cli.kill.tmux_already_dead"
        );
    }

    // Check state + SIGTERM the Claude process directly.
    if let Some(pid) = resolved_pid {
        let pid_json_exists = dirs::home_dir()
            .map(|h| h.join(format!(".claude/sessions/{pid}.json")).exists())
            .unwrap_or(false);
        let process_alive = unsafe { libc::kill(pid as i32, 0) } == 0;
        tracing::debug!(
            tmux_session = %id,
            pane_pid = pid,
            process_alive,
            pid_json_exists,
            "cli.kill.state_snapshot"
        );

        // SIGTERM the Claude CLI process directly — tmux kill only sends SIGHUP
        // which Claude CLI ignores (keeps running its 30s timer).
        if process_alive {
            let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            let delivered = result == 0;
            tracing::debug!(
                tmux_session = %id,
                pane_pid = pid,
                delivered,
                "cli.kill.sigterm_sent"
            );
        }
    }

    // Remove from tmux index.
    state.tmux_index.remove(&id).await;

    tracing::debug!(
        tmux_session = %id,
        "cli.kill.index_removed"
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
            tracing::debug!(
                tmux_session = %id,
                session_id = %key,
                reap_result = ?result,
                "cli.kill.reaped"
            );
        }
    }

    tracing::info!(
        tmux_session = %id,
        pane_pid = ?resolved_pid,
        session_id = ?resolved_session_id,
        "cli.kill.complete"
    );

    Ok(Json(serde_json::json!({ "removed": true, "id": id })))
}
