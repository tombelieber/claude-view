// crates/server/src/routes/search.rs
//! Full-text search endpoint.
//!
//! - GET /search?q=...&scope=...&limit=...&offset=... — Search across all sessions

use crate::error::ApiResult;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_search::types::SearchResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    /// The search query string. Required.
    pub q: Option<String>,
    /// Optional scope filter, e.g. `"project:claude-view"`.
    pub scope: Option<String>,
    /// Maximum number of session groups to return (default: 20, max: 100).
    pub limit: Option<usize>,
    /// Number of session groups to skip for pagination (default: 0).
    pub offset: Option<usize>,
}

/// Build the search sub-router.
///
/// Routes:
/// - `GET /search` — Full-text search across all indexed sessions
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/search", get(search_handler))
}

/// GET /api/search — Execute a full-text search query.
///
/// Returns session-grouped results sorted by BM25 relevance score.
///
/// Query parameters:
/// - `q` (required): Search query string, supports qualifiers like `project:foo`
/// - `scope`: Optional scope filter
/// - `limit`: Max session groups to return (default 20, capped at 100)
/// - `offset`: Number of session groups to skip (default 0)
async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let q = query.q.as_deref().unwrap_or("").trim();
    if q.is_empty() {
        return Err(crate::error::ApiError::BadRequest(
            "query parameter 'q' is required".to_string(),
        ));
    }

    // Read-lock the holder, clone the Option<Arc<SearchIndex>>, drop the lock immediately.
    let search_index = state
        .search_index
        .read()
        .map_err(|_| crate::error::ApiError::Internal("search index lock poisoned".into()))?
        .clone();
    let search_index = search_index.ok_or_else(|| {
        crate::error::ApiError::ServiceUnavailable(
            "Search index is not available. It may still be building.".to_string(),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let scope = query.scope.as_deref();

    // Run Tantivy search on a blocking thread to avoid stalling the Tokio
    // runtime and to catch panics (which otherwise drop the connection silently).
    let q_owned = q.to_string();
    let scope_owned = scope.map(|s| s.to_string());
    let response = tokio::task::spawn_blocking(move || {
        search_index.search(&q_owned, scope_owned.as_deref(), limit, offset)
    })
    .await
    .map_err(|e| {
        // Extract the actual panic message from the JoinError for debugging.
        let msg = if e.is_panic() {
            let panic_payload = e.into_panic();
            if let Some(s) = panic_payload.downcast_ref::<String>() {
                format!("Search panicked: {}", s)
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                format!("Search panicked: {}", s)
            } else {
                "Search panicked (unknown payload)".to_string()
            }
        } else {
            format!("Search task failed: {}", e)
        };
        tracing::error!("{}", msg);
        crate::error::ApiError::Internal(msg)
    })?
    .map_err(|e| crate::error::ApiError::Internal(format!("Search failed: {}", e)))?;

    Ok(Json(response))
}
