//! HTTP upgrade handlers for terminal WebSocket connections.
//!
//! Contains the Axum route handlers that validate sessions, check connection
//! limits, and upgrade HTTP connections to WebSocket.

use std::sync::Arc;

use axum::{
    extract::ws::{CloseFrame, Message},
    extract::{Path, State, WebSocketUpgrade},
    response::Response,
};

use crate::state::AppState;

use super::types::{resolve_jsonl_path, ConnectionGuard};
use super::ws_loop::handle_terminal_ws;

/// HTTP upgrade handler -- validates session, checks connection limits,
/// then upgrades to WebSocket.
///
/// Connection limit check is done INSIDE the on_upgrade callback to prevent
/// count leaks: if `connect()` were called before the upgrade and the client
/// disconnected during the HTTP handshake, `disconnect()` would never run,
/// permanently inflating the viewer count.
pub(super) async fn ws_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    // Look up the session JSONL path: live → recently closed → DB
    let file_path = match resolve_jsonl_path(&state, &session_id).await {
        Some(path) => path,
        None => {
            return ws.on_upgrade(move |mut socket| async move {
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Session '{}' not found", session_id),
                });
                let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: 4004,
                        reason: "Session not found".into(),
                    })))
                    .await;
            });
        }
    };

    // Upgrade to WebSocket — connection limit check is inside the callback
    // so that connect() and disconnect() are always in the same async scope.
    let terminal_connections = state.terminal_connections.clone();
    let sid = session_id.clone();

    ws.on_upgrade(move |mut socket| async move {
        // Check connection limits inside the upgrade callback
        if let Err(e) = terminal_connections.connect(&sid) {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": e.to_string(),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4004,
                    reason: "Connection limit exceeded".into(),
                })))
                .await;
            return;
        }

        // RAII guard ensures disconnect() runs even on panic or task cancellation.
        // Without this, orphaned connections accumulate and hit the global limit.
        let _guard = ConnectionGuard {
            session_id: sid.clone(),
            manager: terminal_connections.clone(),
        };

        handle_terminal_ws(
            socket,
            sid.clone(),
            file_path,
            terminal_connections.clone(),
            state.clone(),
        )
        .await;
        // _guard dropped here (or on panic/cancel), calling disconnect()
    })
}

/// HTTP upgrade handler for sub-agent terminal WebSocket.
///
/// Validates agent_id, resolves the sub-agent JSONL path from the parent
/// session, then delegates to `handle_terminal_ws` for actual streaming.
pub(super) async fn ws_subagent_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path((session_id, agent_id)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> Response {
    // SECURITY: Validate agent_id is alphanumeric (prevents path traversal).
    // Claude Code agent IDs vary in length (7-char short hashes, 17+ char hex strings).
    if agent_id.is_empty()
        || !agent_id.chars().all(|c| c.is_ascii_alphanumeric())
        || agent_id.len() > 64
    {
        return ws.on_upgrade(move |mut socket| async move {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!("Invalid agent ID: '{}'", agent_id),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4004,
                    reason: "Invalid agent ID".into(),
                })))
                .await;
        });
    }

    // Look up parent session JSONL path: live → recently closed → DB
    let parent_file_path = match resolve_jsonl_path(&state, &session_id).await {
        Some(path) => path,
        None => {
            return ws.on_upgrade(move |mut socket| async move {
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Parent session '{}' not found", session_id),
                });
                let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: 4004,
                        reason: "Session not found".into(),
                    })))
                    .await;
            });
        }
    };

    // Resolve sub-agent JSONL path
    let subagent_path =
        crate::live::subagent_file::resolve_subagent_path(&parent_file_path, &agent_id);

    if !subagent_path.exists() {
        return ws.on_upgrade(move |mut socket| async move {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!(
                    "Sub-agent '{}' JSONL file not found for session '{}'",
                    agent_id, session_id
                ),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4004,
                    reason: "Sub-agent file not found".into(),
                })))
                .await;
        });
    }

    // Namespaced connection key to avoid collision with parent
    let connection_key = format!("{}::{}", session_id, agent_id);
    let terminal_connections = state.terminal_connections.clone();

    ws.on_upgrade(move |mut socket| async move {
        if let Err(e) = terminal_connections.connect(&connection_key) {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": e.to_string(),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4004,
                    reason: "Connection limit exceeded".into(),
                })))
                .await;
            return;
        }

        let _guard = ConnectionGuard {
            session_id: connection_key.clone(),
            manager: terminal_connections.clone(),
        };

        handle_terminal_ws(
            socket,
            connection_key,
            subagent_path,
            terminal_connections.clone(),
            state.clone(),
        )
        .await;
    })
}
