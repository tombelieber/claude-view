//! Parser for ~/.claude/todos/{sessionId}-agent-{agentId}.json agent-level todo files.
//!
//! TodoWrite tool outputs — the agent-level equivalent of TaskCreate.
//! When sessionId == agentId → main agent's todos.
//! When sessionId != agentId → subagent's todos.
//!
//! 96% of files are empty arrays. Only non-empty files are returned.
//! On-demand read, NO SQLite indexing — follows task_files.rs pattern.

use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// A single todo item from a TodoWrite output file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    /// Todo description text.
    pub content: String,
    /// Status: "completed", "pending", "in_progress", etc.
    pub status: String,
    /// Human-readable description of the current work on this todo.
    #[serde(default)]
    pub active_form: String,
}

/// A todo file parsed with its session and agent context.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AgentTodos {
    /// Session UUID this todo belongs to.
    pub session_id: String,
    /// Agent UUID (same as session_id for main agent, different for subagents).
    pub agent_id: String,
    /// Whether this is the main agent (session_id == agent_id).
    pub is_main_agent: bool,
    /// The todo items (only non-empty results are returned).
    pub items: Vec<TodoItem>,
}

/// Parse a single todo file by path. Returns None if empty or invalid.
fn parse_todo_file(path: &Path) -> Option<(String, String, Vec<TodoItem>)> {
    let filename = path.file_stem()?.to_str()?;

    // Filename format: {sessionId}-agent-{agentId}
    let (session_id, agent_id) = parse_todo_filename(filename)?;

    let contents = std::fs::read_to_string(path).ok()?;
    let items: Vec<TodoItem> = serde_json::from_str(&contents).ok()?;

    if items.is_empty() {
        return None; // Skip empty arrays (96% of files)
    }

    Some((session_id, agent_id, items))
}

/// Parse the todo filename to extract session_id and agent_id.
///
/// Format: `{sessionId}-agent-{agentId}` where both are UUIDs.
fn parse_todo_filename(filename: &str) -> Option<(String, String)> {
    // Split on "-agent-" — but UUIDs contain hyphens, so we need to find
    // the "-agent-" delimiter. UUID format: 8-4-4-4-12 = 36 chars.
    let marker = "-agent-";
    let idx = filename.find(marker)?;

    let session_id = &filename[..idx];
    let agent_id = &filename[idx + marker.len()..];

    // Basic validation: both should look like UUIDs (contain hyphens, reasonable length)
    if session_id.len() < 36 || agent_id.len() < 36 {
        return None;
    }

    Some((session_id.to_string(), agent_id.to_string()))
}

/// Read all non-empty todo files for a specific session.
pub fn parse_session_todos(todos_dir: &Path, session_id: &str) -> Vec<AgentTodos> {
    if !todos_dir.is_dir() {
        return Vec::new();
    }

    let prefix = format!("{session_id}-agent-");
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(todos_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().map(|e| e == "json").unwrap_or(false) {
            continue;
        }
        let fname = match path.file_stem().and_then(|s| s.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };
        if !fname.starts_with(&prefix) {
            continue;
        }

        if let Some((sid, aid, items)) = parse_todo_file(&path) {
            results.push(AgentTodos {
                is_main_agent: sid == aid,
                session_id: sid,
                agent_id: aid,
                items,
            });
        }
    }

    // Main agent first, then subagents sorted by agent_id
    results.sort_by(|a, b| {
        b.is_main_agent
            .cmp(&a.is_main_agent)
            .then_with(|| a.agent_id.cmp(&b.agent_id))
    });

    results
}

/// Resolve the ~/.claude/todos/ directory.
pub fn claude_todos_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("todos"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_todo_filename() {
        let result = parse_todo_filename(
            "eb7c9838-dbf2-4f98-8861-32cdf60c34c5-agent-eb7c9838-dbf2-4f98-8861-32cdf60c34c5",
        );
        assert!(result.is_some());
        let (sid, aid) = result.unwrap();
        assert_eq!(sid, "eb7c9838-dbf2-4f98-8861-32cdf60c34c5");
        assert_eq!(aid, "eb7c9838-dbf2-4f98-8861-32cdf60c34c5");
    }

    #[test]
    fn test_parse_todo_filename_subagent() {
        let result = parse_todo_filename(
            "e6fa09ce-b09e-48bd-888c-f6bda851ab29-agent-f0003a1d-1564-46e1-af47-37ea85171548",
        );
        assert!(result.is_some());
        let (sid, aid) = result.unwrap();
        assert_eq!(sid, "e6fa09ce-b09e-48bd-888c-f6bda851ab29");
        assert_eq!(aid, "f0003a1d-1564-46e1-af47-37ea85171548");
        assert_ne!(sid, aid);
    }

    #[test]
    fn test_parse_todo_filename_invalid() {
        assert!(parse_todo_filename("not-a-valid-format").is_none());
        assert!(parse_todo_filename("short-agent-short").is_none());
    }

    #[test]
    fn test_parse_session_todos_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let session_id = "eb7c9838-dbf2-4f98-8861-32cdf60c34c5";
        let agent_id = session_id; // main agent

        let filename = format!("{session_id}-agent-{agent_id}.json");
        fs::write(
            tmp.path().join(&filename),
            r#"[
                {"content":"Fix the bug","status":"completed","activeForm":"Fixing bug"},
                {"content":"Write tests","status":"pending","activeForm":"Writing tests"}
            ]"#,
        )
        .unwrap();

        let todos = parse_session_todos(tmp.path(), session_id);
        assert_eq!(todos.len(), 1);
        assert!(todos[0].is_main_agent);
        assert_eq!(todos[0].items.len(), 2);
        assert_eq!(todos[0].items[0].status, "completed");
    }

    #[test]
    fn test_parse_session_todos_skips_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let session_id = "eb7c9838-dbf2-4f98-8861-32cdf60c34c5";

        let filename = format!("{session_id}-agent-{session_id}.json");
        fs::write(tmp.path().join(&filename), "[]").unwrap();

        let todos = parse_session_todos(tmp.path(), session_id);
        assert!(todos.is_empty());
    }

    #[test]
    fn test_parse_session_todos_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("nonexistent");
        let todos = parse_session_todos(&nonexistent, "abc");
        assert!(todos.is_empty());
    }

    #[test]
    fn test_main_agent_sorted_first() {
        let tmp = tempfile::tempdir().unwrap();
        let session_id = "e6fa09ce-b09e-48bd-888c-f6bda851ab29";
        let subagent_id = "f0003a1d-1564-46e1-af47-37ea85171548";
        let item_json = r#"[{"content":"Do work","status":"pending","activeForm":"Working"}]"#;

        // Subagent file
        fs::write(
            tmp.path()
                .join(format!("{session_id}-agent-{subagent_id}.json")),
            item_json,
        )
        .unwrap();

        // Main agent file
        fs::write(
            tmp.path()
                .join(format!("{session_id}-agent-{session_id}.json")),
            item_json,
        )
        .unwrap();

        let todos = parse_session_todos(tmp.path(), session_id);
        assert_eq!(todos.len(), 2);
        assert!(todos[0].is_main_agent); // Main agent first
        assert!(!todos[1].is_main_agent); // Subagent second
    }
}
