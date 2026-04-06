//! Memory viewer API endpoints.
//!
//! - GET /api/memory — all memory entries (global + per-project)
//! - GET /api/memory/:project — memories for a specific project
//! - GET /api/memory/file — read a single memory file by path

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use claude_view_core::memory_files::{self, MemoryEntry, MemoryIndex};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// GET /api/memory — returns all memory entries grouped by scope.
#[utoipa::path(get, path = "/api/memory", tag = "memory",
    responses(
        (status = 200, description = "All memory entries", body = MemoryIndex),
    )
)]
pub async fn get_all_memories(State(_state): State<Arc<AppState>>) -> ApiResult<Json<MemoryIndex>> {
    let index = tokio::task::spawn_blocking(memory_files::discover_all_memories)
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(index))
}

/// GET /api/memory/:project — returns memories for a specific project.
#[utoipa::path(get, path = "/api/memory/{project}", tag = "memory",
    params(("project" = String, Path, description = "Encoded project directory name")),
    responses(
        (status = 200, description = "Project memory entries", body = Vec<MemoryEntry>),
    )
)]
pub async fn get_project_memories(
    State(_state): State<Arc<AppState>>,
    Path(project): Path<String>,
) -> ApiResult<Json<Vec<MemoryEntry>>> {
    let memories =
        tokio::task::spawn_blocking(move || memory_files::read_project_memories(&project))
            .await
            .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?;

    Ok(Json(memories))
}

#[derive(Deserialize)]
pub struct MemoryFileQuery {
    pub path: String,
}

/// GET /api/memory/file?path=... — read a single memory file.
#[utoipa::path(get, path = "/api/memory/file", tag = "memory",
    params(("path" = String, Query, description = "Relative path from ~/.claude/")),
    responses(
        (status = 200, description = "Single memory entry", body = MemoryEntry),
        (status = 404, description = "Memory file not found"),
    )
)]
pub async fn get_memory_file(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<MemoryFileQuery>,
) -> ApiResult<Json<MemoryEntry>> {
    let path = query.path;
    let entry = tokio::task::spawn_blocking(move || memory_files::read_memory_file(&path))
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Memory file not found".to_string()))?;

    Ok(Json(entry))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // /file route MUST be before /:project to avoid ambiguity
        .route("/memory/file", get(get_memory_file))
        .route("/memory/{project}", get(get_project_memories))
        .route("/memory", get(get_all_memories))
}
