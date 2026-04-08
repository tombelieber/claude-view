//! REST endpoints for querying live sessions.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json, Response},
};

use crate::state::AppState;

use super::types::MessagesQuery;

/// GET /api/live/sessions -- List all live sessions, sorted by most recent activity.
#[utoipa::path(get, path = "/api/live/sessions", tag = "live",
    responses(
        (status = 200, description = "Active live sessions", body = serde_json::Value),
    )
)]
pub async fn list_live_sessions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let mut sessions: Vec<_> = map.values().cloned().collect();
    sessions.sort_by(|a, b| b.hook.last_activity_at.cmp(&a.hook.last_activity_at));
    let recently_closed: Vec<_> = {
        let rc = state.recently_closed.read().await;
        let mut v: Vec<_> = rc.values().cloned().collect();
        v.sort_by(|a, b| (b.closed_at.unwrap_or(0)).cmp(&a.closed_at.unwrap_or(0)));
        v
    };
    let process_count = state
        .live_manager
        .as_ref()
        .map(|m| m.process_count())
        .unwrap_or(0);
    Json(serde_json::json!({
        "sessions": sessions,
        "recentlyClosed": recently_closed,
        "total": sessions.len(),
        "processCount": process_count,
    }))
}

/// GET /api/live/sessions/:id -- Get a single live session by ID.
#[utoipa::path(get, path = "/api/live/sessions/{id}", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Single live session data", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_live_session(
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

/// GET /api/live/sessions/:id/messages?limit=20 -- Get the most recent messages
/// for a live session.
///
/// Looks up the session in the live_sessions map to find its JSONL file path,
/// then parses the file and returns the last N messages (most recent).
#[utoipa::path(get, path = "/api/live/sessions/{id}/messages", tag = "live",
    params(
        ("id" = String, Path, description = "Session ID"),
        MessagesQuery,
    ),
    responses(
        (status = 200, description = "Recent messages from a live session", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_live_session_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<MessagesQuery>,
) -> Response {
    let file_path = {
        let map = state.live_sessions.read().await;
        match map.get(&id) {
            Some(session) => session.jsonl.file_path.clone(),
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
#[utoipa::path(get, path = "/api/live/sessions/{id}/statusline", tag = "live",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Raw statusline JSON for a live session", body = serde_json::Value),
        (status = 404, description = "Session not found or no statusline data"),
    )
)]
pub async fn get_session_statusline_debug(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let sessions = state.live_sessions.read().await;
    match sessions.get(&id) {
        Some(session) => {
            let log = &session.statusline.statusline_debug_log;
            if log.is_empty() {
                return (axum::http::StatusCode::NOT_FOUND, "No statusline data yet")
                    .into_response();
            }
            let entries: Vec<serde_json::Value> = log
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "receivedAt": e.received_at,
                        "blocksPresent": e.blocks_present,
                        "payload": e.payload,
                    })
                })
                .collect();
            Json(serde_json::json!({
                "sessionId": id,
                "entryCount": entries.len(),
                "maxEntries": crate::live::state::MAX_STATUSLINE_DEBUG_ENTRIES,
                "entries": entries,
            }))
            .into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Session not found").into_response(),
    }
}
