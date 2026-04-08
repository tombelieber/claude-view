//! Filesystem discovery of session indexes and orphan sessions.

use std::path::Path;
use tracing::warn;

use crate::error::SessionIndexError;

use super::classify::classify_jsonl_file;
use super::parse::parse_session_index;
use super::types::{SessionIndexEntry, SessionKind};

/// Resolve the cwd for a project directory by scanning one JSONL file.
/// Returns the first `cwd` field found in any JSONL file in the directory.
/// Used when sessions-index.json entries don't carry cwd (they never do).
///
/// Scans at most 3 files -- cwd is present in 98.9% of conversation files,
/// typically on the very first line. Returns None only for metadata-only dirs.
pub fn resolve_cwd_for_project(project_dir: &Path) -> Option<String> {
    let entries = match std::fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    let mut tried = 0u8;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        if let Ok(classification) = classify_jsonl_file(&path) {
            if classification.cwd.is_some() {
                return classification.cwd;
            }
        }

        tried += 1;
        if tried >= 3 {
            break;
        }
    }

    None
}

/// Discover and parse all `sessions-index.json` files under `claude_dir/projects/`.
///
/// Returns a list of `(project_dir_name, entries)` tuples. Directories without
/// a `sessions-index.json` are silently skipped. Malformed files produce a
/// warning log but do not stop processing of other projects.
///
/// After reading the index, also scans for `.jsonl` files in the same directory
/// that are not listed in the index (catch-up for stale index files).
pub fn read_all_session_indexes(
    claude_dir: &Path,
) -> Result<Vec<(String, Vec<SessionIndexEntry>)>, SessionIndexError> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Err(SessionIndexError::ProjectsDirNotFound { path: projects_dir });
    }

    let entries =
        std::fs::read_dir(&projects_dir).map_err(|e| SessionIndexError::io(&projects_dir, e))?;

    let mut results = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    "Failed to read directory entry in {}: {}",
                    projects_dir.display(),
                    e
                );
                continue;
            }
        };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let index_path = path.join("sessions-index.json");
        if !index_path.exists() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                warn!("Skipping directory with non-UTF-8 name: {}", path.display());
                continue;
            }
        };

        match parse_session_index(&index_path) {
            Ok(mut session_entries) => {
                // Catch-up: scan for JSONL files not listed in the index.
                // Claude Code may create sessions without updating sessions-index.json.
                let indexed_ids: std::collections::HashSet<String> = session_entries
                    .iter()
                    .map(|e| e.session_id.clone())
                    .collect();

                if let Ok(dir_contents) = std::fs::read_dir(&path) {
                    for file_entry in dir_contents.flatten() {
                        let file_path = file_entry.path();
                        if file_path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                            continue;
                        }
                        let session_id = match file_path.file_stem().and_then(|s| s.to_str()) {
                            Some(stem) => stem,
                            None => continue,
                        };
                        if indexed_ids.contains(session_id) {
                            continue;
                        }
                        // Classify first, skip non-conversation files.
                        // Same filter discover_orphan_sessions() uses.
                        let classification = match classify_jsonl_file(&file_path) {
                            Ok(c) => c,
                            Err(e) => {
                                warn!("Failed to classify {}: {}", file_path.display(), e);
                                continue;
                            }
                        };
                        if classification.kind != SessionKind::Conversation {
                            continue;
                        }
                        session_entries.push(SessionIndexEntry {
                            session_id: session_id.to_string(),
                            full_path: Some(file_path.to_string_lossy().to_string()),
                            file_mtime: None,
                            first_prompt: None,
                            summary: None,
                            message_count: None,
                            created: None,
                            modified: None,
                            git_branch: None,
                            project_path: None,
                            is_sidechain: None,
                            session_cwd: classification.cwd,
                            parent_session_id: classification.parent_id,
                        });
                    }
                }

                results.push((dir_name, session_entries));
            }
            Err(e) => {
                warn!(
                    "Skipping malformed sessions-index.json in {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(results)
}

/// Discover sessions in project directories that lack a `sessions-index.json`.
///
/// These "orphan" sessions are typically found in worktree project directories
/// where Claude Code hasn't written an index file. The function scans for `.jsonl`
/// files and creates minimal `SessionIndexEntry` records from the filenames.
///
/// Returns a list of `(project_dir_name, entries)` tuples, only for directories
/// that have at least one `.jsonl` file and do NOT have a `sessions-index.json`.
pub fn discover_orphan_sessions(
    claude_dir: &Path,
) -> Result<Vec<(String, Vec<SessionIndexEntry>)>, SessionIndexError> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let dir_entries =
        std::fs::read_dir(&projects_dir).map_err(|e| SessionIndexError::io(&projects_dir, e))?;

    let mut results = Vec::new();

    for entry in dir_entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    "Failed to read directory entry in {}: {}",
                    projects_dir.display(),
                    e
                );
                continue;
            }
        };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip directories that already have a sessions-index.json --
        // those are handled by read_all_session_indexes.
        if path.join("sessions-index.json").exists() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                warn!("Skipping directory with non-UTF-8 name: {}", path.display());
                continue;
            }
        };

        let dir_contents = match std::fs::read_dir(&path) {
            Ok(contents) => contents,
            Err(e) => {
                warn!("Failed to read directory {}: {}", path.display(), e);
                continue;
            }
        };

        let mut session_entries = Vec::new();

        for file_entry in dir_contents {
            let file_entry = match file_entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read file entry in {}: {}", path.display(), e);
                    continue;
                }
            };

            let file_path = file_entry.path();

            // Only consider .jsonl files
            if file_path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }

            let session_id = match file_path.file_stem().and_then(|s| s.to_str()) {
                Some(stem) => stem.to_string(),
                None => continue,
            };

            // Classify the file content before including it
            let classification = match classify_jsonl_file(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to classify {}: {}", file_path.display(), e);
                    continue;
                }
            };

            // Only include actual conversation sessions
            if classification.kind != SessionKind::Conversation {
                continue;
            }

            let full_path = file_path.to_string_lossy().to_string();

            session_entries.push(SessionIndexEntry {
                session_id,
                full_path: Some(full_path),
                file_mtime: None,
                first_prompt: None,
                summary: None,
                message_count: None,
                created: None,
                modified: None,
                git_branch: None,
                project_path: None,
                is_sidechain: None,
                session_cwd: classification.cwd,
                parent_session_id: classification.parent_id,
            });
        }

        if !session_entries.is_empty() {
            results.push((dir_name, session_entries));
        }
    }

    Ok(results)
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
