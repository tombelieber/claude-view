// crates/providers/src/parsers/hermes/state.rs
//
// Hermes `state.db` (SQLite, sibling of the sessions dir) — the
// authoritative session registry: `sessions` carries metadata + the only
// token accounting in the format; `messages` is the fallback message
// stream when no transcript file beats it on quality.
//
// Deliberately NOT read (per the port decisions): reasoning_tokens (no
// Anthropic-shape bucket), cost columns (pricing deferred to the shared
// pricing table), parent_session_id (no meta field), and the
// reasoning_content/reasoning_details/codex_* fallback columns (the spec'd
// schema carries `reasoning` only).

use super::msg::{self, Msg};
use crate::model::UsageTotals;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::path::Path;

/// One `sessions` row, reduced to what the normalized model can carry.
pub(super) struct StateSession {
    pub source: String,
    pub model: String,
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    pub title: String,
    /// cache_write_tokens maps to cache_creation (Anthropic shape). Hermes
    /// input_tokens does NOT include cache reads (they are additive), so no
    /// subtraction applies.
    pub usage: UsageTotals,
}

pub(super) fn open_ro(db: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // Hermes holds the DB live — without a busy timeout a transient lock
    // silently drops every Hermes session from the catalog refresh.
    conn.busy_timeout(std::time::Duration::from_millis(3000))?;
    Ok(conn)
}

/// Enumerate session ids for discovery (cheap; no message parsing).
pub(super) fn session_ids(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM sessions ORDER BY started_at ASC, id ASC")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

/// Load one `sessions` row; `None` when the id vanished since discovery.
pub(super) fn session_row(conn: &Connection, id: &str) -> rusqlite::Result<Option<StateSession>> {
    conn.query_row(
        "SELECT COALESCE(source, ''), COALESCE(model, ''), COALESCE(started_at, 0), \
         COALESCE(ended_at, 0), COALESCE(input_tokens, 0), COALESCE(output_tokens, 0), \
         COALESCE(cache_read_tokens, 0), COALESCE(cache_write_tokens, 0), COALESCE(title, '') \
         FROM sessions WHERE id = ?1",
        [id],
        |r| {
            Ok(StateSession {
                source: r.get(0)?,
                model: r.get(1)?,
                started_at: positive(r.get(2)?),
                ended_at: positive(r.get(3)?),
                usage: UsageTotals {
                    input_tokens: clamp(r.get(4)?),
                    output_tokens: clamp(r.get(5)?),
                    cache_read_input_tokens: clamp(r.get(6)?),
                    cache_creation_input_tokens: clamp(r.get(7)?),
                },
                title: r.get(8)?,
            })
        },
    )
    .optional()
}

/// The fallback message stream from the `messages` table, normalized with
/// the same rules as transcripts (skill-prefix strip, compaction boundary,
/// empty-message skips).
pub(super) fn session_messages(conn: &Connection, id: &str) -> rusqlite::Result<Vec<Msg>> {
    let mut stmt = conn.prepare(
        "SELECT role, COALESCE(content, ''), COALESCE(tool_call_id, ''), \
         COALESCE(tool_calls, ''), COALESCE(timestamp, 0), COALESCE(reasoning, '') \
         FROM messages WHERE session_id = ?1 ORDER BY timestamp ASC, id ASC",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok(StateRow {
            role: r.get(0)?,
            content: r.get(1)?,
            tool_call_id: r.get(2)?,
            tool_calls: r.get(3)?,
            timestamp: r.get(4)?,
            reasoning: r.get(5)?,
        })
    })?;
    let mut msgs = Vec::new();
    for row in rows {
        if let Some(m) = row?.into_msg() {
            msgs.push(m);
        }
    }
    Ok(msgs)
}

struct StateRow {
    role: String,
    content: String,
    tool_call_id: String,
    tool_calls: String,
    timestamp: f64,
    reasoning: String,
}

impl StateRow {
    fn into_msg(self) -> Option<Msg> {
        let ts = positive(self.timestamp);
        match self.role.as_str() {
            "user" => {
                let content = self.content.trim();
                if content.is_empty() {
                    return None;
                }
                Some(msg::user_msg(content, ts))
            }
            "assistant" => {
                let text = self.content.trim().to_string();
                let thinking = Some(self.reasoning).filter(|s| !s.is_empty());
                // tool_calls is a JSON-array column ("" when absent).
                let tool_calls = match serde_json::from_str::<Value>(&self.tool_calls) {
                    Ok(Value::Array(items)) => {
                        items.iter().filter_map(msg::tool_call_from).collect()
                    }
                    _ => Vec::new(),
                };
                msg::assistant_msg(text, thinking, tool_calls, ts)
            }
            "tool" => {
                if self.tool_call_id.is_empty() {
                    return None;
                }
                Some(Msg::ToolResult {
                    tool_call_id: self.tool_call_id,
                    output: self.content,
                    timestamp: ts,
                })
            }
            _ => None,
        }
    }
}

fn positive(v: f64) -> Option<f64> {
    (v > 0.0).then_some(v)
}

fn clamp(v: i64) -> u64 {
    v.max(0) as u64
}
