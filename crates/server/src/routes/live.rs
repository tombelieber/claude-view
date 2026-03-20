//! Live session monitoring endpoints (SSE + REST).
//!
//! - `GET /api/live/stream`              -- SSE stream of real-time session events
//! - `GET /api/live/sessions`            -- List all live sessions
//! - `GET /api/live/sessions/:id`        -- Get a single live session
//! - `GET /api/live/sessions/:id/messages` -- Get recent messages for a live session
//! - `POST /api/live/sessions/:id/kill`   -- Send SIGTERM to a session's process
//! - `DELETE /api/live/sessions/:id/dismiss` -- Dismiss a recently closed session
//! - `DELETE /api/live/recently-closed`   -- Dismiss all recently closed sessions
//! - `GET /api/live/summary`             -- Aggregate live session statistics
//! - `GET /api/live/pricing`             -- Model pricing table

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, Sse},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;

use crate::live::state::{AgentStateGroup, ControlBinding, LiveSession, SessionEvent};
use crate::state::AppState;

/// Build the live monitoring sub-router.
///
/// Routes:
/// - `GET /live/stream`                 - SSE stream of live session events
/// - `GET /live/sessions`               - List all live sessions
/// - `GET /live/sessions/:id`           - Get single live session
/// - `GET /live/sessions/:id/messages`  - Get recent messages for a live session
/// - `POST /live/sessions/:id/kill`     - Send SIGTERM to a session's process
/// - `DELETE /live/sessions/:id/dismiss` - Dismiss a recently closed session
/// - `DELETE /live/recently-closed`      - Dismiss all recently closed sessions
/// - `GET /live/summary`                - Aggregate statistics
/// - `GET /live/pricing`                - Model pricing table
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/live/stream", get(live_stream))
        .route("/live/sessions", get(list_live_sessions))
        .route("/live/sessions/{id}", get(get_live_session))
        .route(
            "/live/sessions/{id}/messages",
            get(get_live_session_messages),
        )
        .route("/live/sessions/{id}/kill", post(kill_session))
        .route(
            "/live/sessions/{id}/statusline",
            get(get_session_statusline_debug),
        )
        .route("/live/sessions/{id}/dismiss", delete(dismiss_session))
        .route("/live/sessions/{id}/bind-control", post(bind_control))
        .route("/live/sessions/{id}/unbind-control", post(unbind_control))
        .route("/live/recently-closed", delete(dismiss_all_closed))
        .route("/live/summary", get(get_live_summary))
        .route("/live/pricing", get(get_pricing))
}

// =============================================================================
// SSE Endpoint
// =============================================================================

/// GET /api/live/stream -- SSE stream of real-time session events.
///
/// # Events
///
/// | Event name          | When emitted                           |
/// |---------------------|----------------------------------------|
/// | `summary`           | On connect, and when a client lags     |
/// | `session_discovered`| New session detected                   |
/// | `session_updated`   | Session state changed                  |
/// | `session_closed`    | Session process exited (recently closed) |
/// | `session_completed` | Session ended                          |
/// | `heartbeat`         | Every 15 seconds to keep connection    |
///
/// On initial connection, the server sends the current summary followed by
/// all active sessions so the client can hydrate immediately without a
/// separate REST call.
pub async fn live_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.live_tx.subscribe();
    let sessions = state.live_sessions.clone();
    let live_manager = state.live_manager.clone();
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        // 1. On connect: send current summary + all active sessions
        {
            let map = sessions.read().await;
            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
            let summary = build_summary(&map, pc);
            match serde_json::to_string(&summary) {
                Ok(data) => yield Ok(Event::default().event("summary").data(data)),
                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
            }
            for session in map.values() {
                let event_name = if session.closed_at.is_some() {
                    "session_closed"
                } else {
                    "session_discovered"
                };
                match serde_json::to_string(session) {
                    Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                    Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                }
            }
        }

        // 2. Stream events from broadcast channel with heartbeat
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(session_event) => {
                            let event_name = match &session_event {
                                SessionEvent::SessionDiscovered { .. } => "session_discovered",
                                SessionEvent::SessionUpdated { .. } => "session_updated",
                                SessionEvent::SessionClosed { .. } => "session_closed",
                                SessionEvent::SessionCompleted { .. } => "session_completed",
                                SessionEvent::Summary { .. } => "summary",
                            };
                            match serde_json::to_string(&session_event) {
                                Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, re-sending all sessions",
                                n
                            );
                            // Re-send full state (same as initial connect) so the
                            // client recovers from any missed discover/complete events.
                            let map = sessions.read().await;
                            let pc = live_manager.as_ref().map(|m| m.process_count()).unwrap_or(0);
                            let summary = build_summary(&map, pc);
                            match serde_json::to_string(&summary) {
                                Ok(data) => yield Ok(Event::default().event("summary").data(data)),
                                Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                            }
                            for session in map.values() {
                                let event_name = if session.closed_at.is_some() {
                                    "session_closed"
                                } else {
                                    "session_discovered"
                                };
                                match serde_json::to_string(session) {
                                    Ok(data) => yield Ok(Event::default().event(event_name).data(data)),
                                    Err(e) => tracing::error!("failed to serialize SSE event: {e}"),
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
            }
        }
    };

    Sse::new(stream)
}

// =============================================================================
// REST Endpoints
// =============================================================================

/// GET /api/live/sessions -- List all live sessions, sorted by most recent activity.
async fn list_live_sessions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let mut all_sessions: Vec<_> = map.values().cloned().collect();
    all_sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    let (active, recently_closed): (Vec<_>, Vec<_>) = all_sessions
        .into_iter()
        .partition(|s| s.closed_at.is_none());
    let process_count = state
        .live_manager
        .as_ref()
        .map(|m| m.process_count())
        .unwrap_or(0);
    Json(serde_json::json!({
        "sessions": active,
        "recentlyClosed": recently_closed,
        "total": active.len(),
        "processCount": process_count,
    }))
}

/// GET /api/live/sessions/:id -- Get a single live session by ID.
async fn get_live_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let map = state.live_sessions.read().await;
    match map.get(&id) {
        Some(session) => Json(serde_json::json!({ "session": session })).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        )
            .into_response(),
    }
}

/// Query parameters for the messages endpoint.
#[derive(Debug, Deserialize)]
struct MessagesQuery {
    /// Maximum number of messages to return (default: 20).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

/// GET /api/live/sessions/:id/messages?limit=20 -- Get the most recent messages
/// for a live session.
///
/// Looks up the session in the live_sessions map to find its JSONL file path,
/// then parses the file and returns the last N messages (most recent).
async fn get_live_session_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<MessagesQuery>,
) -> Response {
    let file_path = {
        let map = state.live_sessions.read().await;
        match map.get(&id) {
            Some(session) => session.file_path.clone(),
            None => {
                return (
                    axum::http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "Session not found" })),
                )
                    .into_response();
            }
        }
    };

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session file not found on disk" })),
        )
            .into_response();
    }

    match claude_view_core::parse_session(path).await {
        Ok(session) => {
            let total = session.messages.len();
            let limit = params.limit.min(total);
            // Return the last N messages (most recent activity)
            let messages: Vec<_> = session
                .messages
                .into_iter()
                .skip(total.saturating_sub(limit))
                .collect();

            Json(serde_json::json!({
                "messages": messages,
                "total": total,
                "returned": messages.len(),
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(session_id = %id, error = %e, "Failed to parse live session JSONL");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to parse session: {e}") })),
            )
                .into_response()
        }
    }
}

/// GET /api/live/sessions/{id}/statusline -- debug endpoint returning raw statusline JSON.
async fn get_session_statusline_debug(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let sessions = state.live_sessions.read().await;
    match sessions.get(&id) {
        Some(session) => match &session.statusline_raw {
            Some(raw) => Json(raw.clone()).into_response(),
            None => (axum::http::StatusCode::NOT_FOUND, "No statusline data yet").into_response(),
        },
        None => (axum::http::StatusCode::NOT_FOUND, "Session not found").into_response(),
    }
}

/// POST /api/live/sessions/:id/kill -- Send SIGTERM to the session's Claude process.
///
/// Returns `{ "killed": true, "pid": <pid> }` on success.
/// Returns 500 with `{ "error": "...", "pid": <pid> }` if SIGTERM fails.
/// Returns 404 with `{ "canDismiss": true }` if the session has no PID (already exited).
/// Returns 404 with `{ "error": "Session not found" }` if the session ID is unknown.
async fn kill_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Response {
    // Extract session info then drop the lock before making syscalls
    let session_info = {
        let map = state.live_sessions.read().await;
        map.get(&session_id).map(|s| s.pid)
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

// =============================================================================
// Control Binding (sidecar → Rust server notification)
// =============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindControlRequest {
    control_id: String,
}

/// POST /api/live/sessions/:id/bind-control -- Sidecar notifies that it now controls this session.
///
/// Sets the `control` field on the LiveSession, which flows to SSE clients.
/// Idempotent: re-binding with the same controlId is a no-op success.
async fn bind_control(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(body): Json<BindControlRequest>,
) -> Response {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&session_id) {
        // Already bound with same controlId → idempotent success
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
            cancel: tokio_util::sync::CancellationToken::new(),
        });
        // Notify SSE clients of the control binding change
        let _ = state.live_tx.send(SessionEvent::SessionUpdated {
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnbindControlRequest {
    control_id: String,
}

/// POST /api/live/sessions/:id/unbind-control -- Sidecar notifies it released control.
///
/// Only unbinds if the current controlId matches (CAS semantics).
async fn unbind_control(
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
            let _ = state.live_tx.send(SessionEvent::SessionUpdated {
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

/// DELETE /api/live/sessions/:id/dismiss -- Dismiss a recently closed session.
///
/// Removes the session from the live map and marks it as dismissed in SQLite.
/// Sends `SessionCompleted` to notify the frontend to remove from `recentlyClosed`.
///
/// Note: There is a narrow race window between removing from the map and persisting
/// to SQLite. If the server crashes in that window, the session reappears on restart
/// (user clicks dismiss again). This is accept-and-retry tolerant by design.
async fn dismiss_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let removed = {
        let mut sessions = state.live_sessions.write().await;
        if let Some(session) = sessions.get(&id) {
            if session.closed_at.is_some() {
                sessions.remove(&id);
                true
            } else {
                false // Can't dismiss an active session
            }
        } else {
            false
        }
    };

    if removed {
        // Persist dismissal to SQLite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Err(e) = sqlx::query("UPDATE sessions SET dismissed_at = ?1 WHERE id = ?2")
            .bind(now)
            .bind(&id)
            .execute(state.db.pool())
            .await
        {
            tracing::warn!(session_id = %id, error = %e, "Failed to persist dismiss to SQLite");
        }
        // Notify frontend to remove from recentlyClosed
        let _ = state
            .live_tx
            .send(SessionEvent::SessionCompleted { session_id: id });
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

/// DELETE /api/live/recently-closed -- Dismiss all recently closed sessions.
async fn dismiss_all_closed(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dismissed_ids: Vec<String> = {
        let mut sessions = state.live_sessions.write().await;
        let ids: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| s.closed_at.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            sessions.remove(id);
        }
        ids
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    match state.db.pool().begin().await {
        Ok(mut tx) => {
            for id in &dismissed_ids {
                let _ = sqlx::query("UPDATE sessions SET dismissed_at = ?1 WHERE id = ?2")
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await;
            }
            let _ = tx.commit().await;
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to begin transaction for dismiss_all_closed — dismissal not persisted");
        }
    }
    for id in &dismissed_ids {
        let _ = state.live_tx.send(SessionEvent::SessionCompleted {
            session_id: id.clone(),
        });
    }

    Json(serde_json::json!({"dismissedCount": dismissed_ids.len()}))
}

/// GET /api/live/summary -- Aggregate statistics across all live sessions.
async fn get_live_summary(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let process_count = state
        .live_manager
        .as_ref()
        .map(|m| m.process_count())
        .unwrap_or(0);
    let summary = build_summary(&map, process_count);
    match serde_json::to_value(&summary) {
        Ok(v) => Json(v),
        Err(e) => {
            tracing::error!("failed to serialize live summary: {e}");
            Json(serde_json::json!({ "error": "internal serialization error" }))
        }
    }
}

/// GET /api/live/pricing -- Return the model pricing table.
///
/// Exposes per-model costs in a frontend-friendly format (cost per million tokens).
async fn get_pricing(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let pricing = state.pricing.read().expect("pricing lock poisoned");
    let models: HashMap<String, serde_json::Value> = pricing
        .iter()
        .map(|(name, p)| {
            let mut model = serde_json::json!({
                "inputPerMillion": p.input_cost_per_token * 1_000_000.0,
                "outputPerMillion": p.output_cost_per_token * 1_000_000.0,
                "cacheReadPerMillion": p.cache_read_cost_per_token * 1_000_000.0,
                "cacheWritePerMillion": p.cache_creation_cost_per_token * 1_000_000.0,
            });
            if let Some(rate) = p.input_cost_per_token_above_200k {
                model["inputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            if let Some(rate) = p.output_cost_per_token_above_200k {
                model["outputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            (name.clone(), model)
        })
        .collect();
    Json(serde_json::json!({
        "models": models,
        "modelCount": models.len(),
        "source": "litellm+defaults",
    }))
}

// =============================================================================
// Helpers
// =============================================================================

/// Build a summary JSON object from the current live sessions map.
fn build_summary(map: &HashMap<String, LiveSession>, process_count: u32) -> serde_json::Value {
    let mut needs_you_count = 0usize;
    let mut autonomous_count = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for session in map.values() {
        if session.closed_at.is_some() {
            continue; // Recently closed — excluded from active counts
        }
        match session.agent_state.group {
            AgentStateGroup::NeedsYou => needs_you_count += 1,
            AgentStateGroup::Autonomous => autonomous_count += 1,
        }
        total_cost += session.cost.total_usd;
        total_tokens += session.tokens.total_tokens;
    }

    serde_json::json!({
        "needsYouCount": needs_you_count,
        "autonomousCount": autonomous_count,
        "totalCostTodayUsd": total_cost,
        "totalTokensToday": total_tokens,
        "processCount": process_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{AgentState, AgentStateGroup, LiveSession, SessionStatus};
    use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

    /// Minimal LiveSession for tests with optional closed flag.
    fn test_session(id: &str, closed: bool) -> LiveSession {
        let mut s = LiveSession {
            id: id.to_string(),
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            status: SessionStatus::Working,
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 5,
            started_at: Some(1000),
            last_activity_at: 1000,
            model: None,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            progress_items: Vec::new(),
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            compact_count: 0,
            slug: None,
            user_files: None,
            closed_at: None,
            control: None,
            hook_events: Vec::new(),
            statusline_context_window_size: None,
            statusline_used_pct: None,
            statusline_cost_usd: None,
            model_display_name: None,
            statusline_cwd: None,
            statusline_project_dir: None,
            statusline_total_duration_ms: None,
            statusline_api_duration_ms: None,
            statusline_lines_added: None,
            statusline_lines_removed: None,
            statusline_input_tokens: None,
            statusline_output_tokens: None,
            statusline_cache_read_tokens: None,
            statusline_cache_creation_tokens: None,
            statusline_version: None,
            exceeds_200k_tokens: None,
            statusline_transcript_path: None,
            statusline_raw: None,
        };
        if closed {
            s.status = SessionStatus::Done;
            s.closed_at = Some(1_700_000_000);
        }
        s
    }

    #[test]
    fn test_build_summary_excludes_closed_sessions() {
        let mut map = HashMap::new();
        map.insert("active-1".into(), test_session("active-1", false));
        map.insert("active-2".into(), test_session("active-2", false));
        map.insert("closed-1".into(), test_session("closed-1", true));

        let summary = build_summary(&map, 2);

        assert_eq!(
            summary["autonomousCount"], 2,
            "closed session must not inflate autonomousCount"
        );
        assert_eq!(summary["needsYouCount"], 0);
        assert_eq!(
            summary["processCount"], 2,
            "processCount should be passed through"
        );
    }

    #[test]
    fn test_build_summary_empty_map() {
        let map = HashMap::new();
        let summary = build_summary(&map, 0);

        assert_eq!(summary["autonomousCount"], 0);
        assert_eq!(summary["needsYouCount"], 0);
        assert_eq!(summary["totalCostTodayUsd"], 0.0);
        assert_eq!(summary["totalTokensToday"], 0);
    }
}
