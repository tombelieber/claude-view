// crates/providers/src/parsers/opencode/mod.rs
//
// OpenCode (opencode.ai) — root `~/.local/share/opencode`, TWO backends
// (ported from agentsview's opencode.go + discovery.go):
//   • file storage (current): storage/session/<project>/<id>.json with
//     per-message storage/message/<sessionID>/*.json and per-part
//     storage/part/<messageID>/*.json documents;
//   • legacy SQLite (`opencode.db`): project/session/message/part tables
//     with JSON `data` columns, addressed as `<db>#<sessionID>`.
// Hybrid roots scan both; the storage transcript is canonical, so DB rows
// duplicating a storage session id are skipped during discovery.

mod build;
mod sqlite;
mod storage;

use crate::discover::{split_virtual_path, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct OpencodeProvider;

impl Provider for OpencodeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Opencode
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let mut out = Vec::new();
        let mut storage_ids: HashSet<String> = HashSet::new();
        let session_root = root.join("storage").join("session");
        if session_root.is_dir() {
            discover_storage(&session_root, &mut storage_ids, &mut out);
        }
        let db_path = root.join("opencode.db");
        if db_path.is_file() {
            discover_sqlite(&db_path, &storage_ids, &mut out);
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        if let Some((db_path, raw_id)) = split_virtual_path(path) {
            return sqlite::parse_session(&db_path, &raw_id, path);
        }
        storage::parse_session_file(path)
    }
}

fn discover_storage(
    session_root: &Path,
    storage_ids: &mut HashSet<String>,
    out: &mut Vec<DiscoveredSession>,
) {
    let Ok(projects) = std::fs::read_dir(session_root) else {
        return;
    };
    for project in projects.flatten() {
        let project_dir = project.path();
        if !project_dir.is_dir() {
            continue;
        }
        // "global" is OpenCode's no-project bucket — not a real hint.
        let dir_name = project.file_name();
        let dir_name = dir_name.to_string_lossy();
        let hint = (dir_name != "global").then(|| dir_name.to_string());
        let Ok(files) = std::fs::read_dir(&project_dir) else {
            continue;
        };
        for path in files.flatten().map(|e| e.path()) {
            if path.extension().and_then(|e| e.to_str()) != Some("json") || !path.is_file() {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some((mtime, size_bytes)) = storage::composite_stat(&path) else {
                continue;
            };
            let id = ProviderKind::Opencode.session_id(stem);
            storage_ids.insert(stem.to_string());
            out.push(DiscoveredSession {
                id,
                provider: ProviderKind::Opencode,
                path,
                project_hint: hint.clone(),
                mtime,
                size_bytes,
            });
        }
    }
}

fn discover_sqlite(
    db_path: &Path,
    storage_ids: &HashSet<String>,
    out: &mut Vec<DiscoveredSession>,
) {
    let Ok(rows) = sqlite::list_sessions(db_path) else {
        return; // locked/corrupt DB — providers degrade to absent, never wrong
    };
    for (raw_id, updated_ms) in rows {
        if raw_id.is_empty() || storage_ids.contains(&raw_id) {
            continue;
        }
        out.push(DiscoveredSession {
            id: ProviderKind::Opencode.session_id(&raw_id),
            provider: ProviderKind::Opencode,
            path: PathBuf::from(format!("{}#{raw_id}", db_path.display())),
            project_hint: None,
            mtime: updated_ms as f64 / 1000.0,
            size_bytes: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UsageTotals;
    use claude_view_types::block_types::{AssistantSegment, ConversationBlock, ToolStatus};
    use serde_json::json;

    fn write_json(path: &Path, v: serde_json::Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_string(&v).unwrap()).unwrap();
    }

    /// Full storage-mode fixture: user text, assistant with tool part
    /// (input + completed output) and trailing text, message-level tokens.
    fn storage_fixture(root: &Path) -> PathBuf {
        let session = root.join("storage/session/global/ses_storage.json");
        write_json(
            &session,
            json!({
                "id": "ses_storage",
                "directory": "/home/user/code/myapp",
                "title": "Storage Session",
                "time": { "created": 1_700_000_000_000_i64, "updated": 1_700_000_060_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/message/ses_storage/msg_1.json"),
            json!({
                "id": "msg_1", "sessionID": "ses_storage", "role": "user",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_1/prt_1.json"),
            json!({
                "id": "prt_1", "messageID": "msg_1", "type": "text",
                "text": "Hello from storage",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/message/ses_storage/msg_2.json"),
            json!({
                "id": "msg_2", "sessionID": "ses_storage", "role": "assistant",
                "modelID": "gpt-5.2-codex",
                "tokens": { "input": 11, "output": 7, "cache": { "read": 3, "write": 2 } },
                "time": { "created": 1_700_000_010_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_2/prt_2.json"),
            json!({
                "id": "prt_2", "messageID": "msg_2", "type": "tool",
                "tool": "read", "callID": "call_1",
                "state": {
                    "status": "completed",
                    "input": { "file_path": "main.go" },
                    "output": "package main"
                },
                "time": { "created": 1_700_000_010_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_2/prt_3.json"),
            json!({
                "id": "prt_3", "messageID": "msg_2", "type": "text",
                "text": "Here is the file.",
                "time": { "created": 1_700_000_011_000_i64 }
            }),
        );
        session
    }

    const DB_SCHEMA: &str = "
        CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT NOT NULL);
        CREATE TABLE session (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,
            title TEXT, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL
        );
        CREATE TABLE message (
            id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
            time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
            data TEXT NOT NULL
        );
        CREATE TABLE part (
            id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL,
            time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
            data TEXT NOT NULL
        );
    ";

    fn seed_db(db_path: &Path) {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute_batch(DB_SCHEMA).unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO project VALUES ('prj_1', '/home/user/code/myapp');
            INSERT INTO session VALUES
                ('ses_abc', 'prj_1', NULL, 'Test Session', 1700000000000, 1700000060000);
            INSERT INTO message VALUES
                ('msg_1', 'ses_abc', 1700000000000, 1700000000000, '{"role":"user"}');
            INSERT INTO part VALUES
                ('prt_1', 'msg_1', 'ses_abc', 1700000000000, 1700000000000,
                 '{"type":"text","text":"Hello, help me with Go"}');
            INSERT INTO message VALUES
                ('msg_2', 'ses_abc', 1700000010000, 1700000010000,
                 '{"role":"assistant","modelID":"claude-sonnet-4-20250514","providerID":"anthropic","tokens":{"input":1,"output":102,"reasoning":0,"cache":{"read":500,"write":11969}}}');
            INSERT INTO part VALUES
                ('prt_2', 'msg_2', 'ses_abc', 1700000010000, 1700000010000,
                 '{"type":"text","text":"Sure, I can help with Go."}');
            INSERT INTO message VALUES
                ('msg_bad', 'ses_abc', 1700000020000, 1700000020000, 'not json');
            "#,
        )
        .unwrap();
    }

    #[test]
    fn parses_storage_session() {
        let dir = tempfile::tempdir().unwrap();
        let session = storage_fixture(dir.path());
        let mut sessions = OpencodeProvider.parse(&session).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "opencode:ses_storage");
        assert_eq!(s.meta.project, "myapp");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/user/code/myapp"));
        assert_eq!(s.meta.title.as_deref(), Some("Storage Session"));
        assert_eq!(s.meta.first_message, "Hello from storage");
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.started_at, Some(1_700_000_000.0));
        assert_eq!(s.meta.ended_at, Some(1_700_000_060.0));
        assert_eq!(s.meta.models, vec!["gpt-5.2-codex".to_string()]);
        assert!(s.meta.usage.has_usage);
        assert_eq!(
            s.meta.usage.totals,
            UsageTotals {
                input_tokens: 11,
                output_tokens: 7,
                cache_read_input_tokens: 3,
                cache_creation_input_tokens: 2,
            }
        );

        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        // Parts sort by time: tool part (t=…010000) before text (t=…011000).
        assert_eq!(a.segments.len(), 2);
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment first");
        };
        assert_eq!(execution.tool_name, "read");
        assert_eq!(execution.tool_input, json!({ "file_path": "main.go" }));
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(execution.result.as_ref().unwrap().output, "package main");
    }

    #[test]
    fn part_ordering_prefers_start_over_created() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_json(
            &root.join("storage/session/global/ses_ord.json"),
            json!({
                "id": "ses_ord", "directory": "/tmp/proj",
                "time": { "created": 1_700_000_000_000_i64, "updated": 1_700_000_060_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/message/ses_ord/msg_1.json"),
            json!({
                "id": "msg_1", "role": "assistant",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        // start beats created: part_1 starts LATER despite earlier created.
        write_json(
            &root.join("storage/part/msg_1/part_1.json"),
            json!({
                "id": "part_1", "messageID": "msg_1", "type": "text", "text": "second",
                "time": { "start": 1_700_000_002_000_i64, "created": 1_700_000_001_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_1/part_2.json"),
            json!({
                "id": "part_2", "messageID": "msg_1", "type": "text", "text": "first",
                "time": { "start": 1_700_000_001_000_i64, "created": 1_700_000_002_000_i64 }
            }),
        );

        let sessions = OpencodeProvider
            .parse(&root.join("storage/session/global/ses_ord.json"))
            .unwrap();
        assert_eq!(sessions.len(), 1);
        let ConversationBlock::Assistant(a) = &sessions[0].blocks[0] else {
            panic!("expected assistant block");
        };
        let texts: Vec<&str> = a
            .segments
            .iter()
            .map(|seg| match seg {
                AssistantSegment::Text { text, .. } => text.as_str(),
                _ => panic!("expected text segments"),
            })
            .collect();
        assert_eq!(texts, vec!["first", "second"]);
    }

    #[test]
    fn step_finish_tokens_override_message_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_json(
            &root.join("storage/session/global/ses_tok.json"),
            json!({
                "id": "ses_tok", "directory": "/tmp/proj",
                "time": { "created": 1_700_000_000_000_i64, "updated": 1_700_000_060_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/message/ses_tok/msg_1.json"),
            json!({
                "id": "msg_1", "role": "assistant", "modelID": "gpt-5.2-codex",
                "tokens": { "input": 1, "output": 1 },
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_1/prt_1.json"),
            json!({
                "id": "prt_1", "messageID": "msg_1", "type": "text",
                "text": "reply", "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_1/prt_2.json"),
            json!({
                "id": "prt_2", "messageID": "msg_1", "type": "step-finish",
                "tokens": { "input": 11, "output": 7, "cache": { "read": 3, "write": 2 } },
                "time": { "created": 1_700_000_001_000_i64 }
            }),
        );

        let sessions = OpencodeProvider
            .parse(&root.join("storage/session/global/ses_tok.json"))
            .unwrap();
        assert_eq!(sessions.len(), 1);
        let usage = &sessions[0].meta.usage;
        assert!(usage.has_usage);
        // step-finish fields OVERRIDE message-level ones (no summing).
        let want = UsageTotals {
            input_tokens: 11,
            output_tokens: 7,
            cache_read_input_tokens: 3,
            cache_creation_input_tokens: 2,
        };
        assert_eq!(usage.totals, want);
        assert_eq!(usage.per_model["gpt-5.2-codex"], want);
    }

    #[test]
    fn default_title_placeholder_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_json(
            &root.join("storage/session/global/ses_t.json"),
            json!({
                "id": "ses_t", "directory": "/tmp/proj",
                "title": "New session - 2026-03-22T10:00:00.000Z",
                "time": { "created": 1_700_000_000_000_i64, "updated": 1_700_000_010_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/message/ses_t/msg_1.json"),
            json!({
                "id": "msg_1", "role": "user",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        write_json(
            &root.join("storage/part/msg_1/prt_1.json"),
            json!({
                "id": "prt_1", "messageID": "msg_1", "type": "text",
                "text": "Refactor the auth module",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );

        let sessions = OpencodeProvider
            .parse(&root.join("storage/session/global/ses_t.json"))
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].meta.title, None);
        assert_eq!(sessions[0].meta.first_message, "Refactor the auth module");
        assert!(
            !sessions[0].meta.usage.has_usage,
            "no tokens anywhere — must stay false"
        );
    }

    #[test]
    fn empty_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_json(
            &root.join("storage/session/global/ses_empty.json"),
            json!({
                "id": "ses_empty", "directory": "/tmp/proj",
                "time": { "created": 1_700_000_000_000_i64, "updated": 1_700_000_060_000_i64 }
            }),
        );
        // Assistant message with no parts at all → no content → skip.
        write_json(
            &root.join("storage/message/ses_empty/msg_1.json"),
            json!({
                "id": "msg_1", "role": "assistant", "modelID": "gpt-5.2-codex",
                "time": { "created": 1_700_000_000_000_i64 }
            }),
        );
        let sessions = OpencodeProvider
            .parse(&root.join("storage/session/global/ses_empty.json"))
            .unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn malformed_storage_child_aborts_parse() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let session = storage_fixture(root);
        // Truncated mid-write message file: skipping it would silently drop
        // content (OpenCode sync replaces transcripts) — must abort instead.
        std::fs::write(
            root.join("storage/message/ses_storage/msg_bad.json"),
            r#"{"id":"msg_bad""#,
        )
        .unwrap();
        assert!(OpencodeProvider.parse(&session).is_err());
    }

    #[test]
    fn parses_sqlite_session_via_virtual_path() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("opencode.db");
        seed_db(&db_path);

        let virt = PathBuf::from(format!("{}#ses_abc", db_path.display()));
        let mut sessions = OpencodeProvider.parse(&virt).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "opencode:ses_abc");
        assert_eq!(s.meta.project, "myapp");
        assert_eq!(s.meta.title.as_deref(), Some("Test Session"));
        assert_eq!(s.meta.first_message, "Hello, help me with Go");
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.malformed_lines, 1, "msg_bad must be counted");
        assert_eq!(s.meta.started_at, Some(1_700_000_000.0));
        assert_eq!(s.meta.ended_at, Some(1_700_000_060.0));
        assert_eq!(s.meta.source_path, virt);
        assert!(s.meta.usage.has_usage);
        let want = UsageTotals {
            input_tokens: 1,
            output_tokens: 102,
            cache_read_input_tokens: 500,
            cache_creation_input_tokens: 11969,
        };
        assert_eq!(s.meta.usage.totals, want);
        assert_eq!(s.meta.usage.per_model["claude-sonnet-4-20250514"], want);

        assert_eq!(s.blocks.len(), 2);
        let ConversationBlock::User(u) = &s.blocks[0] else {
            panic!("expected user block");
        };
        assert_eq!(u.text, "Hello, help me with Go");
    }

    #[test]
    fn discovery_scans_both_backends_and_dedups() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        storage_fixture(root);
        let db_path = root.join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(DB_SCHEMA).unwrap();
        conn.execute_batch(
            "INSERT INTO project VALUES ('prj_1', '/tmp/proj');
             INSERT INTO session VALUES
                ('ses_storage', 'prj_1', NULL, NULL, 1700000000000, 1700000010000);
             INSERT INTO session VALUES
                ('ses_db', 'prj_1', NULL, NULL, 1700000020000, 1700000060000);",
        )
        .unwrap();
        drop(conn);

        let mut found = OpencodeProvider.discover(root);
        found.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(found.len(), 2, "ses_storage must not be doubled");

        assert_eq!(found[0].id, "opencode:ses_db");
        let virt = found[0].path.to_str().unwrap();
        assert!(virt.ends_with("opencode.db#ses_db"), "got {virt}");
        assert_eq!(found[0].mtime, 1_700_000_060.0);

        assert_eq!(found[1].id, "opencode:ses_storage");
        assert!(found[1].path.ends_with("ses_storage.json"));
        assert_eq!(found[1].project_hint, None, "global dir is not a hint");
        assert!(found[1].size_bytes > 0);
    }
}
