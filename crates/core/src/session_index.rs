// crates/core/src/session_index.rs
//! Parser for Claude Code's `sessions-index.json` files.
//!
//! Each Claude Code project stores a `sessions-index.json` that lists all
//! sessions with metadata (summary, message count, timestamps, etc.).

use serde::Deserialize;
use std::path::Path;
use tracing::warn;

use crate::error::SessionIndexError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_well_formed_json() {
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
        let json_a = r#"[{"sessionId": "sess-1"}, {"sessionId": "sess-2"}]"#;
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
}
