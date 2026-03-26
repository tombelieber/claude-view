//! Model routes.
//!
//! The model selector now fetches directly from sidecar's /sessions/models
//! endpoint (SDK model cache). The old /api/models endpoint that merged
//! DB + pricing + SDK data has been removed — the alias→real-ID resolution
//! was fragile and caused Opus to never appear in the selector.
//!
//! The models table still exists for usage tracking (indexer writes
//! first_seen/last_seen/total_turns), but it's no longer the source
//! of truth for the model selector dropdown.

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Create the models routes router (currently empty — model selection
/// is handled client-side via sidecar).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
}
