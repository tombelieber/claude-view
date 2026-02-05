//! API route handlers for the vibe-recall server.

pub mod classify;
pub mod export;
pub mod health;
pub mod indexing;
pub mod insights;
pub mod invocables;
pub mod jobs;
pub mod models;
pub mod projects;
pub mod sessions;
pub mod stats;
pub mod status;
pub mod sync;
pub mod system;
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
/// - GET /api/system - Comprehensive system status
/// - POST /api/system/reindex - Trigger full re-index
/// - POST /api/system/clear-cache - Clear search index and cache
/// - POST /api/system/git-resync - Trigger full git re-sync
/// - POST /api/system/reset - Factory reset all data
/// - POST /api/classify - Trigger classification job
/// - GET  /api/classify/status - Get classification status
/// - GET  /api/classify/stream - SSE stream of classification progress
/// - POST /api/classify/cancel - Cancel running classification
/// - GET  /api/insights - Computed behavioral insights and patterns
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
        .nest("/api", jobs::router())
        .nest("/api", system::router())
        .nest("/api", classify::router())
        .nest("/api", insights::router())
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
