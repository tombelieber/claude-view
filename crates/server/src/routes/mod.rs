// crates/server/src/routes/mod.rs
//! API route handlers for the vibe-recall server.

pub mod health;
pub mod indexing;
pub mod projects;
pub mod sessions;

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Create the combined API router with all routes under /api prefix.
///
/// Routes:
/// - GET /api/health - Health check
/// - GET /api/projects - List all projects with sessions
/// - GET /api/session/:project_dir/:session_id - Get a specific session
/// - GET /api/indexing/progress - SSE stream of indexing progress
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", health::router())
        .nest("/api", projects::router())
        .nest("/api", sessions::router())
        .nest("/api", indexing::router())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_routes_creation() {
        let db = vibe_recall_db::Database::new_in_memory().await.expect("in-memory DB");
        let state = AppState::new(db);
        let _router = api_routes(state);
        // Router creation should not panic
    }
}
