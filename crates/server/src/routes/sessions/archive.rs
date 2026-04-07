//! Archive and unarchive handlers (single + bulk).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::types::{ArchiveResponse, BulkArchiveRequest, BulkArchiveResponse};

/// Returns archive dir — respects `CLAUDE_VIEW_DATA_DIR` for sandbox envs.
fn archive_base_dir() -> std::path::PathBuf {
    claude_view_core::paths::archive_dir()
}

#[utoipa::path(post, path = "/api/sessions/{id}/archive", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session archived successfully", body = ArchiveResponse),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn archive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let file_path = state
        .db
        .archive_session(&id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to archive session {id}: {e}");
            ApiError::Internal(format!("archive failed: {e}"))
        })?
        .ok_or(ApiError::NotFound(format!(
            "Session {id} not found or already archived"
        )))?;

    // Move file to ~/.claude-view/archives/
    let src = std::path::PathBuf::from(&file_path);
    let archive_dir = archive_base_dir();
    if let Some(project_dir) = src.parent().and_then(|p| p.file_name()) {
        let dest_dir = archive_dir.join(project_dir);

        // Attempt file move — failure is non-fatal (DB flag is the source of truth)
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            tracing::warn!("Failed to create archive dir: {e}");
        } else if let Some(file_name) = src.file_name() {
            let dest = dest_dir.join(file_name);
            match tokio::fs::rename(&src, &dest).await {
                Ok(()) => {
                    if let Some(dest_str) = dest.to_str() {
                        if let Err(e) = state.db.update_session_file_path(&id, dest_str).await {
                            tracing::error!(session_id = %id, error = %e, "failed to update session file path in DB after move");
                            // Note: file has been moved but DB not updated — this is a data integrity issue
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to move session file to archive: {e}");
                    // DB already marked as archived — indexer guard will skip it
                }
            }
        }
    }

    Ok(Json(ArchiveResponse { archived: true }))
}

#[utoipa::path(post, path = "/api/sessions/{id}/unarchive", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session unarchived successfully", body = ArchiveResponse),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn unarchive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let current_path = state
        .db
        .get_session_file_path(&id)
        .await
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or(ApiError::NotFound(format!("Session {id} not found")))?;

    let archive_base = archive_base_dir();
    let current = std::path::PathBuf::from(&current_path);

    let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
        // Security: validate no path traversal in relative components
        use std::path::Component;
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(ApiError::BadRequest("Invalid archive path".to_string()));
        }

        let Some(home) = dirs::home_dir() else {
            return Err(ApiError::Internal(
                "Cannot determine home directory".to_string(),
            ));
        };
        let original = home.join(".claude").join("projects").join(relative);

        // Move file back — failure is non-fatal
        if current.exists() {
            if let Some(parent) = original.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Err(e) = tokio::fs::rename(&current, &original).await {
                tracing::warn!("Failed to move file back from archive: {e}");
            }
        }

        original.to_string_lossy().to_string()
    } else {
        // File not in archive dir (file move failed during archive) — just clear the flag
        current_path
    };

    state
        .db
        .unarchive_session(&id, &new_path)
        .await
        .map_err(|e| ApiError::Internal(format!("unarchive failed: {e}")))?;

    Ok(Json(ArchiveResponse { archived: false }))
}

#[utoipa::path(post, path = "/api/sessions/archive", tag = "sessions",
    request_body = BulkArchiveRequest,
    responses(
        (status = 200, description = "Sessions archived in bulk", body = BulkArchiveResponse),
    )
)]
pub async fn bulk_archive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let results = state
        .db
        .archive_sessions_bulk(&body.ids)
        .await
        .map_err(|e| ApiError::Internal(format!("bulk archive failed: {e}")))?;

    let archive_dir = archive_base_dir();
    for (id, file_path) in &results {
        let src = std::path::PathBuf::from(file_path);
        let Some(project_dir) = src.parent().and_then(|p| p.file_name()) else {
            continue;
        };
        let dest_dir = archive_dir.join(project_dir);
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            tracing::warn!("Bulk archive: failed to create dir {dest_dir:?}: {e}");
            continue;
        }
        if let Some(file_name) = src.file_name() {
            let dest = dest_dir.join(file_name);
            if let Ok(()) = tokio::fs::rename(&src, &dest).await {
                if let Some(dest_str) = dest.to_str() {
                    if let Err(e) = state.db.update_session_file_path(id, dest_str).await {
                        tracing::error!(session_id = %id, error = %e, "failed to update session file path in DB after move");
                        // Note: file has been moved but DB not updated — this is a data integrity issue
                    }
                }
            }
        }
    }

    Ok(Json(BulkArchiveResponse {
        archived_count: results.len(),
    }))
}

#[utoipa::path(post, path = "/api/sessions/unarchive", tag = "sessions",
    request_body = BulkArchiveRequest,
    responses(
        (status = 200, description = "Sessions unarchived in bulk", body = BulkArchiveResponse),
    )
)]
pub async fn bulk_unarchive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let archive_base = archive_base_dir();
    let mut file_paths: Vec<(String, String)> = Vec::new();

    let Some(home) = dirs::home_dir() else {
        return Err(ApiError::Internal(
            "Cannot determine home directory".to_string(),
        ));
    };

    for id in &body.ids {
        let current_path = match state.db.get_session_file_path(id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                tracing::warn!("Bulk unarchive: session {id} not found, skipping");
                continue;
            }
            Err(e) => {
                tracing::warn!("Bulk unarchive: DB error for session {id}: {e}");
                continue;
            }
        };
        let current = std::path::PathBuf::from(&current_path);
        let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
            use std::path::Component;
            if !relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            {
                tracing::warn!("Bulk unarchive: path traversal in {id}, skipping");
                continue;
            }
            let original = home.join(".claude").join("projects").join(relative);
            if current.exists() {
                if let Some(parent) = original.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Err(e) = tokio::fs::rename(&current, &original).await {
                    tracing::warn!(
                        "Bulk unarchive: failed to move {current:?} → {original:?}: {e}"
                    );
                }
            }
            original.to_string_lossy().to_string()
        } else {
            current_path
        };
        file_paths.push((id.clone(), new_path));
    }

    let count = state
        .db
        .unarchive_sessions_bulk(&file_paths)
        .await
        .map_err(|e| ApiError::Internal(format!("bulk unarchive failed: {e}")))?;

    Ok(Json(BulkArchiveResponse {
        archived_count: count,
    }))
}
