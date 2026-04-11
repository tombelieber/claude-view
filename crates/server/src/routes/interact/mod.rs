//! Session interaction API routes.
//!
//! Endpoints for resolving pending interactions (permission, question, plan,
//! elicitation) and fetching full interaction data.
//!
//! - POST /sessions/{session_id}/interact    — resolve a pending interaction
//! - GET  /sessions/{session_id}/interaction  — fetch full interaction block

pub mod handlers;

// Re-export handlers for openapi.rs path annotations.
pub use handlers::get_interaction_handler;
pub use handlers::interact_handler;

#[cfg(test)]
mod tests;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

/// Interaction API router. Mounted under `/api`.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/sessions/{session_id}/interact",
            post(handlers::interact_handler),
        )
        .route(
            "/sessions/{session_id}/interaction",
            get(handlers::get_interaction_handler),
        )
}
