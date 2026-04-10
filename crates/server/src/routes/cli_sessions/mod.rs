//! CLI session management API routes.
//!
//! Manages tmux-based CLI sessions that run Claude Code.
//! Sessions are tracked in-memory with periodic health checks
//! against the actual tmux process state.
//!
//! - POST   /cli-sessions      -- Create a new CLI session
//! - GET    /cli-sessions      -- List all CLI sessions
//! - DELETE /cli-sessions/{id} -- Kill a CLI session

pub mod handlers;
pub mod health;
pub mod ring_buffer;
pub mod store;
pub mod terminal;
pub mod terminal_ws;
pub mod tmux;
pub mod types;

#[cfg(test)]
mod tests;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

// Re-export key types.
pub use store::CliSessionStore;
pub use tmux::{RealTmux, TmuxCommand};
pub use types::{CliSession, CliSessionStatus, CreateRequest, CreateResponse, ListResponse};

/// CLI sessions API router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/cli-sessions", post(handlers::create_session))
        .route("/cli-sessions", get(handlers::list_sessions))
        .route("/cli-sessions/{id}", delete(handlers::kill_session))
}
