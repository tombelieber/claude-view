//! API route handlers for the claude-view server.

pub mod classify;
pub mod coaching;
pub mod config;
pub mod contributions;
pub mod control;
pub mod docs;
pub mod export;
pub mod facets;
pub mod file_history;
pub mod grep;
pub mod health;
pub mod hooks;
pub mod ide;
pub mod indexing;
pub mod insights;
pub mod invocables;
pub mod jobs;
pub mod live;
pub mod marketplace_refresh;
pub mod metrics;
pub mod models;
pub mod monitor;
pub mod oauth;
pub mod pairing;
pub mod plans;
pub mod plugin_ops;
pub mod plugins;
pub mod processes;
pub mod projects;
pub mod prompts;
pub mod reports;
pub mod score;
pub mod search;
pub mod sessions;
pub mod settings;
pub mod share;
pub mod sidecar_proxy;
pub mod stats;
pub mod status;
pub mod statusline;
pub mod sync;
pub mod system;
pub mod teams;
pub mod telemetry;
pub mod terminal;
pub mod trends;
pub mod turns;
pub mod workflows;

use std::sync::Arc;

use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::{self, Next},
    response::Response,
    Router,
};

use crate::state::AppState;

/// Create the combined API router with all routes under /api prefix.
///
/// Routes:
/// - GET /api/config - Runtime capabilities endpoint
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
/// - GET /api/search?q=...&scope=...&limit=...&offset=... - Full-text search
/// - GET /api/settings - Read current app settings (model, timeout)
/// - PUT /api/settings - Update app settings (partial, validates model + timeout)
/// - GET /api/oauth/usage - OAuth usage (reads credentials, fetches from Anthropic API)
/// - POST /api/oauth/usage/refresh - Force-refresh OAuth usage (bypasses cache, 60s spam guard)
/// - GET /api/oauth/identity - Cached auth identity (email, org, plan)
/// - GET /api/plugins — unified view of installed + available plugins
/// - GET /api/teams - List all teams (summaries)
/// - GET /api/teams/:name - Get team detail (config + members)
/// - GET /api/teams/:name/inbox - Get team inbox messages
/// - GET /metrics - Prometheus metrics (not under /api prefix)
//
// V1-hardening M1.3 — Build the full route tree under a given prefix
// (e.g. "/api" or "/api/v1"). Extracted so we can mount the same tree at
// both `/api/v1/*` (canonical) and `/api/*` (legacy alias with
// deprecation header).
fn build_api_tree(prefix: &str) -> Router<Arc<AppState>> {
    let live_prefix = format!("{prefix}/live");
    let local_llm_prefix = format!("{prefix}/local-llm");
    Router::new()
        .nest(prefix, config::router())
        .nest(prefix, health::router())
        .nest(prefix, projects::router())
        .nest(prefix, sessions::router())
        .nest(prefix, indexing::router())
        .nest(prefix, invocables::router())
        .nest(prefix, models::router())
        .nest(prefix, stats::router())
        .nest(prefix, trends::router())
        .nest(prefix, status::router())
        .nest(prefix, export::router())
        .nest(prefix, sync::router())
        .nest(prefix, system::router())
        .nest(prefix, classify::router())
        .nest(prefix, coaching::router())
        .nest(prefix, control::router())
        .nest(prefix, insights::router())
        .nest(prefix, contributions::router())
        .nest(prefix, score::router())
        .nest(prefix, facets::router())
        .nest(prefix, file_history::router())
        .nest(prefix, live::router())
        .nest(&live_prefix, terminal::router())
        .nest(prefix, turns::router())
        .nest(prefix, hooks::router())
        .nest(prefix, ide::router())
        .nest(prefix, search::router())
        .nest(prefix, reports::router())
        .nest(prefix, settings::router())
        .nest(prefix, oauth::router())
        .nest(prefix, pairing::router())
        .nest(prefix, plans::router())
        .nest(prefix, prompts::router())
        .nest(prefix, share::router())
        .nest(prefix, statusline::router())
        .nest(prefix, plugins::router())
        .nest(prefix, plugin_ops::router())
        .nest(prefix, marketplace_refresh::router())
        .nest(prefix, teams::router())
        .nest(prefix, workflows::router())
        .nest(prefix, monitor::router())
        .nest(prefix, processes::router())
        .nest(prefix, telemetry::router())
        .nest(&local_llm_prefix, crate::local_llm::local_llm_routes())
}

/// Middleware: mark legacy `/api/*` responses with `Deprecation` + `Link`
/// headers pointing clients at `/api/v1/*`. Kept through one major version.
async fn deprecation_header_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut resp = next.run(req).await;
    // Point clients at the v1 equivalent (same suffix after /api/).
    if let Some(suffix) = path.strip_prefix("/api/") {
        let successor = format!("</api/v1/{suffix}>; rel=\"successor-version\"");
        resp.headers_mut()
            .insert("deprecation", HeaderValue::from_static("version=\"1.0\""));
        if let Ok(link) = HeaderValue::from_str(&successor) {
            resp.headers_mut().insert("link", link);
        }
    }
    resp
}

/// Create the combined API router.
///
/// V1-hardening M1.3 — all routes are mounted at `/api/v1/*` (canonical).
/// Legacy `/api/*` aliases are kept for backwards compatibility and emit
/// `Deprecation` + `Link` headers pointing at the v1 equivalent. Plan to
/// remove the legacy alias in 2.0.
pub fn api_routes(state: Arc<AppState>) -> Router {
    let v1 = build_api_tree("/api/v1");
    let legacy = build_api_tree("/api").layer(middleware::from_fn(deprecation_header_middleware));

    Router::new()
        .merge(v1)
        .merge(legacy)
        // Swagger UI + OpenAPI spec (unversioned — served at root).
        .merge(docs::router())
        // Metrics endpoint at root level (Prometheus convention)
        .merge(metrics::router())
        // Sidecar reverse proxy (HTTP + WS) — mounted at root level because
        // /ws/chat/* sits outside the /api prefix. In dev, Vite handles this;
        // in production, the Rust server proxies to sidecar on :3001.
        .merge(sidecar_proxy::router())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_api_routes_creation() {
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let state = AppState::new(db);
        let _router = api_routes(state);
    }

    /// V1-hardening M1.3 — /api/v1/* is the canonical path.
    #[tokio::test]
    async fn api_v1_health_responds_200() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = api_routes(state);

        let req = axum::http::Request::builder()
            .uri("/api/v1/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // v1 responses do NOT carry a Deprecation header.
        assert!(
            resp.headers().get("deprecation").is_none(),
            "v1 responses must not carry Deprecation header"
        );
    }

    /// V1-hardening M1.3 — legacy /api/* works but emits Deprecation header.
    #[tokio::test]
    async fn legacy_api_health_responds_with_deprecation_header() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = AppState::new(db);
        let app = api_routes(state);

        let req = axum::http::Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let deprecation = resp
            .headers()
            .get("deprecation")
            .expect("legacy route must carry Deprecation header");
        assert_eq!(deprecation.to_str().unwrap(), "version=\"1.0\"");
        let link = resp
            .headers()
            .get("link")
            .expect("legacy route must carry Link header")
            .to_str()
            .unwrap();
        assert!(link.contains("/api/v1/health"));
        assert!(link.contains("successor-version"));
    }
}
