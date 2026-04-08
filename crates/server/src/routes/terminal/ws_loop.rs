//! Core WebSocket event loop for terminal streaming.
//!
//! Manages the full lifecycle of a terminal WebSocket connection:
//! 1. Wait for handshake
//! 2. Send scrollback buffer
//! 3. Stream live updates via file watcher
//! 4. Handle client messages and heartbeat

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use tokio::sync::mpsc;

use claude_view_core::hook_to_block::make_hook_progress_block;

use crate::state::AppState;

use super::format::format_line_for_mode;
use super::types::{
    default_mode, default_scrollback, ClientMessage, HandshakeMessage, RichModeFinders, WatchEvent,
    MAX_SCROLLBACK,
};
use super::watcher::start_file_watcher;
use super::ws_init;

/// Core WebSocket handler -- manages the full lifecycle:
/// 1. Wait for handshake
/// 2. Send scrollback buffer
/// 3. Stream live updates via file watcher
/// 4. Handle client messages and heartbeat
pub(super) async fn handle_terminal_ws(
    mut socket: WebSocket,
    session_id: String,
    file_path: PathBuf,
    _terminal_connections: Arc<crate::terminal_state::TerminalConnectionManager>,
    state: Arc<AppState>,
) {
    // 1. Wait for handshake message from client (with timeout)
    let handshake = match tokio::time::timeout(Duration::from_secs(10), socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => {
            match serde_json::from_str::<HandshakeMessage>(&text) {
                Ok(hs) => hs,
                Err(_) => {
                    tracing::debug!(
                        session_id = %session_id,
                        "Malformed handshake message, using defaults"
                    );
                    HandshakeMessage {
                        mode: default_mode(),
                        scrollback: default_scrollback(),
                    }
                }
            }
        }
        Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
            tracing::debug!(session_id = %session_id, "Client disconnected before handshake");
            return;
        }
        _ => {
            tracing::debug!(
                session_id = %session_id,
                "Handshake timeout or unexpected message type, using defaults"
            );
            HandshakeMessage {
                mode: default_mode(),
                scrollback: default_scrollback(),
            }
        }
    };

    // Track current mode (mutable -- can be switched mid-stream)
    let mut current_mode = handshake.mode.clone();

    // Create SIMD finders once for rich mode parsing (per CLAUDE.md rules)
    let finders = RichModeFinders::new();

    // 2. Verify JSONL file exists
    if !file_path.exists() {
        let err_msg = serde_json::json!({
            "type": "error",
            "message": "Session JSONL file not found on disk",
        });
        let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: 4004,
                reason: "Session JSONL file not found".into(),
            })))
            .await;
        return;
    }

    // 3. Send scrollback buffer
    if ws_init::send_scrollback(
        &mut socket,
        &file_path,
        &handshake,
        &current_mode,
        &finders,
        &session_id,
    )
    .await
    .is_err()
    {
        return;
    }

    // 3b. Send buffered hook events from in-memory LiveSession
    if ws_init::send_buffered_hook_events(&mut socket, &state, &session_id, &current_mode)
        .await
        .is_err()
    {
        return;
    }

    // 4. Set up file watcher for live streaming
    let (watch_tx, watch_rx) = mpsc::channel::<WatchEvent>(64);

    let mut tracker =
        match crate::file_tracker::FilePositionTracker::new_at_end(file_path.clone()).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to create file tracker"
                );
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Failed to initialize file tracker: {e}"),
                });
                let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: 4500,
                        reason: "File tracker init failed".into(),
                    })))
                    .await;
                return;
            }
        };

    // Start the notify watcher for this specific file
    let _watcher = match start_file_watcher(&file_path, watch_tx) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to start file watcher"
            );
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!("Failed to start file watcher: {e}"),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 4500,
                    reason: "Watch failed".into(),
                })))
                .await;
            return;
        }
    };

    tracing::info!(
        session_id = %session_id,
        scrollback = handshake.scrollback.min(MAX_SCROLLBACK),
        "Terminal WebSocket connected"
    );

    // Persistent BlockAccumulator for block mode — correlates multi-line constructs
    // (e.g., incremental assistant entries with the same message.id) that a per-line
    // accumulator cannot. The CC CLI writes thinking, text, and tool_use as separate
    // JSONL lines with the same message.id; without persistence, each line produces a
    // separate block that replaces the previous one via the frontend's ID-based merge.
    let mut block_acc = claude_view_core::block_accumulator::BlockAccumulator::new();
    // Track previously-sent block serializations by ID to send only changes.
    let mut sent_blocks: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // 4b. Subscribe to hook event broadcasts for this session
    let hook_rx = {
        let mut channels = state.hook_event_channels.write().await;
        let tx = channels
            .entry(session_id.clone())
            .or_insert_with(|| tokio::sync::broadcast::channel(256).0);
        tx.subscribe()
    };

    // 3b replays the in-memory vec snapshot (historical events).
    // 4b receives new broadcast events (live events going forward).
    // These are separate data sources — no dedup needed.
    // See CLAUDE.md: "Separate Channels = Separate Data = No Dedup".
    let live_hook_counter: usize = 0;

    // 5. Main event loop
    run_event_loop(
        &mut socket,
        &session_id,
        &mut current_mode,
        &finders,
        watch_rx,
        &mut tracker,
        &mut block_acc,
        &mut sent_blocks,
        hook_rx,
        live_hook_counter,
    )
    .await;
    // _watcher is dropped here, stopping the file watch
}

/// Main event loop: multiplex file watcher, client messages, heartbeat,
/// and hook event broadcasts.
///
/// Heartbeat is 15s to detect dead connections fast — during dev hot reloads,
/// the browser drops TCP without a close frame, and stale connections linger
/// until the next failed send. WebSocket Ping frames (not application-level
/// JSON) are used because they're handled at the protocol level and fail
/// faster for broken TCP connections.
#[allow(clippy::too_many_arguments)]
async fn run_event_loop(
    socket: &mut WebSocket,
    session_id: &str,
    current_mode: &mut String,
    finders: &RichModeFinders,
    mut watch_rx: mpsc::Receiver<WatchEvent>,
    tracker: &mut crate::file_tracker::FilePositionTracker,
    block_acc: &mut claude_view_core::block_accumulator::BlockAccumulator,
    sent_blocks: &mut std::collections::HashMap<String, String>,
    mut hook_rx: tokio::sync::broadcast::Receiver<crate::live::state::HookEvent>,
    mut live_hook_counter: usize,
) {
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
    // Skip the first immediate tick
    heartbeat_interval.tick().await;

    loop {
        tokio::select! {
            // File watcher events
            watch_event = watch_rx.recv() => {
                match watch_event {
                    Some(WatchEvent::Modified) => {
                        if handle_file_modified(
                            socket, session_id, current_mode, finders,
                            tracker, block_acc, sent_blocks,
                        ).await.is_err() {
                            return;
                        }
                    }
                    Some(WatchEvent::Error(e)) => {
                        tracing::warn!(
                            session_id = %session_id,
                            error = %e,
                            "File watcher error"
                        );
                        let err_msg = serde_json::json!({
                            "type": "error",
                            "message": format!("File watcher error: {e}"),
                        });
                        let _ = socket
                            .send(Message::Text(err_msg.to_string().into()))
                            .await;
                        // Don't close -- transient errors are possible
                    }
                    None => {
                        // Watcher channel closed -- file watcher dropped
                        tracing::info!(
                            session_id = %session_id,
                            "File watcher channel closed"
                        );
                        let err_msg = serde_json::json!({
                            "type": "error",
                            "message": "File watcher stopped",
                        });
                        let _ = socket
                            .send(Message::Text(err_msg.to_string().into()))
                            .await;
                        let _ = socket.send(Message::Close(Some(CloseFrame {
                            code: 4500,
                            reason: "Watch failed".into(),
                        }))).await;
                        return;
                    }
                }
            }

            // Client messages
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(socket, session_id, current_mode, &text).await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!(
                            session_id = %session_id,
                            "Terminal WebSocket disconnected"
                        );
                        return;
                    }
                    Some(Err(e)) => {
                        tracing::debug!(
                            session_id = %session_id,
                            error = %e,
                            "WebSocket receive error"
                        );
                        return;
                    }
                    _ => {
                        // Binary or Pong messages -- ignore
                    }
                }
            }

            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if socket
                    .send(Message::Ping(vec![].into()))
                    .await
                    .is_err()
                {
                    tracing::debug!(
                        session_id = %session_id,
                        "Client disconnected during heartbeat ping"
                    );
                    return;
                }
            }

            // Hook event broadcasts from handle_hook()
            hook_event = hook_rx.recv() => {
                match hook_event {
                    Ok(event) => {
                        live_hook_counter += 1;
                        let text = if current_mode.as_str() == "block" {
                            let block = make_hook_progress_block(
                                format!("hook-live-{}-{}", event.timestamp, live_hook_counter),
                                event.timestamp as f64,
                                &event.event_name,
                                event.tool_name.as_deref(),
                                &event.label,
                            );
                            let json = serde_json::to_string(&block).unwrap_or_default();
                            sent_blocks.insert(block.id().to_string(), json.clone());
                            json
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
                        if socket
                            .send(Message::Text(text.into()))
                            .await
                            .is_err()
                        {
                            tracing::debug!(
                                session_id = %session_id,
                                "Client disconnected during hook event send"
                            );
                            return;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(
                            session_id = %session_id,
                            lagged = n,
                            "Hook event broadcast lagged — missed events are in the in-memory vec"
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Channel closed — session ended, no more events
                    }
                }
            }
        }
    }
}

/// Handle a file modification event: read new lines and send them.
async fn handle_file_modified(
    socket: &mut WebSocket,
    session_id: &str,
    current_mode: &str,
    finders: &RichModeFinders,
    tracker: &mut crate::file_tracker::FilePositionTracker,
    block_acc: &mut claude_view_core::block_accumulator::BlockAccumulator,
    sent_blocks: &mut std::collections::HashMap<String, String>,
) -> Result<(), ()> {
    match tracker.read_new_lines().await {
        Ok(lines) => {
            if current_mode == "block" {
                // Reset accumulator on file truncation (compaction).
                if tracker.was_truncated() {
                    block_acc.reset();
                    sent_blocks.clear();
                }
                // Block mode: feed lines into the persistent accumulator,
                // then send only new/changed blocks.
                for line in &lines {
                    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                        block_acc.process_line(&entry);
                    }
                }
                let current = block_acc.snapshot();
                for block in &current {
                    let id = block.id().to_string();
                    if let Ok(json) = serde_json::to_string(block) {
                        let changed = sent_blocks.get(&id).is_none_or(|prev| *prev != json);
                        if changed {
                            sent_blocks.insert(id, json.clone());
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                tracing::debug!(
                                    session_id = %session_id,
                                    "Client disconnected during live stream"
                                );
                                return Err(());
                            }
                        }
                    }
                }
            } else {
                for line in &lines {
                    for formatted in format_line_for_mode(line, current_mode, finders) {
                        if socket.send(Message::Text(formatted.into())).await.is_err() {
                            tracing::debug!(
                                session_id = %session_id,
                                "Client disconnected during live stream"
                            );
                            return Err(());
                        }
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to read new lines"
            );
            // Don't close -- the file might recover
            Ok(())
        }
    }
}

/// Handle a client text message (ping, mode switch, resize, etc.).
async fn handle_client_message(
    socket: &mut WebSocket,
    session_id: &str,
    current_mode: &mut String,
    text: &str,
) {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(msg) => match msg.msg_type.as_str() {
            "ping" => {
                let pong = serde_json::json!({ "type": "pong" });
                let _ = socket.send(Message::Text(pong.to_string().into())).await;
            }
            "mode" => {
                // Update the display mode mid-stream
                if let Some(new_mode) = &msg.mode {
                    if new_mode == "raw" || new_mode == "rich" {
                        *current_mode = new_mode.clone();
                        tracing::debug!(
                            session_id = %session_id,
                            mode = %current_mode,
                            "Mode switched"
                        );
                    } else {
                        tracing::debug!(
                            session_id = %session_id,
                            mode = %new_mode,
                            "Unknown mode requested, ignoring"
                        );
                    }
                }
            }
            "resize" => {
                // Acknowledged but no action needed server-side
                tracing::debug!(
                    session_id = %session_id,
                    msg_type = %msg.msg_type,
                    "Client message acknowledged"
                );
            }
            _ => {
                tracing::debug!(
                    session_id = %session_id,
                    msg_type = %msg.msg_type,
                    "Unknown client message type"
                );
            }
        },
        Err(_) => {
            tracing::debug!(
                session_id = %session_id,
                "Malformed client message, ignoring"
            );
        }
    }
}
