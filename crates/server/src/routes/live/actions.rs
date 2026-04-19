//! Session action endpoints: kill, dismiss, control binding.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json, Response},
};

use crate::live::state::{ControlBinding, SessionEvent};
use crate::state::AppState;

use super::types::{BindControlRequest, UnbindControlRequest};

/// POST /api/live/sessions/:id/kill -- Send SIGTERM to the session's Claude process.
///
/// Returns `{ "killed": true, "pid": <pid> }` on success.
/// Returns 500 with `{ "error": "...", "pid": <pid> }` if SIGTERM fails.
/// Returns 404 with `{ "canDismiss": true }` if the session has no PID (already exited).
/// Returns 404 with `{ "error": "Session not found" }` if the session ID is unknown.
#[utoipa::path(post, path = "/api/live/sessions/{id}/kill", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "SIGTERM sent to session process", body = serde_json::Value),
        (status = 404, description = "Session not found or already exited"),
        (status = 500, description = "Failed to send SIGTERM"),
    )
)]
pub async fn kill_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Response {
    // Extract session info then drop the lock before making syscalls
    let session_info = {
        let map = state.live_sessions.read().await;
        map.get(&session_id).map(|s| s.hook.pid)
    };

    match session_info {
        Some(Some(pid)) => {
            let pid_i32 = pid as i32; // safe: macOS PIDs max ~99999, Linux ~4M
            let result = unsafe { libc::kill(pid_i32, libc::SIGTERM) };
            if result != 0 {
                let errno = std::io::Error::last_os_error();
                tracing::warn!(session_id = %session_id, pid, %errno, "Failed to send SIGTERM");
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": format!("SIGTERM failed: {}", errno), "pid": pid })),
                )
                    .into_response();
            }
            Json(serde_json::json!({ "killed": true, "pid": pid })).into_response()
        }
        Some(None) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "canDismiss": true })),
        )
            .into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        )
            .into_response(),
    }
}

/// POST /api/live/sessions/:id/bind-control -- Sidecar notifies that it now controls this session.
///
/// Sets the `control` field on the LiveSession, which flows to SSE clients.
/// Idempotent: re-binding with the same controlId is a no-op success.
#[utoipa::path(post, path = "/api/live/sessions/{id}/bind-control", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Control binding registered", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn bind_control(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(body): Json<BindControlRequest>,
) -> Response {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&session_id) {
        // Already bound with same controlId -> idempotent success
        if session
            .control
            .as_ref()
            .is_some_and(|c| c.control_id == body.control_id)
        {
            return Json(serde_json::json!({ "bound": true, "status": "already_bound" }))
                .into_response();
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs() as i64;
        session.control = Some(ControlBinding {
            control_id: body.control_id,
            bound_at: now,
            bound_at_generation: state.sidecar.generation(),
            cancel: tokio_util::sync::CancellationToken::new(),
        });
        // Control binding = sidecar Agent SDK -- set source immediately
        session.jsonl.source = Some(crate::live::process::SessionSourceInfo {
            category: crate::live::process::SessionSource::AgentSdk,
            label: None,
        });
        // Compute and store ownership in the session record
        session.ownership = Some(crate::live::ownership::compute_ownership(session));
        // Notify SSE clients of the control binding change
        let _ = state.live_tx.send(SessionEvent::SessionUpsert {
            session: session.clone(),
        });
        Json(serde_json::json!({ "bound": true })).into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        )
            .into_response()
    }
}

/// POST /api/live/sessions/:id/unbind-control -- Sidecar notifies it released control.
///
/// Only unbinds if the current controlId matches (CAS semantics).
#[utoipa::path(post, path = "/api/live/sessions/{id}/unbind-control", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Control binding released", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn unbind_control(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(body): Json<UnbindControlRequest>,
) -> Response {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&session_id) {
        if session
            .control
            .as_ref()
            .is_some_and(|c| c.control_id == body.control_id)
        {
            if let Some(binding) = session.control.take() {
                binding.cancel.cancel();
            }
            // Recompute ownership after clearing control binding
            session.ownership = Some(crate::live::ownership::compute_ownership(session));
            let _ = state.live_tx.send(SessionEvent::SessionUpsert {
                session: session.clone(),
            });
            Json(serde_json::json!({ "unbound": true })).into_response()
        } else {
            Json(serde_json::json!({ "unbound": false, "reason": "control_id_mismatch" }))
                .into_response()
        }
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        )
            .into_response()
    }
}

/// DELETE /api/live/sessions/:id/dismiss -- Dismiss from recently closed (in-memory only).
#[utoipa::path(delete, path = "/api/live/sessions/{id}/dismiss", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session dismissed", body = serde_json::Value),
        (status = 404, description = "Session not found in recently closed"),
    )
)]
pub async fn dismiss_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let dismissed = {
        let mut ring = state.closed_ring.write().await;
        let before = ring.len();
        ring.retain(|s| s.id != id);
        ring.len() < before
    };

    if dismissed {
        // CQRS Phase 5 PR 5.2: log the dismissal to the action log so the
        // PR 5.3 fold writer can populate `session_flags.dismissed_at`.
        // Best-effort — a persistence failure must not flip the user's
        // visible state, because the ring update already succeeded and
        // any retry would double-dismiss. A WARN trace is sufficient;
        // Phase 5.4's shadow-parity monitor will flag repeated gaps.
        let at_ms = chrono::Utc::now().timestamp_millis();
        if let Err(e) = state
            .db
            .insert_action_log(&id, "dismiss", "{}", "user", at_ms)
            .await
        {
            tracing::warn!(session_id = %id, error = %e, "failed to log dismiss to session_action_log");
        }

        (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({"dismissed": true})),
        )
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"dismissed": false})),
        )
    }
}

/// DELETE /api/live/recently-closed -- Dismiss all recently closed (in-memory only).
#[utoipa::path(delete, path = "/api/live/recently-closed", tag = "live",
    responses(
        (status = 200, description = "All recently closed sessions dismissed", body = serde_json::Value),
    )
)]
pub async fn dismiss_all_closed(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Capture the IDs BEFORE clearing so Phase 5's action-log inserts
    // can reference each dismissed session by ID; after `ring.clear()`
    // that information is gone.
    let ids: Vec<String> = {
        let mut ring = state.closed_ring.write().await;
        let ids = ring.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        ring.clear();
        ids
    };

    let count = ids.len();
    if count > 0 {
        let at_ms = chrono::Utc::now().timestamp_millis();
        for id in &ids {
            if let Err(e) = state
                .db
                .insert_action_log(id, "dismiss", "{}", "user", at_ms)
                .await
            {
                tracing::warn!(session_id = %id, error = %e, "failed to log bulk dismiss to session_action_log");
            }
        }
    }

    Json(serde_json::json!({"dismissedCount": count}))
}
