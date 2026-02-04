//! API route handlers for the vibe-recall server.

pub mod contributions;
pub mod export;
pub mod health;
pub mod indexing;
pub mod invocables;
pub mod models;
pub mod projects;
pub mod sessions;
pub mod stats;
pub mod status;
pub mod sync;
pub mod trends;

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Create the combined API router with all routes under /api prefix.
///
/// Routes:
/// - GET /api/health - Health check
/// - GET /api/projects - List all projects (summaries)
/// - GET /api/projects/:id/sessions - Paginated sessions for a project
/// - GET /api/session/:project_dir/:session_id - Get a specific session (full JSONL parse)
/// - GET /api/sessions - List all sessions with filter/sort
/// - GET /api/sessions/:id - Get extended session detail with commits
/// - GET /api/indexing/progress - SSE stream of indexing progress
/// - GET /api/invocables - List all invocables with usage counts
/// - GET /api/stats/dashboard - Pre-computed dashboard stats with trends
/// - GET /api/models - List all observed models with usage counts
/// - GET /api/trends - Week-over-week trend metrics
/// - GET /api/status - Index metadata and data freshness
/// - GET /api/export/sessions - Export sessions as JSON or CSV
/// - POST /api/sync/git - Trigger git commit scanning
/// - PUT /api/settings/git-sync-interval - Update git sync interval
/// - GET /api/contributions - Contribution metrics and insights
/// - GET /api/contributions/sessions/:id - Session contribution detail
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", health::router())
        .nest("/api", projects::router())
        .nest("/api", sessions::router())
        .nest("/api", indexing::router())
        .nest("/api", invocables::router())
        .nest("/api", models::router())
        .nest("/api", stats::router())
        .nest("/api", trends::router())
        .nest("/api", status::router())
        .nest("/api", export::router())
        .nest("/api", sync::router())
        .nest("/api", contributions::router())
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
