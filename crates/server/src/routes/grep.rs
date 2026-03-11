//! Grep search endpoint.
//!
//! - GET /grep?pattern=...&project=...&limit=...&caseSensitive=...&wholeWord=...
//!   Regex search over raw JSONL session files using ripgrep core crates.

use std::sync::Arc;

use axum::{extract::Query, routing::get, Json, Router};
use serde::Deserialize;
use tokio::task::spawn_blocking;

use claude_view_core::discovery::{claude_projects_dir, resolve_project_path_with_cwd};
use claude_view_search::grep_types::GrepResponse;
use claude_view_search::{grep_files, GrepOptions, JsonlFile};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct GrepQuery {
    pub pattern: Option<String>,
    pub project: Option<String>,
    pub limit: Option<usize>,
    #[serde(rename = "caseSensitive")]
    pub case_sensitive: Option<bool>,
    #[serde(rename = "wholeWord")]
    pub whole_word: Option<bool>,
}

/// Build the grep sub-router.
///
/// Routes:
/// - `GET /grep` — Regex search over raw JSONL files
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/grep", get(grep_handler))
}

/// GET /api/grep — Regex search over raw JSONL session files.
///
/// Query parameters:
/// - `pattern` (required): Regex pattern to search for
/// - `project`: Optional project name filter
/// - `limit`: Max matches to return (default 200, capped at 1000)
/// - `caseSensitive`: Whether the search is case-sensitive (default false)
/// - `wholeWord`: Whether to match whole words only (default false)
async fn grep_handler(Query(params): Query<GrepQuery>) -> ApiResult<Json<GrepResponse>> {
    let pattern = params
        .pattern
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| ApiError::BadRequest("Missing 'pattern' parameter".into()))?;

    let limit = params.limit.unwrap_or(200).min(1000);
    let case_sensitive = params.case_sensitive.unwrap_or(false);
    let whole_word = params.whole_word.unwrap_or(false);

    let project_filter = params.project;

    // Move all blocking filesystem I/O into spawn_blocking to avoid blocking
    // the Tokio event loop (directory scan + grep search).
    let result = spawn_blocking(move || {
        let files = collect_jsonl_files(project_filter.as_deref())?;

        let opts = GrepOptions {
            pattern,
            case_sensitive,
            whole_word,
            limit,
        };

        grep_files(&files, &opts).map_err(|e| ApiError::BadRequest(format!("{e}")))
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Grep task failed: {e}")))??;

    Ok(Json(result))
}

/// Scan ~/.claude/projects/ for all JSONL session files.
/// Optionally filter by project display name or full path.
///
/// Used by both `/api/grep` and `/api/search` (unified search grep fallback).
///
/// NOTE: project filter checks BOTH display_name AND full_path to match
/// the polymorphic project filter pattern (CLAUDE.md Hard Rule).
pub fn collect_jsonl_files(project_filter: Option<&str>) -> Result<Vec<JsonlFile>, ApiError> {
    let projects_dir =
        claude_projects_dir().map_err(|e| ApiError::Internal(format!("Projects dir: {e}")))?;

    let mut files: Vec<JsonlFile> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            let resolved = resolve_project_path_with_cwd(&dir_name, None);

            if let Some(proj) = project_filter {
                if resolved.display_name != proj && resolved.full_path != proj {
                    continue;
                }
            }

            if let Ok(sessions) = std::fs::read_dir(&project_dir) {
                for session in sessions.flatten() {
                    let path = session.path();
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        let session_id = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let modified_at = path
                            .metadata()
                            .and_then(|m| m.modified())
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                            })
                            .unwrap_or(0);

                        files.push(JsonlFile {
                            path,
                            session_id,
                            project: resolved.display_name.clone(),
                            project_path: resolved.full_path.clone(),
                            modified_at,
                        });
                    }
                }
            }
        }
    }

    Ok(files)
}
