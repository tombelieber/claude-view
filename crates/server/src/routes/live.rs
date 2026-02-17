//! Live session monitoring endpoints (SSE + REST).
//!
//! - `GET /api/live/stream`              -- SSE stream of real-time session events
//! - `GET /api/live/sessions`            -- List all live sessions
//! - `GET /api/live/sessions/:id`        -- Get a single live session
//! - `GET /api/live/sessions/:id/messages` -- Get recent messages for a live session
//! - `POST /api/live/sessions/:id/kill`   -- Send SIGTERM to a session's process
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
    routing::{get, post},
    Router,
};
use serde::Deserialize;

use crate::live::state::{AgentStateGroup, LiveSession, SessionEvent};
use crate::state::AppState;

/// Build the live monitoring sub-router.
///
/// Routes:
/// - `GET /live/stream`                 - SSE stream of live session events
/// - `GET /live/sessions`               - List all live sessions
/// - `GET /live/sessions/:id`           - Get single live session
/// - `GET /live/sessions/:id/messages`  - Get recent messages for a live session
/// - `POST /live/sessions/:id/kill`     - Send SIGTERM to a session's process
/// - `GET /live/summary`                - Aggregate statistics
/// - `GET /live/pricing`                - Model pricing table
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/live/stream", get(live_stream))
        .route("/live/sessions", get(list_live_sessions))
        .route("/live/sessions/{id}", get(get_live_session))
        .route("/live/sessions/{id}/messages", get(get_live_session_messages))
        .route("/live/sessions/{id}/kill", post(kill_session))
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

    let stream = async_stream::stream! {
        // 1. On connect: send current summary + all active sessions
        {
            let map = sessions.read().await;
            let summary = build_summary(&map);
            yield Ok(Event::default().event("summary").data(
                serde_json::to_string(&summary).unwrap_or_default()
            ));
            for session in map.values() {
                yield Ok(Event::default().event("session_discovered").data(
                    serde_json::to_string(session).unwrap_or_default()
                ));
            }
        }

        // 2. Stream events from broadcast channel with heartbeat
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(session_event) => {
                            let (event_name, data) = match &session_event {
                                SessionEvent::SessionDiscovered { .. } => (
                                    "session_discovered",
                                    serde_json::to_string(&session_event).unwrap_or_default(),
                                ),
                                SessionEvent::SessionUpdated { .. } => (
                                    "session_updated",
                                    serde_json::to_string(&session_event).unwrap_or_default(),
                                ),
                                SessionEvent::SessionCompleted { .. } => (
                                    "session_completed",
                                    serde_json::to_string(&session_event).unwrap_or_default(),
                                ),
                                SessionEvent::Summary { .. } => (
                                    "summary",
                                    serde_json::to_string(&session_event).unwrap_or_default(),
                                ),
                            };
                            yield Ok(Event::default().event(event_name).data(data));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, re-sending all sessions",
                                n
                            );
                            // Re-send full state (same as initial connect) so the
                            // client recovers from any missed discover/complete events.
                            let map = sessions.read().await;
                            let summary = build_summary(&map);
                            yield Ok(Event::default().event("summary").data(
                                serde_json::to_string(&summary).unwrap_or_default()
                            ));
                            for session in map.values() {
                                yield Ok(Event::default().event("session_discovered").data(
                                    serde_json::to_string(session).unwrap_or_default()
                                ));
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

// =============================================================================
// REST Endpoints
// =============================================================================

/// GET /api/live/sessions -- List all live sessions, sorted by most recent activity.
async fn list_live_sessions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let mut sessions: Vec<_> = map.values().cloned().collect();
    sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    Json(serde_json::json!({
        "sessions": sessions,
        "total": sessions.len(),
    }))
}

/// GET /api/live/sessions/:id -- Get a single live session by ID.
async fn get_live_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
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

    match vibe_recall_core::parse_session(path).await {
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
            tracing::warn!(session_id = %id, error = %e, "Failed to parse live session JSONL");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to parse session: {e}") })),
            )
                .into_response()
        }
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

/// GET /api/live/summary -- Aggregate statistics across all live sessions.
async fn get_live_summary(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let summary = build_summary(&map);
    Json(serde_json::to_value(&summary).unwrap_or_default())
}

/// GET /api/live/pricing -- Return the model pricing table.
///
/// Exposes per-model costs in a frontend-friendly format (cost per million tokens).
async fn get_pricing(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let models: HashMap<String, serde_json::Value> = state
        .pricing
        .iter()
        .map(|(name, p)| {
            (
                name.clone(),
                serde_json::json!({
                    "inputPerMillion": p.input_cost_per_token * 1_000_000.0,
                    "outputPerMillion": p.output_cost_per_token * 1_000_000.0,
                    "cacheReadPerMillion": p.cache_read_cost_per_token * 1_000_000.0,
                    "cacheWritePerMillion": p.cache_creation_cost_per_token * 1_000_000.0,
                }),
            )
        })
        .collect();
    Json(serde_json::json!({
        "models": models,
        "lastUpdated": "2026-02-12",
    }))
}

// =============================================================================
// Helpers
// =============================================================================

/// Build a summary JSON object from the current live sessions map.
fn build_summary(map: &HashMap<String, LiveSession>) -> serde_json::Value {
    let mut needs_you_count = 0usize;
    let mut autonomous_count = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for session in map.values() {
        match session.agent_state.group {
            AgentStateGroup::NeedsYou | AgentStateGroup::Delivered => needs_you_count += 1,
            AgentStateGroup::Autonomous => autonomous_count += 1,
        }
        total_cost += session.cost.total_usd;
        total_tokens += session.tokens.total_tokens;
    }

    serde_json::json!({
        "needsYouCount": needs_you_count,
        "autonomousCount": autonomous_count,
        "deliveredCount": 0,
        "totalCostTodayUsd": total_cost,
        "totalTokensToday": total_tokens,
    })
}
