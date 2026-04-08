//! Shared helpers for stats endpoints (session breakdown, filesystem size).

use std::path::Path;
use std::sync::Arc;

use claude_view_core::{claude_projects_dir, AnalyticsSessionBreakdown};

use crate::error::ApiResult;
use crate::state::AppState;

/// Fetch primary/sidechain session breakdown for the given filters.
pub(super) async fn fetch_session_breakdown(
    state: &Arc<AppState>,
    from: Option<i64>,
    to: Option<i64>,
    project: Option<&str>,
    branch: Option<&str>,
) -> ApiResult<AnalyticsSessionBreakdown> {
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN is_sidechain = 0 THEN 1 ELSE 0 END), 0) AS primary_sessions,
            COALESCE(SUM(CASE WHEN is_sidechain = 1 THEN 1 ELSE 0 END), 0) AS sidechain_sessions
        FROM sessions
        WHERE archived_at IS NULL
          AND (?1 IS NULL OR last_message_at >= ?1)
          AND (?2 IS NULL OR last_message_at <= ?2)
          AND (?3 IS NULL OR project_id = ?3
               OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ?3)
               OR (project_path IS NOT NULL AND project_path <> '' AND project_path = ?3))
          AND (?4 IS NULL OR git_branch = ?4)
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(project)
    .bind(branch)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        crate::error::ApiError::Internal(format!("Failed to fetch session breakdown: {e}"))
    })?;

    Ok(AnalyticsSessionBreakdown::new(
        primary_sessions,
        sidechain_sessions,
    ))
}

/// Calculate total size of JSONL session files in ~/.claude/projects/.
pub(super) async fn calculate_jsonl_size() -> u64 {
    let projects_dir = match claude_projects_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to locate Claude projects directory for JSONL size calculation");
            return 0;
        }
    };

    calculate_directory_jsonl_size(&projects_dir).await
}

/// Recursively calculate the total size of .jsonl files in a directory.
async fn calculate_directory_jsonl_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, dir = %dir.display(), "Failed to read directory for JSONL size calculation");
            return 0;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "Failed to get file type during JSONL size calculation");
                continue;
            }
        };

        if file_type.is_dir() {
            // Recurse into subdirectories (project directories)
            total += Box::pin(calculate_directory_jsonl_size(&path)).await;
        } else if file_type.is_file() {
            // Only count .jsonl files
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => {
                        total += metadata.len();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, path = %path.display(), "Failed to get metadata for JSONL file");
                    }
                }
            }
        }
    }

    total
}

/// Recursively calculate the total size of all files in a directory.
pub(super) async fn calculate_directory_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return 0,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            total += Box::pin(calculate_directory_size(&path)).await;
        } else if file_type.is_file() {
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                total += metadata.len();
            }
        }
    }

    total
}
