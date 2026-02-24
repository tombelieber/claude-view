// crates/core/src/session_index.rs
//! Parser for Claude Code's `sessions-index.json` files.
//!
//! Each Claude Code project stores a `sessions-index.json` that lists all
//! sessions with metadata (summary, message count, timestamps, etc.).

use memchr::memmem;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::warn;

use crate::error::SessionIndexError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind {
    Conversation,  // has user + assistant lines
    MetadataOnly,  // file-history-snapshot, summary, etc.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartType {
    User,
    FileHistorySnapshot,
    QueueOperation,
    Progress,
    Summary,
    Assistant,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SessionClassification {
    pub kind: SessionKind,
    pub start_type: StartType,
    pub cwd: Option<String>,
    pub parent_id: Option<String>,
}

/// Classify a JSONL file by scanning its content.
/// Uses memmem SIMD pre-filter — only JSON-parses lines that match.
pub fn classify_jsonl_file(path: &Path) -> Result<SessionClassification, SessionIndexError> {
    let file = File::open(path).map_err(|e| SessionIndexError::io(path, e))?;
    let reader = BufReader::new(file);

    let user_finder = memmem::Finder::new(br#""type":"user""#);
    let user_finder_spaced = memmem::Finder::new(br#""type": "user""#);
    let assistant_finder = memmem::Finder::new(br#""type":"assistant""#);
    let assistant_finder_spaced = memmem::Finder::new(br#""type": "assistant""#);

    let mut start_type = StartType::Unknown;
    let mut has_user = false;
    let mut has_assistant = false;
    let mut cwd: Option<String> = None;
    let mut parent_id: Option<String> = None;
    let mut first_line = true;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let bytes = line.as_bytes();

        // Determine start_type from first non-empty line
        if first_line && !line.trim().is_empty() {
            first_line = false;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                start_type = match obj.get("type").and_then(|v| v.as_str()) {
                    Some("user") => StartType::User,
                    Some("file-history-snapshot") => StartType::FileHistorySnapshot,
                    Some("queue-operation") => StartType::QueueOperation,
                    Some("progress") => StartType::Progress,
                    Some("summary") => StartType::Summary,
                    Some("assistant") => StartType::Assistant,
                    _ => StartType::Unknown,
                };
            }
        }

        // SIMD pre-filter: check for user/assistant type markers
        let is_user = user_finder.find(bytes).is_some()
            || user_finder_spaced.find(bytes).is_some();
        let is_assistant = assistant_finder.find(bytes).is_some()
            || assistant_finder_spaced.find(bytes).is_some();

        // Parse user lines to extract cwd and parentUuid
        if is_user && !has_user {
            has_user = true;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                if cwd.is_none() {
                    cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                parent_id = obj.get("parentUuid").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }

        if is_assistant {
            has_assistant = true;
        }

        // Extract cwd from any line if we haven't found it yet
        if cwd.is_none() && line.contains("\"cwd\"") {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }

        // Early exit: both conditions resolved
        if has_user && has_assistant && cwd.is_some() {
            break;
        }
    }

    let kind = if has_user && has_assistant {
        SessionKind::Conversation
    } else {
        SessionKind::MetadataOnly
    };

    Ok(SessionClassification {
        kind,
        start_type,
        cwd,
        parent_id,
    })
}

/// Resolve the cwd for a project directory by scanning one JSONL file.
/// Returns the first `cwd` field found in any JSONL file in the directory.
/// Used when sessions-index.json entries don't carry cwd (they never do).
///
/// Scans at most 3 files — cwd is present in 98.9% of conversation files,
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
    #[serde(default)]
    pub session_cwd: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
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
///
/// After reading the index, also scans for `.jsonl` files in the same directory
/// that are not listed in the index (catch-up for stale index files).
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
            Ok(mut session_entries) => {
                // Catch-up: scan for JSONL files not listed in the index.
                // Claude Code may create sessions without updating sessions-index.json.
                let indexed_ids: std::collections::HashSet<String> =
                    session_entries.iter().map(|e| e.session_id.clone()).collect();

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
                        // Same filter discover_orphan_sessions() uses (lines 381-393).
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
    fn test_read_all_catches_unlisted_jsonl_files() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let proj = projects_dir.join("my-project");
        std::fs::create_dir(&proj).unwrap();

        // Index lists only sess-1
        let json = r#"{"version":1,"entries":[{"sessionId": "sess-1"}]}"#;
        std::fs::write(proj.join("sessions-index.json"), json).unwrap();

        // But there are also sess-2.jsonl and sess-3.jsonl on disk
        // Real conversation content so classify_jsonl_file returns Conversation
        std::fs::write(proj.join("sess-1.jsonl"), concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();
        std::fs::write(proj.join("sess-2.jsonl"), concat!(
            r#"{"type":"user","uuid":"u2","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a2","message":{"content":"ok"}}"#, "\n",
        )).unwrap();
        std::fs::write(proj.join("sess-3.jsonl"), concat!(
            r#"{"type":"user","uuid":"u3","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a3","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        let results = read_all_session_indexes(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "my-project");
        // Should discover all 3: 1 from index + 2 unlisted
        assert_eq!(results[0].1.len(), 3);

        let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
        assert!(ids.contains(&"sess-1"));
        assert!(ids.contains(&"sess-2"));
        assert!(ids.contains(&"sess-3"));

        // Unlisted entries should have full_path set
        let unlisted: Vec<_> = results[0].1.iter().filter(|e| e.session_id != "sess-1").collect();
        for entry in unlisted {
            assert!(entry.full_path.is_some());
            assert!(entry.full_path.as_ref().unwrap().ends_with(".jsonl"));
        }
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
    // Catch-Up Classification Tests
    // ========================================================================

    #[test]
    fn test_read_all_catchup_skips_metadata_files() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let proj = projects_dir.join("my-project");
        std::fs::create_dir(&proj).unwrap();

        // Index lists only sess-1
        let json = r#"{"version":1,"entries":[{"sessionId": "sess-1"}]}"#;
        std::fs::write(proj.join("sessions-index.json"), json).unwrap();

        // Real conversation file (not in index — should be caught up)
        std::fs::write(proj.join("conv-2.jsonl"), concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/Users/dev/@org/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        // Metadata-only file (file-history-snapshot — should be SKIPPED)
        std::fs::write(proj.join("fhs-3.jsonl"), concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#, "\n",
        )).unwrap();

        // Another metadata file (summary with timestamp — the dangerous case)
        std::fs::write(proj.join("sum-4.jsonl"), concat!(
            r#"{"type":"summary","summary":"did stuff","timestamp":"2026-02-25T10:00:00Z"}"#, "\n",
        )).unwrap();

        let results = read_all_session_indexes(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "my-project");

        // Should have 2 entries: sess-1 from index + conv-2 from catch-up
        // fhs-3 and sum-4 should be filtered out by classification
        assert_eq!(results[0].1.len(), 2);

        let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
        assert!(ids.contains(&"sess-1"));
        assert!(ids.contains(&"conv-2"));
        assert!(!ids.contains(&"fhs-3"));
        assert!(!ids.contains(&"sum-4"));
    }

    #[test]
    fn test_read_all_catchup_captures_cwd_and_parent() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();

        let proj = projects_dir.join("test-proj");
        std::fs::create_dir(&proj).unwrap();

        // Empty index
        std::fs::write(proj.join("sessions-index.json"), r#"{"version":1,"entries":[]}"#).unwrap();

        // Forked conversation with cwd and parentUuid
        std::fs::write(proj.join("fork-1.jsonl"), concat!(
            r#"{"type":"user","uuid":"u1","parentUuid":"parent-abc","message":{"content":"continue"},"cwd":"/Users/dev/@org/my-project"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        let results = read_all_session_indexes(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.len(), 1);

        let entry = &results[0].1[0];
        assert_eq!(entry.session_id, "fork-1");
        assert_eq!(entry.session_cwd.as_deref(), Some("/Users/dev/@org/my-project"));
        assert_eq!(entry.parent_session_id.as_deref(), Some("parent-abc"));
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
        std::fs::write(orphan_proj.join("abc-123.jsonl"), concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();
        std::fs::write(orphan_proj.join("def-456.jsonl"), concat!(
            r#"{"type":"user","uuid":"u2","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a2","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

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
        std::fs::write(indexed_proj.join("abc-123.jsonl"), concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

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

    #[test]
    fn test_discover_orphan_sessions_skips_metadata_files() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&proj_dir).unwrap();

        // Real session file (has user + assistant)
        let session = proj_dir.join("abc-123.jsonl");
        std::fs::write(&session, concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hello"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"hi"}}"#, "\n",
        )).unwrap();

        // Metadata-only file (should NOT count as session)
        let snapshot = proj_dir.join("fhs-456.jsonl");
        std::fs::write(&snapshot, concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#, "\n",
        )).unwrap();

        let results = discover_orphan_sessions(tmp.path()).unwrap();
        let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

        // Only the real session should be discovered
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "abc-123");
        assert_eq!(entries[0].session_cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn test_discover_orphan_sessions_captures_parent_id() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&proj_dir).unwrap();

        let session = proj_dir.join("fork-789.jsonl");
        std::fs::write(&session, concat!(
            r#"{"type":"user","uuid":"u1","parentUuid":"parent-abc","message":{"content":"continue"},"cwd":"/proj"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        let results = discover_orphan_sessions(tmp.path()).unwrap();
        let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].parent_session_id.as_deref(), Some("parent-abc"));
    }

    // ========================================================================
    // Classification Tests
    // ========================================================================

    #[test]
    fn test_classify_normal_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::User);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
        assert!(c.parent_id.is_none());
    }

    #[test]
    fn test_classify_forked_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","parentUuid":"p1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert!(c.parent_id.is_some());
    }

    #[test]
    fn test_classify_file_history_snapshot() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
    }

    #[test]
    fn test_classify_resumed_session_with_preamble() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"file-history-snapshot","messageId":"m2","snapshot":{{}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","message":{{"content":"continue"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","uuid":"a1","message":{{"content":"sure"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn test_classify_metadata_only_no_conversation() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#).unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert!(c.cwd.is_none());
    }

    // ========================================================================
    // resolve_cwd_for_project Tests
    // ========================================================================

    #[test]
    fn test_resolve_cwd_for_project_finds_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path();

        // Write a JSONL with cwd
        let session = proj_dir.join("abc-123.jsonl");
        std::fs::write(&session, concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/Users/dev/@org/my-project"}"#, "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
        )).unwrap();

        let cwd = resolve_cwd_for_project(proj_dir);
        assert_eq!(cwd.as_deref(), Some("/Users/dev/@org/my-project"));
    }

    #[test]
    fn test_resolve_cwd_for_project_returns_none_for_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = resolve_cwd_for_project(tmp.path());
        assert!(cwd.is_none());
    }

    #[test]
    fn test_resolve_cwd_for_project_skips_metadata_files() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path();

        // Only metadata file — no cwd
        let snapshot = proj_dir.join("fhs-456.jsonl");
        std::fs::write(&snapshot, concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#, "\n",
        )).unwrap();

        let cwd = resolve_cwd_for_project(proj_dir);
        assert!(cwd.is_none());
    }
}
