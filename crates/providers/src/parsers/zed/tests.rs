// crates/providers/src/parsers/zed/tests.rs

use super::ZedProvider;
use crate::discover::Provider;
use crate::util::time;
use claude_view_types::block_types::{AssistantSegment, ConversationBlock, ToolStatus};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

const THREAD_ID: &str = "10431c84-c47b-4e6c-b2df-f9f3b9ad025b";

const DOC: &str = r#"{
    "model": {"model": "claude-opus-4", "provider": "anthropic"},
    "request_token_usage": {
        "req-1": {"input_tokens": 1000, "output_tokens": 200},
        "req-2": {"input_tokens": 1500, "output_tokens": 300}
    },
    "messages": [
        {"User": {"content": [{"Text": "Generate code"}]}},
        {"Agent": {"content": [
            {"Thinking": {"text": "Plan"}},
            {"Text": {"text": "Done"}},
            {"ToolUse": {"id": "call_1", "name": "terminal",
                         "input": {"command": "make test"}}}
        ], "tool_results": {"call_1": {"content": [{"Text": "ok"}]}}}}
    ]
}"#;

struct TestThread<'a> {
    id: &'a str,
    summary: &'a str,
    created_at: &'a str,
    updated_at: &'a str,
    data_type: &'a str,
    data: Vec<u8>,
    parent_id: Option<&'a str>,
    folder_paths: &'a str,
}

impl Default for TestThread<'_> {
    fn default() -> Self {
        Self {
            id: THREAD_ID,
            summary: "",
            created_at: "",
            updated_at: "",
            data_type: "json",
            data: Vec::new(),
            parent_id: None,
            folder_paths: "",
        }
    }
}

fn create_db(db_path: &Path, threads: &[TestThread]) {
    let conn = Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL, \
         updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL, \
         parent_id TEXT, folder_paths TEXT, folder_paths_order TEXT, created_at TEXT)",
    )
    .unwrap();
    for t in threads {
        conn.execute(
            "INSERT INTO threads (id, summary, updated_at, data_type, data, \
             parent_id, folder_paths, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                t.id,
                t.summary,
                t.updated_at,
                t.data_type,
                t.data,
                t.parent_id,
                t.folder_paths,
                t.created_at
            ],
        )
        .unwrap();
    }
}

fn fixture_db(dir: &Path) -> PathBuf {
    let db = dir.join("threads.db");
    create_db(
        &db,
        &[TestThread {
            summary: "WP Record Scaffold Generation",
            created_at: "2026-06-08T09:12:41.962819Z",
            updated_at: "2026-06-08T09:14:10.475149Z",
            folder_paths: "/Users/alice/code/my-app",
            data: DOC.as_bytes().to_vec(),
            ..Default::default()
        }],
    );
    db
}

#[test]
fn parses_json_thread_with_usage() {
    let dir = tempfile::tempdir().unwrap();
    let db = fixture_db(dir.path());
    let mut sessions = ZedProvider.parse(&db).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = sessions.remove(0);

    assert_eq!(s.meta.id, format!("zed:{THREAD_ID}"));
    assert_eq!(s.meta.project, "my-app");
    assert_eq!(s.meta.cwd.as_deref(), Some("/Users/alice/code/my-app"));
    assert_eq!(
        s.meta.title.as_deref(),
        Some("WP Record Scaffold Generation")
    );
    assert_eq!(s.meta.first_message, "Generate code");
    assert_eq!(s.meta.message_count, 2);
    assert_eq!(s.meta.user_message_count, 1);
    assert_eq!(s.meta.models, vec!["claude-opus-4".to_string()]);
    assert_eq!(
        s.meta.source_path,
        PathBuf::from(format!("{}#{THREAD_ID}", db.display()))
    );
    // request_token_usage summed: 1000+1500 in, 200+300 out.
    assert!(s.meta.usage.has_usage);
    assert_eq!(s.meta.usage.totals.input_tokens, 2500);
    assert_eq!(s.meta.usage.totals.output_tokens, 500);
    assert_eq!(s.meta.usage.per_model["claude-opus-4"].input_tokens, 2500);
    // started from created_at, ended from updated_at.
    assert_eq!(
        s.meta.started_at,
        time::parse_timestamp("2026-06-08T09:12:41.962819Z", false)
    );
    assert_eq!(
        s.meta.ended_at,
        time::parse_timestamp("2026-06-08T09:14:10.475149Z", false)
    );

    assert_eq!(s.blocks.len(), 2);
    let ConversationBlock::User(u) = &s.blocks[0] else {
        panic!("expected user block")
    };
    assert_eq!(u.text, "Generate code");
    let ConversationBlock::Assistant(a) = &s.blocks[1] else {
        panic!("expected assistant block")
    };
    assert_eq!(a.thinking.as_deref(), Some("Plan"));
    let AssistantSegment::Text { text, .. } = &a.segments[0] else {
        panic!("expected text segment")
    };
    assert_eq!(text, "Done");
    let AssistantSegment::Tool { execution } = &a.segments[1] else {
        panic!("expected tool segment")
    };
    assert_eq!(execution.tool_name, "terminal");
    assert_eq!(
        execution.tool_input,
        serde_json::json!({"command": "make test"})
    );
    assert_eq!(execution.status, ToolStatus::Complete);
    assert_eq!(execution.result.as_ref().unwrap().output, "ok");
}

#[test]
fn zstd_threads_decode_and_children_are_excluded() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("threads.db");
    let payload = r#"{"messages":[{"User":{"content":[{"Text":"Hello"}]}}]}"#;
    let compressed = zstd::encode_all(payload.as_bytes(), 0).unwrap();
    create_db(
        &db,
        &[
            TestThread {
                id: "parent",
                summary: "Parent",
                updated_at: "2026-06-08T09:14:10Z",
                data_type: "zstd",
                data: compressed,
                ..Default::default()
            },
            TestThread {
                id: "child",
                summary: "Child",
                parent_id: Some("parent"),
                updated_at: "2026-06-08T09:14:11Z",
                data: br#"{"messages":[{"User":{"content":[{"Text":"skip"}]}}]}"#.to_vec(),
                ..Default::default()
            },
        ],
    );
    let sessions = ZedProvider.parse(&db).unwrap();
    assert_eq!(sessions.len(), 1, "child thread must be excluded");
    assert_eq!(sessions[0].meta.id, "zed:parent");
    let ConversationBlock::User(u) = &sessions[0].blocks[0] else {
        panic!("expected user block")
    };
    assert_eq!(u.text, "Hello");
    // No model + no request_token_usage → truthful absence.
    assert!(!sessions[0].meta.usage.has_usage);
    assert_eq!(sessions[0].meta.project, "zed");
}

#[test]
fn unsupported_data_type_and_empty_threads_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("threads.db");
    create_db(
        &db,
        &[
            TestThread {
                id: "bad",
                data_type: "brotli",
                data: b"x".to_vec(),
                ..Default::default()
            },
            TestThread {
                id: "empty",
                data: br#"{"messages":[]}"#.to_vec(),
                ..Default::default()
            },
        ],
    );
    assert!(ZedProvider.parse(&db).unwrap().is_empty());
}

#[test]
fn discovery_emits_virtual_paths_with_updated_at_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let threads_dir = dir.path().join("threads");
    std::fs::create_dir_all(&threads_dir).unwrap();
    let db = threads_dir.join("threads.db");
    create_db(
        &db,
        &[
            TestThread {
                id: "t-one",
                updated_at: "2026-06-08T09:14:10Z",
                data: DOC.as_bytes().to_vec(),
                ..Default::default()
            },
            TestThread {
                id: "kid",
                parent_id: Some("t-one"),
                data: br#"{"messages":[]}"#.to_vec(),
                ..Default::default()
            },
        ],
    );
    let found = ZedProvider.discover(dir.path());
    assert_eq!(found.len(), 1, "child thread must not be discovered");
    assert_eq!(found[0].id, "zed:t-one");
    assert_eq!(
        found[0].path,
        PathBuf::from(format!("{}#t-one", db.display()))
    );
    assert_eq!(
        Some(found[0].mtime),
        time::parse_timestamp("2026-06-08T09:14:10Z", false)
    );
    assert_eq!(found[0].size_bytes, 0);
    // A root without threads/threads.db discovers nothing.
    let empty = tempfile::tempdir().unwrap();
    assert!(ZedProvider.discover(empty.path()).is_empty());
}

#[test]
fn virtual_path_parses_single_thread() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("threads.db");
    create_db(
        &db,
        &[
            TestThread {
                id: "t-one",
                data: DOC.as_bytes().to_vec(),
                ..Default::default()
            },
            TestThread {
                id: "t-two",
                data: br#"{"messages":[{"User":{"content":[{"Text":"second"}]}}]}"#.to_vec(),
                ..Default::default()
            },
        ],
    );
    let virt = PathBuf::from(format!("{}#t-two", db.display()));
    let sessions = ZedProvider.parse(&virt).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].meta.id, "zed:t-two");
    assert_eq!(sessions[0].meta.first_message, "second");
    // Vanished thread id → tolerant empty, not an error.
    let gone = PathBuf::from(format!("{}#missing", db.display()));
    assert!(ZedProvider.parse(&gone).unwrap().is_empty());
    // Path-like ids are rejected outright.
    let evil = PathBuf::from(format!("{}#../bad", db.display()));
    assert!(ZedProvider.parse(&evil).is_err());
}

#[test]
fn legacy_schema_without_folder_paths_still_parses() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("threads.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL, \
         updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL, \
         parent_id TEXT, created_at TEXT)",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO threads (id, summary, updated_at, data_type, data, parent_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
        params![
            "legacy",
            "Old thread",
            "2026-06-08T09:14:10Z",
            "json",
            br#"{"messages":[{"User":{"content":[{"Text":"hi"}]}},
                 {"Agent":{"content":[{"Text":"yo"}],
                  "tool_results":{"c1":{"output":"direct output"}}}}]}"#
                .to_vec(),
            "2026-06-08T09:12:41Z"
        ],
    )
    .unwrap();
    drop(conn);
    let sessions = ZedProvider.parse(&db).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].meta.project, "zed",
        "no folder_paths → provider fallback"
    );
    assert!(sessions[0].meta.cwd.is_none());
    // tool_results 'output' string form resolves nothing here (no matching
    // call) but must not panic or invent a block.
    assert_eq!(sessions[0].meta.message_count, 2);
}

#[test]
fn structured_user_content_without_text_is_kept_empty() {
    // Go semantics: keep messages with structured blocks but no Text leaf
    // (continuity), drop only truly-empty messages.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("threads.db");
    let doc = r#"{"messages":[
        {"User":{"content":[{"Image":{"source":"x.png"}}]}},
        {"User":{"content":[]}},
        {"User":{"content":[{"Text":"real question"}]}}
    ]}"#;
    create_db(
        &db,
        &[TestThread {
            id: "t-structured",
            data: doc.as_bytes().to_vec(),
            ..Default::default()
        }],
    );
    let sessions = ZedProvider.parse(&db).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    // Image-only kept (empty text), empty-array dropped, text kept.
    assert_eq!(s.meta.message_count, 2);
    assert_eq!(s.meta.user_message_count, 2);
    assert_eq!(s.meta.first_message, "real question");
    assert_eq!(s.blocks.len(), 2);
}
