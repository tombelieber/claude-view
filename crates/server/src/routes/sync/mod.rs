//! Sync endpoints for triggering git commit scanning and deep index rebuilds.

mod deep_index;
mod git_sync;
mod mutex;
mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

// Re-export all public items to preserve the module's public API.
pub use deep_index::trigger_deep_index;
pub use git_sync::{git_sync_progress, trigger_git_sync};
pub use types::{SyncAcceptedResponse, SyncStatus};

// Re-export utoipa hidden path types for OpenAPI schema generation.
pub use deep_index::__path_trigger_deep_index;
pub use git_sync::__path_git_sync_progress;
pub use git_sync::__path_trigger_git_sync;

/// Create the sync routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sync/git", post(trigger_git_sync))
        .route("/sync/git/progress", get(git_sync_progress))
        .route("/sync/deep", post(trigger_deep_index))
}
