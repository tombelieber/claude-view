//! Parser for ~/.claude/tasks/{sessionId}/*.json persistent task files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// A persistent task item from ~/.claude/tasks/{sessionId}/{id}.json.
///
/// Richer than ProgressItem — includes description and dependency edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub active_form: String,
    pub status: String,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

/// Read all task JSON files for a session from ~/.claude/tasks/{sessionId}/.
///
/// Returns an empty Vec if the directory doesn't exist or contains no valid JSON.
/// Tasks are sorted by numeric ID ascending.
pub fn parse_session_tasks(tasks_dir: &Path, session_id: &str) -> Vec<TaskItem> {
    let session_dir = tasks_dir.join(session_id);
    if !session_dir.is_dir() {
        return Vec::new();
    }

    let mut tasks: Vec<TaskItem> = Vec::new();

    let entries = match std::fs::read_dir(&session_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(task) = serde_json::from_str::<TaskItem>(&contents) {
                    tasks.push(task);
                }
            }
        }
    }

    // Sort by numeric ID (tasks are "1", "2", "3", ...)
    tasks.sort_by(|a, b| {
        let a_num: u32 = a.id.parse().unwrap_or(u32::MAX);
        let b_num: u32 = b.id.parse().unwrap_or(u32::MAX);
        a_num.cmp(&b_num)
    });

    tasks
}

/// Resolve the ~/.claude/tasks/ directory.
pub fn claude_tasks_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("tasks"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_session_tasks_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("abc-123");
        fs::create_dir_all(&session_dir).unwrap();

        fs::write(
            session_dir.join("1.json"),
            r#"{"id":"1","subject":"Fix bug","description":"Fix the SQL bug","activeForm":"Fixing bug","status":"completed","blocks":["2"],"blockedBy":[]}"#,
        ).unwrap();

        fs::write(
            session_dir.join("2.json"),
            r#"{"id":"2","subject":"Write tests","description":"Add regression tests","activeForm":"Writing tests","status":"in_progress","blocks":[],"blockedBy":["1"]}"#,
        ).unwrap();

        let tasks = parse_session_tasks(tmp.path(), "abc-123");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[0].subject, "Fix bug");
        assert_eq!(tasks[0].blocks, vec!["2"]);
        assert_eq!(tasks[1].id, "2");
        assert_eq!(tasks[1].blocked_by, vec!["1"]);
    }

    #[test]
    fn test_parse_session_tasks_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks = parse_session_tasks(tmp.path(), "nonexistent");
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_session_tasks_invalid_json_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-1");
        fs::create_dir_all(&session_dir).unwrap();

        fs::write(session_dir.join("1.json"), "not json").unwrap();
        fs::write(
            session_dir.join("2.json"),
            r#"{"id":"2","subject":"Good task","description":"","activeForm":"","status":"pending","blocks":[],"blockedBy":[]}"#,
        ).unwrap();

        let tasks = parse_session_tasks(tmp.path(), "sess-1");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn test_parse_session_tasks_sorted_by_numeric_id() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("sess-sort");
        fs::create_dir_all(&session_dir).unwrap();

        for id in &["10", "2", "1", "3"] {
            fs::write(
                session_dir.join(format!("{id}.json")),
                format!(r#"{{"id":"{id}","subject":"Task {id}","description":"","activeForm":"","status":"pending","blocks":[],"blockedBy":[]}}"#),
            ).unwrap();
        }

        let tasks = parse_session_tasks(tmp.path(), "sess-sort");
        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2", "3", "10"]);
    }
}
