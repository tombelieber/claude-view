//! Parser for Claude Code's `sessions-index.json` files.

use std::path::Path;

use crate::error::SessionIndexError;

use super::types::{SessionIndexEntry, SessionIndexFile};

/// Parse a single `sessions-index.json` file into a list of entries.
pub fn parse_session_index(path: &Path) -> Result<Vec<SessionIndexEntry>, SessionIndexError> {
    let contents = std::fs::read_to_string(path).map_err(|e| SessionIndexError::io(path, e))?;
    // Try wrapper format {"version": N, "entries": [...]} first,
    // fall back to bare array [...] for backward compatibility.
    if let Ok(file) = serde_json::from_str::<SessionIndexFile>(&contents) {
        return Ok(file.entries);
    }
    let entries: Vec<SessionIndexEntry> =
        serde_json::from_str(&contents).map_err(|e| SessionIndexError::MalformedJson {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(entries)
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
        let path = std::path::Path::new("/nonexistent/sessions-index.json");
        let result = parse_session_index(path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SessionIndexError::NotFound { .. }
        ));
    }
}
