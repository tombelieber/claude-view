//! Unified search service.
//!
//! The ONE search function. Both `/api/search` and `/api/sessions?q=`
//! call `execute_search()` with the same `SearchFilters`.
//! Only the response shape differs per endpoint.

use std::collections::HashSet;
use std::sync::Arc;

use claude_view_db::SearchPrefilter;
use claude_view_search::types::SearchResponse;
use claude_view_search::{unified_search, UnifiedSearchOptions};

use crate::error::ApiError;
use crate::routes::grep::collect_jsonl_files;
use crate::state::AppState;

/// Shared filter struct for all search entry points.
#[derive(Debug, Default)]
pub struct SearchFilters {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    /// ISO date string "YYYY-MM-DD"
    pub after: Option<String>,
    /// ISO date string "YYYY-MM-DD"
    pub before: Option<String>,
}

/// Parse ISO date string ("YYYY-MM-DD") to Unix timestamp (midnight UTC).
fn parse_iso_date(s: &str) -> Option<i64> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
}

/// The ONE search function. Every search entry point calls this.
pub async fn execute_search(
    state: &Arc<AppState>,
    query: &str,
    filters: &SearchFilters,
    limit: usize,
    offset: usize,
    skip_snippets: bool,
) -> Result<SearchResponse, ApiError> {
    // 1. Build SQLite pre-filter from shared filters.
    let prefilter = SearchPrefilter {
        project: filters.project.clone(),
        branch: filters.branch.clone(),
        model: filters.model.clone(),
        after: filters.after.as_deref().and_then(parse_iso_date),
        before: filters.before.as_deref().and_then(parse_iso_date),
    };

    // 2. SQL pre-filter (only if any filters set).
    let session_ids: Option<HashSet<String>> = if !prefilter.is_empty() {
        Some(
            state
                .db
                .search_prefilter_session_ids(&prefilter)
                .await
                .map_err(|e| ApiError::Internal(format!("Pre-filter: {e}")))?,
        )
    } else {
        None
    };

    // 3. Collect JSONL files (narrowed by session IDs when filtered).
    let project_filter = prefilter.project.clone();
    let session_ids_clone = session_ids.clone();
    let jsonl_files = tokio::task::spawn_blocking(move || {
        collect_jsonl_files(project_filter.as_deref(), session_ids_clone.as_ref())
    })
    .await
    .map_err(|e| ApiError::Internal(format!("File collection join: {e}")))?
    .unwrap_or_else(|e| {
        tracing::warn!("Failed to collect JSONL files: {e}");
        vec![]
    });

    // 4. Get search index (optional — Tantivy primary, grep fallback).
    let search_index = state
        .search_index
        .read()
        .map_err(|_| ApiError::Internal("search index lock poisoned".into()))?
        .clone();

    // 5. Run unified search in spawn_blocking.
    let q_owned = query.to_string();
    let start = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        let opts = UnifiedSearchOptions {
            query: q_owned,
            scope: None,
            limit,
            offset,
            skip_snippets,
        };
        unified_search(search_index.as_deref(), &jsonl_files, &opts)
    })
    .await
    .map_err(|e| {
        let msg = if e.is_panic() {
            let panic_payload = e.into_panic();
            if let Some(s) = panic_payload.downcast_ref::<String>() {
                format!("Search panicked: {s}")
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                format!("Search panicked: {s}")
            } else {
                "Search panicked (unknown payload)".to_string()
            }
        } else {
            format!("Search task failed: {e}")
        };
        tracing::error!("{msg}");
        ApiError::Internal(msg)
    })?
    .map_err(|e| ApiError::Internal(format!("Search failed: {e}")))?;

    let mut response = result.response;
    response.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(response)
}
