//! Handler functions for system endpoints.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use claude_view_core::ClaudeCliStatus;
use claude_view_db::SystemStorageStats;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::types::{
    ActionResponse, CheckPathQuery, CheckPathResponse, ClearCacheResponse, IndexRunInfo,
    IntegrityInfo, PerformanceInfo, ResetRequest, SystemResponse,
};

/// GET /api/system - Get comprehensive system status.
///
/// Runs storage, health, and classification queries in parallel
/// using tokio::join!, then detects Claude CLI status.
#[utoipa::path(get, path = "/api/system", tag = "system",
    responses(
        (status = 200, description = "Comprehensive system status including storage, health, and classification", body = crate::routes::system::SystemResponse),
    )
)]
pub async fn get_system_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SystemResponse>> {
    // Run independent queries in parallel
    let (
        storage_result,
        health_result,
        metadata_result,
        index_runs_result,
        classification_result,
        integrity_result,
    ) = tokio::join!(
        state.db.get_storage_stats(),
        state.db.get_health_stats(),
        state.db.get_index_metadata(),
        state.db.get_recent_index_runs(),
        state.db.get_classification_status(),
        state.db.get_latest_integrity_counters(),
    );

    let storage_stats = storage_result?;
    let health_stats = health_result?;
    let metadata = metadata_result?;
    let index_runs = index_runs_result?;
    let classification = classification_result?;
    let integrity_counters = integrity_result?;

    // Detect Claude CLI (runs shell commands - fast enough for API call)
    let claude_cli = tokio::task::spawn_blocking(ClaudeCliStatus::detect)
        .await
        .unwrap_or_else(|_| ClaudeCliStatus::default());

    // Calculate performance metrics from index metadata
    let performance = calculate_performance(&metadata, &storage_stats);

    // Convert index runs to response format
    let index_history: Vec<IndexRunInfo> = index_runs
        .into_iter()
        .map(|run| IndexRunInfo {
            timestamp: run.started_at,
            run_type: run.run_type.as_db_str().to_string(),
            sessions_count: run.sessions_after,
            duration_ms: run.duration_ms,
            status: run.status.as_db_str().to_string(),
            error_message: run.error_message,
        })
        .collect();

    let response = SystemResponse {
        storage: storage_stats.into(),
        performance,
        health: health_stats.into(),
        integrity: IntegrityInfo {
            counters: integrity_counters.into(),
        },
        index_history,
        classification: classification.into(),
        claude_cli,
    };

    Ok(Json(response))
}

/// Calculate performance metrics from index metadata and storage stats.
pub(super) fn calculate_performance(
    metadata: &claude_view_db::IndexMetadata,
    storage: &SystemStorageStats,
) -> PerformanceInfo {
    let last_index_duration_ms = metadata.last_index_duration_ms;

    let throughput_bytes_per_sec = match (last_index_duration_ms, storage.jsonl_bytes) {
        (Some(duration_ms), bytes) if duration_ms > 0 => {
            Some((bytes as f64 / (duration_ms as f64 / 1000.0)) as u64)
        }
        _ => None,
    };

    let sessions_per_sec = match (last_index_duration_ms, metadata.sessions_indexed) {
        (Some(duration_ms), sessions) if duration_ms > 0 => {
            Some(sessions as f64 / (duration_ms as f64 / 1000.0))
        }
        _ => None,
    };

    PerformanceInfo {
        last_index_duration_ms,
        throughput_bytes_per_sec,
        sessions_per_sec,
    }
}

/// POST /api/system/reindex - Trigger a full re-index.
///
/// This is a lightweight endpoint that signals the server to start
/// a background re-index. The actual work happens asynchronously.
#[utoipa::path(post, path = "/api/system/reindex", tag = "system",
    responses(
        (status = 200, description = "Re-index triggered", body = serde_json::Value),
    )
)]
pub async fn trigger_reindex(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    // Record the index run in the database
    let _run_id = state
        .db
        .create_index_run("full", None, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create index run: {}", e)))?;

    // Signal the indexing state to trigger a re-index
    state.indexing.trigger_reindex();

    if let Some(ref client) = state.telemetry {
        client.track(
            "reindex_triggered",
            serde_json::json!({ "trigger": "manual" }),
        );
    }

    Ok(Json(ActionResponse {
        status: "started".to_string(),
        message: Some("Full re-index started".to_string()),
    }))
}

/// POST /api/system/clear-cache - Clear search index and cached data.
///
/// Uses a take-drop-recreate pattern:
/// 1. Write-lock the holder, `.take()` the old `Arc<SearchIndex>` (holder becomes `None`)
/// 2. Call `clear_all()` on the old index (flushes deletes via Tantivy API)
/// 3. Drop the old index — releases mmap handles and file locks
/// 4. `remove_dir_all()` — now safe, no live handles
/// 5. `SearchIndex::open()` — creates fresh empty index
/// 6. Write-lock holder, swap in the new `Arc<SearchIndex>`
#[utoipa::path(post, path = "/api/system/clear-cache", tag = "system",
    responses(
        (status = 200, description = "Cache cleared, returns bytes freed", body = serde_json::Value),
    )
)]
pub async fn clear_cache(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ClearCacheResponse>> {
    let cache_dir = claude_view_core::paths::search_index_dir();

    // Measure size before clearing
    let size_before = cache_dir
        .as_ref()
        .filter(|d| d.exists())
        .map(|d| calculate_dir_size(d))
        .unwrap_or(0);

    // Step 1: Take the old index out of the holder (sets holder to None)
    let old_index = state
        .search_index
        .write()
        .map_err(|_| ApiError::Internal("search index lock poisoned".into()))?
        .take();

    // Step 2-3: Clear and drop the old index (releases mmap handles)
    if let Some(old) = old_index {
        if let Err(e) = old.clear_all() {
            tracing::warn!("Failed to clear old search index: {}", e);
        }
        drop(old);
    }

    // Step 4: Remove the directory on disk (safe — no live handles)
    if let Some(ref dir) = cache_dir {
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(dir) {
                tracing::warn!("Failed to remove search index directory: {}", e);
            }
        }
    }

    // Step 5-6: Create a fresh index and swap it into the holder
    if let Some(ref dir) = cache_dir {
        match claude_view_search::SearchIndex::open(dir) {
            Ok(new_idx) => {
                let mut guard = state
                    .search_index
                    .write()
                    .map_err(|_| ApiError::Internal("search index lock poisoned".into()))?;
                *guard = Some(Arc::new(new_idx));
                tracing::info!("Search index recreated at {}", dir.display());
            }
            Err(e) => {
                tracing::warn!("Failed to recreate search index: {}. Search will be unavailable until next restart.", e);
            }
        }
    }

    // Measure size after clearing to compute freed bytes
    let size_after = cache_dir
        .as_ref()
        .filter(|d| d.exists())
        .map(|d| calculate_dir_size(d))
        .unwrap_or(0);

    let cleared_bytes = size_before.saturating_sub(size_after);

    if let Some(ref client) = state.telemetry {
        client.track(
            "reindex_triggered",
            serde_json::json!({ "trigger": "clear_cache" }),
        );
    }

    Ok(Json(ClearCacheResponse {
        status: "success".to_string(),
        cleared_bytes,
    }))
}

/// POST /api/system/git-resync - Trigger full git re-sync.
#[utoipa::path(post, path = "/api/system/git-resync", tag = "system",
    responses(
        (status = 200, description = "Git re-sync triggered (stub)", body = serde_json::Value),
    )
)]
pub async fn trigger_git_resync(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    Ok(Json(ActionResponse {
        status: "not_implemented".to_string(),
        message: Some("Git re-sync is not yet available".to_string()),
    }))
}

/// POST /api/system/reset - Factory reset all data.
///
/// Requires a confirmation string "RESET_ALL_DATA" in the request body.
#[utoipa::path(post, path = "/api/system/reset", tag = "system",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Factory reset completed", body = serde_json::Value),
        (status = 400, description = "Missing or invalid confirmation string"),
    )
)]
pub async fn reset_all(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResetRequest>,
) -> ApiResult<Json<ActionResponse>> {
    // Require exact confirmation string
    if body.confirm != "RESET_ALL_DATA" {
        return Err(ApiError::BadRequest(
            "Invalid confirmation. Send {\"confirm\": \"RESET_ALL_DATA\"} to confirm.".to_string(),
        ));
    }

    state
        .db
        .reset_all_data()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset data: {}", e)))?;

    // Also clear the search index
    if let Ok(guard) = state.search_index.read() {
        if let Some(ref search) = *guard {
            if let Err(e) = search.clear_all() {
                tracing::warn!("Failed to clear search index during reset: {}", e);
            }
        }
    }

    Ok(Json(ActionResponse {
        status: "success".to_string(),
        message: Some("All data has been reset".to_string()),
    }))
}

/// Calculate the size of a directory recursively.
fn calculate_dir_size(dir: &std::path::Path) -> u64 {
    if !dir.exists() {
        return 0;
    }

    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += calculate_dir_size(&path);
            }
        }
    }
    total
}

/// GET /api/check-path?path=... — Check whether a filesystem path still exists.
///
/// Used by the frontend to validate project paths before offering "resume session".
/// Worktree directories can be removed, making the session un-resumable.
#[utoipa::path(get, path = "/api/check-path", tag = "system",
    params(CheckPathQuery),
    responses((status = 200, description = "Path existence check", body = CheckPathResponse))
)]
pub async fn check_path(Query(q): Query<CheckPathQuery>) -> Json<CheckPathResponse> {
    let exists = std::path::Path::new(&q.path).exists();
    Json(CheckPathResponse { exists })
}
