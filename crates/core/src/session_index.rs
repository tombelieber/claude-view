// crates/core/src/session_index.rs
//! Parser for Claude Code's `sessions-index.json` files.
//!
//! Each Claude Code project stores a `sessions-index.json` that lists all
//! sessions with metadata (summary, message count, timestamps, etc.).

use serde::Deserialize;
use std::path::Path;
use tracing::warn;

use crate::error::SessionIndexError;

/// Wrapper for the `sessions-index.json` file format.
/// The file is `{"version": N, "entries": [...]}`.
#[derive(Debug, Clone, Deserialize)]
struct SessionIndexFile {
    #[allow(dead_code)]
    version: Option<u32>,
    entries: Vec<SessionIndexEntry>,
}

/// A single entry from a `sessions-index.json` file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    pub session_id: String,
    #[serde(default)]
    pub full_path: Option<String>,
    #[serde(default)]
    pub file_mtime: Option<u64>,
    #[serde(default)]
    pub first_prompt: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub message_count: Option<usize>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub modified: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,
}

/// Parse a single `sessions-index.json` file into a list of entries.
pub fn parse_session_index(path: &Path) -> Result<Vec<SessionIndexEntry>, SessionIndexError> {
    let contents = std::fs::read_to_string(path).map_err(|e| SessionIndexError::io(path, e))?;
    // Try wrapper format {"version": N, "entries": [...]} first,
    // fall back to bare array [...] for backward compatibility.
    if let Ok(file) = serde_json::from_str::<SessionIndexFile>(&contents) {
        return Ok(file.entries);
    }
    let entries: Vec<SessionIndexEntry> = serde_json::from_str(&contents).map_err(|e| {
        SessionIndexError::MalformedJson {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    Ok(entries)
}

/// Discover and parse all `sessions-index.json` files under `claude_dir/projects/`.
///
/// Returns a list of `(project_dir_name, entries)` tuples. Directories without
/// a `sessions-index.json` are silently skipped. Malformed files produce a
/// warning log but do not stop processing of other projects.
pub fn read_all_session_indexes(
    claude_dir: &Path,
) -> Result<Vec<(String, Vec<SessionIndexEntry>)>, SessionIndexError> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Err(SessionIndexError::ProjectsDirNotFound {
            path: projects_dir,
        });
    }

    let entries =
        std::fs::read_dir(&projects_dir).map_err(|e| SessionIndexError::io(&projects_dir, e))?;

    let mut results = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read directory entry in {}: {}", projects_dir.display(), e);
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
            Ok(session_entries) => {
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

        // Skip directories that already have a sessions-index.json —
        // those are handled by read_all_session_indexes.
        if path.join("sessions-index.json").exists() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                warn!(
                    "Skipping directory with non-UTF-8 name: {}",
                    path.display()
                );
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
                    warn!(
                        "Failed to read file entry in {}: {}",
                        path.display(),
                        e
                    );
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
            });
        }

        if !session_entries.is_empty() {
            results.push((dir_name, session_entries));
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_wrapper_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions-index.json");
        let json = r#"{"version": 1, "entries": [
            {
                "sessionId": "abc-123",
                "fullPath": "/tmp/abc-123.jsonl",
                "firstPrompt": "hello world",
                "summary": "Test session",
                "messageCount": 10,
                "gitBranch": "main",
                "isSidechain": false
            }
        ]}"#;
        std::fs::write(&path, json).unwrap();

        let entries = parse_session_index(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "abc-123");
        assert_eq!(entries[0].summary.as_deref(), Some("Test session"));
    }

    #[test]
    fn test_parse_bare_array_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions-index.json");
        let json = r#"[
            {
                "sessionId": "abc-123",
                "fullPath": "/tmp/abc-123.jsonl",
                "fileMtime": 1769364547212,
                "firstPrompt": "hello world",
                "summary": "Test session",
                "messageCount": 10,
                "created": "2026-01-25T16:42:56.852Z",
                "modified": "2026-01-25T17:18:30.718Z",
                "gitBranch": "main",
                "projectPath": "/home/user/project",
                "isSidechain": false
            }
        ]"#;
        std::fs::write(&path, json).unwrap();

        let entries = parse_session_index(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "abc-123");
        assert_eq!(entries[0].full_path.as_deref(), Some("/tmp/abc-123.jsonl"));
        assert_eq!(entries[0].file_mtime, Some(1769364547212));
        assert_eq!(entries[0].first_prompt.as_deref(), Some("hello world"));
        assert_eq!(entries[0].summary.as_deref(), Some("Test session"));
        assert_eq!(entries[0].message_count, Some(10));
        assert_eq!(entries[0].git_branch.as_deref(), Some("main"));
        assert_eq!(entries[0].is_sidechain, Some(false));
    }

    #[test]
    fn test_parse_empty_array() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions-index.json");
        std::fs::write(&path, "[]").unwrap();

        let entries = parse_session_index(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_missing_optional_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions-index.json");
        let json = r#"[{"sessionId": "minimal-entry"}]"#;
        std::fs::write(&path, json).unwrap();

        let entries = parse_session_index(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "minimal-entry");
        assert!(entries[0].full_path.is_none());
        assert!(entries[0].file_mtime.is_none());
        assert!(entries[0].first_prompt.is_none());
        assert!(entries[0].summary.is_none());
        assert!(entries[0].message_count.is_none());
        assert!(entries[0].created.is_none());
        assert!(entries[0].modified.is_none());
        assert!(entries[0].git_branch.is_none());
        assert!(entries[0].project_path.is_none());
        assert!(entries[0].is_sidechain.is_none());
    }

    #[test]
    fn test_malformed_json_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions-index.json");
        std::fs::write(&path, "not valid json {{{").unwrap();

        let result = parse_session_index(&path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SessionIndexError::MalformedJson { .. }
        ));
    }

    #[test]
    fn test_file_not_found_returns_error() {
        let path = Path::new("/nonexistent/sessions-index.json");
        let result = parse_session_index(path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SessionIndexError::NotFound { .. }
        ));
    }

    #[test]
    fn test_read_all_session_indexes() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        // Project with a valid sessions-index.json
        let proj_a = projects_dir.join("project-a");
        std::fs::create_dir(&proj_a).unwrap();
        let json_a = r#"{"version":1,"entries":[{"sessionId": "sess-1"}, {"sessionId": "sess-2"}]}"#;
        std::fs::write(proj_a.join("sessions-index.json"), json_a).unwrap();

        // Project without sessions-index.json (should be skipped)
        let proj_b = projects_dir.join("project-b");
        std::fs::create_dir(&proj_b).unwrap();

        // Project with malformed JSON (should be skipped with warning)
        let proj_c = projects_dir.join("project-c");
        std::fs::create_dir(&proj_c).unwrap();
        std::fs::write(proj_c.join("sessions-index.json"), "broken").unwrap();

        let results = read_all_session_indexes(dir.path()).unwrap();
        // Only project-a should succeed
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "project-a");
        assert_eq!(results[0].1.len(), 2);
    }

    #[test]
    fn test_read_all_missing_projects_dir() {
        let dir = TempDir::new().unwrap();
        let result = read_all_session_indexes(dir.path());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SessionIndexError::ProjectsDirNotFound { .. }
        ));
    }

    // ========================================================================
    // discover_orphan_sessions Tests
    // ========================================================================

    #[test]
    fn test_discover_orphan_sessions_finds_jsonl_without_index() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let orphan_proj = projects_dir.join("orphan-project");
        std::fs::create_dir(&orphan_proj).unwrap();
        std::fs::write(orphan_proj.join("abc-123.jsonl"), "{}").unwrap();
        std::fs::write(orphan_proj.join("def-456.jsonl"), "{}").unwrap();

        let results = discover_orphan_sessions(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "orphan-project");
        assert_eq!(results[0].1.len(), 2);

        let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
        assert!(ids.contains(&"abc-123"));
        assert!(ids.contains(&"def-456"));

        // Verify full_path is set
        for entry in &results[0].1 {
            assert!(entry.full_path.is_some());
            assert!(entry.full_path.as_ref().unwrap().ends_with(".jsonl"));
        }
    }

    #[test]
    fn test_discover_orphan_sessions_skips_indexed_dirs() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let indexed_proj = projects_dir.join("indexed-project");
        std::fs::create_dir(&indexed_proj).unwrap();
        std::fs::write(indexed_proj.join("sessions-index.json"), "[]").unwrap();
        std::fs::write(indexed_proj.join("abc-123.jsonl"), "{}").unwrap();

        let results = discover_orphan_sessions(dir.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_discover_orphan_sessions_ignores_non_jsonl() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let proj = projects_dir.join("some-project");
        std::fs::create_dir(&proj).unwrap();
        std::fs::write(proj.join("notes.txt"), "text").unwrap();
        std::fs::write(proj.join("config.json"), "{}").unwrap();

        let results = discover_orphan_sessions(dir.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_discover_orphan_sessions_empty_projects_dir() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let results = discover_orphan_sessions(dir.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_discover_orphan_sessions_no_projects_dir() {
        let dir = TempDir::new().unwrap();
        let results = discover_orphan_sessions(dir.path()).unwrap();
        assert!(results.is_empty());
    }
}
