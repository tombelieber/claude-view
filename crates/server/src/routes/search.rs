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
use serde::Deserialize;
use std::sync::Arc;
use claude_view_search::types::SearchResponse;

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

    let search_index = state.search_index.as_ref().ok_or_else(|| {
        crate::error::ApiError::ServiceUnavailable(
            "Search index is not available. It may still be building.".to_string(),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let scope = query.scope.as_deref();

    let response = search_index
        .search(q, scope, limit, offset)
        .map_err(|e| crate::error::ApiError::Internal(format!("Search failed: {}", e)))?;

    Ok(Json(response))
}
