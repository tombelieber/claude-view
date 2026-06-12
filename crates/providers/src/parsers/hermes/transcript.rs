// crates/providers/src/parsers/hermes/transcript.rs
//
// The two transcript file formats Hermes writes under
// `~/.hermes/sessions/`:
//   - `<yyyymmdd_hhmmss_hex>.jsonl` — first line `{role:"session_meta",
//     model, platform}`, then OpenAI-style user/assistant/tool lines;
//   - `session_<id>.json` — one envelope `{platform, session_start,
//     last_updated, messages:[…]}` with the same message shape.
// Naive ISO timestamps ("2026-04-03T15:27:21.014566", no TZ) are LOCAL
// wall-clock time → parse_timestamp(s, assume_local=true).

use super::msg::{self, Msg};
use crate::util::{jsonl, time};
use serde_json::Value;
use std::path::Path;

/// One parsed transcript: envelope metadata + the normalized messages.
#[derive(Default)]
pub(super) struct TranscriptDoc {
    pub platform: String,
    /// Model id from the JSONL `session_meta` header (display only — usage
    /// tokens exist solely in state.db).
    pub model: String,
    pub msgs: Vec<Msg>,
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    pub malformed_lines: u32,
}

impl TranscriptDoc {
    /// Widen the [started_at, ended_at] envelope (Go observes every line's
    /// timestamp, including session_meta and skipped messages).
    fn observe(&mut self, ts: Option<f64>) {
        let Some(ts) = ts else { return };
        if ts <= 0.0 {
            return;
        }
        match self.started_at {
            Some(s) if s <= ts => {}
            _ => self.started_at = Some(ts),
        }
        match self.ended_at {
            Some(e) if e >= ts => {}
            _ => self.ended_at = Some(ts),
        }
    }
}

/// Parse a `<id>.jsonl` transcript. Malformed lines are skipped but counted.
pub(super) fn parse_jsonl(path: &Path) -> anyhow::Result<TranscriptDoc> {
    let read = jsonl::read_values(path)?;
    let mut doc = TranscriptDoc {
        malformed_lines: read.malformed,
        ..TranscriptDoc::default()
    };
    for value in &read.values {
        let ts = timestamp_of(value);
        doc.observe(ts);
        match value.get("role").and_then(Value::as_str).unwrap_or("") {
            "session_meta" => {
                if let Some(p) = value.get("platform").and_then(Value::as_str) {
                    doc.platform = p.to_string();
                }
                if let Some(m) = value.get("model").and_then(Value::as_str) {
                    doc.model = m.to_string();
                }
            }
            role => push_message(&mut doc.msgs, role, value, ts),
        }
    }
    Ok(doc)
}

/// Parse a `session_<id>.json` envelope.
pub(super) fn parse_json(path: &Path) -> anyhow::Result<TranscriptDoc> {
    let raw = std::fs::read_to_string(path)?;
    let root: Value = serde_json::from_str(&raw)?;
    let Some(obj) = root.as_object() else {
        anyhow::bail!("hermes: {} is not a JSON object", path.display());
    };
    let mut doc = TranscriptDoc {
        platform: obj
            .get("platform")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ..TranscriptDoc::default()
    };
    for key in ["session_start", "last_updated"] {
        let ts = obj
            .get(key)
            .and_then(Value::as_str)
            .and_then(|s| time::parse_timestamp(s, true));
        doc.observe(ts);
    }
    for m in obj
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let ts = timestamp_of(m);
        // Per-message timestamps can extend the bounds beyond the envelope's
        // (possibly missing or stale) session_start / last_updated.
        doc.observe(ts);
        let role = m.get("role").and_then(Value::as_str).unwrap_or("");
        push_message(&mut doc.msgs, role, m, ts);
    }
    Ok(doc)
}

fn timestamp_of(v: &Value) -> Option<f64> {
    v.get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| time::parse_timestamp(s, true))
}

/// Shared per-message normalization for both transcript formats.
fn push_message(msgs: &mut Vec<Msg>, role: &str, v: &Value, ts: Option<f64>) {
    match role {
        "user" => {
            let content = v
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !content.is_empty() {
                msgs.push(msg::user_msg(content, ts));
            }
        }
        "assistant" => {
            let text = v
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            // `reasoning_details` is the JSON-envelope fallback (Go parity).
            let thinking = ["reasoning", "reasoning_details"]
                .iter()
                .find_map(|k| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()))
                .map(str::to_string);
            let tool_calls = v
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(msg::tool_call_from).collect())
                .unwrap_or_default();
            if let Some(m) = msg::assistant_msg(text, thinking, tool_calls, ts) {
                msgs.push(m);
            }
        }
        "tool" => {
            let id = v.get("tool_call_id").and_then(Value::as_str).unwrap_or("");
            if !id.is_empty() {
                msgs.push(Msg::ToolResult {
                    tool_call_id: id.to_string(),
                    output: v
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    timestamp: ts,
                });
            }
        }
        _ => {}
    }
}
