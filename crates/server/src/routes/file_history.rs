//! File history endpoints for session file diffs.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use claude_view_core::file_history::{self, FileDiffResponse, FileHistoryResponse};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct DiffQuery {
    pub from: u32,
    pub to: u32,
    /// If provided, skip JSONL re-parse and use this path directly.
    /// The frontend already has the resolved path from the file-history response.
    pub file_path: Option<String>,
}

/// GET /api/sessions/:id/file-history — List all file changes for a session.
pub async fn get_file_history(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<FileHistoryResponse>> {
    // Verify session exists and get its JSONL path
    let jsonl_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let history_dir = file_history::claude_file_history_dir()
        .ok_or_else(|| ApiError::Internal("Cannot determine home directory".to_string()))?;

    let jsonl = PathBuf::from(jsonl_path);
    let sid = session_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        let file_path_map = file_history::extract_file_path_map(&jsonl);
        file_history::scan_file_history(&history_dir, &session_id, &file_path_map)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error for session {sid}: {e}")))?;

    Ok(Json(result))
}

/// GET /api/sessions/:id/file-history/:file_hash/diff?from=N&to=M
pub async fn get_file_diff(
    State(state): State<Arc<AppState>>,
    Path((session_id, file_hash)): Path<(String, String)>,
    Query(query): Query<DiffQuery>,
) -> ApiResult<Json<FileDiffResponse>> {
    // Validate file_hash against path traversal
    file_history::validate_file_hash(&file_hash).map_err(ApiError::BadRequest)?;

    // Verify session exists and get its JSONL path
    let jsonl_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let history_dir = file_history::claude_file_history_dir()
        .ok_or_else(|| ApiError::Internal("Cannot determine home directory".to_string()))?;

    let from = query.from;
    let to = query.to;
    let hash = file_hash.clone();
    let jsonl = PathBuf::from(jsonl_path);
    let provided_file_path = query.file_path;
    let sid = session_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        // Use the provided file_path if available, otherwise fall back to JSONL parsing
        let file_path_display = if let Some(fp) = provided_file_path {
            fp
        } else {
            let file_path_map = file_history::extract_file_path_map(&jsonl);
            file_path_map
                .get(&file_hash)
                .cloned()
                .unwrap_or_else(|| file_hash.clone())
        };
        file_history::compute_diff(
            &history_dir,
            &session_id,
            &hash,
            from,
            to,
            &file_path_display,
            3,
        )
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error for session {sid}: {e}")))?
    .map_err(ApiError::Internal)?;

    Ok(Json(result))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{id}/file-history", get(get_file_history))
        .route(
            "/sessions/{id}/file-history/{file_hash}/diff",
            get(get_file_diff),
        )
}
