//! Extract agent-level todo checklists from session JSONL.
//!
//! ## Why this exists
//!
//! Claude Code historically wrote per-agent todo files to
//! `~/.claude/todos/{sessionId}-agent-{agentId}.json` (parsed by
//! [`crate::todo_files`]). That directory is **no longer written** —
//! TodoWrite output now lives **inline in the session JSONL** as a
//! `tool_use` block:
//!
//! ```json
//! {"type":"assistant","message":{"content":[
//!   {"type":"tool_use","name":"TodoWrite",
//!    "input":{"todos":[
//!      {"content":"...","status":"completed","activeForm":"..."}
//!    ]}}
//! ]}}
//! ```
//!
//! Each TodoWrite call rewrites the **whole** list, so only the
//! **latest** TodoWrite per agent reflects the current checklist.
//!
//! ## Data shape (verified against real `~/.claude/projects`)
//!
//! - Item keys: `content`, `status`, `activeForm` (status ∈
//!   `pending` | `in_progress` | `completed`). `id` / `priority` may
//!   appear but were absent in the live sample.
//! - In every observed session TodoWrite is emitted **only by the main
//!   agent** (`isSidechain=false`, no `agentId`, the session's own id).
//!   Subagent JSONLs (`<session>/subagents/agent-<hex>.jsonl`) carry an
//!   `agentId` matching their filename hex but did not emit TodoWrite.
//!   We still scan the subagents dir so the per-agent grouping contract
//!   holds the moment a subagent does write todos.
//!
//! Output is the **same** [`AgentTodos`] contract the on-disk reader
//! produced, so the API response shape is byte-identical.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::jsonl_reader;
use crate::todo_files::{AgentTodos, TodoItem};

/// Extract the latest-per-agent todo checklists for a session by
/// reading the session JSONL (and any `subagents/agent-*.jsonl`).
///
/// `main_jsonl` is the session's primary `.jsonl` (or `.jsonl.gz`)
/// file; `is_compressed` mirrors the catalog flag. `session_id` is used
/// as the main agent's `agent_id` (so `is_main_agent` is true for it).
///
/// Returns main agent first, then subagents sorted by `agent_id` —
/// matching the legacy on-disk reader's ordering. Agents whose latest
/// TodoWrite is an empty list are skipped (same as the old "skip empty
/// arrays" behaviour).
pub fn extract_session_todos(
    main_jsonl: &Path,
    is_compressed: bool,
    session_id: &str,
) -> Vec<AgentTodos> {
    let mut results = Vec::new();

    // Main agent: latest TodoWrite in the primary JSONL.
    if let Some(items) = latest_todos_in_file(main_jsonl, is_compressed) {
        if !items.is_empty() {
            results.push(AgentTodos {
                is_main_agent: true,
                session_id: session_id.to_string(),
                agent_id: session_id.to_string(),
                items,
            });
        }
    }

    // Subagents: <session>/subagents/agent-<hex>.jsonl. The directory
    // sits next to the main JSONL, named after the session id.
    for (agent_id, path) in subagent_jsonls(main_jsonl, session_id) {
        // Subagent files on disk are never gzip-compressed (only the
        // archived primary session JSONL is), so read as plain.
        if let Some(items) = latest_todos_in_file(&path, false) {
            if !items.is_empty() {
                results.push(AgentTodos {
                    is_main_agent: false,
                    session_id: session_id.to_string(),
                    agent_id,
                    items,
                });
            }
        }
    }

    // Main agent first, then subagents sorted by agent_id — identical
    // ordering to the legacy `parse_session_todos`.
    results.sort_by(|a, b| {
        b.is_main_agent
            .cmp(&a.is_main_agent)
            .then_with(|| a.agent_id.cmp(&b.agent_id))
    });

    results
}

/// Scan one JSONL file and return the `todos` array from its **last**
/// TodoWrite `tool_use` block. Returns `None` if the file is unreadable
/// or contains no TodoWrite block.
fn latest_todos_in_file(path: &Path, is_compressed: bool) -> Option<Vec<TodoItem>> {
    let lines: Vec<Value> = jsonl_reader::read_all(path, is_compressed).ok()?;

    // Later lines overwrite earlier ones — walk forward and keep the
    // last match so we end on the current checklist.
    let mut latest: Option<Vec<TodoItem>> = None;
    for line in &lines {
        if let Some(items) = todos_from_line(line) {
            latest = Some(items);
        }
    }
    latest
}

/// Pull the `input.todos` of a TodoWrite `tool_use` block out of one
/// assistant JSONL line, if present.
fn todos_from_line(line: &Value) -> Option<Vec<TodoItem>> {
    let content = line.get("message")?.get("content")?.as_array()?;

    for block in content {
        let is_todo_write = block.get("type").and_then(Value::as_str) == Some("tool_use")
            && block.get("name").and_then(Value::as_str) == Some("TodoWrite");
        if !is_todo_write {
            continue;
        }

        let todos = block.get("input").and_then(|i| i.get("todos"))?;
        // Tolerate a malformed todos payload the same way the JSONL
        // reader tolerates bad lines: skip, don't abort.
        return serde_json::from_value::<Vec<TodoItem>>(todos.clone()).ok();
    }

    None
}

/// Resolve `(agent_id, path)` pairs for every `agent-<hex>.jsonl` under
/// the session's `subagents/` directory. The directory lives at
/// `<project>/<session_id>/subagents/` — i.e. a sibling of the main
/// JSONL named after the session id.
fn subagent_jsonls(main_jsonl: &Path, session_id: &str) -> Vec<(String, PathBuf)> {
    let parent = match main_jsonl.parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let subagents_dir = parent.join(session_id).join("subagents");

    let entries = match std::fs::read_dir(&subagents_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "jsonl") {
            continue;
        }
        // Filename: agent-<hex>.jsonl → agent_id = <hex>.
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let agent_id = match stem.strip_prefix("agent-") {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => continue,
        };
        out.push((agent_id, path));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    /// One assistant JSONL line carrying a TodoWrite tool_use block with
    /// the given todos JSON array literal.
    ///
    /// Newlines in `todos_json` are collapsed to spaces so the result is a
    /// single physical line — real Claude Code JSONL always emits one
    /// record per line, and `jsonl_reader::read_all` is line-based (a
    /// record split across lines would be silently dropped as malformed).
    fn todo_write_line(todos_json: &str) -> String {
        let todos_json = todos_json.replace('\n', " ");
        format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"TodoWrite","input":{{"todos":{todos_json}}}}}]}}}}"#
        )
    }

    fn write_jsonl(path: &Path, lines: &[String]) {
        let mut f = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    const SESSION_ID: &str = "860ec9f4-502a-4b45-ae5e-b560eecfe57a";

    #[test]
    fn extracts_main_agent_todos_with_counts() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join(format!("{SESSION_ID}.jsonl"));
        write_jsonl(
            &main,
            &[
                // An unrelated line is ignored.
                r#"{"type":"user","message":{"content":"hi"}}"#.to_string(),
                todo_write_line(
                    r#"[
                        {"content":"Fix the bug","status":"completed","activeForm":"Fixing bug"},
                        {"content":"Write tests","status":"in_progress","activeForm":"Writing tests"},
                        {"content":"Ship it","status":"pending","activeForm":"Shipping"}
                    ]"#,
                ),
            ],
        );

        let todos = extract_session_todos(&main, false, SESSION_ID);
        assert_eq!(todos.len(), 1, "one main-agent group expected");
        let g = &todos[0];
        assert!(g.is_main_agent);
        assert_eq!(g.session_id, SESSION_ID);
        assert_eq!(g.agent_id, SESSION_ID);
        assert_eq!(g.items.len(), 3, "all 3 todos must be present");

        let completed = g.items.iter().filter(|t| t.status == "completed").count();
        let in_progress = g.items.iter().filter(|t| t.status == "in_progress").count();
        let pending = g.items.iter().filter(|t| t.status == "pending").count();
        assert_eq!((completed, in_progress, pending), (1, 1, 1));
        assert_eq!(g.items[0].active_form, "Fixing bug");
    }

    #[test]
    fn keeps_only_latest_todo_write() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join(format!("{SESSION_ID}.jsonl"));
        write_jsonl(
            &main,
            &[
                todo_write_line(r#"[{"content":"old","status":"pending"}]"#),
                todo_write_line(
                    r#"[
                        {"content":"new a","status":"completed"},
                        {"content":"new b","status":"pending"}
                    ]"#,
                ),
            ],
        );

        let todos = extract_session_todos(&main, false, SESSION_ID);
        assert_eq!(todos.len(), 1);
        assert_eq!(
            todos[0].items.len(),
            2,
            "latest TodoWrite wins, not the old"
        );
        assert_eq!(todos[0].items[0].content, "new a");
    }

    #[test]
    fn no_todo_write_yields_empty() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join(format!("{SESSION_ID}.jsonl"));
        write_jsonl(
            &main,
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#
                    .to_string(),
            ],
        );

        let todos = extract_session_todos(&main, false, SESSION_ID);
        assert!(todos.is_empty());
    }

    #[test]
    fn empty_todo_array_is_skipped() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join(format!("{SESSION_ID}.jsonl"));
        write_jsonl(&main, &[todo_write_line("[]")]);

        let todos = extract_session_todos(&main, false, SESSION_ID);
        assert!(todos.is_empty(), "empty checklist produces no group");
    }

    #[test]
    fn subagent_todos_form_separate_group_after_main() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join(format!("{SESSION_ID}.jsonl"));
        write_jsonl(
            &main,
            &[todo_write_line(
                r#"[{"content":"main work","status":"in_progress","activeForm":"Working"}]"#,
            )],
        );

        // Subagent dir: <project>/<session_id>/subagents/agent-<hex>.jsonl
        let agent_hex = "abd7eb1899d37a69f";
        let subagents_dir = tmp.path().join(SESSION_ID).join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        write_jsonl(
            &subagents_dir.join(format!("agent-{agent_hex}.jsonl")),
            &[todo_write_line(
                r#"[
                    {"content":"sub task 1","status":"completed"},
                    {"content":"sub task 2","status":"pending"}
                ]"#,
            )],
        );

        let todos = extract_session_todos(&main, false, SESSION_ID);
        assert_eq!(todos.len(), 2, "main + one subagent group");

        // Main agent first.
        assert!(todos[0].is_main_agent);
        assert_eq!(todos[0].agent_id, SESSION_ID);
        assert_eq!(todos[0].items.len(), 1);

        // Subagent group, keyed by the filename hex.
        assert!(!todos[1].is_main_agent);
        assert_eq!(todos[1].agent_id, agent_hex);
        assert_eq!(todos[1].session_id, SESSION_ID);
        assert_eq!(todos[1].items.len(), 2);
        assert_eq!(
            todos[1]
                .items
                .iter()
                .filter(|t| t.status == "completed")
                .count(),
            1
        );
    }

    #[test]
    fn missing_file_yields_empty() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist.jsonl");
        let todos = extract_session_todos(&missing, false, SESSION_ID);
        assert!(todos.is_empty());
    }
}
