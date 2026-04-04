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

use claude_view_core::category::{categorize_progress, categorize_tool};
use claude_view_core::hook_to_block::make_hook_progress_block;

use crate::state::AppState;

/// Resolve the JSONL file path for a session.
///
/// Checks (in order): live sessions → recently closed → database.
/// This allows terminal WebSocket handlers to serve both live and historical
/// sessions — not just the ones currently in the `live_sessions` map.
async fn resolve_jsonl_path(state: &AppState, session_id: &str) -> Option<PathBuf> {
    // 1. Live sessions (in-memory, actively monitored)
    {
        let map = state.live_sessions.read().await;
        if let Some(fp) = map.get(session_id).map(|s| s.jsonl.file_path.clone()) {
            if !fp.is_empty() {
                return Some(PathBuf::from(fp));
            }
        }
    }

    // 2. Recently closed (ephemeral, post-reap)
    {
        let map = state.recently_closed.read().await;
        if let Some(fp) = map.get(session_id).map(|s| s.jsonl.file_path.clone()) {
            if !fp.is_empty() {
                return Some(PathBuf::from(fp));
            }
        }
    }

    // 3. Database (historical sessions)
    if let Ok(Some(fp)) = state.db.get_session_file_path(session_id).await {
        let path = PathBuf::from(&fp);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

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
        // Multiplexed WS — Stage 1+4. Carries block + raw + sdk + hook events
        // over a single connection with typed frames.
        .route(
            "/sessions/{id}/ws",
            get(crate::live::session_ws::handler::ws_session_handler),
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
async fn ws_subagent_terminal_handler(
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
pub(crate) enum WatchEvent {
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
    if mode == "block" {
        // Block mode: parse via BlockAccumulator, return ConversationBlock JSON.
        // Per-line accumulator works for independent lines (user, progress, system).
        // Multi-line constructs (AssistantBlock spanning assistant + tool_result)
        // produce separate blocks per line — the frontend stream accumulator handles assembly.
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            let mut acc = claude_view_core::block_accumulator::BlockAccumulator::new();
            acc.process_line(&entry);
            let blocks = acc.finalize();
            return blocks
                .into_iter()
                .filter_map(|block| serde_json::to_string(&block).ok())
                .collect();
        }
        return vec![];
    }

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

    // Extract timestamp early so all match arms can use it
    let timestamp = if finders.timestamp_key.find(line_bytes).is_some() {
        parsed.get("timestamp").and_then(|v| v.as_str())
    } else {
        None
    };

    // Handle structured types with categories
    match line_type {
        "progress" => {
            let data = parsed.get("data");
            let data_type = data
                .and_then(|d| d.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");

            let category = categorize_progress(data_type);

            let hook_name = data
                .and_then(|d| d.get("hookName"))
                .and_then(|v| v.as_str());
            let command = data.and_then(|d| d.get("command")).and_then(|v| v.as_str());
            let content = if let Some(hn) = hook_name {
                format!("{}: {}", data_type, hn)
            } else if let Some(cmd) = command {
                format!("{}: {}", data_type, cmd)
            } else {
                data_type.to_string()
            };

            let mut result = serde_json::json!({
                "type": "progress",
                "content": content,
                "metadata": data,
            });
            if let Some(cat) = category {
                result["category"] = serde_json::Value::String(cat.to_string());
            }
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
        "file-history-snapshot" => {
            let mut result = serde_json::json!({
                "type": "system",
                "content": "file-history-snapshot",
                "category": "snapshot",
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
        "system" => {
            let subtype = parsed
                .get("subtype")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let duration_ms = parsed.get("durationMs").and_then(|v| v.as_u64());
            let content = if let Some(ms) = duration_ms {
                format!("{}: {}ms", subtype, ms)
            } else {
                subtype.to_string()
            };
            let mut result = serde_json::json!({
                "type": "system",
                "content": content,
                "category": "system",
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
        "queue-operation" => {
            let operation = parsed
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let op_content = parsed.get("content").and_then(|v| v.as_str());

            let content = if let Some(c) = op_content {
                format!("queue-{}: {}", operation, c)
            } else {
                format!("queue-{}", operation)
            };

            let mut metadata = serde_json::json!({
                "type": "queue-operation",
                "operation": operation,
            });
            if let Some(c) = op_content {
                metadata["content"] = serde_json::Value::String(c.to_string());
            }

            let mut result = serde_json::json!({
                "type": "system",
                "content": content,
                "category": "queue",
                "metadata": metadata,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
        "summary" => {
            let summary_text = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let mut result = serde_json::json!({
                "type": "summary",
                "content": summary_text,
            });
            if let Some(ts) = timestamp {
                result["ts"] = serde_json::Value::String(ts.to_string());
            }
            return vec![result.to_string()];
        }
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
    // Track the last tool category so tool_result can inherit it
    let mut last_tool_category: Option<&str> = None;

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
                let category = categorize_tool(tool_name);
                last_tool_category = Some(category);
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let mut result = serde_json::json!({
                    "type": "tool_use",
                    "name": tool_name,
                    "input": input,
                    "category": category,
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
                if let Some(cat) = last_tool_category {
                    result["category"] = serde_json::Value::String(cat.to_string());
                }
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

    // 3. Send scrollback buffer using tail_lines (capped to MAX_SCROLLBACK)
    let scrollback_count = handshake.scrollback.min(MAX_SCROLLBACK);
    match claude_view_core::tail::tail_lines(&file_path, scrollback_count).await {
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

    // 3b. Send buffered hook events from in-memory LiveSession
    {
        let sessions = state.live_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
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
                    return;
                }
            }
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
    let _watcher: RecommendedWatcher = match start_file_watcher(&watched_path, watch_tx) {
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
    let mut hook_rx = {
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
    let mut live_hook_counter: usize = 0;

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
                                if current_mode == "block" {
                                    // Reset accumulator on file truncation (compaction).
                                    if tracker.was_truncated() {
                                        block_acc.reset();
                                        sent_blocks.clear();
                                    }
                                    // Block mode: feed lines into the persistent accumulator,
                                    // then send only new/changed blocks. This correctly
                                    // correlates incremental assistant entries (same message.id,
                                    // different content blocks) that the CC CLI writes as
                                    // separate JSONL lines during streaming.
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
                                                if socket
                                                    .send(Message::Text(json.into()))
                                                    .await
                                                    .is_err()
                                                {
                                                    tracing::debug!(
                                                        session_id = %session_id,
                                                        "Client disconnected during live stream"
                                                    );
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                } else {
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

            // Hook event broadcasts from handle_hook()
            hook_event = hook_rx.recv() => {
                match hook_event {
                    Ok(event) => {
                        live_hook_counter += 1;
                        let text = if current_mode == "block" {
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
    // _watcher is dropped here, stopping the file watch
}

/// Start a notify watcher for a single JSONL file.
///
/// Watches the file's parent directory (notify cannot watch individual files
/// on all platforms) and filters events to only the target file.
/// Modified events are sent through the `mpsc::Sender<WatchEvent>` channel.
pub(crate) fn start_file_watcher(
    file_path: &std::path::Path,
    tx: mpsc::Sender<WatchEvent>,
) -> notify::Result<RecommendedWatcher> {
    // Canonicalize the target path so that the comparison against event paths
    // works on macOS where symlinks like /var -> /private/var cause mismatches
    // (e.g. NamedTempFile returns /var/folders/... but FSEvents reports
    // /private/var/folders/...).
    let canonical_path = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    let target_for_closure = canonical_path.clone();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Filter to only events for our target file
                    let is_target = event.paths.iter().any(|p| p == &target_for_closure);
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
    // individual files on all platforms (e.g., macOS FSEvents).
    // Use the canonical path's parent so the watched directory
    // matches the resolved event paths.
    let watch_dir = canonical_path
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
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = Arc::new(AppState {
            start_time: std::time::Instant::now(),
            db,
            indexing: Arc::new(crate::IndexingState::new()),
            registry: Arc::new(std::sync::RwLock::new(None)),
            jobs: Arc::new(crate::jobs::JobRunner::new()),
            classify: Arc::new(crate::classify_state::ClassifyState::new()),
            facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
            git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
            pricing: Arc::new(std::collections::HashMap::new()),
            live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            recently_closed: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            live_tx: tokio::sync::broadcast::channel(256).0,

            rules_dir: std::env::temp_dir().join("claude-rules-test"),
            terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
            live_manager: None,
            search_index: Arc::new(std::sync::RwLock::new(None)),
            shutdown: tokio::sync::watch::channel(false).1,
            hook_event_channels: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            sidecar: Arc::new(crate::sidecar::SidecarManager::new()),
            jwks: None,
            share: None,
            auth_identity: tokio::sync::OnceCell::new(),
            oauth_usage_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(
                300,
            )),
            plugin_cli_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(
                300,
            )),
            teams: Arc::new(crate::teams::TeamsStore::empty()),
            prompt_index: Arc::new(std::sync::RwLock::new(None)),
            prompt_stats: Arc::new(std::sync::RwLock::new(None)),
            prompt_templates: Arc::new(std::sync::RwLock::new(None)),
            available_ides: Vec::new(),
            monitor_tx: tokio::sync::broadcast::channel(64).0,
            monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            oracle_rx: crate::live::process_oracle::stub(),
            plugin_op_queue: Arc::new(crate::routes::plugin_ops::PluginOpQueue::new()),
            plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
            marketplace_refresh: Arc::new(
                crate::routes::marketplace_refresh::MarketplaceRefreshTracker::new(),
            ),
            transcript_to_session: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            pending_statusline: tokio::sync::Mutex::new(
                crate::live::buffer::PendingMutations::new(std::time::Duration::from_secs(120)),
            ),
            coordinator: std::sync::Arc::new(crate::live::coordinator::SessionCoordinator::new()),
            telemetry: None,
            telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
            debug_statusline_log: None,
            debug_hooks_log: None,
            debug_omlx_log: None,
            local_llm: Arc::new(crate::local_llm::LocalLlmService::new(
                Arc::new(crate::local_llm::LocalLlmConfig::new_disabled()),
                Arc::new(crate::local_llm::LlmStatus::new(10710)),
            )),
            session_channels: Arc::new(
                crate::live::session_ws::registry::SessionChannelRegistry::new(),
            ),
        });

        // Register the session in the live sessions map
        {
            let mut map = state.live_sessions.write().await;
            let session = crate::live::state::LiveSession {
                id: session_id.to_string(),
                status: crate::live::state::SessionStatus::Working,
                started_at: None,
                closed_at: None,
                control: None,
                model: None,
                model_display_name: None,
                model_set_at: 0,
                context_window_tokens: 0,
                statusline: crate::live::state::StatuslineFields::default(),
                hook: crate::live::state::HookFields {
                    agent_state: crate::live::state::AgentState {
                        group: crate::live::state::AgentStateGroup::Autonomous,
                        state: "working".to_string(),
                        label: "Working".to_string(),
                        context: None,
                    },
                    pid: None,
                    title: "Test session".to_string(),
                    last_user_message: "test".to_string(),
                    current_activity: "testing".to_string(),
                    turn_count: 0,
                    last_activity_at: 0,
                    current_turn_started_at: None,
                    sub_agents: Vec::new(),
                    progress_items: Vec::new(),
                    compact_count: 0,
                    agent_state_set_at: 0,
                    hook_events: Vec::new(),
                    last_assistant_preview: None,
                    last_error: None,
                    last_error_details: None,
                },
                jsonl: crate::live::state::JsonlFields {
                    project: "test-project".to_string(),
                    project_display_name: "test-project".to_string(),
                    project_path: "/tmp/test-project".to_string(),
                    file_path: file_path.to_string(),
                    ..crate::live::state::JsonlFields::default()
                },
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
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = Arc::new(AppState {
            start_time: std::time::Instant::now(),
            db,
            indexing: Arc::new(crate::IndexingState::new()),
            registry: Arc::new(std::sync::RwLock::new(None)),
            jobs: Arc::new(crate::jobs::JobRunner::new()),
            classify: Arc::new(crate::classify_state::ClassifyState::new()),
            facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
            git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
            pricing: Arc::new(std::collections::HashMap::new()),
            live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            recently_closed: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            live_tx: tokio::sync::broadcast::channel(256).0,

            rules_dir: std::env::temp_dir().join("claude-rules-test"),
            terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
            live_manager: None,
            search_index: Arc::new(std::sync::RwLock::new(None)),
            shutdown: tokio::sync::watch::channel(false).1,
            hook_event_channels: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            sidecar: Arc::new(crate::sidecar::SidecarManager::new()),
            jwks: None,
            share: None,
            auth_identity: tokio::sync::OnceCell::new(),
            oauth_usage_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(
                300,
            )),
            plugin_cli_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(
                300,
            )),
            teams: Arc::new(crate::teams::TeamsStore::empty()),
            prompt_index: Arc::new(std::sync::RwLock::new(None)),
            prompt_stats: Arc::new(std::sync::RwLock::new(None)),
            prompt_templates: Arc::new(std::sync::RwLock::new(None)),
            available_ides: Vec::new(),
            monitor_tx: tokio::sync::broadcast::channel(64).0,
            monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            oracle_rx: crate::live::process_oracle::stub(),
            plugin_op_queue: Arc::new(crate::routes::plugin_ops::PluginOpQueue::new()),
            plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
            marketplace_refresh: Arc::new(
                crate::routes::marketplace_refresh::MarketplaceRefreshTracker::new(),
            ),
            transcript_to_session: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            pending_statusline: tokio::sync::Mutex::new(
                crate::live::buffer::PendingMutations::new(std::time::Duration::from_secs(120)),
            ),
            coordinator: std::sync::Arc::new(crate::live::coordinator::SessionCoordinator::new()),
            telemetry: None,
            telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
            debug_statusline_log: None,
            debug_hooks_log: None,
            debug_omlx_log: None,
            local_llm: Arc::new(crate::local_llm::LocalLlmService::new(
                Arc::new(crate::local_llm::LocalLlmConfig::new_disabled()),
                Arc::new(crate::local_llm::LlmStatus::new(10710)),
            )),
            session_channels: Arc::new(
                crate::live::session_ws::registry::SessionChannelRegistry::new(),
            ),
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

        // Wait for the live line to arrive (file watcher + debounce delay).
        // Use a generous outer timeout and keep looping even when individual
        // recv_text calls time out — on macOS the FSEvents watcher may take
        // several seconds to coalesce and fire.
        let live_msg = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(text) = recv_text(&mut ws).await {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        if v["type"] == "line"
                            && v["data"].as_str().unwrap_or("").contains("live response")
                        {
                            return v;
                        }
                    }
                }
                // recv_text returned None (per-message timeout) — keep waiting
                // for the outer timeout to expire rather than giving up early.
            }
        })
        .await;

        write_handle.abort();

        assert!(
            live_msg.is_ok(),
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

        // Wait for the rich-mode message.
        // Keep looping even when individual recv_text calls time out — on
        // macOS the FSEvents watcher may take several seconds to fire.
        let rich_msg = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(text) = recv_text(&mut ws).await {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        // In rich mode, the type should be "message" (not "line")
                        if v["type"] == "message" {
                            return v;
                        }
                    }
                }
                // recv_text returned None (per-message timeout) — keep waiting
                // for the outer timeout to expire rather than giving up early.
            }
        })
        .await;

        write_handle.abort();

        assert!(rich_msg.is_ok(), "Timed out waiting for rich mode message");

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
    fn format_line_rich_mode_progress_emits_category() {
        let finders = RichModeFinders::new();
        let line = r#"{"type":"progress","data":{"type":"hook_progress","hookName":"pre-commit"}}"#;
        let result = format_line_for_mode(line, "rich", &finders);
        assert_eq!(result.len(), 1, "Progress events should emit one message");
        let parsed: serde_json::Value = serde_json::from_str(&result[0]).unwrap();
        assert_eq!(parsed["type"], "progress");
        assert_eq!(parsed["content"], "hook_progress: pre-commit");
        assert_eq!(parsed["category"], "hook");
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

    // =========================================================================
    // Unit test: queue-operation includes metadata in rich mode
    // =========================================================================

    #[test]
    fn format_line_rich_mode_queue_operation_includes_metadata() {
        let finders = RichModeFinders::new();
        let line = r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-09T10:00:00Z","content":"fix the bug"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert_eq!(results.len(), 1, "queue-operation should emit one message");
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "system");
        assert_eq!(parsed["category"], "queue");

        // Key assertions: metadata must exist with operation, content, and type
        let metadata = &parsed["metadata"];
        assert!(
            !metadata.is_null(),
            "queue-operation must include metadata object"
        );
        assert_eq!(metadata["type"], "queue-operation");
        assert_eq!(metadata["operation"], "enqueue");
        assert_eq!(metadata["content"], "fix the bug");
    }

    #[test]
    fn format_line_rich_mode_queue_operation_without_content() {
        let finders = RichModeFinders::new();
        let line =
            r#"{"type":"queue-operation","operation":"cancel","timestamp":"2026-03-09T10:01:00Z"}"#;
        let results = format_line_for_mode(line, "rich", &finders);
        assert_eq!(results.len(), 1, "queue-operation should emit one message");
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "system");
        assert_eq!(parsed["category"], "queue");
        assert_eq!(parsed["content"], "queue-cancel");

        let metadata = &parsed["metadata"];
        assert_eq!(metadata["type"], "queue-operation");
        assert_eq!(metadata["operation"], "cancel");
        // content should not be present when the source line has none
        assert!(
            metadata.get("content").is_none() || metadata["content"].is_null(),
            "metadata.content should be absent when source has no content"
        );
    }

    // =========================================================================
    // Integration test: queue-operation metadata via WebSocket
    // =========================================================================

    #[tokio::test]
    async fn test_rich_mode_queue_operation_includes_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let jsonl_path = dir.path().join("test-session.jsonl");
        {
            let mut f = std::fs::File::create(&jsonl_path).unwrap();
            writeln!(f, r#"{{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-09T10:00:00Z","content":"fix the bug"}}"#).unwrap();
        }

        let session_id = "test-queue-meta";
        let state = test_state_with_session(session_id, jsonl_path.to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;
        let mut ws = ws_connect(addr, session_id).await;

        // Send handshake (rich mode)
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"rich","scrollback":100}"#.into(),
        ))
        .await
        .unwrap();

        // Collect messages until buffer_end (with timeout via recv_text)
        let mut messages = Vec::new();
        loop {
            match recv_text(&mut ws).await {
                Some(text) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        if v.get("type").and_then(|t| t.as_str()) == Some("buffer_end") {
                            break;
                        }
                    }
                    messages.push(text);
                }
                None => break, // timeout — no more messages
            }
        }

        // Find the queue-operation message
        let queue_msg = messages
            .iter()
            .find(|m| m.contains("queue"))
            .expect("should have a queue message");

        let parsed: serde_json::Value = serde_json::from_str(queue_msg).unwrap();
        assert_eq!(parsed["type"], "system");
        assert_eq!(parsed["category"], "queue");

        // Key assertions: metadata must exist with operation, content, and type
        let metadata = &parsed["metadata"];
        assert_eq!(metadata["type"], "queue-operation");
        assert_eq!(metadata["operation"], "enqueue");
        assert_eq!(metadata["content"], "fix the bug");

        server_handle.abort();
    }

    // =========================================================================
    // Test: format_line_block_mode_produces_conversation_blocks
    // =========================================================================

    // =========================================================================
    // Test: ws_block_mode_scrollback
    // =========================================================================

    #[tokio::test]
    async fn ws_block_mode_scrollback() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp.as_file(),
            r#"{{"type":"user","uuid":"u-1","message":{{"content":[{{"type":"text","text":"hello"}}]}},"timestamp":"2026-03-21T01:00:00.000Z"}}"#
        )
        .unwrap();

        let state = test_state_with_session("ws-block-test", tmp.path().to_str().unwrap()).await;
        let (addr, server_handle) = start_test_server(state).await;
        let mut ws = ws_connect(addr, "ws-block-test").await;

        // Send handshake with block mode
        ws.send(tungstenite::Message::Text(
            r#"{"mode":"block","scrollback":50}"#.into(),
        ))
        .await
        .unwrap();

        // Collect scrollback messages until we see buffer_end or timeout
        let mut received: Vec<serde_json::Value> = Vec::new();
        let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(text) = recv_text(&mut ws).await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if msg_type == "buffer_end" {
                        break;
                    }
                    received.push(json);
                }
            }
        })
        .await;
        assert!(
            timeout_result.is_ok(),
            "Should receive buffer_end within timeout"
        );

        // Should have received at least one block
        assert!(
            !received.is_empty(),
            "Should receive scrollback blocks in block mode"
        );

        // First received block should have a type discriminator
        let first = &received[0];
        assert!(
            first.get("type").is_some(),
            "Block should have 'type' discriminator"
        );

        ws.close(None).await.ok();
        server_handle.abort();
    }

    // =========================================================================
    // Test: format_line_block_mode_produces_conversation_blocks
    // =========================================================================

    #[test]
    fn format_line_block_mode_produces_conversation_blocks() {
        let finders = RichModeFinders::new();
        // A user message line
        let line = r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello world"}]},"timestamp":"2026-03-21T01:00:00.000Z"}"#;
        let results = format_line_for_mode(line, "block", &finders);
        assert!(!results.is_empty(), "Block mode should produce output");
        let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(parsed["type"], "user", "Should produce a UserBlock");
        assert_eq!(parsed["text"], "hello world");
    }
}
