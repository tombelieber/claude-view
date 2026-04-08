//! WebSocket terminal endpoint for Live Monitor.
//!
//! Streams JSONL file content to the browser over WebSocket, providing
//! real-time terminal monitoring for active Claude Code sessions.
//!
//! - `WS /api/live/sessions/:id/terminal` -- WebSocket stream of JSONL lines

mod format;
mod handler;
#[cfg(test)]
mod tests;
mod types;
mod watcher;
mod ws_init;
mod ws_loop;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::state::AppState;

// Re-export public items that were visible from the original module
pub(crate) use types::WatchEvent;
pub(crate) use watcher::start_file_watcher;

/// Build the terminal WebSocket sub-router.
///
/// Routes:
/// - `WS /sessions/:id/terminal` - WebSocket stream of JSONL lines
/// - `WS /sessions/:id/subagents/:agent_id/terminal` - WebSocket stream of sub-agent JSONL lines
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{id}/terminal", get(handler::ws_terminal_handler))
        .route(
            "/sessions/{id}/subagents/{agent_id}/terminal",
            get(handler::ws_subagent_terminal_handler),
        )
        // Multiplexed WS — Stage 1+4. Carries block + raw + sdk + hook events
        // over a single connection with typed frames.
        .route(
            "/sessions/{id}/ws",
            get(crate::live::session_ws::handler::ws_session_handler),
        )
}
