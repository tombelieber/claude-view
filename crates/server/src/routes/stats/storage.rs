//! GET /api/stats/storage — Storage statistics for the settings page.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::Json;
use claude_view_core::claude_projects_dir;

use crate::error::ApiResult;
use crate::metrics::record_request;
use crate::state::AppState;

use super::helpers::{calculate_directory_size, calculate_jsonl_size};
use super::types::StorageStats;

/// GET /api/stats/storage - Storage statistics for the settings page.
///
/// Returns:
/// - Storage sizes: JSONL files, SQLite database, obsolete search index cache
/// - Counts: sessions, projects, commits
/// - Timing: oldest session, last index, last git sync
#[utoipa::path(get, path = "/api/stats/storage", tag = "stats",
    responses(
        (status = 200, description = "Storage usage statistics (JSONL, SQLite, cache)", body = StorageStats),
    )
)]
pub async fn storage_stats(State(state): State<Arc<AppState>>) -> ApiResult<Json<StorageStats>> {
    let start = Instant::now();

    // Get index metadata for timing info
    let metadata = match state.db.get_index_metadata().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(
                endpoint = "storage_stats",
                error = %e,
                "Failed to fetch index metadata"
            );
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Get all counts in a single query (replaces 4 separate queries)
    let (session_count, project_count, commit_count, oldest_session_date) = match state
        .db
        .get_storage_counts()
        .await
    {
        Ok(counts) => counts,
        Err(e) => {
            tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get storage counts");
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Calculate JSONL storage size
    let jsonl_bytes = calculate_jsonl_size().await;

    // Calculate SQLite database size
    let sqlite_bytes = match state.db.get_database_size().await {
        Ok(size) => size as u64,
        Err(e) => {
            tracing::error!(endpoint = "storage_stats", error = %e, "Failed to get database size");
            record_request("storage_stats", "500", start.elapsed());
            return Err(e.into());
        }
    };

    // Obsolete session search cache size — measured from actual directory on disk.
    // Startup normally removes this now that session search is grep-only.
    let index_bytes = match claude_view_core::paths::obsolete_session_search_index_dir() {
        Some(dir) if dir.exists() => calculate_directory_size(&dir).await,
        _ => 0,
    };

    // Resolve display paths (replace $HOME with ~ for readability)
    let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
    let shorten = |p: Option<std::path::PathBuf>| -> Option<String> {
        p.map(|path| {
            let s = path.to_string_lossy().to_string();
            match &home {
                Some(h) if s.starts_with(h.as_str()) => format!("~{}", &s[h.len()..]),
                _ => s,
            }
        })
    };

    let jsonl_path = shorten(claude_projects_dir().ok());
    let sqlite_path = shorten(claude_view_core::paths::db_path());
    let index_path = shorten(claude_view_core::paths::obsolete_session_search_index_dir());
    let app_data_path = shorten(Some(claude_view_core::paths::data_dir()));

    record_request("storage_stats", "200", start.elapsed());

    Ok(Json(StorageStats {
        jsonl_bytes,
        sqlite_bytes,
        index_bytes,
        session_count,
        project_count,
        commit_count,
        oldest_session_date,
        last_index_at: metadata.last_indexed_at,
        last_index_duration_ms: metadata.last_index_duration_ms,
        last_index_session_count: metadata.sessions_indexed,
        last_git_sync_at: metadata.last_git_sync_at,
        last_git_sync_duration_ms: None, // Not tracked currently
        last_git_sync_repo_count: 0,     // Not tracked currently
        jsonl_path,
        sqlite_path,
        index_path,
        app_data_path,
    }))
}
