//! Live session monitoring endpoints (SSE + REST).
//!
//! - `GET /api/live/stream`              -- SSE stream of real-time session events
//! - `GET /api/live/sessions`            -- List all live sessions
//! - `GET /api/live/sessions/:id`        -- Get a single live session
//! - `GET /api/live/sessions/:id/messages` -- Get recent messages for a live session
//! - `POST /api/live/sessions/:id/kill`   -- Send SIGTERM to a session's process
//! - `GET /api/live/summary`             -- Aggregate live session statistics
//! - `GET /api/live/pricing`             -- Model pricing table

mod actions;
mod sessions;
mod sse;
mod summary;
mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;

// ---- Re-export all public handler functions ----
pub use actions::{
    bind_control, dismiss_all_closed, dismiss_session, kill_session, unbind_control,
};
pub use sessions::{
    get_live_session, get_live_session_messages, get_session_statusline_debug, list_live_sessions,
};
pub use sse::live_stream;
pub use summary::{get_live_summary, get_pricing};

// ---- Re-export request/response types ----
pub use types::{BindControlRequest, MessagesQuery, UnbindControlRequest};

// ---- Re-export utoipa hidden path types ----
pub use actions::__path_bind_control;
pub use actions::__path_dismiss_all_closed;
pub use actions::__path_dismiss_session;
pub use actions::__path_kill_session;
pub use actions::__path_unbind_control;
pub use sessions::__path_get_live_session;
pub use sessions::__path_get_live_session_messages;
pub use sessions::__path_get_session_statusline_debug;
pub use sessions::__path_list_live_sessions;
pub use sse::__path_live_stream;
pub use summary::__path_get_live_summary;
pub use summary::__path_get_pricing;

/// Build the live monitoring sub-router.
///
/// Routes:
/// - `GET /live/stream`                 - SSE stream of live session events
/// - `GET /live/sessions`               - List all live sessions
/// - `GET /live/sessions/:id`           - Get single live session
/// - `GET /live/sessions/:id/messages`  - Get recent messages for a live session
/// - `POST /live/sessions/:id/kill`     - Send SIGTERM to a session's process
/// - `DELETE /live/sessions/:id/dismiss` - Dismiss a recently closed session
/// - `DELETE /live/recently-closed`      - Dismiss all recently closed sessions
/// - `GET /live/summary`                - Aggregate statistics
/// - `GET /live/pricing`                - Model pricing table
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/live/stream", get(live_stream))
        .route("/live/sessions", get(list_live_sessions))
        .route("/live/sessions/{id}", get(get_live_session))
        .route(
            "/live/sessions/{id}/messages",
            get(get_live_session_messages),
        )
        .route("/live/sessions/{id}/kill", post(kill_session))
        .route(
            "/live/sessions/{id}/statusline",
            get(get_session_statusline_debug),
        )
        .route("/live/sessions/{id}/dismiss", delete(dismiss_session))
        .route("/live/sessions/{id}/bind-control", post(bind_control))
        .route("/live/sessions/{id}/unbind-control", post(unbind_control))
        .route("/live/recently-closed", delete(dismiss_all_closed))
        .route("/live/summary", get(get_live_summary))
        .route("/live/pricing", get(get_pricing))
}
