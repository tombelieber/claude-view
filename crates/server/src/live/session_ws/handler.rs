//! WebSocket upgrade handler for multiplexed session connections.
//!
//! Combines terminal JSONL streaming + hook events (from existing terminal WS)
//! with sidecar SDK event relay into a single multiplexed connection.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use super::frames::*;
use crate::state::AppState;

/// WS /api/live/sessions/{id}/ws — multiplexed session WebSocket.
///
/// Combines:
/// - Block/raw JSONL streaming (from terminal WS)
/// - Hook event broadcasts
/// - Sidecar SDK event relay (future — currently just JSONL + hooks)
///
/// Connection limit enforcement via SessionChannelRegistry.
pub async fn ws_session_handler(
    Path(session_id): Path<String>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Resolve the JSONL file path from live sessions
    let file_path = {
        let sessions = state.live_sessions.read().await;
        sessions
            .get(&session_id)
            .map(|s| PathBuf::from(&s.jsonl.file_path))
    };

    let sid = session_id.clone();
    ws.on_upgrade(move |socket| async move {
        // Check connection limits inside the callback (prevents count leak on handshake failure)
        if let Err(reason) = state.session_channels.try_connect(&sid) {
            let frame = SessionFrame::Error {
                message: reason.to_string(),
                code: "CONNECTION_LIMIT".to_string(),
            };
            let mut socket = socket;
            let _ = socket
                .send(Message::Text(serde_json::to_string(&frame).unwrap().into()))
                .await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4429,
                    reason: reason.into(),
                })))
                .await;
            return;
        }

        // RAII guard — disconnect on drop (even on panic)
        let _guard = ConnectionGuard {
            registry: state.session_channels.clone(),
            session_id: sid.clone(),
        };

        handle_multiplexed_ws(socket, sid, file_path, state).await;
    })
}

/// RAII guard that decrements the connection count when dropped.
struct ConnectionGuard {
    registry: Arc<super::registry::SessionChannelRegistry>,
    session_id: String,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.registry.disconnect(&self.session_id);
    }
}

/// Core multiplexed WS handler.
async fn handle_multiplexed_ws(
    mut socket: WebSocket,
    session_id: String,
    file_path: Option<PathBuf>,
    state: Arc<AppState>,
) {
    // 1. Wait for client handshake
    let handshake = match tokio::time::timeout(Duration::from_secs(10), socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<ClientHandshake>(&text) {
            Ok(hs) => hs,
            Err(_) => {
                debug!(session_id = %session_id, "Malformed handshake, using defaults");
                ClientHandshake {
                    modes: vec![FrameMode::Block],
                    scrollback: ScrollbackConfig::default(),
                }
            }
        },
        Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
            debug!(session_id = %session_id, "Client disconnected before handshake");
            return;
        }
        _ => {
            debug!(session_id = %session_id, "Handshake timeout, using defaults");
            ClientHandshake {
                modes: vec![FrameMode::Block],
                scrollback: ScrollbackConfig::default(),
            }
        }
    };

    let wants_blocks = handshake.modes.contains(&FrameMode::Block);
    let wants_raw = handshake.modes.contains(&FrameMode::TerminalRaw);

    // 2. Send handshake ack
    let ack = SessionFrame::HandshakeAck {
        session_id: session_id.clone(),
        modes: handshake.modes.clone(),
    };
    if send_frame(&mut socket, &ack).await.is_err() {
        return;
    }

    // 3. Set up JSONL file tracking (if session has a file)
    let file_path = match file_path {
        Some(p) if p.exists() => p,
        Some(p) => {
            // File doesn't exist yet — send error but don't close (file may appear)
            let _ = send_frame(
                &mut socket,
                &SessionFrame::Error {
                    message: format!("JSONL file not found: {}", p.display()),
                    code: "FILE_NOT_FOUND".to_string(),
                },
            )
            .await;
            // Still proceed — hook events can still flow
            p
        }
        None => {
            let _ = send_frame(
                &mut socket,
                &SessionFrame::Error {
                    message: "Session not found in live sessions".to_string(),
                    code: "SESSION_NOT_FOUND".to_string(),
                },
            )
            .await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4404,
                    reason: "Session not found".into(),
                })))
                .await;
            return;
        }
    };

    // 4. Set up file watcher + scrollback
    let (watch_tx, mut watch_rx) =
        tokio::sync::mpsc::channel::<crate::routes::terminal::WatchEvent>(256);
    let _watcher = match crate::routes::terminal::start_file_watcher(&file_path, watch_tx) {
        Ok(w) => w,
        Err(e) => {
            warn!(session_id = %session_id, error = %e, "Failed to start file watcher");
            let _ = send_frame(
                &mut socket,
                &SessionFrame::Error {
                    message: format!("File watcher failed: {e}"),
                    code: "WATCH_FAILED".to_string(),
                },
            )
            .await;
            return;
        }
    };

    // Start from position 0 to read entire file for scrollback
    let mut tracker = crate::file_tracker::FilePositionTracker::new(file_path.clone());

    // Send scrollback
    let mut block_acc = claude_view_core::block_accumulator::BlockAccumulator::new();
    let mut sent_blocks: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    if let Ok(lines) = tracker.read_new_lines().await {
        if wants_blocks {
            for line in &lines {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    block_acc.process_line(&entry);
                }
            }
            for block in block_acc.snapshot() {
                if let Ok(json) = serde_json::to_string(&block) {
                    sent_blocks.insert(block.id().to_string(), json.clone());
                    let frame = SessionFrame::BlockDelta {
                        block: serde_json::from_str(&json).unwrap_or_default(),
                    };
                    if send_frame(&mut socket, &frame).await.is_err() {
                        return;
                    }
                }
            }
            let _ = send_frame(&mut socket, &SessionFrame::BlockBufferEnd).await;
        }
        if wants_raw {
            // Send last N lines as scrollback
            let start = lines
                .len()
                .saturating_sub(handshake.scrollback.raw as usize);
            for line in &lines[start..] {
                let frame = SessionFrame::TerminalRaw { line: line.clone() };
                if send_frame(&mut socket, &frame).await.is_err() {
                    return;
                }
            }
            let _ = send_frame(&mut socket, &SessionFrame::TerminalBufferEnd).await;
        }
    }

    // 5. Subscribe to hook event broadcasts
    let mut hook_rx: broadcast::Receiver<crate::live::state::HookEvent> = {
        let mut channels = state.hook_event_channels.write().await;
        let tx = channels
            .entry(session_id.clone())
            .or_insert_with(|| broadcast::channel(256).0);
        tx.subscribe()
    };

    let replayed_hook_count: usize = {
        let sessions = state.live_sessions.read().await;
        sessions
            .get(&session_id)
            .map(|s| s.hook.hook_events.len())
            .unwrap_or(0)
    };
    let mut hook_events_seen: usize = 0;

    // 6. Main event loop
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
    heartbeat_interval.tick().await; // skip first tick

    info!(session_id = %session_id, modes = ?handshake.modes, "Multiplexed WS connected");

    loop {
        tokio::select! {
            // JSONL file changes
            watch_event = watch_rx.recv() => {
                match watch_event {
                    Some(crate::routes::terminal::WatchEvent::Modified) => {
                        match tracker.read_new_lines().await {
                            Ok(lines) if !lines.is_empty() => {
                                if wants_blocks {
                                    if tracker.was_truncated() {
                                        block_acc.reset();
                                        sent_blocks.clear();
                                    }
                                    for line in &lines {
                                        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                                            block_acc.process_line(&entry);
                                        }
                                    }
                                    for block in block_acc.snapshot() {
                                        let id = block.id().to_string();
                                        if let Ok(json) = serde_json::to_string(&block) {
                                            let changed = sent_blocks.get(&id).is_none_or(|prev| *prev != json);
                                            if changed {
                                                sent_blocks.insert(id, json.clone());
                                                let frame = SessionFrame::BlockDelta {
                                                    block: serde_json::from_str(&json).unwrap_or_default(),
                                                };
                                                if send_frame(&mut socket, &frame).await.is_err() {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                                if wants_raw {
                                    for line in &lines {
                                        let frame = SessionFrame::TerminalRaw { line: line.clone() };
                                        if send_frame(&mut socket, &frame).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                            Ok(_) => {} // empty read
                            Err(e) => {
                                warn!(session_id = %session_id, error = %e, "Failed to read new lines");
                            }
                        }
                    }
                    Some(crate::routes::terminal::WatchEvent::Error(e)) => {
                        warn!(session_id = %session_id, error = %e, "File watcher error");
                    }
                    None => {
                        info!(session_id = %session_id, "File watcher channel closed");
                        return;
                    }
                }
            }

            // Client messages
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(msg) = serde_json::from_str::<ClientMessage>(&text) {
                            match msg {
                                ClientMessage::Ping => {
                                    let _ = send_frame(&mut socket, &SessionFrame::Pong).await;
                                }
                                ClientMessage::SdkSend { .. } => {
                                    // SDK relay — future: forward to sidecar WS
                                    debug!(session_id = %session_id, "SdkSend received (relay not yet wired)");
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!(session_id = %session_id, "Multiplexed WS disconnected");
                        return;
                    }
                    Some(Err(e)) => {
                        debug!(session_id = %session_id, error = %e, "WS receive error");
                        return;
                    }
                    _ => {} // Binary, Pong — ignore
                }
            }

            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    debug!(session_id = %session_id, "Client disconnected during heartbeat");
                    return;
                }
            }

            // Hook events
            hook_event = hook_rx.recv() => {
                if let Ok(event) = hook_event {
                    hook_events_seen += 1;
                    if hook_events_seen <= replayed_hook_count {
                        continue;
                    }
                    if wants_blocks {
                        let block = claude_view_core::hook_to_block::make_hook_progress_block(
                            format!("hook-live-{}-{}", event.timestamp, hook_events_seen),
                            event.timestamp as f64,
                            &event.event_name,
                            event.tool_name.as_deref(),
                            &event.label,
                        );
                        if let Ok(json) = serde_json::to_string(&block) {
                            sent_blocks.insert(block.id().to_string(), json.clone());
                            let frame = SessionFrame::BlockDelta {
                                block: serde_json::from_str(&json).unwrap_or_default(),
                            };
                            if send_frame(&mut socket, &frame).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Send a `SessionFrame` as JSON text over the WebSocket.
async fn send_frame(socket: &mut WebSocket, frame: &SessionFrame) -> Result<(), ()> {
    let json = serde_json::to_string(frame).map_err(|_| ())?;
    socket
        .send(Message::Text(json.into()))
        .await
        .map_err(|_| ())
}
