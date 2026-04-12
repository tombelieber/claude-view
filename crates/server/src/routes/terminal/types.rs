//! Types, constants, and helpers for the terminal WebSocket module.

use std::sync::Arc;

use memchr::memmem;

use crate::state::AppState;

/// Handshake message sent by the client on connection.
#[derive(Debug, serde::Deserialize)]
pub(super) struct HandshakeMessage {
    /// Display mode: "raw" (default) or "rich" (structured JSONL parsing).
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Number of scrollback lines to send on connect (default: 100).
    #[serde(default = "default_scrollback")]
    pub scrollback: usize,
}

pub(super) fn default_mode() -> String {
    "raw".to_string()
}

pub(super) fn default_scrollback() -> usize {
    100
}

/// Maximum scrollback lines the server will send, regardless of client request.
/// Protects against OOM from malicious/misconfigured clients requesting huge scrollbacks.
pub(super) const MAX_SCROLLBACK: usize = 5_000;

/// Client-to-server message types.
#[derive(Debug, serde::Deserialize)]
pub(super) struct ClientMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    /// New mode for "mode" messages: "raw" or "rich".
    #[serde(default)]
    pub mode: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub cols: Option<u32>,
    #[allow(dead_code)]
    #[serde(default)]
    pub rows: Option<u32>,
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
pub(super) struct RichModeFinders {
    pub type_key: memmem::Finder<'static>,
    pub role_key: memmem::Finder<'static>,
    pub timestamp_key: memmem::Finder<'static>,
}

impl RichModeFinders {
    pub fn new() -> Self {
        Self {
            type_key: memmem::Finder::new(b"\"type\""),
            role_key: memmem::Finder::new(b"\"role\""),
            timestamp_key: memmem::Finder::new(b"\"timestamp\""),
        }
    }
}

/// RAII guard that calls `disconnect()` when dropped, ensuring connection
/// count is always decremented even if the WebSocket handler panics or the
/// tokio task is cancelled (e.g., during server shutdown or HMR reload).
/// Without this, orphaned connections accumulate and hit the global limit.
pub(super) struct ConnectionGuard {
    pub session_id: String,
    pub manager: Arc<crate::terminal_state::TerminalConnectionManager>,
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

/// Resolve the JSONL file path for a session.
///
/// Checks (in order): live sessions → recently closed → database.
/// This allows terminal WebSocket handlers to serve both live and historical
/// sessions — not just the ones currently in the `live_sessions` map.
pub(super) async fn resolve_jsonl_path(
    state: &AppState,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    // 1. Live sessions (in-memory, actively monitored)
    {
        let map = state.live_sessions.read().await;
        if let Some(fp) = map.get(session_id).map(|s| s.jsonl.file_path.clone()) {
            if !fp.is_empty() {
                return Some(std::path::PathBuf::from(fp));
            }
        }
    }

    // 2. Closed ring buffer (ephemeral, post-reap)
    {
        let ring = state.closed_ring.read().await;
        if let Some(session) = ring.iter().find(|s| s.id == session_id) {
            if !session.jsonl.file_path.is_empty() {
                return Some(std::path::PathBuf::from(&session.jsonl.file_path));
            }
        }
    }

    // 3. Database (historical sessions)
    if let Ok(Some(fp)) = state.db.get_session_file_path(session_id).await {
        let path = std::path::PathBuf::from(&fp);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
