//! Tantivy full-text search index bootstrap.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Uses the blue-green
//! versioned layout: if a schema bump is detected, the previous-version
//! index keeps serving queries while a background task builds the new
//! version at `v{N}/`. Returns a pending-migration descriptor that the
//! indexer spawner consumes after Pass 2 completes.

use std::sync::{Arc, RwLock};

use claude_view_core::app_config::AppConfig;

use crate::SearchIndexHolder;

/// Open the search index using the blue-green versioned layout.
///
/// Returns `(holder, pending_migration)`:
/// - `holder` is `Some(Arc<SearchIndex>)` when the current version opened
///   successfully, `None` when the feature is disabled or the open failed.
/// - `pending_migration` carries the rebuild plan when the open detected a
///   schema bump (holder points at the previous version as a fallback).
pub fn open_index(
    app_config: &AppConfig,
) -> (
    SearchIndexHolder,
    Option<claude_view_search::migration::PendingMigration>,
) {
    if !app_config.features.search {
        tracing::info!("Search feature disabled by config");
        return (Arc::new(RwLock::new(None)), None);
    }

    let index_dir = claude_view_core::paths::search_index_dir()
        .expect("search_index_dir() always returns Some after data_dir() refactor");

    match claude_view_search::SearchIndex::open_versioned(&index_dir) {
        Ok(result) => {
            tracing::info!(
                "Search index opened at {} (pending_migration={})",
                index_dir.display(),
                result.pending_migration.is_some()
            );
            let holder = Arc::new(RwLock::new(Some(Arc::new(result.index))));
            (holder, result.pending_migration)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to open search index: {}. Search will be unavailable.",
                e
            );
            (Arc::new(RwLock::new(None)), None)
        }
    }
}
