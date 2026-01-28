//! API route handlers for the vibe-recall server.

pub mod health;
pub mod indexing;
pub mod invocables;
pub mod models;
pub mod projects;
pub mod sessions;
pub mod stats;

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Create the combined API router with all routes under /api prefix.
///
/// Routes:
/// - GET /api/health - Health check
/// - GET /api/projects - List all projects (summaries)
/// - GET /api/projects/:id/sessions - Paginated sessions for a project
/// - GET /api/session/:project_dir/:session_id - Get a specific session
/// - GET /api/indexing/progress - SSE stream of indexing progress
/// - GET /api/invocables - List all invocables with usage counts
/// - GET /api/stats/overview - Aggregate usage statistics
/// - GET /api/stats/tokens - Aggregate token usage statistics
/// - GET /api/stats/dashboard - Pre-computed dashboard stats
/// - GET /api/models - List all observed models with usage counts
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", health::router())
        .nest("/api", projects::router())
        .nest("/api", sessions::router())
        .nest("/api", indexing::router())
        .nest("/api", invocables::router())
        .nest("/api", models::router())
        .nest("/api", stats::router())
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
    }
}
