//! JSONL file collection utility for search.
//!
//! Scans `~/.claude/projects/` for session JSONL files.
//! Used by `search_service::execute_search()` for grep fallback.

use std::collections::HashSet;

use claude_view_core::discovery::{claude_projects_dir, resolve_project_path_with_cwd};
use claude_view_search::JsonlFile;

use crate::error::ApiError;

/// Scan ~/.claude/projects/ for all JSONL session files.
/// Optionally filter by project display name or full path.
///
/// Used by `search_service::execute_search()` (unified search grep fallback).
///
/// NOTE: project filter checks BOTH display_name AND full_path to match
/// the polymorphic project filter pattern (CLAUDE.md Hard Rule).
pub fn collect_jsonl_files(
    project_filter: Option<&str>,
    session_ids: Option<&HashSet<String>>,
) -> Result<Vec<JsonlFile>, ApiError> {
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
                        // If session_ids filter provided, skip files not in the set
                        if let Some(ids) = session_ids {
                            if !ids.contains(&session_id) {
                                continue;
                            }
                        }
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
