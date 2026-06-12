// crates/providers/src/parsers/forge.rs
//
// Forge (forgecode.dev) — every conversation lives in ONE SQLite database at
// `<root>/.forge.db`, table `conversations(conversation_id, title, context,
// created_at, updated_at, metrics)` filtered on `context IS NOT NULL`.
//
// The `context` column is a single JSON blob: `context.messages[]`, where
// each item is either
//   • a text message  `{ message: { text: { role, content |
//     raw_content.Text (capital T), model, timestamp,
//     reasoning_details[].text, tool_calls[{name, call_id, arguments}] } } }`
//     with a SIBLING item-level `usage` object
//     (`{prompt|completion|cached}_tokens.actual`), or
//   • a tool output   `{ message: { tool: { call_id,
//     output.values[].text | output.text } } }` attached by call_id.
//
// System-role messages are prompt scaffolding (skipped), but any message may
// carry the `<current_working_directory>…</current_working_directory>` tag we
// scrape for cwd/project. The row-level `metrics` column
// (`{input_tokens, output_tokens, cached_input_tokens}`) is authoritative for
// session totals when present — it REPLACES the per-message sums.
//
// Discovery emits `<db-path>#<conversation_id>` virtual paths; mtime comes
// from `COALESCE(updated_at, created_at)` (RFC3339[Nano] or SQL
// space-separated layouts, parsed as UTC by util::time).

use crate::discover::{split_virtual_path, stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, ForeignUsage, UsageTotals};
use crate::util::{blocks, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};

const DB_FILENAME: &str = ".forge.db";

pub struct ForgeProvider;

impl Provider for ForgeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Forge
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let Some(db_path) = find_db(root) else {
            return Vec::new();
        };
        let Some((db_mtime, db_size)) = stat_entry(&db_path) else {
            return Vec::new();
        };
        let Ok(conn) = open_db(&db_path) else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT conversation_id, COALESCE(updated_at, created_at) \
             FROM conversations WHERE context IS NOT NULL",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        }) else {
            return Vec::new();
        };
        rows.flatten()
            .filter(|(raw_id, _)| !raw_id.is_empty())
            .map(|(raw_id, updated)| DiscoveredSession {
                id: ProviderKind::Forge.session_id(&raw_id),
                provider: ProviderKind::Forge,
                path: virtual_path(&db_path, &raw_id),
                project_hint: None,
                mtime: updated
                    .as_deref()
                    .and_then(|s| time::parse_timestamp(s, false))
                    .unwrap_or(db_mtime),
                size_bytes: db_size,
            })
            .collect()
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        // Virtual `<db>#<id>` parses one conversation; a plain db path parses
        // every conversation in the store.
        if let Some((db_path, raw_id)) = split_virtual_path(path) {
            let conn = open_db(&db_path)?;
            let row = load_one(&conn, &raw_id)?;
            Ok(build_session(&row, &db_path).into_iter().collect())
        } else {
            let conn = open_db(path)?;
            Ok(load_all(&conn)?
                .iter()
                .filter_map(|row| build_session(row, path))
                .collect())
        }
    }
}

fn find_db(root: &Path) -> Option<PathBuf> {
    // Env overrides may point straight at the db file; the default root is
    // the `~/.forge` directory containing it.
    if root.is_file() {
        return Some(root.to_path_buf());
    }
    let p = root.join(DB_FILENAME);
    p.is_file().then_some(p)
}

fn open_db(path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(3000))?;
    Ok(conn)
}

fn virtual_path(db_path: &Path, raw_id: &str) -> PathBuf {
    PathBuf::from(format!("{}#{}", db_path.display(), raw_id))
}

struct ConversationRow {
    id: String,
    title: String,
    context: String,
    created_at: String,
    updated_at: String,
    metrics: String,
}

const SELECT: &str = "SELECT conversation_id, COALESCE(title, ''), COALESCE(context, ''), \
     COALESCE(created_at, ''), COALESCE(updated_at, created_at, ''), COALESCE(metrics, '') \
     FROM conversations";

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationRow> {
    Ok(ConversationRow {
        id: row.get(0)?,
        title: row.get(1)?,
        context: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        metrics: row.get(5)?,
    })
}

fn load_all(conn: &Connection) -> anyhow::Result<Vec<ConversationRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE context IS NOT NULL ORDER BY COALESCE(updated_at, created_at)"
    ))?;
    let rows = stmt.query_map([], row_to_conversation)?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn load_one(conn: &Connection, raw_id: &str) -> anyhow::Result<ConversationRow> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE conversation_id = ?1"))?;
    Ok(stmt.query_row([raw_id], row_to_conversation)?)
}

/// Build one normalized session from a conversation row. Returns `None` for
/// non-interactive rows (unparseable context, no `messages` array, or zero
/// contentful user/assistant messages).
fn build_session(row: &ConversationRow, db_path: &Path) -> Option<ForeignSession> {
    let context: Value = serde_json::from_str(&row.context).ok()?;
    let messages = context.get("messages").and_then(Value::as_array)?;

    let mut meta =
        ForeignSessionMeta::new(ProviderKind::Forge, &row.id, virtual_path(db_path, &row.id));
    meta.title = Some(row.title.clone()).filter(|t| !t.is_empty());

    let mut out: Vec<ConversationBlock> = Vec::new();
    let mut cwd: Option<String> = None;

    for item in messages {
        // First cwd tag wins; the System prompt usually carries it, but any
        // message can (port of extractForgeCurrentWorkingDirectory).
        if cwd.is_none() {
            cwd = extract_cwd(item);
        }
        if let Some(text) = item.pointer("/message/text") {
            handle_text_message(&mut out, &mut meta, &row.id, item, text);
        } else if let Some(tool) = item.pointer("/message/tool") {
            let call_id = tool.get("call_id").and_then(Value::as_str).unwrap_or("");
            if call_id.is_empty() {
                continue;
            }
            let output = tool_output_text(tool.get("output"));
            let is_error = tool
                .pointer("/output/is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            blocks::attach_tool_result(&mut out, call_id, output, is_error);
        } else {
            // Neither a text message nor a tool output — unknown shape.
            meta.malformed_lines += 1;
        }
    }

    if meta.message_count == 0 {
        return None;
    }

    if let Some(c) = cwd {
        meta.project = project_from_cwd(&c);
        meta.cwd = Some(c);
    }
    if meta.project.is_empty() {
        meta.project = ProviderKind::Forge.as_str().to_string();
    }
    if let Some(ts) = time::parse_timestamp(&row.created_at, false) {
        meta.observe_timestamp(ts);
    }
    if let Some(ts) = time::parse_timestamp(&row.updated_at, false) {
        meta.observe_timestamp(ts);
    }
    apply_metrics(&mut meta.usage, &row.metrics);

    Some(ForeignSession { meta, blocks: out })
}

fn handle_text_message(
    out: &mut Vec<ConversationBlock>,
    meta: &mut ForeignSessionMeta,
    raw_id: &str,
    item: &Value,
    text: &Value,
) {
    let role = text
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let body = text_content(text);
    let id = blocks::block_id(raw_id, out.len());
    let ts = text
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| time::parse_timestamp(s, false));

    let block = match role.as_str() {
        "user" => {
            if body.is_empty() {
                return;
            }
            if meta.first_message.is_empty() {
                meta.first_message = preview(&body, 200);
            }
            meta.user_message_count += 1;
            blocks::user(id, body, ts)
        }
        "assistant" => {
            let thinking = collect_reasoning(text.get("reasoning_details"));
            let mut segments: Vec<AssistantSegment> = Vec::new();
            if !body.is_empty() {
                segments.push(blocks::text_segment(body));
            }
            append_tool_segments(&mut segments, text.get("tool_calls"));
            if segments.is_empty() && thinking.is_empty() {
                return;
            }
            blocks::assistant(id, segments, (!thinking.is_empty()).then_some(thinking), ts)
        }
        // "system" is prompt scaffolding; unknown roles carry nothing
        // renderable. Their sibling usage is discarded too (port of forge.go).
        _ => return,
    };

    meta.message_count += 1;
    let model = text.get("model").and_then(Value::as_str).unwrap_or("");
    meta.record_model(model);
    if let Some(u) = parse_usage(item.get("usage")) {
        meta.usage.record(model, u);
    }
    if let Some(ts) = ts {
        meta.observe_timestamp(ts);
    }
    out.push(block);
}

/// `content`, falling back to `raw_content.Text` (capital T), both trimmed.
fn text_content(text: &Value) -> String {
    let direct = text
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !direct.is_empty() {
        return direct.to_string();
    }
    text.pointer("/raw_content/Text")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// Join non-empty `reasoning_details[].text` entries → thinking.
fn collect_reasoning(details: Option<&Value>) -> String {
    let Some(items) = details.and_then(Value::as_array) else {
        return String::new();
    };
    items
        .iter()
        .filter_map(|d| d.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn append_tool_segments(segments: &mut Vec<AssistantSegment>, tool_calls: Option<&Value>) {
    for tc in tool_calls.and_then(Value::as_array).into_iter().flatten() {
        let Some(name) = tc
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        let call_id = tc.get("call_id").and_then(Value::as_str).unwrap_or("");
        segments.push(blocks::tool_segment(
            name.to_string(),
            tc.get("arguments").cloned().unwrap_or(Value::Null),
            call_id.to_string(),
        ));
    }
}

/// Item-level usage → Anthropic buckets. Forge accounts prompt and cached
/// tokens DISJOINTLY (its own session metrics sum them separately), so this
/// is a direct mapping — no cached subtraction (port of forgeUsageJSON).
fn parse_usage(usage: Option<&Value>) -> Option<UsageTotals> {
    let usage = usage?;
    let prompt = usage.pointer("/prompt_tokens/actual").and_then(num_u64);
    let completion = usage.pointer("/completion_tokens/actual").and_then(num_u64);
    let cached = usage.pointer("/cached_tokens/actual").and_then(num_u64);
    if prompt.is_none() && completion.is_none() && cached.is_none() {
        return None;
    }
    Some(UsageTotals {
        input_tokens: prompt.unwrap_or(0),
        output_tokens: completion.unwrap_or(0),
        cache_read_input_tokens: cached.unwrap_or(0),
        cache_creation_input_tokens: 0,
    })
}

/// Session `metrics` column is authoritative when any field is present:
/// REPLACES the per-message summed totals (port of applyForgeMetrics's
/// aggregateTokenPresenceKnown short-circuit). Per-model breakdown keeps the
/// message-level sums.
fn apply_metrics(usage: &mut ForeignUsage, metrics_raw: &str) {
    let Ok(metrics) = serde_json::from_str::<Value>(metrics_raw) else {
        return;
    };
    let input = metrics.get("input_tokens").and_then(num_u64);
    let output = metrics.get("output_tokens").and_then(num_u64);
    let cached = metrics.get("cached_input_tokens").and_then(num_u64);
    if input.is_none() && output.is_none() && cached.is_none() {
        return;
    }
    usage.totals = UsageTotals {
        input_tokens: input.unwrap_or(0),
        output_tokens: output.unwrap_or(0),
        cache_read_input_tokens: cached.unwrap_or(0),
        cache_creation_input_tokens: 0,
    };
    usage.has_usage = true;
}

fn num_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| {
        v.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| f as u64)
    })
}

/// Fallback ladder ported from forgeToolOutputText: `values[].text` joined
/// with NO separator → `output.text` → raw JSON (suppressing `null`).
fn tool_output_text(output: Option<&Value>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    let parts: Vec<&str> = output
        .get("values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.get("text").and_then(Value::as_str))
        .filter(|t| !t.is_empty())
        .collect();
    if !parts.is_empty() {
        return parts.concat();
    }
    if let Some(text) = output
        .get("text")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
    {
        return text.to_string();
    }
    if output.is_null() {
        return String::new();
    }
    output.to_string()
}

/// Scrape `<current_working_directory>…</current_working_directory>` from a
/// message's content (or raw_content.Text). Returns non-empty values only.
fn extract_cwd(item: &Value) -> Option<String> {
    let text = item.pointer("/message/text")?;
    let content = text
        .get("content")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
        .or_else(|| text.pointer("/raw_content/Text").and_then(Value::as_str))?;
    const START: &str = "<current_working_directory>";
    const END: &str = "</current_working_directory>";
    let start = content.find(START)? + START.len();
    let end = content[start..].find(END)?;
    let cwd = content[start..start + end].trim();
    (!cwd.is_empty()).then(|| cwd.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::block_types::ToolStatus;

    /// Port of the agentsview standard fixture: system prompt carrying the
    /// cwd tag, a user message, an assistant tool call with reasoning, the
    /// tool output item, and a closing assistant message — all with sibling
    /// usage objects.
    const STANDARD_CONTEXT: &str = r#"{
      "conversation_id": "conv-001",
      "messages": [
        {
          "message": { "text": {
            "role": "System",
            "content": "<system_information>\n<current_working_directory>/home/mj/dev/projects/agentsview</current_working_directory>\n</system_information>",
            "model": "gpt-5.4",
            "timestamp": "2026-05-02T09:58:15.741021507Z"
          } },
          "usage": {
            "prompt_tokens": {"actual": 0},
            "completion_tokens": {"actual": 0},
            "cached_tokens": {"actual": 0}
          }
        },
        {
          "message": { "text": {
            "role": "User",
            "content": "Please add Forge support.",
            "raw_content": {"Text": "Please add Forge support."},
            "model": "gpt-5.4",
            "timestamp": "2026-05-02T09:58:16.000000000Z"
          } },
          "usage": {
            "prompt_tokens": {"actual": 100},
            "completion_tokens": {"actual": 5},
            "cached_tokens": {"actual": 20}
          }
        },
        {
          "message": { "text": {
            "role": "Assistant",
            "content": "",
            "tool_calls": [
              {
                "name": "read",
                "call_id": "call_read_1",
                "arguments": {"file_path": "/tmp/example.go", "show_line_numbers": true}
              }
            ],
            "model": "gpt-5.4",
            "reasoning_details": [ {"text": "Inspecting the code first."} ],
            "timestamp": "2026-05-02T09:58:17.000000000Z"
          } },
          "usage": {
            "prompt_tokens": {"actual": 120},
            "completion_tokens": {"actual": 10},
            "cached_tokens": {"actual": 30}
          }
        },
        {
          "message": { "tool": {
            "name": "read",
            "call_id": "call_read_1",
            "output": {
              "is_error": false,
              "values": [ {"text": "<file path=\"/tmp/example.go\">package main</file>"} ]
            }
          } }
        },
        {
          "message": { "text": {
            "role": "Assistant",
            "content": "Added Forge support.",
            "raw_content": {"Text": "Added Forge support."},
            "model": "gpt-5.4",
            "timestamp": "2026-05-02T09:58:18.000000000Z"
          } },
          "usage": {
            "prompt_tokens": {"actual": 140},
            "completion_tokens": {"actual": 40},
            "cached_tokens": {"actual": 35}
          }
        }
      ]
    }"#;

    fn seed_db(dir: &Path) -> PathBuf {
        let db = dir.join(DB_FILENAME);
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations (
                conversation_id TEXT PRIMARY KEY NOT NULL,
                title TEXT,
                workspace_id BIGINT NOT NULL DEFAULT 1,
                context TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP,
                metrics TEXT
            );",
        )
        .unwrap();
        db
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        db: &Path,
        id: &str,
        title: &str,
        context: Option<&str>,
        created: &str,
        updated: Option<&str>,
        metrics: Option<&str>,
    ) {
        let conn = Connection::open(db).unwrap();
        conn.execute(
            "INSERT INTO conversations
             (conversation_id, title, context, created_at, updated_at, metrics)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, title, context, created, updated, metrics],
        )
        .unwrap();
    }

    fn insert_standard(db: &Path) {
        insert(
            db,
            "conv-001",
            "Add Forge Support",
            Some(STANDARD_CONTEXT),
            "2026-05-02 09:58:15.741021507",
            Some("2026-05-02 10:00:16.848497543"),
            Some(r#"{"input_tokens":360,"output_tokens":55,"cached_input_tokens":85}"#),
        );
    }

    #[test]
    fn parses_conversation_from_virtual_path() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        insert_standard(&db);

        let vpath = virtual_path(&db, "conv-001");
        let mut sessions = ForgeProvider.parse(&vpath).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = sessions.remove(0);

        assert_eq!(s.meta.id, "forge:conv-001");
        assert_eq!(s.meta.title.as_deref(), Some("Add Forge Support"));
        assert_eq!(
            s.meta.cwd.as_deref(),
            Some("/home/mj/dev/projects/agentsview")
        );
        assert_eq!(s.meta.project, "agentsview");
        assert_eq!(s.meta.first_message, "Please add Forge support.");
        assert_eq!(s.meta.user_message_count, 1);
        // user + 2 assistants; the system prompt and the tool item are not blocks.
        assert_eq!(s.meta.message_count, 3);
        assert_eq!(s.meta.models, vec!["gpt-5.4".to_string()]);
        assert_eq!(s.meta.malformed_lines, 0);

        // metrics column is authoritative for the session totals.
        assert!(s.meta.usage.has_usage);
        assert_eq!(s.meta.usage.totals.input_tokens, 360);
        assert_eq!(s.meta.usage.totals.output_tokens, 55);
        assert_eq!(s.meta.usage.totals.cache_read_input_tokens, 85);
        // per-model breakdown keeps the per-message sums (100+120+140, …);
        // the skipped System message's zero usage is NOT recorded.
        let pm = &s.meta.usage.per_model["gpt-5.4"];
        assert_eq!(pm.input_tokens, 360);
        assert_eq!(pm.output_tokens, 55);
        assert_eq!(pm.cache_read_input_tokens, 85);

        // Envelope comes from the row columns (SQL space-separated, UTC).
        assert_eq!(
            s.meta.started_at,
            time::parse_timestamp("2026-05-02 09:58:15.741021507", false)
        );
        assert_eq!(
            s.meta.ended_at,
            time::parse_timestamp("2026-05-02 10:00:16.848497543", false)
        );

        assert_eq!(s.blocks.len(), 3);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("Inspecting the code first."));
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.tool_name, "read");
        assert_eq!(execution.tool_use_id, "call_read_1");
        assert_eq!(execution.status, ToolStatus::Complete);
        assert_eq!(
            execution.result.as_ref().unwrap().output,
            "<file path=\"/tmp/example.go\">package main</file>"
        );
    }

    #[test]
    fn metrics_column_replaces_summed_usage() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        let ctx = r#"{
          "messages": [
            { "message": { "text": { "role": "User", "content": "Hello there.",
                "timestamp": "2026-05-02T10:00:00Z" } },
              "usage": { "prompt_tokens": {"actual": 50},
                         "completion_tokens": {"actual": 10},
                         "cached_tokens": {"actual": 20} } },
            { "message": { "text": { "role": "Assistant", "content": "Hi back.",
                "timestamp": "2026-05-02T10:00:01Z" } },
              "usage": { "prompt_tokens": {"actual": 80},
                         "completion_tokens": {"actual": 15},
                         "cached_tokens": {"actual": 30} } }
          ]
        }"#;
        // Metrics deliberately disagree with the sums to prove the override.
        insert(
            &db,
            "with-metrics",
            "",
            Some(ctx),
            "2026-05-02 10:00:00",
            Some("2026-05-02 10:00:01"),
            Some(r#"{"input_tokens":999,"output_tokens":1,"cached_input_tokens":2}"#),
        );
        insert(
            &db,
            "no-metrics",
            "",
            Some(ctx),
            "2026-05-02 10:00:00",
            Some("2026-05-02 10:00:01"),
            None,
        );

        // Plain db path → all conversations.
        let sessions = ForgeProvider.parse(&db).unwrap();
        assert_eq!(sessions.len(), 2);
        let by_id = |id: &str| sessions.iter().find(|s| s.meta.id == id).unwrap();

        let m = by_id("forge:with-metrics");
        assert!(m.meta.usage.has_usage);
        assert_eq!(m.meta.usage.totals.input_tokens, 999);
        assert_eq!(m.meta.usage.totals.output_tokens, 1);
        assert_eq!(m.meta.usage.totals.cache_read_input_tokens, 2);
        assert_eq!(m.meta.title, None);
        // No cwd anywhere → provider-name fallback.
        assert_eq!(m.meta.project, "forge");

        let f = by_id("forge:no-metrics");
        assert!(f.meta.usage.has_usage);
        assert_eq!(f.meta.usage.totals.input_tokens, 130);
        assert_eq!(f.meta.usage.totals.output_tokens, 25);
        assert_eq!(f.meta.usage.totals.cache_read_input_tokens, 50);
    }

    #[test]
    fn degenerate_items_and_raw_content_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        let ctx = r#"{
          "messages": [
            { "what": "unknown shape" },
            { "message": { "text": { "role": "User", "content": "",
                "raw_content": {"Text": "raw text fallback"},
                "timestamp": "2026-05-02T10:00:00Z" } } },
            { "message": { "text": { "role": "Assistant", "content": "",
                "timestamp": "2026-05-02T10:00:01Z" } } },
            { "message": { "tool": { "name": "bash", "call_id": "",
                "output": {"values": [{"text": "orphan"}]} } } }
          ]
        }"#;
        insert(
            &db,
            "degen",
            "Degenerate",
            Some(ctx),
            "2026-05-02 10:00:00",
            Some("2026-05-02 10:00:01"),
            None,
        );

        let sessions = ForgeProvider.parse(&virtual_path(&db, "degen")).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        // Only the raw_content-fallback user message survives: the empty
        // assistant is skipped, the empty-call_id tool item is dropped.
        assert_eq!(s.blocks.len(), 1);
        assert_eq!(s.meta.message_count, 1);
        assert_eq!(s.meta.first_message, "raw text fallback");
        // The unknown-shape item is counted, never hidden.
        assert_eq!(s.meta.malformed_lines, 1);
        // No usage anywhere and no metrics → truthful absence.
        assert!(!s.meta.usage.has_usage);
    }

    #[test]
    fn non_interactive_sessions_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        // System-only conversation (prompt scaffolding, nothing contentful).
        let system_only = r#"{
          "messages": [
            { "message": { "text": { "role": "System",
                "content": "<current_working_directory>/tmp/x</current_working_directory>",
                "timestamp": "2026-05-02T10:00:00Z" } } }
          ]
        }"#;
        insert(
            &db,
            "sys-only",
            "",
            Some(system_only),
            "2026-05-02 10:00:00",
            None,
            None,
        );
        insert(
            &db,
            "empty",
            "",
            Some(r#"{"messages": []}"#),
            "2026-05-02 10:00:00",
            None,
            None,
        );
        insert(
            &db,
            "garbage",
            "",
            Some("not json at all"),
            "2026-05-02 10:00:00",
            None,
            None,
        );

        // Parse-all tolerantly skips every non-interactive row.
        assert!(ForgeProvider.parse(&db).unwrap().is_empty());
        // Single-conversation parse agrees.
        assert!(ForgeProvider
            .parse(&virtual_path(&db, "sys-only"))
            .unwrap()
            .is_empty());
        assert!(ForgeProvider
            .parse(&virtual_path(&db, "garbage"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn discover_lists_conversations_as_virtual_paths() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        insert_standard(&db);
        insert(
            &db,
            "conv-002",
            "",
            Some(r#"{"messages": []}"#),
            "2026-05-01 08:00:00",
            None, // NULL updated_at → COALESCE(created_at)
            None,
        );
        // NULL context rows are invisible to discovery.
        insert(
            &db,
            "no-context",
            "",
            None,
            "2026-05-01 08:00:00",
            None,
            None,
        );

        let found = ForgeProvider.discover(dir.path());
        assert_eq!(found.len(), 2);
        let one = found.iter().find(|d| d.id == "forge:conv-001").unwrap();
        assert!(one.path.to_str().unwrap().ends_with("#conv-001"));
        assert_eq!(
            Some(one.mtime),
            time::parse_timestamp("2026-05-02 10:00:16.848497543", false)
        );
        let two = found.iter().find(|d| d.id == "forge:conv-002").unwrap();
        assert_eq!(
            Some(two.mtime),
            time::parse_timestamp("2026-05-01 08:00:00", false)
        );

        // A root without a .forge.db discovers nothing.
        let empty = tempfile::tempdir().unwrap();
        assert!(ForgeProvider.discover(empty.path()).is_empty());
    }

    #[test]
    fn tool_error_sets_error_status() {
        let dir = tempfile::tempdir().unwrap();
        let db = seed_db(dir.path());
        let ctx = r#"{
          "messages": [
            { "message": { "text": { "role": "User", "content": "run it",
                "timestamp": "2026-05-02T10:00:00Z" } } },
            { "message": { "text": { "role": "Assistant", "content": "",
                "tool_calls": [ {"name": "bash", "call_id": "c1",
                                 "arguments": {"command": "false"}} ],
                "timestamp": "2026-05-02T10:00:01Z" } } },
            { "message": { "tool": { "name": "bash", "call_id": "c1",
                "output": {"is_error": true, "values": [{"text": "boom"}]} } } }
          ]
        }"#;
        insert(&db, "err", "", Some(ctx), "2026-05-02 10:00:00", None, None);

        let sessions = ForgeProvider.parse(&virtual_path(&db, "err")).unwrap();
        let ConversationBlock::Assistant(a) = &sessions[0].blocks[1] else {
            panic!("expected assistant block")
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment")
        };
        assert_eq!(execution.status, ToolStatus::Error);
        assert_eq!(execution.result.as_ref().unwrap().output, "boom");
        assert!(execution.result.as_ref().unwrap().is_error);
    }

    #[test]
    fn tool_output_text_fallback_ladder() {
        // values[].text joined with NO separator (port of strings.Join(parts, "")).
        let values = serde_json::json!({"values": [{"text": "a"}, {"text": "b"}]});
        assert_eq!(tool_output_text(Some(&values)), "ab");
        // top-level text fallback.
        let top = serde_json::json!({"text": "top-level text output"});
        assert_eq!(tool_output_text(Some(&top)), "top-level text output");
        // raw JSON fallback when no text carriers exist.
        let raw = serde_json::json!({"is_error": false});
        assert_eq!(tool_output_text(Some(&raw)), "{\"is_error\":false}");
        // null / absent → empty (never fabricate output).
        assert_eq!(tool_output_text(Some(&Value::Null)), "");
        assert_eq!(tool_output_text(None), "");
    }
}
