// crates/server/src/routes/search.rs
//! Full-text search endpoint.
//!
//! - GET /search?q=...&scope=...&limit=...&offset=... — Search across all sessions
//!
//! Uses co-primary search: grep (primary) + Tantivy (supplement).
//! SQLite pre-filters narrow the session set before search engines run.

use crate::error::ApiResult;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use claude_view_db::SearchPrefilter;
use claude_view_search::types::SearchResponse;
use claude_view_search::{unified_search, UnifiedSearchOptions};
use serde::Deserialize;
use std::collections::HashSet;
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
    // Structured filters for SQLite pre-filter:
    /// Filter by project path or project_id.
    pub project: Option<String>,
    /// Filter by git branch name.
    pub branch: Option<String>,
    /// Filter by model name.
    pub model: Option<String>,
    /// Filter sessions after this date (ISO format: "2026-03-01").
    pub after: Option<String>,
    /// Filter sessions before this date (ISO format: "2026-03-11").
    pub before: Option<String>,
}

/// Build the search sub-router.
///
/// Routes:
/// - `GET /search` — Co-primary smart search across all indexed sessions
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/search", get(search_handler))
}

/// Parse ISO date string ("YYYY-MM-DD") to Unix timestamp (midnight UTC).
fn parse_iso_date(s: &str) -> Option<i64> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
}

/// GET /api/search — Co-primary smart search.
///
/// Runs grep (primary) and Tantivy (supplement) concurrently.
/// SQLite pre-filter narrows the session set before engines run.
/// Returns a single `SearchResponse` regardless of which engines contributed.
///
/// Does NOT return 503 when Tantivy index is missing — grep is primary and
/// does not depend on the index.
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

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let scope = query.scope.clone();

    // Build SQLite pre-filter from structured params.
    // Also extract project from scope string for backward compat ("project:<name>").
    let scope_project = scope
        .as_deref()
        .and_then(|s| s.strip_prefix("project:").map(|p| p.to_string()));

    let prefilter = SearchPrefilter {
        project: query.project.clone().or(scope_project),
        branch: query.branch.clone(),
        model: query.model.clone(),
        after: query.after.as_deref().and_then(parse_iso_date),
        before: query.before.as_deref().and_then(parse_iso_date),
    };

    // Step 1: SQLite pre-filter (only if any structured filters are set).
    let session_ids: Option<HashSet<String>> = if !prefilter.is_empty() {
        Some(
            state
                .db
                .search_prefilter_session_ids(&prefilter)
                .await
                .map_err(|e| crate::error::ApiError::Internal(format!("Pre-filter: {e}")))?,
        )
    } else {
        None
    };

    // Step 2: Collect JSONL files (narrowed by session IDs when filtered).
    let project_filter = prefilter.project.clone();
    let session_ids_clone = session_ids.clone();
    let jsonl_files = tokio::task::spawn_blocking(move || {
        collect_jsonl_files(project_filter.as_deref(), session_ids_clone.as_ref())
    })
    .await
    .map_err(|e| crate::error::ApiError::Internal(format!("File collection join: {e}")))?
    .unwrap_or_else(|e| {
        tracing::warn!("Failed to collect JSONL files: {e}");
        vec![]
    });

    // Step 3: Get search index (optional — grep is primary, Tantivy supplements).
    // Read-lock the holder, clone the Option<Arc<SearchIndex>>, drop the lock immediately.
    let search_index = state
        .search_index
        .read()
        .map_err(|_| crate::error::ApiError::Internal("search index lock poisoned".into()))?
        .clone();
    // search_index is Option<Arc<SearchIndex>> — pass as Option<&SearchIndex> via .as_deref()

    // Step 4: Run unified search (both engines co-primary) in spawn_blocking.
    let q_owned = q.to_string();
    let start = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        let opts = UnifiedSearchOptions {
            query: q_owned,
            scope,
            limit,
            offset,
            skip_snippets: false,
        };
        // .as_deref() converts Option<Arc<SearchIndex>> to Option<&SearchIndex>
        unified_search(search_index.as_deref(), &jsonl_files, &opts)
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

    let mut response = result.response;
    response.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(response))
}
