// crates/providers/src/parsers/hermes/msg.rs
//
// Intermediate Hermes message model shared by all three sources (state.db
// rows, JSONL transcripts, JSON envelopes), the quality score used to pick
// the richest stream, and the conversion into ConversationBlocks.

use crate::model::ForeignSessionMeta;
use crate::util::{blocks, preview};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;

const COMPACT_HYPHEN: &str = "[CONTEXT COMPACTION - REFERENCE ONLY]";
const COMPACT_EN_DASH: &str = "[CONTEXT COMPACTION \u{2013} REFERENCE ONLY]";

/// One Hermes tool invocation (OpenAI function-call shape).
pub(super) struct ToolCall {
    pub id: String,
    pub name: String,
    /// `function.arguments` decoded from its string-encoded JSON form
    /// (kept as a raw string Value when it fails to decode).
    pub arguments: Value,
}

/// A normalized Hermes message, source-agnostic.
pub(super) enum Msg {
    User {
        text: String,
        timestamp: Option<f64>,
    },
    /// `[CONTEXT COMPACTION - REFERENCE ONLY]` boundary (system noise, both
    /// hyphen and en-dash variants exist in real data).
    Compaction {
        summary: Option<String>,
        timestamp: Option<f64>,
    },
    Assistant {
        text: String,
        thinking: Option<String>,
        tool_calls: Vec<ToolCall>,
        timestamp: Option<f64>,
    },
    /// `role:"tool"` result row, linked back via tool_call_id.
    ToolResult {
        tool_call_id: String,
        output: String,
        timestamp: Option<f64>,
    },
}

/// Classify a trimmed, non-empty user content string into a User or
/// Compaction message (after skill-prefix unwrapping).
pub(super) fn user_msg(content: &str, timestamp: Option<f64>) -> Msg {
    let display = strip_skill_prefix(content);
    let trimmed = display.trim();
    for marker in [COMPACT_HYPHEN, COMPACT_EN_DASH] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            let summary = rest.trim();
            return Msg::Compaction {
                summary: (!summary.is_empty()).then(|| summary.to_string()),
                timestamp,
            };
        }
    }
    Msg::User {
        text: display,
        timestamp,
    }
}

/// Build an assistant message; `None` when there is nothing to show
/// (no text, no thinking, no tool calls) — mirrors the Go skip.
pub(super) fn assistant_msg(
    text: String,
    thinking: Option<String>,
    tool_calls: Vec<ToolCall>,
    timestamp: Option<f64>,
) -> Option<Msg> {
    if text.is_empty() && thinking.is_none() && tool_calls.is_empty() {
        return None;
    }
    Some(Msg::Assistant {
        text,
        thinking,
        tool_calls,
        timestamp,
    })
}

/// One OpenAI-style tool call. Name comes from `function.name` with the
/// state-db `name` fallback; entries without a name are dropped (Go parity).
pub(super) fn tool_call_from(tc: &Value) -> Option<ToolCall> {
    let name = tc
        .pointer("/function/name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            tc.get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })?;
    let id = tc
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(ToolCall {
        id,
        name: name.to_string(),
        arguments: decode_arguments(tc.pointer("/function/arguments")),
    })
}

/// `function.arguments` is a string-encoded JSON document.
fn decode_arguments(v: Option<&Value>) -> Value {
    match v {
        Some(Value::String(s)) if !s.is_empty() => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        Some(Value::String(_)) | None => Value::Null,
        Some(other) => other.clone(),
    }
}

/// Quality score for picking the richest message stream — ported from the
/// Go reconciliation: count*1000 + total content length + 100 per message
/// with tool calls + 50 per message with thinking. Applied symmetrically to
/// transcript and state.db streams (Go is slightly asymmetric: transcripts
/// embed thinking into the scored content).
pub(super) fn quality(msgs: &[Msg]) -> u64 {
    let mut score = msgs.len() as u64 * 1000;
    for m in msgs {
        match m {
            Msg::User { text, .. } => score += text.len() as u64,
            Msg::Compaction { summary, .. } => {
                score += summary.as_deref().map_or(0, str::len) as u64;
            }
            Msg::Assistant {
                text,
                thinking,
                tool_calls,
                ..
            } => {
                score += text.len() as u64;
                if !tool_calls.is_empty() {
                    score += 100;
                }
                if thinking.is_some() {
                    score += 50;
                }
            }
            Msg::ToolResult { output, .. } => score += output.len() as u64,
        }
    }
    score
}

/// Convert the chosen message stream into ConversationBlocks, updating the
/// session meta (counts, first_message, timestamp envelope) as we go.
pub(super) fn build_blocks(
    raw_id: &str,
    msgs: Vec<Msg>,
    meta: &mut ForeignSessionMeta,
) -> Vec<ConversationBlock> {
    let mut out: Vec<ConversationBlock> = Vec::with_capacity(msgs.len());
    for (ordinal, m) in msgs.into_iter().enumerate() {
        let id = blocks::block_id(raw_id, ordinal);
        match m {
            Msg::User { text, timestamp } => {
                observe(meta, timestamp);
                if meta.first_message.is_empty() {
                    meta.first_message = preview(&text, 200);
                }
                meta.message_count += 1;
                meta.user_message_count += 1;
                out.push(blocks::user(id, text, timestamp));
            }
            Msg::Compaction { summary, timestamp } => {
                observe(meta, timestamp);
                out.push(blocks::compaction_notice(id, summary));
            }
            Msg::Assistant {
                text,
                thinking,
                tool_calls,
                timestamp,
            } => {
                observe(meta, timestamp);
                let mut segments: Vec<AssistantSegment> = Vec::new();
                if !text.is_empty() {
                    segments.push(blocks::text_segment(text));
                }
                for tc in tool_calls {
                    segments.push(blocks::tool_segment(tc.name, tc.arguments, tc.id));
                }
                meta.message_count += 1;
                out.push(blocks::assistant(id, segments, thinking, timestamp));
            }
            Msg::ToolResult {
                tool_call_id,
                output,
                timestamp,
            } => {
                observe(meta, timestamp);
                // Hermes tool results are never error-flagged in the format.
                blocks::attach_tool_result(&mut out, &tool_call_id, output, false);
            }
        }
    }
    out
}

fn observe(meta: &mut ForeignSessionMeta, ts: Option<f64>) {
    if let Some(ts) = ts {
        meta.observe_timestamp(ts);
    }
}

/// Unwrap the skill-injection header Hermes prepends to user messages:
/// `[SYSTEM: The user has invoked the "<name>" skill…]` followed by the
/// skill body and optionally the real user instruction. Returns the
/// instruction when present (with any trailing `[Runtime note:…]` block
/// stripped), else a compact `[Skill: <name>]` placeholder.
pub(super) fn strip_skill_prefix(s: &str) -> String {
    const PREFIX: &str = "[SYSTEM: The user has invoked the \"";
    const MARKER: &str =
        "The user has provided the following instruction alongside the skill invocation: ";
    let Some(rest) = s.strip_prefix(PREFIX) else {
        return s.to_string();
    };
    let skill_name = rest
        .find('"')
        .filter(|&i| i > 0)
        .map(|i| &rest[..i])
        .unwrap_or("");
    if let Some((_, after)) = s.split_once(MARKER) {
        let after = match after.find("\n\n[Runtime note:") {
            Some(i) => &after[..i],
            None => after,
        };
        let after = after.trim();
        if !after.is_empty() {
            return after.to_string();
        }
    }
    if !skill_name.is_empty() {
        return format!("[Skill: {skill_name}]");
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_prefix_variants() {
        // No prefix passes through untouched.
        assert_eq!(strip_skill_prefix("Fix the bug"), "Fix the bug");
        // Instruction extracted.
        let with_instr = "[SYSTEM: The user has invoked the \"commit\" skill.]\n\n---\nname: commit\n---\nbody\n\nThe user has provided the following instruction alongside the skill invocation: Please commit my changes";
        assert_eq!(strip_skill_prefix(with_instr), "Please commit my changes");
        // Runtime note stripped.
        let with_note = "[SYSTEM: The user has invoked the \"debug\" skill.]\n\nThe user has provided the following instruction alongside the skill invocation: Fix it\n\n[Runtime note: internal]";
        assert_eq!(strip_skill_prefix(with_note), "Fix it");
        // No instruction → placeholder.
        let no_instr = "[SYSTEM: The user has invoked the \"review\" skill.]\n\n---\nname: review\n---\nbody\n\n";
        assert_eq!(strip_skill_prefix(no_instr), "[Skill: review]");
        // Blank instruction falls back to the placeholder too.
        let blank = "[SYSTEM: The user has invoked the \"test\" skill.]\n\nThe user has provided the following instruction alongside the skill invocation:   ";
        assert_eq!(strip_skill_prefix(blank), "[Skill: test]");
    }

    #[test]
    fn compaction_markers_both_dash_variants() {
        for marker in [COMPACT_HYPHEN, COMPACT_EN_DASH] {
            let m = user_msg(&format!("{marker}\nold context"), None);
            let Msg::Compaction { summary, .. } = m else {
                panic!("expected compaction for {marker}");
            };
            assert_eq!(summary.as_deref(), Some("old context"));
        }
        assert!(matches!(user_msg("real prompt", None), Msg::User { .. }));
    }
}
