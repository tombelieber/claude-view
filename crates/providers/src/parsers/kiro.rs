// crates/providers/src/parsers/kiro.rs
//
// Kiro CLI — two storage generations, one per default root (kind.rs):
//
//   legacy  ~/.kiro/sessions/cli/<uuid>.jsonl
//           events {kind: Prompt|AssistantMessage|ToolResults,
//                   data.content: [{kind: text|toolUse|toolResult, data}]}
//           plus sidecar <uuid>.json {session_id, cwd, title, created_at,
//           updated_at} — the sidecar session_id overrides the filename id.
//
//   current ~/.local/share/kiro-cli/data.sqlite3
//           conversations_v2(key=cwd, conversation_id, value=JSON,
//           created_at/updated_at epoch-ms, PK(key, conversation_id));
//           value: {conversation_id, history: [{user{env_context.env_state.
//           current_working_directory, content.Prompt.prompt, timestamp},
//           assistant{Response{content} | ToolUse{content, tool_uses}},
//           request_metadata.stream_end_timestamp_ms}]}.
//
// The same logical conversation can exist in BOTH stores (kiro migrated
// JSONL → SQLite); the SQLite copy is canonical. discover() runs once per
// root, so the legacy scan defensively collects the sibling store's
// conversation ids and skips shadowed files — port of agentsview's
// filterShadowedLegacyKiroFiles.
//
// SQLite quirk: the SAME conversation_id may appear under multiple cwd
// keys; the newest updated_at row is canonical and its key wins as cwd.
// kiro's generic "write" tool is split into Edit/Write on
// input.command == "strReplace" (both generations). The format carries no
// model ids and no token usage — usage.has_usage stays false.

use crate::discover::{split_virtual_path, stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DB_NAME: &str = "data.sqlite3";

pub struct KiroProvider;

impl Provider for KiroProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Kiro
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let db = if root.file_name().and_then(|n| n.to_str()) == Some(DB_NAME) {
            root.to_path_buf()
        } else {
            root.join(DB_NAME)
        };
        if db.is_file() {
            discover_sqlite(&db)
        } else {
            discover_legacy(root, &sibling_sqlite_ids(root))
        }
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        if let Some((db, raw_id)) = split_virtual_path(path).filter(|(db, id)| {
            !id.is_empty() && db.file_name().and_then(|n| n.to_str()) == Some(DB_NAME)
        }) {
            return parse_sqlite(&db, &raw_id);
        }
        parse_legacy(path)
    }
}

// ---------------------------------------------------------------- discovery

fn discover_sqlite(db_path: &Path) -> Vec<DiscoveredSession> {
    let Ok(conn) = open_ro(db_path) else {
        return Vec::new();
    };
    // SQLite bare-column rule: alongside MAX(updated_at) the other selected
    // columns come from the row that achieved the max (the canonical row).
    let Ok(mut stmt) = conn.prepare(
        "SELECT conversation_id, MAX(updated_at), LENGTH(CAST(value AS BLOB))
           FROM conversations_v2
          GROUP BY conversation_id",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    }) else {
        return Vec::new();
    };
    let mut out: Vec<DiscoveredSession> = rows
        .flatten()
        .filter(|(id, _, _)| !id.is_empty())
        .map(|(id, updated_ms, size)| DiscoveredSession {
            id: ProviderKind::Kiro.session_id(&id),
            provider: ProviderKind::Kiro,
            path: virtual_path(db_path, &id),
            project_hint: None,
            mtime: time::from_millis(updated_ms as f64),
            size_bytes: size.max(0) as u64,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn discover_legacy(root: &Path, shadowed: &HashSet<String>) -> Vec<DiscoveredSession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let sidecar = load_sidecar(&path);
        let Some(raw_id) = legacy_raw_id(&path, sidecar.as_ref()) else {
            continue;
        };
        // The SQLite copy of the same logical conversation is canonical.
        if shadowed.contains(&raw_id) {
            continue;
        }
        let Some((mtime, size_bytes)) = stat_entry(&path) else {
            continue;
        };
        let project_hint = sidecar
            .as_ref()
            .and_then(|m| m.get("cwd").and_then(Value::as_str))
            .map(project_from_cwd)
            .filter(|p| !p.is_empty());
        out.push(DiscoveredSession {
            id: ProviderKind::Kiro.session_id(&raw_id),
            provider: ProviderKind::Kiro,
            path,
            project_hint,
            mtime,
            size_bytes,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Conversation ids living in the current-generation SQLite store. The
/// store sits at `<home>/.local/share/kiro-cli/data.sqlite3`; home is
/// derived from the default legacy root shape (`~/.kiro/sessions/cli` →
/// three ancestors up), with `$HOME` as the fallback for overridden roots.
fn sibling_sqlite_ids(legacy_root: &Path) -> HashSet<String> {
    legacy_root
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .into_iter()
        .chain(dirs::home_dir())
        .map(|home| home.join(".local/share/kiro-cli").join(DB_NAME))
        .find(|db| db.is_file())
        .map(|db| sqlite_ids(&db))
        .unwrap_or_default()
}

fn sqlite_ids(db_path: &Path) -> HashSet<String> {
    let Ok(conn) = open_ro(db_path) else {
        return HashSet::new();
    };
    let Ok(mut stmt) = conn.prepare("SELECT DISTINCT conversation_id FROM conversations_v2") else {
        return HashSet::new();
    };
    let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) else {
        return HashSet::new();
    };
    rows.flatten().filter(|s| !s.is_empty()).collect()
}

fn open_ro(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(3000))?;
    Ok(conn)
}

fn virtual_path(db_path: &Path, raw_id: &str) -> PathBuf {
    PathBuf::from(format!("{}#{raw_id}", db_path.display()))
}

// ------------------------------------------------------------ legacy JSONL

/// Logical session id for a legacy JSONL file: the sidecar `session_id`
/// wins (the filename is only a storage detail), else the file stem.
fn legacy_raw_id(path: &Path, sidecar: Option<&Value>) -> Option<String> {
    sidecar
        .and_then(|m| m.get("session_id").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Companion `<uuid>.json` metadata for a legacy `<uuid>.jsonl` file.
fn load_sidecar(jsonl_path: &Path) -> Option<Value> {
    let data = std::fs::read_to_string(jsonl_path.with_extension("json")).ok()?;
    serde_json::from_str::<Value>(&data)
        .ok()
        .filter(Value::is_object)
}

fn parse_legacy(path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
    let read = jsonl::read_values(path)?;
    let sidecar = load_sidecar(path);
    let raw_id = legacy_raw_id(path, sidecar.as_ref())
        .ok_or_else(|| anyhow::anyhow!("no kiro session id for {}", path.display()))?;

    let mut meta = ForeignSessionMeta::new(ProviderKind::Kiro, &raw_id, path.to_path_buf());
    meta.malformed_lines = read.malformed;

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    let mut ordinal = 0usize;
    for value in &read.values {
        let data = value.get("data");
        match value.get("kind").and_then(Value::as_str) {
            Some("Prompt") => {
                let text = extract_text(data);
                if text.is_empty() {
                    continue;
                }
                if meta.first_message.is_empty() {
                    meta.first_message = preview(&text, 200);
                }
                meta.message_count += 1;
                meta.user_message_count += 1;
                out_blocks.push(blocks::user(blocks::block_id(&raw_id, ordinal), text, None));
                ordinal += 1;
            }
            Some("AssistantMessage") => {
                let segments = legacy_assistant_segments(data);
                if segments.is_empty() {
                    continue;
                }
                meta.message_count += 1;
                out_blocks.push(blocks::assistant(
                    blocks::block_id(&raw_id, ordinal),
                    segments,
                    None,
                    None,
                ));
                ordinal += 1;
            }
            Some("ToolResults") => attach_legacy_results(&mut out_blocks, data),
            _ => {}
        }
    }
    // No contentful messages → non-interactive noise, skip the session.
    if meta.message_count == 0 {
        return Ok(Vec::new());
    }

    if let Some(m) = &sidecar {
        if let Some(cwd) = m
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
        {
            meta.project = project_from_cwd(cwd);
            meta.cwd = Some(cwd.to_string());
        }
        if let Some(title) = m
            .get("title")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            if meta.first_message.is_empty() {
                meta.first_message = preview(title, 200);
            }
            meta.title = Some(title.to_string());
        }
        for field in ["created_at", "updated_at"] {
            if let Some(ts) = m
                .get(field)
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false))
            {
                meta.observe_timestamp(ts);
            }
        }
    }
    if meta.project.is_empty() {
        meta.project = "unknown".to_string();
    }
    Ok(vec![ForeignSession {
        meta,
        blocks: out_blocks,
    }])
}

fn content_blocks(data: Option<&Value>) -> impl Iterator<Item = &Value> {
    data.and_then(|d| d.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// Concatenated `text` parts of a legacy `data.content` array.
fn extract_text(data: Option<&Value>) -> String {
    let parts: Vec<&str> = content_blocks(data)
        .filter(|b| b.get("kind").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("data").and_then(Value::as_str))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    parts.join("\n\n")
}

fn legacy_assistant_segments(data: Option<&Value>) -> Vec<AssistantSegment> {
    let mut segments = Vec::new();
    for block in content_blocks(data) {
        match block.get("kind").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = block.get("data").and_then(Value::as_str) {
                    let t = t.trim();
                    if !t.is_empty() {
                        segments.push(blocks::text_segment(t.to_string()));
                    }
                }
            }
            Some("toolUse") => {
                let Some(tu) = block.get("data") else {
                    continue;
                };
                let Some(name) = tu
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                else {
                    continue;
                };
                let input = tu.get("input").cloned().unwrap_or(Value::Null);
                let tool_id = tu
                    .get("toolUseId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                segments.push(blocks::tool_segment(
                    normalize_write_tool(name, &input),
                    input,
                    tool_id,
                ));
            }
            _ => {}
        }
    }
    segments
}

/// kiro's generic "write" tool covers both in-place edits and full file
/// writes; the input `command` disambiguates (agentsview normalization,
/// applied in both generations).
fn normalize_write_tool(name: &str, input: &Value) -> String {
    if name != "write" {
        return name.to_string();
    }
    if input.get("command").and_then(Value::as_str) == Some("strReplace") {
        "Edit".to_string()
    } else {
        "Write".to_string()
    }
}

fn attach_legacy_results(out: &mut [ConversationBlock], data: Option<&Value>) {
    for block in content_blocks(data) {
        if block.get("kind").and_then(Value::as_str) != Some("toolResult") {
            continue;
        }
        let Some(tr) = block.get("data") else {
            continue;
        };
        let Some(tool_use_id) = tr
            .get("toolUseId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let output = match tr.get("content") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(s)) => s.clone(),
            // Non-string payloads surface as raw JSON — agentsview keeps
            // ContentRaw verbatim; never invent a prettier rendering.
            Some(other) => other.to_string(),
        };
        blocks::attach_tool_result(out, tool_use_id, output, false);
    }
}

// ------------------------------------------------------------ SQLite store

fn parse_sqlite(db_path: &Path, raw_id: &str) -> anyhow::Result<Vec<ForeignSession>> {
    let conn = open_ro(db_path)
        .map_err(|e| anyhow::anyhow!("opening kiro sqlite db {}: {e}", db_path.display()))?;
    // Same conversation_id under multiple cwd keys → newest row is canonical.
    let (key, value, created_ms, updated_ms) = conn
        .query_row(
            "SELECT key, value, created_at, updated_at
               FROM conversations_v2
              WHERE conversation_id = ?1
              ORDER BY updated_at DESC, created_at DESC, key DESC
              LIMIT 1",
            rusqlite::params![raw_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|e| anyhow::anyhow!("loading kiro sqlite session {raw_id}: {e}"))?;
    let doc: Value = serde_json::from_str(&value)
        .map_err(|_| anyhow::anyhow!("kiro sqlite session {raw_id} has malformed payload"))?;

    let mut meta =
        ForeignSessionMeta::new(ProviderKind::Kiro, raw_id, virtual_path(db_path, raw_id));
    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    let mut cwd = String::new();
    let mut ordinal = 0usize;

    for turn in doc
        .get("history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if cwd.is_empty() {
            if let Some(c) = turn
                .pointer("/user/env_context/env_state/current_working_directory")
                .and_then(Value::as_str)
            {
                cwd = c.to_string();
            }
        }
        let prompt = turn
            .pointer("/user/content/Prompt/prompt")
            .and_then(Value::as_str)
            .map_or("", str::trim);
        if !prompt.is_empty() {
            let ts = turn
                .pointer("/user/timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            if meta.first_message.is_empty() {
                meta.first_message = preview(prompt, 200);
            }
            meta.message_count += 1;
            meta.user_message_count += 1;
            out_blocks.push(blocks::user(
                blocks::block_id(raw_id, ordinal),
                prompt.to_string(),
                ts,
            ));
            ordinal += 1;
        }

        let segments = sqlite_assistant_segments(turn.get("assistant"));
        if !segments.is_empty() {
            let ts = turn
                .pointer("/request_metadata/stream_end_timestamp_ms")
                .and_then(Value::as_f64)
                .filter(|ms| *ms > 0.0)
                .map(time::from_millis);
            if let Some(t) = ts {
                meta.observe_timestamp(t);
            }
            meta.message_count += 1;
            out_blocks.push(blocks::assistant(
                blocks::block_id(raw_id, ordinal),
                segments,
                None,
                ts,
            ));
            ordinal += 1;
        }
    }

    // No contentful messages → non-interactive noise, skip the session.
    if meta.message_count == 0 {
        return Ok(Vec::new());
    }
    if !key.is_empty() {
        cwd = key;
    }
    if !cwd.is_empty() {
        meta.project = project_from_cwd(&cwd);
        meta.cwd = Some(cwd);
    }
    if meta.project.is_empty() {
        meta.project = "unknown".to_string();
    }
    meta.observe_timestamp(time::from_millis(created_ms as f64));
    meta.observe_timestamp(time::from_millis(updated_ms as f64));
    Ok(vec![ForeignSession {
        meta,
        blocks: out_blocks,
    }])
}

/// Segments for one sqlite history turn's assistant payload — either a
/// plain `Response` or a `ToolUse` carrying text + tool_uses.
fn sqlite_assistant_segments(assistant: Option<&Value>) -> Vec<AssistantSegment> {
    let Some(assistant) = assistant else {
        return Vec::new();
    };
    if let Some(response) = assistant.get("Response") {
        let text = response
            .get("content")
            .and_then(Value::as_str)
            .map_or("", str::trim);
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![blocks::text_segment(text.to_string())]
        };
    }
    let Some(tool_use) = assistant.get("ToolUse") else {
        return Vec::new();
    };
    let mut segments = Vec::new();
    let text = tool_use
        .get("content")
        .and_then(Value::as_str)
        .map_or("", str::trim);
    if !text.is_empty() {
        segments.push(blocks::text_segment(text.to_string()));
    }
    for tu in tool_use
        .get("tool_uses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = tu
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        let args = tu.get("args").cloned().unwrap_or(Value::Null);
        let tool_id = tu
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        segments.push(blocks::tool_segment(
            normalize_write_tool(name, &args),
            args,
            tool_id,
        ));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    // ------------------------------------------------------ legacy fixtures

    const LEGACY_JSONL: &str = concat!(
        r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"Fix the parser"}]}}"#,
        "\n",
        r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"text","data":"Editing now."},{"kind":"toolUse","data":{"toolUseId":"tu-1","name":"write","input":{"command":"strReplace","path":"src/main.rs"}}}]}}"#,
        "\n",
        r#"{"kind":"ToolResults","data":{"content":[{"kind":"toolResult","data":{"toolUseId":"tu-1","content":"replaced 1 occurrence"}}]}}"#,
        "\n",
        r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"toolUse","data":{"toolUseId":"tu-2","name":"write","input":{"command":"create","path":"new.rs"}}}]}}"#,
        "\n",
        "not json\n",
    );

    const LEGACY_SIDECAR: &str = r#"{
      "session_id": "meta-id-1",
      "cwd": "/home/u/code/kiro-app",
      "title": "Fix parser",
      "created_at": "2026-05-17T10:00:00Z",
      "updated_at": "2026-05-17T10:05:00Z"
    }"#;

    fn write_legacy(dir: &Path, stem: &str, lines: &str, sidecar: Option<&str>) -> PathBuf {
        let path = dir.join(format!("{stem}.jsonl"));
        std::fs::write(&path, lines).unwrap();
        if let Some(s) = sidecar {
            std::fs::write(dir.join(format!("{stem}.json")), s).unwrap();
        }
        path
    }

    // ------------------------------------------------------ sqlite fixtures

    /// Port of agentsview's testdata/kiro_sqlite/standard_payload.json.
    const SQLITE_PAYLOAD: &str = r#"{
      "conversation_id": "sqlite-session",
      "history": [
        {
          "user": {
            "env_context": { "env_state": { "current_working_directory": "/env/ignored" } },
            "content": { "Prompt": { "prompt": "Build the Kiro parser" } },
            "timestamp": "2026-05-17T10:00:00Z"
          },
          "assistant": { "Response": { "message_id": "a-1", "content": "I can do that." } },
          "request_metadata": { "stream_end_timestamp_ms": 1779012010000 }
        },
        {
          "user": {
            "content": { "Prompt": { "prompt": "Read the source first" } },
            "timestamp": "2026-05-17T10:00:20Z"
          },
          "assistant": {
            "ToolUse": {
              "message_id": "a-2",
              "content": "",
              "tool_uses": [
                { "id": "tool-1", "name": "execute_bash",
                  "args": { "command": "sed -n '1,120p' kiro.go" } },
                { "id": "tool-2", "name": "write",
                  "args": { "command": "strReplace", "path": "x.rs" } }
              ]
            }
          },
          "request_metadata": { "stream_end_timestamp_ms": 1779012030000 }
        }
      ]
    }"#;

    fn fixture_db(dir: &Path) -> PathBuf {
        let db_path = dir.join(DB_NAME);
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations_v2 (
                key TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                value TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (key, conversation_id)
            );",
        )
        .unwrap();
        db_path
    }

    fn seed(db: &Path, key: &str, id: &str, value: &str, created: i64, updated: i64) {
        let conn = Connection::open(db).unwrap();
        conn.execute(
            "INSERT INTO conversations_v2
                (key, conversation_id, value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![key, id, value, created, updated],
        )
        .unwrap();
    }

    // -------------------------------------------------------------- tests

    #[test]
    fn parses_legacy_jsonl_with_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_legacy(dir.path(), "0a1b", LEGACY_JSONL, Some(LEGACY_SIDECAR));
        let mut sessions = KiroProvider.parse(&path).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        // Sidecar session_id overrides the filename id.
        assert_eq!(s.meta.id, "kiro:meta-id-1");
        assert_eq!(s.meta.project, "kiro-app");
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/u/code/kiro-app"));
        assert_eq!(s.meta.title.as_deref(), Some("Fix parser"));
        assert_eq!(s.meta.first_message, "Fix the parser");
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.user_message_count, 1);
        assert_eq!(s.meta.malformed_lines, 1);
        assert!(!s.meta.usage.has_usage, "kiro carries no token usage");
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-05-17T10:00:00Z", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-05-17T10:05:00Z", false)
        );
        assert_eq!(s.blocks.len(), 3);

        // write/strReplace → Edit, with the ToolResults output attached.
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block");
        };
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment");
        };
        assert_eq!(execution.tool_name, "Edit");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "replaced 1 occurrence"
        );

        // write with any other command → Write, no result.
        let ConversationBlock::Assistant(b) = &s.blocks[2] else {
            panic!("expected assistant block");
        };
        let AssistantSegment::Tool { execution } = &b.segments[0] else {
            panic!("expected tool segment");
        };
        assert_eq!(execution.tool_name, "Write");
        assert!(execution.result.is_none());
    }

    #[test]
    fn legacy_without_content_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let lines = concat!(
            r#"{"kind":"ToolResults","data":{"content":[{"kind":"toolResult","data":{"toolUseId":"tu-9","content":"x"}}]}}"#,
            "\n",
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"   "}]}}"#,
            "\n",
        );
        let path = write_legacy(dir.path(), "empty", lines, None);
        assert!(KiroProvider.parse(&path).unwrap().is_empty());
    }

    #[test]
    fn parses_sqlite_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        seed(
            &db,
            "/home/user/code/kiro-app",
            "sqlite-session",
            SQLITE_PAYLOAD,
            1_779_012_000_000,
            1_779_012_030_000,
        );
        let mut sessions = KiroProvider
            .parse(&virtual_path(&db, "sqlite-session"))
            .unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);
        assert_eq!(s.meta.id, "kiro:sqlite-session");
        // The row key wins over the env_context cwd.
        assert_eq!(s.meta.cwd.as_deref(), Some("/home/user/code/kiro-app"));
        assert_eq!(s.meta.project, "kiro-app");
        assert_eq!(s.meta.first_message, "Build the Kiro parser");
        assert_eq!(s.meta.message_count, 4);
        assert_eq!(s.meta.user_message_count, 2);
        assert_eq!(s.meta.started_at, Some(1_779_012_000.0));
        assert_eq!(s.meta.ended_at, Some(1_779_012_030.0));
        assert!(!s.meta.usage.has_usage);
        assert_eq!(s.blocks.len(), 4);

        let ConversationBlock::Assistant(a) = &s.blocks[3] else {
            panic!("expected assistant block");
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment");
        };
        // Provider-native names pass through verbatim…
        assert_eq!(execution.tool_name, "execute_bash");
        assert_eq!(execution.tool_use_id, "tool-1");
        let AssistantSegment::Tool { execution } = &a.segments[1] else {
            panic!("expected tool segment");
        };
        // …except "write", normalized in the sqlite generation too.
        assert_eq!(execution.tool_name, "Edit");
    }

    #[test]
    fn sqlite_newest_row_wins_and_discovery_dedups() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let payload = r#"{"conversation_id":"dupe","history":[{"user":{"content":{"Prompt":{"prompt":"hello"}}},"assistant":{"Response":{"content":"hi"}}}]}"#;
        seed(&db, "/tmp/old", "dupe", payload, 1_000, 2_000);
        seed(&db, "/tmp/new", "dupe", payload, 1_000, 9_000);

        // One logical conversation, mtime from the newest row.
        let found = KiroProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "kiro:dupe");
        assert_eq!(found[0].mtime, 9.0);

        let sessions = KiroProvider.parse(&found[0].path).unwrap();
        let s = &sessions[0];
        assert_eq!(s.meta.cwd.as_deref(), Some("/tmp/new"));
        assert_eq!(s.meta.started_at, Some(1.0));
        assert_eq!(s.meta.ended_at, Some(9.0));
        assert_eq!(s.meta.message_count, 2);
    }

    #[test]
    fn legacy_shadowed_by_sqlite_store_is_skipped() {
        // Fake home: <home>/.kiro/sessions/cli + <home>/.local/share/kiro-cli
        // — the legacy scan derives the sibling store from the root shape.
        let home = tempfile::tempdir().unwrap();
        let legacy = home.path().join(".kiro/sessions/cli");
        std::fs::create_dir_all(&legacy).unwrap();
        let store = home.path().join(".local/share/kiro-cli");
        std::fs::create_dir_all(&store).unwrap();
        let db = fixture_db(&store);
        seed(&db, "/tmp/p", "shadowed-conv", "{}", 1, 2);

        write_legacy(&legacy, "live-conv", LEGACY_JSONL, None);
        write_legacy(&legacy, "shadowed-conv", LEGACY_JSONL, None);

        let found = KiroProvider.discover(&legacy);
        assert_eq!(found.len(), 1, "sqlite copy is canonical");
        assert_eq!(found[0].id, "kiro:live-conv");
    }

    #[test]
    fn malformed_sqlite_payload_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        seed(&db, "/tmp/p", "broken", "this is not json {", 1, 2);
        let err = KiroProvider
            .parse(&virtual_path(&db, "broken"))
            .unwrap_err();
        assert!(err.to_string().contains("malformed payload"));
    }
}
