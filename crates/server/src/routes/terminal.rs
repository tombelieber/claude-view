//! WebSocket terminal endpoint for Live Monitor.
//!
//! Streams JSONL file content to the browser over WebSocket, providing
//! real-time terminal monitoring for active Claude Code sessions.
//!
//! - `WS /api/live/sessions/:id/terminal` -- WebSocket stream of JSONL lines

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::ws::{CloseFrame, Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use memchr::memmem;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::state::AppState;

/// RAII guard that calls `disconnect()` when dropped, ensuring connection
/// count is always decremented even if the WebSocket handler panics or the
/// tokio task is cancelled (e.g., during server shutdown or HMR reload).
/// Without this, orphaned connections accumulate and hit the global limit.
struct ConnectionGuard {
    session_id: String,
    manager: Arc<crate::terminal_state::TerminalConnectionManager>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.manager.disconnect(&self.session_id);
        tracing::debug!(
            session_id = %self.session_id,
            "ConnectionGuard dropped — connection count decremented"
        );
    }
}

/// Build the terminal WebSocket sub-router.
///
/// Routes:
/// - `WS /sessions/:id/terminal` - WebSocket stream of JSONL lines
/// - `WS /sessions/:id/subagents/:agent_id/terminal` - WebSocket stream of sub-agent JSONL lines
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{id}/terminal", get(ws_terminal_handler))
        .route(
            "/sessions/{id}/subagents/{agent_id}/terminal",
            get(ws_subagent_terminal_handler),
        )
}

/// HTTP upgrade handler -- validates session, checks connection limits,
/// then upgrades to WebSocket.
///
/// Connection limit check is done INSIDE the on_upgrade callback to prevent
/// count leaks: if `connect()` were called before the upgrade and the client
/// disconnected during the HTTP handshake, `disconnect()` would never run,
/// permanently inflating the viewer count.
async fn ws_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    // Look up the session to get its JSONL file path
    let file_path = {
        let map = state.live_sessions.read().await;
        map.get(&session_id).map(|s| s.file_path.clone())
    };

    let file_path = match file_path {
        Some(fp) if !fp.is_empty() => PathBuf::from(fp),
        _ => {
            // Session not found in live sessions map -- return an error
            // via a WebSocket that immediately sends an error and closes.
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

        handle_terminal_ws(socket, sid.clone(), file_path, terminal_connections.clone()).await;
        // _guard dropped here (or on panic/cancel), calling disconnect()
    })
}

/// HTTP upgrade handler for sub-agent terminal WebSocket.
///
/// Validates agent_id, resolves the sub-agent JSONL path from the parent
/// session, then delegates to `handle_terminal_ws` for actual streaming.
async fn ws_subagent_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path((session_id, agent_id)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> Response {
    // SECURITY: Validate agent_id is alphanumeric (prevents path traversal)
    if agent_id.is_empty()
        || !agent_id.chars().all(|c| c.is_ascii_alphanumeric())
        || agent_id.len() > 16
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

    // Look up parent session to get its JSONL file path
    let parent_file_path = {
        let map = state.live_sessions.read().await;
        map.get(&session_id).map(|s| s.file_path.clone())
    };

    let parent_file_path = match parent_file_path {
        Some(fp) if !fp.is_empty() => PathBuf::from(fp),
        _ => {
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
        )
        .await;
    })
}

/// Handshake message sent by the client on connection.
#[derive(Debug, serde::Deserialize)]
struct HandshakeMessage {
    /// Display mode: "raw" (default) or "rich" (structured JSONL parsing).
    #[serde(default = "default_mode")]
    mode: String,
    /// Number of scrollback lines to send on connect (default: 100).
    #[serde(default = "default_scrollback")]
    scrollback: usize,
}

fn default_mode() -> String {
    "raw".to_string()
}

fn default_scrollback() -> usize {
    100
}

/// Maximum scrollback lines the server will send, regardless of client request.
/// Protects against OOM from malicious/misconfigured clients requesting huge scrollbacks.
const MAX_SCROLLBACK: usize = 5_000;

/// Client-to-server message types.
#[derive(Debug, serde::Deserialize)]
struct ClientMessage {
    #[serde(rename = "type")]
    msg_type: String,
    /// New mode for "mode" messages: "raw" or "rich".
    #[serde(default)]
    mode: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    cols: Option<u32>,
    #[allow(dead_code)]
    #[serde(default)]
    rows: Option<u32>,
}

/// Events from the file watcher, bridged into async.
#[derive(Debug)]
enum WatchEvent {
    Modified,
    Error(String),
}

/// Pre-compiled SIMD substring finders for rich mode JSONL parsing.
/// Created once per WebSocket connection, reused across all lines.
/// Per CLAUDE.md: never create a Finder inside a per-line function.
///
/// Only finders that gate a code path are kept here. The refactored
/// `format_line_for_mode` does a full JSON parse for content extraction,
/// so content_key/tool_use/tool_result/name_key finders were removed.
struct RichModeFinders {
    type_key: memmem::Finder<'static>,
    role_key: memmem::Finder<'static>,
    timestamp_key: memmem::Finder<'static>,
}

impl RichModeFinders {
    fn new() -> Self {
        Self {
            type_key: memmem::Finder::new(b"\"type\""),
            role_key: memmem::Finder::new(b"\"role\""),
            timestamp_key: memmem::Finder::new(b"\"timestamp\""),
        }
    }
}

/// Strip Claude Code internal command tags from content.
///
/// Removes matched pairs of tags like `<command-name>...</command-name>` that
/// Claude Code injects for internal command routing. These are noise in the
/// terminal monitor and should never reach the WebSocket stream.
fn strip_command_tags(content: &str) -> String {
    let mut result = content.to_string();
    let tags = [
        "command-name",
        "command-message",
        "command-args",
        "local-command-stdout",
        "system-reminder",
    ];

    for tag in &tags {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");

        // Loop until no more opening tags are found
        while let Some(start) = result.find(&open) {
            // Search for closing tag AFTER the opening tag position
            match result[start..].find(&close) {
                Some(offset) => {
                    let end = start + offset + close.len();
                    result.replace_range(start..end, "");
                }
                None => {
                    // No closing tag found — break to avoid infinite loop
                    break;
                }
            }
        }
    }
    result.trim().to_string()
}

/// Format a JSONL line for sending over WebSocket.
///
/// - In "raw" mode: wraps the line in `{ "type": "line", "data": "..." }`.
/// - In "rich" mode: SIMD pre-filters the line, parses as JSON, and extracts
///   structured fields (type/role/content/tool names/timestamp). Returns an
///   empty vec for lines that shouldn't be displayed (progress events, metadata,
///   empty messages). Returns multiple messages when a single JSONL line contains
///   multiple content blocks (e.g., thinking + text, or multiple tool_use calls).
fn format_line_for_mode(line: &str, mode: &str, finders: &RichModeFinders) -> Vec<String> {
    if mode != "rich" {
        // Raw mode: send as-is
        let msg = serde_json::json!({
            "type": "line",
            "data": line,
        });
        return vec![msg.to_string()];
    }

    // Rich mode: SIMD pre-filter before JSON parse
    let line_bytes = line.as_bytes();

    // Quick check: does the line even look like it has a "type" key?
    if finders.type_key.find(line_bytes).is_none() {
        return vec![]; // No "type" key — not a structured message
    }

    // Parse as JSON
    let parsed: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return vec![], // JSON parse failed — skip in rich mode
    };

    // Extract the top-level "type" field (e.g., "assistant", "user", "system")
    let line_type = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Skip non-displayable types (noise in the session log)
    match line_type {
        "progress" | "file-history-snapshot" => return vec![],
        _ => {}
    }

    // Skip meta messages (internal system prompts, skill loading, etc.)
    if parsed.get("isMeta").and_then(|v| v.as_bool()) == Some(true) {
        return vec![];
    }

    // The nested "message" object (Claude Code JSONL wraps fields under "message")
    let msg_obj = parsed.get("message");

    // Extract role from top-level or nested message
    let role = if finders.role_key.find(line_bytes).is_some() {
        parsed
            .get("role")
            .or_else(|| msg_obj.and_then(|m| m.get("role")))
            .and_then(|v| v.as_str())
    } else {
        None
    };

    // Extract timestamp
    let timestamp = if finders.timestamp_key.find(line_bytes).is_some() {
        parsed.get("timestamp").and_then(|v| v.as_str())
    } else {
        None
    };

    // Resolve content source (top-level or nested message)
    let content_source = if parsed.get("content").is_some() {
        Some(&parsed)
    } else {
        msg_obj
    };

    // For plain string content, return a single message
    if let Some(src) = content_source {
        if let Some(serde_json::Value::String(s)) = src.get("content") {
            let stripped = strip_command_tags(s);
            if stripped.is_empty() {
                return vec![];
            }
            let mut result = serde_json::json!({
                "type": "message",
                "role": role.unwrap_or(line_type),
                "content": stripped,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
    }

    // For array content, extract ALL blocks — no truncation, no dropping.
    let blocks = content_source
        .and_then(|src| src.get("content"))
        .and_then(|c| c.as_array());

    let blocks = match blocks {
        Some(b) => b,
        None => return vec![],
    };

    let mut results: Vec<String> = Vec::new();

    // Collect ALL thinking text (concatenated)
    let mut thinking_parts: Vec<&str> = Vec::new();
    // Collect ALL text blocks (concatenated)
    let mut text_parts: Vec<&str> = Vec::new();

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match block_type {
            "thinking" => {
                if let Some(thinking) = block.get("thinking").and_then(|t| t.as_str()) {
                    thinking_parts.push(thinking);
                }
            }
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            "tool_use" => {
                let tool_name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let mut result = serde_json::json!({
                    "type": "tool_use",
                    "name": tool_name,
                    "input": input,
                });
                if let Some(ts) = timestamp {
                    result["ts"] = serde_json::Value::String(ts.to_string());
                }
                results.push(result.to_string());
            }
            "tool_result" => {
                let content = block
                    .get("content")
                    .map(|c| match c {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let mut result = serde_json::json!({
                    "type": "tool_result",
                    "content": content,
                });
                if let Some(ts) = timestamp {
                    result["ts"] = serde_json::Value::String(ts.to_string());
                }
                results.push(result.to_string());
            }
            _ => {}
        }
    }

    // Emit concatenated thinking (all thinking blocks joined)
    if !thinking_parts.is_empty() {
        let full_thinking = thinking_parts.join("\n");
        let mut result = serde_json::json!({
            "type": "thinking",
            "content": full_thinking,
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        results.push(result.to_string());
    }

    // Emit concatenated text (all text blocks joined), with command tags stripped
    if !text_parts.is_empty() {
        let full_text = text_parts.join("\n");
        let stripped = strip_command_tags(&full_text);
        if !stripped.is_empty() {
            let mut result = serde_json::json!({
                "type": "message",
                "role": role.unwrap_or(line_type),
                "content": stripped,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            results.push(result.to_string());
        }
    }

    results
}

/// Core WebSocket handler -- manages the full lifecycle:
/// 1. Wait for handshake
/// 2. Send scrollback buffer
/// 3. Stream live updates via file watcher
/// 4. Handle client messages and heartbeat
async fn handle_terminal_ws(
    mut socket: WebSocket,
    session_id: String,
    file_path: PathBuf,
    _terminal_connections: Arc<crate::terminal_state::TerminalConnectionManager>,
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

    // 3. Send scrollback buffer using tail_lines (capped to MAX_SCROLLBACK)
    let scrollback_count = handshake.scrollback.min(MAX_SCROLLBACK);
    match vibe_recall_core::tail::tail_lines(&file_path, scrollback_count).await {
        Ok(lines) => {
            for line in &lines {
                for formatted in format_line_for_mode(line, &current_mode, &finders) {
                    if socket.send(Message::Text(formatted.into())).await.is_err() {
                        return; // client disconnected
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
                return;
            }
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
            return;
        }
    }

    // 4. Set up file watcher for live streaming
    let (watch_tx, mut watch_rx) = mpsc::channel::<WatchEvent>(64);

    // Create a file position tracker starting at the current end of file
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
    let watched_path = file_path.clone();
    let _watcher: RecommendedWatcher = match start_file_watcher(watch_tx, &watched_path) {
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
        scrollback = scrollback_count,
        "Terminal WebSocket connected"
    );

    // 5. Main event loop: multiplex file watcher, client messages, and heartbeat
    //
    // Heartbeat is 10s (not 30s) to detect dead connections fast — during dev
    // hot reloads, the browser drops TCP without a close frame, and stale
    // connections linger until the next failed send. WebSocket Ping frames
    // (not application-level JSON) are used because they're handled at the
    // protocol level and fail faster for broken TCP connections.
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
    // Skip the first immediate tick
    heartbeat_interval.tick().await;

    loop {
        tokio::select! {
            // File watcher events
            watch_event = watch_rx.recv() => {
                match watch_event {
                    Some(WatchEvent::Modified) => {
                        // Read new lines from the file
                        match tracker.read_new_lines().await {
                            Ok(lines) => {
                                for line in &lines {
                                    for formatted in format_line_for_mode(line, &current_mode, &finders) {
                                        if socket
                                            .send(Message::Text(formatted.into()))
                                            .await
                                            .is_err()
                                        {
                                            tracing::debug!(
                                                session_id = %session_id,
                                                "Client disconnected during live stream"
                                            );
                                            return; // watcher dropped on return
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    session_id = %session_id,
                                    error = %e,
                                    "Failed to read new lines"
                                );
                                // Don't close -- the file might recover
                            }
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
                        // Parse client message
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(msg) => {
                                match msg.msg_type.as_str() {
                                    "ping" => {
                                        let pong = serde_json::json!({ "type": "pong" });
                                        let _ = socket
                                            .send(Message::Text(pong.to_string().into()))
                                            .await;
                                    }
                                    "mode" => {
                                        // Update the display mode mid-stream
                                        if let Some(new_mode) = &msg.mode {
                                            if new_mode == "raw" || new_mode == "rich" {
                                                current_mode = new_mode.clone();
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
                                }
                            }
                            Err(_) => {
                                tracing::debug!(
                                    session_id = %session_id,
                                    "Malformed client message, ignoring"
                                );
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!(
                            session_id = %session_id,
                            "Terminal WebSocket disconnected"
                        );
                        return; // watcher dropped on return
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

            // Heartbeat: WebSocket Ping frame (protocol-level, not app-level JSON).
            // Fails fast on broken TCP connections — stale connections are cleaned
            // up within 10s instead of lingering until the next data send.
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
        }
    }
    // _watcher is dropped here, stopping the file watch
}

/// Start a notify watcher for a single JSONL file.
///
/// Watches the file's parent directory (notify cannot watch individual files
/// on all platforms) and filters events to only the target file.
/// Modified events are sent through the `mpsc::Sender<WatchEvent>` channel.
fn start_file_watcher(
    tx: mpsc::Sender<WatchEvent>,
    file_path: &PathBuf,
) -> notify::Result<RecommendedWatcher> {
    let target_path = file_path.clone();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Filter to only events for our target file
                    let is_target = event.paths.iter().any(|p| p == &target_path);
                    if !is_target {
                        return;
                    }

                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            // Best-effort send; if the channel is full, skip this event
                            // (the next modify event will pick up all new lines)
                            let _ = tx.try_send(WatchEvent::Modified);
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    let _ = tx.try_send(WatchEvent::Error(e.to_string()));
                }
            }
        })?;

    // Watch the parent directory since notify may not support watching
    // individual files on all platforms (e.g., macOS FSEvents)
    let watch_dir = file_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;

    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use std::io::Write;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite;

    /// Helper: create an AppState with an in-memory database and a live session
    /// registered pointing to the given JSONL file path.
    async fn test_state_with_session(session_id: &str, file_path: &str) -> Arc<AppState> {
        let db = vibe_recall_db::Database::new_in_memory().await.unwrap();
        let state = Arc::new(AppState {
            start_time: std::time::Instant::now(),
            db,
            indexing: Arc::new(crate::IndexingState::new()),
            registry: Arc::new(std::sync::RwLock::new(None)),
            jobs: Arc::new(crate::jobs::JobRunner::new()),
            classify: Arc::new(crate::classify_state::ClassifyState::new()),
            facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
            git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
            pricing: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            live_tx: tokio::sync::broadcast::channel(256).0,

            rules_dir: std::env::temp_dir().join("claude-rules-test"),
            terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
            live_manager: None,
            search_index: None,
        });

        // Register the session in the live sessions map
        {
            use vibe_recall_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};
            let mut map = state.live_sessions.write().await;
            let session = crate::live::state::LiveSession {
                id: session_id.to_string(),
                project: "test-project".to_string(),
                project_display_name: "test-project".to_string(),
                project_path: "/tmp/test-project".to_string(),
                file_path: file_path.to_string(),
                status: crate::live::state::SessionStatus::Working,
                agent_state: crate::live::state::AgentState {
                    group: crate::live::state::AgentStateGroup::Autonomous,
                    state: "working".to_string(),
                    label: "Working".to_string(),
                    context: None,
                },
                git_branch: None,
                pid: None,
                title: "Test session".to_string(),
                last_user_message: "test".to_string(),
                current_activity: "testing".to_string(),
                turn_count: 0,
                started_at: None,
                last_activity_at: 0,
                model: None,
                tokens: TokenUsage::default(),
                context_window_tokens: 0,
                cost: CostBreakdown::default(),
                cache_status: CacheStatus::Unknown,
                current_turn_started_at: None,
                last_turn_task_seconds: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                last_cache_hit_at: None,
            };
            map.insert(session_id.to_string(), session);
        }

        state
    }

    /// Helper: start an Axum server on a random port, returning the bound address.
    /// The server runs as a background task that is cancelled when the returned
    /// `JoinHandle` is aborted.
    async fn start_test_server(
        state: Arc<AppState>,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let app = Router::new().nest("/api/live", router()).with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (addr, handle)
    }

    /// Helper: connect a WebSocket client to the test server.
    async fn ws_connect(
        addr: std::net::SocketAddr,
        session_id: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let url = format!(
            "ws://127.0.0.1:{}/api/live/sessions/{}/terminal",
            addr.port(),
            session_id
        );
        let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();
        ws_stream
    }

    /// Helper: receive a text message with a timeout.
    async fn recv_text(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Option<String> {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Ok(Some(Ok(tungstenite::Message::Text(text)))) => Some(text.to_string()),
            _ => None,
        }
    }

    /// Helper: receive text messages until we find one matching the given type.
    async fn recv_until_type(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        target_type: &str,
    ) -> Option<serde_json::Value> {
        for _ in 0..50 {
            if let Some(text) = recv_text(ws).await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("type").and_then(|t| t.as_str()) == Some(target_type) {
                        return Some(v);
                    }
                }
            } else {
                return None;
            }
        }
        None
    }

    // =========================================================================
    // Test 1: ws_upgrade_returns_101
    // =========================================================================

    #[tokio::test]
    async fn ws_upgrade_returns_101() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
        )
        .unwrap();

        let state = test_state_with_session("test-ws-upgrade", tmp.path().to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;

        // Connecting successfully means we got a 101 Switching Protocols response.
        // tokio-tungstenite would error if the upgrade failed.
        let mut ws = ws_connect(addr, "test-ws-upgrade").await;

        // Send handshake and verify we get a response
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"raw","scrollback":10}"#.into(),
        ))
        .await
        .unwrap();

        // Should receive at least one message (scrollback line or buffer_end)
        let msg = recv_text(&mut ws).await;
        assert!(
            msg.is_some(),
            "Expected at least one message after handshake"
        );

        ws.close(None).await.ok();
        server_handle.abort();
    }

    // =========================================================================
    // Test 2: ws_unknown_session_returns_error
    // =========================================================================

    #[tokio::test]
    async fn ws_unknown_session_returns_error() {
        // Create state with NO sessions registered
        let db = vibe_recall_db::Database::new_in_memory().await.unwrap();
        let state = Arc::new(AppState {
            start_time: std::time::Instant::now(),
            db,
            indexing: Arc::new(crate::IndexingState::new()),
            registry: Arc::new(std::sync::RwLock::new(None)),
            jobs: Arc::new(crate::jobs::JobRunner::new()),
            classify: Arc::new(crate::classify_state::ClassifyState::new()),
            facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
            git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
            pricing: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            live_tx: tokio::sync::broadcast::channel(256).0,

            rules_dir: std::env::temp_dir().join("claude-rules-test"),
            terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
            live_manager: None,
            search_index: None,
        });

        let (addr, server_handle) = start_test_server(state).await;
        let mut ws = ws_connect(addr, "nonexistent-session-id").await;

        // The server should send an error message and close
        let msg = recv_text(&mut ws).await;
        assert!(msg.is_some(), "Expected error message");

        let parsed: serde_json::Value = serde_json::from_str(&msg.unwrap()).unwrap();
        assert_eq!(parsed["type"], "error");
        assert!(
            parsed["message"].as_str().unwrap().contains("not found"),
            "Error message should mention 'not found'"
        );

        // Should receive a close frame next
        match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
            Ok(Some(Ok(tungstenite::Message::Close(frame)))) => {
                if let Some(cf) = frame {
                    assert_eq!(
                        cf.code,
                        tungstenite::protocol::frame::coding::CloseCode::from(4004)
                    );
                }
            }
            _ => {
                // Connection may already be closed -- that's acceptable
            }
        }

        server_handle.abort();
    }

    // =========================================================================
    // Test 3: ws_initial_buffer_sent
    // =========================================================================

    #[tokio::test]
    async fn ws_initial_buffer_sent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Write 3 JSONL lines
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","message":{{"role":"user","content":"line 1"}}}}"#
        )
        .unwrap();
        writeln!(
            tmp.as_file(),
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"line 2"}}}}"#
        )
        .unwrap();
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","message":{{"role":"user","content":"line 3"}}}}"#
        )
        .unwrap();
        tmp.as_file().flush().unwrap();

        let state = test_state_with_session("test-buffer", tmp.path().to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;

        let mut ws = ws_connect(addr, "test-buffer").await;

        // Send handshake requesting all 3 scrollback lines
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"raw","scrollback":10}"#.into(),
        ))
        .await
        .unwrap();

        // Collect messages until buffer_end
        let mut lines = Vec::new();
        let mut found_buffer_end = false;
        for _ in 0..20 {
            if let Some(text) = recv_text(&mut ws).await {
                let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
                match parsed["type"].as_str() {
                    Some("line") => lines.push(parsed),
                    Some("buffer_end") => {
                        found_buffer_end = true;
                        break;
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }

        assert!(found_buffer_end, "Expected buffer_end marker");
        assert_eq!(
            lines.len(),
            3,
            "Expected 3 scrollback lines, got {}",
            lines.len()
        );

        // Verify lines contain the original data
        assert!(lines[0]["data"].as_str().unwrap().contains("line 1"));
        assert!(lines[1]["data"].as_str().unwrap().contains("line 2"));
        assert!(lines[2]["data"].as_str().unwrap().contains("line 3"));

        ws.close(None).await.ok();
        server_handle.abort();
    }

    // =========================================================================
    // Test 4: ws_live_lines_streamed
    // =========================================================================

    #[tokio::test]
    async fn ws_live_lines_streamed() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Write initial content
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","message":{{"role":"user","content":"initial"}}}}"#
        )
        .unwrap();
        tmp.as_file().flush().unwrap();

        let state = test_state_with_session("test-live", tmp.path().to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;

        let mut ws = ws_connect(addr, "test-live").await;

        // Send handshake
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"raw","scrollback":10}"#.into(),
        ))
        .await
        .unwrap();

        // Wait for buffer_end
        let _buffer_end = recv_until_type(&mut ws, "buffer_end").await;
        assert!(_buffer_end.is_some(), "Expected buffer_end");

        // Append new lines to the file in a loop to reliably trigger the
        // file watcher (macOS FSEvents can batch/coalesce events).
        let path = tmp.path().to_path_buf();
        let write_path = path.clone();
        let write_handle = tokio::spawn(async move {
            // Write the target line, then keep poking the file to ensure
            // the watcher fires. On macOS FSEvents, a single small write
            // may not reliably trigger a notification within the test timeout.
            for i in 0..10 {
                {
                    let mut f = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&write_path)
                        .unwrap();
                    if i == 0 {
                        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":"live response"}}}}"#).unwrap();
                    } else {
                        writeln!(f, r#"{{"type":"system","message":{{"role":"system","content":"poke {i}"}}}}"#).unwrap();
                    }
                    f.flush().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        });

        // Wait for the live line to arrive (file watcher + debounce delay)
        let live_msg = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(text) = recv_text(&mut ws).await {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        if v["type"] == "line"
                            && v["data"].as_str().unwrap_or("").contains("live response")
                        {
                            return Some(v);
                        }
                    }
                } else {
                    return None;
                }
            }
        })
        .await;

        write_handle.abort();

        assert!(
            live_msg.is_ok() && live_msg.unwrap().is_some(),
            "Expected live streamed line containing 'live response'"
        );

        ws.close(None).await.ok();
        server_handle.abort();
    }

    // =========================================================================
    // Test 5: ws_disconnect_drops_watcher
    // =========================================================================

    #[tokio::test]
    async fn ws_disconnect_drops_watcher() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
        )
        .unwrap();
        tmp.as_file().flush().unwrap();

        let state = test_state_with_session("test-disconnect", tmp.path().to_str().unwrap()).await;
        let terminal_connections = state.terminal_connections.clone();

        let (addr, server_handle) = start_test_server(state).await;

        // Connect and do handshake
        let mut ws = ws_connect(addr, "test-disconnect").await;
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"raw","scrollback":1}"#.into(),
        ))
        .await
        .unwrap();

        // Wait for buffer_end
        let _ = recv_until_type(&mut ws, "buffer_end").await;

        // Verify connection is tracked
        // Allow a small delay for the server to register the connection
        tokio::time::sleep(Duration::from_millis(100)).await;
        let count_before = terminal_connections.viewer_count("test-disconnect");
        assert_eq!(count_before, 1, "Expected 1 viewer before disconnect");

        // Disconnect
        ws.close(None).await.ok();

        // Wait for the server to process the disconnect
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify connection count decremented
        let count_after = terminal_connections.viewer_count("test-disconnect");
        assert_eq!(count_after, 0, "Expected 0 viewers after disconnect");

        server_handle.abort();
    }

    // =========================================================================
    // Test 6: ws_mode_switch
    // =========================================================================

    #[tokio::test]
    async fn ws_mode_switch() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Write a JSONL line with structured content
        writeln!(
            tmp.as_file(),
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"initial data"}}}}"#
        )
        .unwrap();
        tmp.as_file().flush().unwrap();

        let state = test_state_with_session("test-mode-switch", tmp.path().to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;

        let mut ws = ws_connect(addr, "test-mode-switch").await;

        // Start in raw mode
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"raw","scrollback":1}"#.into(),
        ))
        .await
        .unwrap();

        // Wait for buffer_end -- the scrollback lines should be in raw format
        let mut raw_lines = Vec::new();
        for _ in 0..10 {
            if let Some(text) = recv_text(&mut ws).await {
                let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
                match parsed["type"].as_str() {
                    Some("line") => {
                        // Raw mode: should have "data" field with the original line
                        assert!(
                            parsed.get("data").is_some(),
                            "Raw mode should have 'data' field"
                        );
                        raw_lines.push(parsed);
                    }
                    Some("buffer_end") => break,
                    _ => {}
                }
            }
        }
        assert!(
            !raw_lines.is_empty(),
            "Should have received at least 1 raw line"
        );

        // Switch to rich mode
        ws.send(tungstenite::Message::Text(
            r#"{"type":"mode","mode":"rich"}"#.into(),
        ))
        .await
        .unwrap();

        // Small delay to let the mode switch be processed
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Append new lines to reliably trigger the file watcher
        let path = tmp.path().to_path_buf();
        let write_path = path.clone();
        let write_handle = tokio::spawn(async move {
            for i in 0..10 {
                {
                    let mut f = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&write_path)
                        .unwrap();
                    if i == 0 {
                        writeln!(
                            f,
                            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"rich content here"}}]}},"timestamp":"2026-01-15T10:30:00Z"}}"#
                        )
                        .unwrap();
                    } else {
                        writeln!(f, r#"{{"type":"system","message":{{"role":"system","content":"poke {i}"}}}}"#).unwrap();
                    }
                    f.flush().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        });

        // Wait for the rich-mode message
        let rich_msg = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(text) = recv_text(&mut ws).await {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        // In rich mode, the type should be "message" (not "line")
                        if v["type"] == "message" {
                            return Some(v);
                        }
                    }
                } else {
                    return None;
                }
            }
        })
        .await;

        write_handle.abort();

        assert!(rich_msg.is_ok(), "Timed out waiting for rich mode message");
        let rich_msg = rich_msg.unwrap();
        assert!(rich_msg.is_some(), "Expected rich mode message");

        let msg = rich_msg.unwrap();
        assert_eq!(msg["type"], "message");
        assert_eq!(msg["role"], "assistant");
        assert!(
            msg.get("content").is_some(),
            "Rich mode message should have content"
        );
        assert_eq!(msg["ts"], "2026-01-15T10:30:00Z");

        ws.close(None).await.ok();
        server_handle.abort();
    }

    // =========================================================================
    // Unit tests for format_line_for_mode
    // =========================================================================

    #[test]
    fn format_line_raw_mode() {
        let finders = RichModeFinders::new();
        let results = format_line_for_mode("some raw line", "raw", &finders);
        assert_eq!(results.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "line");
        assert_eq!(parsed["data"], "some raw line");
    }

    #[test]
    fn format_line_rich_mode_assistant_message() {
        let finders = RichModeFinders::new();
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":"Hello world"},"timestamp":"2026-01-15T10:30:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert_eq!(results.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "message");
        assert_eq!(parsed["role"], "assistant");
        assert_eq!(parsed["content"], "Hello world");
        assert_eq!(parsed["ts"], "2026-01-15T10:30:00Z");
    }

    #[test]
    fn format_line_rich_mode_tool_use() {
        let finders = RichModeFinders::new();
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","id":"123","input":{"path":"src/main.rs"}}]}}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert_eq!(results.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "tool_use");
        assert_eq!(parsed["name"], "Read");
        assert_eq!(parsed["input"]["path"], "src/main.rs");
    }

    #[test]
    fn format_line_rich_mode_invalid_json_skipped() {
        let finders = RichModeFinders::new();
        // Has "type" substring but isn't valid JSON — should be skipped in rich mode
        let line = r#"this is not json but has "type" in it"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert!(
            result.is_empty(),
            "Invalid JSON should be skipped in rich mode"
        );
    }

    #[test]
    fn format_line_rich_mode_no_type_key_skipped() {
        let finders = RichModeFinders::new();
        // Valid JSON but no "type" key — skipped in rich mode
        let line = r#"{"role":"user","content":"hello"}"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert!(
            result.is_empty(),
            "Line without type key should be skipped in rich mode"
        );
    }

    #[test]
    fn format_line_rich_mode_progress_skipped() {
        let finders = RichModeFinders::new();
        let line =
            r#"{"type":"progress","data":{"type":"hook_progress","hookEvent":"SessionStart"}}"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert!(
            result.is_empty(),
            "Progress events should be skipped in rich mode"
        );
    }

    #[test]
    fn format_line_rich_mode_meta_skipped() {
        let finders = RichModeFinders::new();
        let line =
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"system prompt"}}"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert!(
            result.is_empty(),
            "Meta messages should be skipped in rich mode"
        );
    }

    #[test]
    fn format_line_rich_mode_thinking_extracted() {
        let finders = RichModeFinders::new();
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze this...","signature":"abc"}]},"timestamp":"2026-01-15T10:30:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert_eq!(results.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "thinking");
        assert!(parsed["content"]
            .as_str()
            .unwrap()
            .contains("Let me analyze this"));
        assert_eq!(parsed["ts"], "2026-01-15T10:30:00Z");
    }

    #[test]
    fn format_line_rich_mode_no_content_skipped() {
        let finders = RichModeFinders::new();
        // Has type but no extractable content
        let line = r#"{"type":"assistant","message":{"role":"assistant"}}"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert!(
            result.is_empty(),
            "Messages without content should be skipped"
        );
    }

    #[test]
    fn format_line_rich_mode_multiple_content_blocks() {
        let finders = RichModeFinders::new();
        // A message with thinking + text + tool_use — all blocks should be extracted
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"reasoning here"},{"type":"text","text":"Part 1"},{"type":"text","text":"Part 2"},{"type":"tool_use","name":"Read","id":"1","input":{"file":"a.rs"}},{"type":"tool_use","name":"Write","id":"2","input":{"file":"b.rs"}}]},"timestamp":"2026-02-16T00:00:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        // Should produce: 2 tool_use + 1 thinking + 1 text (concatenated) = 4 messages
        assert_eq!(
            results.len(),
            4,
            "Expected 4 messages, got {}: {:?}",
            results.len(),
            results
        );

        // Check tool_use messages
        let tool1: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(tool1["type"], "tool_use");
        assert_eq!(tool1["name"], "Read");
        let tool2: serde_json::Value = serde_json::from_str(&results[1]).unwrap();
        assert_eq!(tool2["type"], "tool_use");
        assert_eq!(tool2["name"], "Write");

        // Check thinking (concatenated)
        let thinking: serde_json::Value = serde_json::from_str(&results[2]).unwrap();
        assert_eq!(thinking["type"], "thinking");
        assert_eq!(thinking["content"], "reasoning here");

        // Check text (concatenated from Part 1 + Part 2)
        let text: serde_json::Value = serde_json::from_str(&results[3]).unwrap();
        assert_eq!(text["type"], "message");
        assert!(text["content"].as_str().unwrap().contains("Part 1"));
        assert!(text["content"].as_str().unwrap().contains("Part 2"));
    }

    // =========================================================================
    // Unit tests for strip_command_tags
    // =========================================================================

    #[test]
    fn strip_command_tags_removes_all_known_tags() {
        let input = r#"<command-name>/clear</command-name>
<command-message>clear</command-message>
<command-args></command-args>

NaN ago
<local-command-stdout></local-command-stdout>"#;
        let result = strip_command_tags(input);
        assert!(!result.contains("<command-name>"));
        assert!(!result.contains("<local-command-stdout>"));
        // After stripping all tags and trimming, only "NaN ago" should remain
        assert_eq!(result, "NaN ago");
    }

    #[test]
    fn strip_command_tags_preserves_normal_content() {
        let input = "Here is a table:\n\n| Col1 | Col2 |\n|------|------|\n| a    | b    |";
        let result = strip_command_tags(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_command_tags_handles_missing_close_tag() {
        let input = "<command-name>unclosed content but normal text after";
        let result = strip_command_tags(input);
        // Should not infinite loop; returns input unchanged since no closing tag
        assert_eq!(result, input);
    }

    #[test]
    fn strip_command_tags_empty_after_stripping_skips_string_content() {
        let finders = RichModeFinders::new();
        // Content is entirely command tags — should produce empty vec after stripping
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":"<command-name>/clear</command-name><command-args></command-args>"},"timestamp":"2026-02-16T00:00:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert!(
            results.is_empty(),
            "Messages that become empty after tag stripping should not be emitted"
        );
    }

    #[test]
    fn strip_command_tags_empty_after_stripping_skips_text_blocks() {
        let finders = RichModeFinders::new();
        // Content is array with a text block that is entirely command tags
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"<command-name>/clear</command-name>"}]},"timestamp":"2026-02-16T00:00:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert!(
            results.is_empty(),
            "Text blocks that become empty after tag stripping should not be emitted"
        );
    }
}
