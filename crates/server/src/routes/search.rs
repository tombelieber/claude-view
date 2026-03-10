// crates/server/src/routes/search.rs
//! Full-text search endpoint.
//!
//! - GET /search?q=...&scope=...&limit=...&offset=... — Search across all sessions
//!
//! Uses unified search: Tantivy first, grep fallback if 0 results.
//! Returns 503 if the search index is still building.

use crate::error::ApiResult;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_search::types::SearchResponse;
use claude_view_search::{unified_search, UnifiedSearchOptions};
use serde::Deserialize;
use std::sync::Arc;

use super::grep::collect_jsonl_files;

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
/// - `GET /search` — Unified smart search across all indexed sessions
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/search", get(search_handler))
}

/// GET /api/search — Unified smart search.
///
/// Tries Tantivy full-text index first. If 0 results, falls back to
/// grep over raw JSONL files. Returns a single `SearchResponse` shape
/// regardless of which engine produced the results.
///
/// Returns 503 if the search index is still building (grep is NOT
/// a substitute for the missing index — it's a fallback for Tantivy
/// misses, not Tantivy absence).
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

    // 503 if index not ready — grep is NOT a substitute for missing index
    let search_index = search_index.ok_or_else(|| {
        crate::error::ApiError::ServiceUnavailable(
            "Search index is not available. It may still be building.".to_string(),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let scope = query.scope.clone();

    let q_owned = q.to_string();

    let response = tokio::task::spawn_blocking(move || {
        // Parse scope for project filter (e.g. "project:claude-view" -> "claude-view")
        let project_filter = scope
            .as_deref()
            .and_then(|s| s.strip_prefix("project:").map(|p| p.to_string()));

        // Collect JSONL files for grep fallback, scoped by project if specified.
        // Log errors but don't fail the request — grep is a fallback, not primary.
        let jsonl_files = match collect_jsonl_files(project_filter.as_deref()) {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!("Failed to collect JSONL files for grep fallback: {e}");
                vec![]
            }
        };

        let opts = UnifiedSearchOptions {
            query: q_owned,
            scope,
            limit,
            offset,
        };

        // search_index is Arc<SearchIndex> — .as_ref() dereferences to &SearchIndex.
        // Rust does NOT auto-deref Arc<T> to &T in function argument position.
        unified_search(Some(search_index.as_ref()), &jsonl_files, &opts)
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

    Ok(Json(response.response))
}
