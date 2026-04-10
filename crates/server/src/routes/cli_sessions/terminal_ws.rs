//! Axum WebSocket handler for terminal sessions.
//!
//! Protocol (same as the Node.js relay — frontend unchanged):
//!   Client -> Server (text):   JSON `{ "type": "resize", "cols": N, "rows": N }`
//!   Client -> Server (text):   raw keystrokes (non-JSON text)
//!   Client -> Server (binary): raw keystrokes
//!   Server -> Client (binary): PTY output
//!   Server -> Client (text):   JSON `{ "type": "exit", "code": N }`
//!   Server -> Client (text):   JSON `{ "type": "error", "message": "..." }`

use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    response::Response,
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::{error::ApiError, state::AppState};

use super::terminal::{is_valid_session_id, TerminalManager};

/// GET /ws/terminal/{session_id} — upgrade to WebSocket.
pub async fn ws_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if !is_valid_session_id(&session_id) {
        return Err(ApiError::BadRequest("Invalid session ID format".into()));
    }

    Ok(ws.on_upgrade(move |socket| {
        handle_terminal_ws(socket, session_id, state.terminal_manager.clone())
    }))
}

/// Terminal reset: clear screen + cursor home. Sent before scrollback
/// replay on lag re-sync so xterm.js doesn't show duplicated output.
const TERMINAL_RESET: &[u8] = b"\x1b[2J\x1b[H";

async fn handle_terminal_ws(socket: WebSocket, session_id: String, manager: Arc<TerminalManager>) {
    let session = match manager.acquire(&session_id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(id = %session_id, error = %e, "Failed to create terminal session");
            let (mut tx, _rx) = socket.split();
            let msg = serde_json::json!({ "type": "error", "message": e });
            let _ = tx
                .send(axum::extract::ws::Message::Text(msg.to_string().into()))
                .await;
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to broadcast BEFORE reading snapshot — any output arriving
    // during snapshot read is buffered in the receiver (no data loss gap).
    let mut rx = session.tx.subscribe();
    let mut pty_dead_rx = session.pty_dead.subscribe();

    // Replay scrollback so reconnecting client sees recent output.
    let scrollback_data = {
        let sb = session.scrollback.lock().await;
        sb.as_bytes()
    };
    if !scrollback_data.is_empty() {
        let _ = ws_tx
            .send(axum::extract::ws::Message::Binary(scrollback_data.into()))
            .await;
    }

    // Send task: broadcast -> client WS (with lag re-sync + PTY exit)
    let scrollback_for_resync = Arc::clone(&session.scrollback);
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(chunk) => {
                            if ws_tx
                                .send(axum::extract::ws::Message::Binary(chunk))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "terminal broadcast lagged, re-syncing");
                            let data = {
                                let sb = scrollback_for_resync.lock().await;
                                sb.as_bytes()
                            };
                            if !data.is_empty() {
                                let _ = ws_tx.send(axum::extract::ws::Message::Binary(
                                    TERMINAL_RESET.to_vec().into()
                                )).await;
                                if ws_tx.send(axum::extract::ws::Message::Binary(
                                    data.into()
                                )).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = pty_dead_rx.changed() => {
                    let msg = serde_json::json!({ "type": "exit", "code": 0 });
                    let _ = ws_tx.send(axum::extract::ws::Message::Text(
                        msg.to_string().into()
                    )).await;
                    break;
                }
            }
        }
    });

    // Recv loop: client WS -> PTY writer channel
    let write_tx = session.write_tx.clone();
    while let Some(msg) = ws_rx.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };

        match msg {
            axum::extract::ws::Message::Text(text) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if parsed.get("type").and_then(|t| t.as_str()) == Some("resize") {
                        let cols =
                            parsed.get("cols").and_then(|c| c.as_u64()).unwrap_or(120) as u16;
                        let rows = parsed.get("rows").and_then(|r| r.as_u64()).unwrap_or(40) as u16;
                        if cols > 0 && cols <= 500 && rows > 0 && rows <= 200 {
                            session.resize(cols, rows).await;
                        }
                        continue;
                    }
                }
                if write_tx
                    .send(Bytes::copy_from_slice(text.as_bytes()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            axum::extract::ws::Message::Binary(data) => {
                if write_tx.send(data).await.is_err() {
                    break;
                }
            }
            axum::extract::ws::Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
    manager.disconnect(&session_id).await;
}
