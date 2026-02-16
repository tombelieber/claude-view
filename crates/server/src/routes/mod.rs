//! API route handlers for the vibe-recall server.

pub mod classify;
pub mod coaching;
pub mod contributions;
pub mod export;
pub mod facets;
pub mod health;
pub mod hooks;
pub mod indexing;
pub mod insights;
pub mod invocables;
pub mod jobs;
pub mod live;
pub mod metrics;
pub mod models;
pub mod projects;
pub mod score;
pub mod sessions;
pub mod stats;
pub mod status;
pub mod sync;
pub mod system;
pub mod terminal;
pub mod trends;
pub mod turns;

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
/// - GET /api/indexing/status - JSON snapshot of indexing progress (polling)
/// - GET /api/invocables - List all invocables with usage counts
/// - GET /api/stats/dashboard - Pre-computed dashboard stats with trends
/// - GET /api/models - List all observed models with usage counts
/// - GET /api/trends - Week-over-week trend metrics
/// - GET /api/status - Index metadata and data freshness
/// - GET /api/export/sessions - Export sessions as JSON or CSV
/// - POST /api/sync/git - Trigger git commit scanning
/// - GET  /api/sync/git/progress - SSE stream of git sync progress
/// - POST /api/sync/deep - Trigger full deep index rebuild
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
/// - GET    /api/coaching/rules      - List all coaching rules
/// - POST   /api/coaching/rules      - Apply (create) a coaching rule
/// - DELETE  /api/coaching/rules/{id} - Remove a coaching rule
/// - GET  /api/insights - Computed behavioral insights and patterns
/// - GET /api/contributions - Contribution metrics and insights
/// - GET /api/contributions/sessions/:id - Session contribution detail
/// - GET /api/score - AI Fluency Score (composite 0-100)
/// - GET  /api/facets/ingest/stream  - SSE stream of facet ingest progress
/// - POST /api/facets/ingest/trigger - Trigger facet ingest
/// - GET  /api/facets/stats          - Aggregate facet statistics
/// - GET  /api/facets/badges         - Quality badges for sessions
/// - GET  /api/facets/pattern-alert  - Negative satisfaction pattern alert
/// - GET  /api/live/stream              - SSE stream of live session events
/// - GET  /api/live/sessions            - List all live sessions
/// - GET  /api/live/sessions/:id        - Get single live session
/// - GET  /api/live/sessions/:id/messages - Get recent messages for a live session
/// - WS   /api/live/sessions/:id/terminal - WebSocket terminal stream
/// - GET  /api/live/summary             - Aggregate live session statistics
/// - GET  /api/live/pricing             - Model pricing table
/// - GET /api/sessions/:id/turns - Per-turn breakdown for a session
/// - GET /metrics - Prometheus metrics (not under /api prefix)
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
        .nest("/api", system::router())
        .nest("/api", classify::router())
        .nest("/api", coaching::router())
        .nest("/api", insights::router())
        .nest("/api", contributions::router())
        .nest("/api", score::router())
        .nest("/api", facets::router())
        .nest("/api", live::router())
        .nest("/api/live", terminal::router())
        .nest("/api", turns::router())
        .nest("/api", hooks::router())
        // Metrics endpoint at root level (Prometheus convention)
        .merge(metrics::router())
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
