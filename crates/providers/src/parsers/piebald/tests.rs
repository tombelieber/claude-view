// crates/providers/src/parsers/piebald/tests.rs
//
// Fixture DB built per-test with rusqlite; schema mirrors the Go fixture
// (piebald_test.go) column-for-column.

use super::PiebaldProvider;
use crate::discover::Provider;
use crate::util::time;
use claude_view_types::block_types::{
    AssistantSegment, ConversationBlock, ToolExecution, ToolStatus,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "
CREATE TABLE projects (id INTEGER PRIMARY KEY, directory TEXT NOT NULL, name TEXT NOT NULL);
CREATE TABLE chats (
  id INTEGER PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, is_deleted BOOLEAN NOT NULL DEFAULT 0,
  message_count INTEGER NOT NULL DEFAULT 0, current_directory TEXT,
  worktree_path TEXT, branch_name TEXT, project_id INTEGER
);
CREATE TABLE messages (
  id INTEGER PRIMARY KEY, parent_chat_id INTEGER NOT NULL, parent_message_id INTEGER,
  role TEXT NOT NULL, model TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  input_tokens BIGINT, output_tokens BIGINT, reasoning_tokens BIGINT,
  cache_read_tokens BIGINT, cache_write_tokens BIGINT,
  status TEXT NOT NULL DEFAULT 'completed', finish_reason TEXT, error TEXT,
  enabled INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE message_parts (
  id INTEGER PRIMARY KEY, parent_chat_message_id INTEGER NOT NULL,
  part_index INTEGER NOT NULL, part_type TEXT NOT NULL
);
CREATE TABLE message_part_text (
  message_part_id INTEGER PRIMARY KEY, is_thinking BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE TABLE message_content_nodes (
  id INTEGER PRIMARY KEY, parent_text_part_id INTEGER NOT NULL,
  node_index INTEGER NOT NULL, node_type TEXT NOT NULL
);
CREATE TABLE message_node_text (node_id INTEGER PRIMARY KEY, content TEXT NOT NULL);
CREATE TABLE message_part_tool_call (
  message_part_id INTEGER PRIMARY KEY, provider_tool_use_id TEXT NOT NULL,
  tool_name TEXT NOT NULL, tool_input TEXT NOT NULL, tool_result TEXT,
  tool_error TEXT, tool_state TEXT NOT NULL DEFAULT 'pending', sub_agent_chat_id INTEGER
);
";

fn new_db(dir: &Path) -> PathBuf {
    let db = dir.join("app.db");
    Connection::open(&db)
        .unwrap()
        .execute_batch(SCHEMA)
        .unwrap();
    db
}

fn exec(db: &Path, sql: &str) {
    Connection::open(db).unwrap().execute_batch(sql).unwrap();
}

/// Seed one text part with a single content node (node id = part id + 1000).
fn seed_text_part(db: &Path, part_id: i64, msg_id: i64, idx: i64, text: &str, thinking: bool) {
    exec(
        db,
        &format!(
            "INSERT INTO message_parts (id, parent_chat_message_id, part_index, part_type)
             VALUES ({part_id}, {msg_id}, {idx}, 'text');
             INSERT INTO message_part_text (message_part_id, is_thinking) VALUES ({part_id}, {});
             INSERT INTO message_content_nodes (id, parent_text_part_id, node_index, node_type)
             VALUES ({}, {part_id}, 0, 'text');
             INSERT INTO message_node_text (node_id, content) VALUES ({}, '{text}');",
            thinking as i64,
            part_id + 1000,
            part_id + 1000
        ),
    );
}

fn virtual_path(db: &Path, chat_id: i64) -> PathBuf {
    PathBuf::from(format!("{}#{chat_id}", db.display()))
}

fn tool_seg(block: &ConversationBlock, i: usize) -> &ToolExecution {
    let ConversationBlock::Assistant(a) = block else {
        panic!("expected assistant block")
    };
    let AssistantSegment::Tool { execution } = &a.segments[i] else {
        panic!("expected tool segment at {i}")
    };
    execution
}

#[test]
fn parses_chat_with_chunked_text_and_usage() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO projects (id, directory, name) VALUES (1, '/repo/app', 'app');
         INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count,
                            current_directory, branch_name, project_id)
         VALUES (42, 'Fix bug', '2026-05-01T10:00:00Z', '2026-05-01T10:05:00Z', 0, 2,
                 '/repo/app', 'main', 1);
         INSERT INTO messages (id, parent_chat_id, role, model, created_at, updated_at)
         VALUES (100, 42, 'user', '', '2026-05-01T10:00:01Z', '2026-05-01T10:00:01Z');
         INSERT INTO messages (id, parent_chat_id, parent_message_id, role, model, created_at,
                               updated_at, input_tokens, output_tokens, reasoning_tokens,
                               cache_read_tokens, cache_write_tokens)
         VALUES (101, 42, 100, 'assistant', 'claude-test', '2026-05-01T10:00:02Z',
                 '2026-05-01T10:00:03Z', 10, 20, 4, 5, 7);",
    );
    seed_text_part(&db, 200, 100, 0, "Please fix this", false);
    seed_text_part(&db, 201, 101, 0, "deep thought", true);
    // The hardest quirk: assistant text arrives CHUNKED across content-node
    // rows and must be joined with NO separator.
    exec(
        &db,
        "INSERT INTO message_parts (id, parent_chat_message_id, part_index, part_type)
         VALUES (202, 101, 1, 'text');
         INSERT INTO message_part_text (message_part_id, is_thinking) VALUES (202, 0);
         INSERT INTO message_content_nodes (id, parent_text_part_id, node_index, node_type)
         VALUES (1202, 202, 0, 'text'), (1203, 202, 1, 'text');
         INSERT INTO message_node_text (node_id, content) VALUES (1202, 'I fixed'), (1203, ' it');",
    );

    let mut sessions = PiebaldProvider.parse(&virtual_path(&db, 42)).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = sessions.remove(0);
    assert_eq!(s.meta.id, "piebald:42");
    assert_eq!(s.meta.project, "app");
    assert_eq!(s.meta.cwd.as_deref(), Some("/repo/app"));
    assert_eq!(s.meta.git_branch.as_deref(), Some("main"));
    assert_eq!(s.meta.title.as_deref(), Some("Fix bug"));
    assert_eq!(s.meta.first_message, "Please fix this");
    assert_eq!(s.meta.message_count, 2);
    assert_eq!(s.meta.user_message_count, 1);
    assert_eq!(s.meta.models, vec!["claude-test".to_string()]);
    assert_eq!(s.meta.malformed_lines, 0);
    // The chat envelope bounds the main session.
    assert_eq!(
        s.meta.started_at,
        time::parse_timestamp("2026-05-01T10:00:00Z", false)
    );
    assert_eq!(
        s.meta.ended_at,
        time::parse_timestamp("2026-05-01T10:05:00Z", false)
    );
    // Tokens: reasoning folds into output; cache_write → cache_creation;
    // input is NOT cache-adjusted (the columns are already Anthropic-shaped).
    assert!(s.meta.usage.has_usage);
    let t = s.meta.usage.totals;
    assert_eq!(
        (
            t.input_tokens,
            t.output_tokens,
            t.cache_read_input_tokens,
            t.cache_creation_input_tokens
        ),
        (10, 24, 5, 7)
    );
    assert_eq!(s.meta.usage.per_model["claude-test"].output_tokens, 24);
    assert_eq!(s.blocks.len(), 2);
    let ConversationBlock::User(u) = &s.blocks[0] else {
        panic!("expected user block")
    };
    assert_eq!(u.text, "Please fix this");
    let ConversationBlock::Assistant(a) = &s.blocks[1] else {
        panic!("expected assistant block")
    };
    assert_eq!(a.thinking.as_deref(), Some("deep thought"));
    let AssistantSegment::Text { text, .. } = &a.segments[0] else {
        panic!("expected text segment")
    };
    assert_eq!(text, "I fixed it");
}

#[test]
fn splits_dag_into_main_and_nested_fork_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count)
         VALUES (42, 'Nested', '2026-05-01T10:00:00Z', '2026-05-01T10:10:00Z', 0, 10);",
    );
    // DAG (same shape as the Go fixture):
    //   100 ─ 101 ─┬─ 102 ─ 103                  (main; 102 enabled)
    //              └─ 200 ─ 201 ─┬─ 202 ─ 203    (fork @101; 202 enabled)
    //                            └─ 300 ─ 301    (nested fork @201)
    let rows: [(i64, &str, &str, &str, i64); 10] = [
        (100, "NULL", "user", "10:00:01", 1),
        (101, "100", "assistant", "10:00:02", 1),
        (102, "101", "user", "10:00:03", 1),
        (103, "102", "assistant", "10:00:04", 1),
        (200, "101", "user", "10:01:00", 0),
        (201, "200", "assistant", "10:01:01", 1),
        (202, "201", "user", "10:01:02", 1),
        (203, "202", "assistant", "10:01:03", 1),
        (300, "201", "user", "10:02:00", 0),
        (301, "300", "assistant", "10:02:01", 1),
    ];
    for (id, parent, role, hm, enabled) in rows {
        exec(
            &db,
            &format!(
                "INSERT INTO messages (id, parent_chat_id, parent_message_id, role, created_at,
                                       updated_at, enabled)
                 VALUES ({id}, 42, {parent}, '{role}', '2026-05-01T{hm}Z',
                         '2026-05-01T{hm}Z', {enabled});"
            ),
        );
        seed_text_part(&db, id * 10, id, 0, &format!("msg {id}"), false);
    }

    let sessions = PiebaldProvider.parse(&virtual_path(&db, 42)).unwrap();
    assert_eq!(sessions.len(), 3, "main + outer fork + nested fork");
    let find = |id: &str| {
        sessions
            .iter()
            .find(|s| s.meta.id == id)
            .unwrap_or_else(|| panic!("missing session {id}"))
    };

    let main = find("piebald:42");
    assert_eq!(main.blocks.len(), 4);
    assert_eq!(main.meta.first_message, "msg 100");

    let outer = find("piebald:42-200");
    assert_eq!(outer.blocks.len(), 4);
    assert_eq!(outer.meta.first_message, "msg 200");
    // Fork branches span only their own messages, not the chat envelope.
    assert_eq!(
        outer.meta.started_at,
        time::parse_timestamp("2026-05-01T10:01:00Z", false)
    );
    assert_eq!(
        outer.meta.ended_at,
        time::parse_timestamp("2026-05-01T10:01:03Z", false)
    );

    let nested = find("piebald:42-300");
    assert_eq!(nested.blocks.len(), 2);
    assert_eq!(nested.meta.first_message, "msg 300");
}

#[test]
fn tool_results_attach_with_error_and_state_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count)
         VALUES (7, 'Tools', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 1);
         INSERT INTO messages (id, parent_chat_id, role, created_at, updated_at)
         VALUES (70, 7, 'assistant', '2026-05-01T10:00:01Z', '2026-05-01T10:00:01Z');
         INSERT INTO message_parts (id, parent_chat_message_id, part_index, part_type)
         VALUES (700, 70, 0, 'tool_call'), (701, 70, 1, 'tool_call'), (702, 70, 2, 'tool_call');
         INSERT INTO message_part_tool_call (message_part_id, provider_tool_use_id, tool_name,
                                             tool_input, tool_result, tool_error, tool_state,
                                             sub_agent_chat_id)
         VALUES (700, 'tu_1', 'Read', '{\"path\":\"README.md\"}', 'file contents', NULL, 'completed', 99),
                (701, 'tu_2', 'Bash', 'echo hi', NULL, 'boom', 'failed', NULL),
                (702, 'tu_3', 'Edit', '{}', NULL, NULL, 'canceled', NULL);",
    );

    let sessions = PiebaldProvider.parse(&virtual_path(&db, 7)).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    assert_eq!(s.meta.message_count, 1);
    assert_eq!(s.meta.user_message_count, 0);
    // No token columns anywhere → usage must stay truthfully absent.
    assert!(!s.meta.usage.has_usage);
    assert_eq!(s.blocks.len(), 1);

    let read = tool_seg(&s.blocks[0], 0);
    assert_eq!(read.tool_name, "Read");
    assert_eq!(read.tool_input, serde_json::json!({"path": "README.md"}));
    assert_eq!(read.result.as_ref().unwrap().output, "file contents");
    assert_eq!(read.status, ToolStatus::Complete);

    // NULL tool_result falls back to tool_error, flagged as an error;
    // non-JSON tool_input passes through as a plain string.
    let bash = tool_seg(&s.blocks[0], 1);
    assert_eq!(bash.tool_input, serde_json::Value::String("echo hi".into()));
    let result = bash.result.as_ref().unwrap();
    assert_eq!(result.output, "boom");
    assert!(result.is_error);
    assert_eq!(bash.status, ToolStatus::Error);

    // Nothing but a non-completed state → `[state]` marker.
    let edit = tool_seg(&s.blocks[0], 2);
    assert_eq!(edit.result.as_ref().unwrap().output, "[canceled]");
    assert_eq!(edit.status, ToolStatus::Complete);
}

#[test]
fn discovery_lists_only_live_contentful_chats() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count)
         VALUES (1, 'active', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 1),
                (2, 'empty', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 0),
                (3, 'deleted', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 1, 1);",
    );
    let found = PiebaldProvider.discover(dir.path());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "piebald:1");
    assert_eq!(found[0].path, virtual_path(&db, 1));
    assert_eq!(
        Some(found[0].mtime),
        time::parse_timestamp("2026-05-01T10:01:00Z", false)
    );

    // No app.db at all → silence, not an error.
    let empty = tempfile::tempdir().unwrap();
    assert!(PiebaldProvider.discover(empty.path()).is_empty());
}

#[test]
fn contentless_chats_are_skipped_and_bare_db_parse_works() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count)
         VALUES (1, 'real', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 1),
                (2, 'partless', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 1);
         INSERT INTO messages (id, parent_chat_id, role, created_at, updated_at)
         VALUES (10, 1, 'user', '2026-05-01T10:00:01Z', '2026-05-01T10:00:01Z'),
                (20, 2, 'user', '2026-05-01T10:00:01Z', '2026-05-01T10:00:01Z');",
    );
    seed_text_part(&db, 100, 10, 0, "hello", false);

    // A chat whose only message has no contentful parts is omitted entirely.
    assert!(PiebaldProvider
        .parse(&virtual_path(&db, 2))
        .unwrap()
        .is_empty());
    // A bare DB path parses every chat (the Go ParsePiebaldDB shape).
    let all = PiebaldProvider.parse(&db).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].meta.id, "piebald:1");
    // A directly-requested missing chat is an error, not silence.
    assert!(PiebaldProvider.parse(&virtual_path(&db, 999)).is_err());
}

#[test]
fn missing_detail_rows_are_counted_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let db = new_db(dir.path());
    exec(
        &db,
        "INSERT INTO chats (id, title, created_at, updated_at, is_deleted, message_count)
         VALUES (5, 'corrupt', '2026-05-01T10:00:00Z', '2026-05-01T10:01:00Z', 0, 1);
         INSERT INTO messages (id, parent_chat_id, role, created_at, updated_at)
         VALUES (50, 5, 'user', '2026-05-01T10:00:01Z', '2026-05-01T10:00:01Z');
         -- text part WITHOUT its message_part_text detail row:
         INSERT INTO message_parts (id, parent_chat_message_id, part_index, part_type)
         VALUES (500, 50, 0, 'text');",
    );
    seed_text_part(&db, 501, 50, 1, "still here", false);

    let sessions = PiebaldProvider.parse(&virtual_path(&db, 5)).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].meta.malformed_lines, 1);
    assert_eq!(sessions[0].meta.first_message, "still here");
    assert_eq!(sessions[0].blocks.len(), 1);
}
