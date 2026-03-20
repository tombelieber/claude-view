// crates/server/src/routes/control.rs
//! Legacy control routes — kept as an empty router for backward compatibility.
//!
//! All interactive control is now handled directly by the sidecar on TCP :3001.
//! The frontend connects to sidecar endpoints via Vite proxy:
//!   - POST /api/sidecar/sessions         → create session
//!   - POST /api/sidecar/sessions/:id/resume → resume session
//!   - POST /api/sidecar/sessions/:id/fork   → fork session
//!   - DELETE /api/sidecar/sessions/:id       → terminate session
//!   - GET  /api/sidecar/sessions             → list sessions
//!   - WS   /ws/chat/:sessionId       → stream events
//!   - POST /api/estimate             → cost estimate (Rust server)

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Empty router — all control routes have been migrated.
///
/// Kept so `routes/mod.rs` compiles without changes; will be fully
/// removed once the module declaration is cleaned up.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
