//! Parser for ~/.claude/sessions/{pid}.json active session files.
//!
//! Ephemeral JSON files created by Claude Code CLI on session start, deleted on exit.
//! Only live sessions are visible. Provides hook-free session lifecycle detection.
//!
//! On-demand read, NO SQLite indexing — follows task_files.rs pattern.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// A parsed session file from ~/.claude/sessions/{pid}.json.
///
/// Written by Claude Code CLI — external data we don't control.
/// Every field uses `#[serde(default)]` so missing fields never
/// cause silent deserialization failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    /// Process ID of the Claude Code process.
    #[serde(default)]
    pub pid: u32,
    /// Session UUID.
    #[serde(default)]
    pub session_id: String,
    /// Working directory where the session was started.
    #[serde(default)]
    pub cwd: String,
    /// Unix timestamp (milliseconds) when the session started.
    #[serde(default)]
    #[ts(type = "number")]
    pub started_at: i64,
    /// Session kind: "interactive" or "background" (subagent).
    #[serde(default)]
    pub kind: String,
    /// Entrypoint: "cli", "claude-vscode", "claude-desktop", "claude-web", etc.
    #[serde(default)]
    pub entrypoint: String,
    /// Optional session name (from `claude --name "foo"`).
    /// Present when user starts session with explicit name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Scan ~/.claude/sessions/ for all active session files.
///
/// Returns sessions sorted by started_at ascending.
pub fn scan_active_sessions(sessions_dir: &Path) -> Vec<ActiveSession> {
    if !sessions_dir.is_dir() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(sessions_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().map(|e| e == "json").unwrap_or(false) {
            continue;
        }

        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(session) = serde_json::from_str::<ActiveSession>(&contents) {
                sessions.push(session);
            }
        }
    }

    sessions.sort_by_key(|s| s.started_at);
    sessions
}

/// Parse a single session file by path. Returns None if invalid.
pub fn parse_session_file(path: &Path) -> Option<ActiveSession> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Resolve the ~/.claude/sessions/ directory.
pub fn claude_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("sessions"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_session_file_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("12345.json");
        fs::write(
            &path,
            r#"{
                "pid": 12345,
                "sessionId": "abc-def-123",
                "cwd": "/Users/test/project",
                "startedAt": 1775492920444,
                "kind": "interactive",
                "entrypoint": "cli"
            }"#,
        )
        .unwrap();

        let session = parse_session_file(&path).unwrap();
        assert_eq!(session.pid, 12345);
        assert_eq!(session.session_id, "abc-def-123");
        assert_eq!(session.kind, "interactive");
        assert_eq!(session.entrypoint, "cli");
    }

    #[test]
    fn test_parse_session_file_vscode() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("99999.json");
        fs::write(
            &path,
            r#"{
                "pid": 99999,
                "sessionId": "xyz-789",
                "cwd": "/Users/test/other",
                "startedAt": 1775493686696,
                "kind": "interactive",
                "entrypoint": "claude-vscode"
            }"#,
        )
        .unwrap();

        let session = parse_session_file(&path).unwrap();
        assert_eq!(session.entrypoint, "claude-vscode");
    }

    #[test]
    fn test_scan_active_sessions() {
        let tmp = tempfile::tempdir().unwrap();

        fs::write(
            tmp.path().join("100.json"),
            r#"{"pid":100,"sessionId":"sess-1","cwd":"/a","startedAt":2000,"kind":"interactive","entrypoint":"cli"}"#,
        ).unwrap();

        fs::write(
            tmp.path().join("200.json"),
            r#"{"pid":200,"sessionId":"sess-2","cwd":"/b","startedAt":1000,"kind":"background","entrypoint":"cli"}"#,
        ).unwrap();

        // Invalid file should be skipped
        fs::write(tmp.path().join("bad.json"), "not json").unwrap();

        // Non-JSON file should be skipped
        fs::write(tmp.path().join("readme.txt"), "hello").unwrap();

        let sessions = scan_active_sessions(tmp.path());
        assert_eq!(sessions.len(), 2);
        // Sorted by started_at ascending
        assert_eq!(sessions[0].pid, 200); // started_at=1000
        assert_eq!(sessions[1].pid, 100); // started_at=2000
    }

    #[test]
    fn test_scan_active_sessions_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("nonexistent");
        let sessions = scan_active_sessions(&nonexistent);
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_background_kind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("555.json");
        fs::write(
            &path,
            r#"{"pid":555,"sessionId":"bg-sess","cwd":"/bg","startedAt":3000,"kind":"background","entrypoint":"cli"}"#,
        ).unwrap();

        let session = parse_session_file(&path).unwrap();
        assert_eq!(session.kind, "background");
    }

    #[test]
    fn test_parse_session_with_name_field() {
        let tmp = tempfile::tempdir().unwrap();
        // New format: includes `name` field
        let path = tmp.path().join("11111.json");
        fs::write(
            &path,
            r#"{"pid":11111,"sessionId":"named-sess","cwd":"/x","startedAt":9000,"kind":"interactive","entrypoint":"cli","name":"my-feature"}"#,
        ).unwrap();
        let session = parse_session_file(&path).unwrap();
        assert_eq!(session.name, Some("my-feature".to_string()));

        // Old format: no `name` field → None
        let path2 = tmp.path().join("22222.json");
        fs::write(
            &path2,
            r#"{"pid":22222,"sessionId":"old-sess","cwd":"/y","startedAt":8000,"kind":"interactive","entrypoint":"cli"}"#,
        ).unwrap();
        let session2 = parse_session_file(&path2).unwrap();
        assert_eq!(session2.name, None);
    }

    #[test]
    fn test_parse_session_with_missing_and_extra_fields() {
        let tmp = tempfile::tempdir().unwrap();
        // Minimal: only pid and sessionId (Claude Code could change schema)
        let path = tmp.path().join("777.json");
        fs::write(
            &path,
            r#"{"pid":777,"sessionId":"min-sess","cwd":"/x","startedAt":5000}"#,
        )
        .unwrap();
        let session = parse_session_file(&path).unwrap();
        assert_eq!(session.pid, 777);
        assert_eq!(session.kind, ""); // defaulted
        assert_eq!(session.entrypoint, ""); // defaulted

        // Extra unknown fields must not break deserialization
        let path2 = tmp.path().join("888.json");
        fs::write(
            &path2,
            r#"{"pid":888,"sessionId":"extra","cwd":"/y","startedAt":6000,"kind":"interactive","entrypoint":"cli","newField":"surprise","nested":{"a":1}}"#,
        ).unwrap();
        let session2 = parse_session_file(&path2).unwrap();
        assert_eq!(session2.pid, 888);
    }
}
