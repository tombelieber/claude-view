//! WebSocket connection initialization: scrollback buffer and hook event replay.
//!
//! These helpers run once during WebSocket setup before the main event loop
//! begins streaming live updates.

use std::path::PathBuf;

use axum::extract::ws::{CloseFrame, Message, WebSocket};

use claude_view_core::hook_to_block::make_hook_progress_block;

use crate::state::AppState;

use super::format::format_line_for_mode;
use super::types::{HandshakeMessage, RichModeFinders, MAX_SCROLLBACK};

/// Send the initial scrollback buffer to the client.
pub(super) async fn send_scrollback(
    socket: &mut WebSocket,
    file_path: &PathBuf,
    handshake: &HandshakeMessage,
    current_mode: &str,
    finders: &RichModeFinders,
    session_id: &str,
) -> Result<(), ()> {
    let scrollback_count = handshake.scrollback.min(MAX_SCROLLBACK);
    match claude_view_core::tail::tail_lines(file_path, scrollback_count).await {
        Ok(lines) => {
            for line in &lines {
                for formatted in format_line_for_mode(line, current_mode, finders) {
                    if socket.send(Message::Text(formatted.into())).await.is_err() {
                        return Err(()); // client disconnected
                    }
                }
            }
            // Send buffer_end marker
            let end_msg = serde_json::json!({ "type": "buffer_end" });
            if socket
                .send(Message::Text(end_msg.to_string().into()))
                .await
                .is_err()
            {
                return Err(());
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to read scrollback"
            );
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!("Failed to read scrollback: {e}"),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4500,
                    reason: "Scrollback read failed".into(),
                })))
                .await;
            Err(())
        }
    }
}

/// Send buffered hook events from in-memory LiveSession (step 3b).
pub(super) async fn send_buffered_hook_events(
    socket: &mut WebSocket,
    state: &AppState,
    session_id: &str,
    current_mode: &str,
) -> Result<(), ()> {
    let sessions = state.live_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        for (i, event) in session.hook.hook_events.iter().enumerate() {
            let text = if current_mode == "block" {
                let block = make_hook_progress_block(
                    format!("hook-{}-{}", event.timestamp, i),
                    event.timestamp as f64,
                    &event.event_name,
                    event.tool_name.as_deref(),
                    &event.label,
                );
                serde_json::to_string(&block).unwrap_or_default()
            } else {
                serde_json::json!({
                    "type": "hook_event",
                    "timestamp": event.timestamp,
                    "eventName": event.event_name,
                    "toolName": event.tool_name,
                    "label": event.label,
                    "group": event.group,
                    "context": event.context,
                    "source": event.source,
                })
                .to_string()
            };
            if socket.send(Message::Text(text.into())).await.is_err() {
                return Err(());
            }
        }
    }
    Ok(())
}
