//! Per-agent JSONL parsing (`subagents/workflows/<run>/agent-<id>.jsonl`) and the
//! agent-id <-> file mapping. Event/meta previews are built with the redacting
//! `preview_*` helpers, so transcript content is scrubbed before it leaves here.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde_json::Value;

use super::fsjson::{json_string, read_text_capped};
use super::preview::{preview_message_content, preview_value, safe_preview, truncate};
use super::types::{WorkflowAgentEvent, WorkflowAgentSummary};
use super::{MAX_AGENT_EVENTS, MAX_AGENT_EVENT_CHARS};

const MAX_TOOL_INPUT_PREVIEW_CHARS: usize = 1200;
const MAX_TOOL_NAME_PREVIEW_CHARS: usize = 80;
const MAX_TOOL_VALUE_PREVIEW_CHARS: usize = 180;

/// Discover bare agent stubs from `agent-*.jsonl` files (no summary metadata).
pub(crate) fn scan_agent_files(run_dir: &Path) -> Vec<WorkflowAgentSummary> {
    let mut agents = Vec::new();
    let entries = match fs::read_dir(run_dir) {
        Ok(entries) => entries,
        Err(_) => return agents,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(agent_id) = canonical_agent_id_from_path(&path) else {
            continue;
        };
        agents.push(WorkflowAgentSummary {
            agent_id,
            label: None,
            phase_index: None,
            phase_title: None,
            model: None,
            state: "unknown".to_string(),
            started_at: None,
            queued_at: None,
            last_progress_at: None,
            tokens: 0,
            tool_calls: 0,
            duration_ms: None,
            prompt_preview: None,
            result_preview: None,
            last_tool_name: None,
            last_tool_summary: None,
            events_available: true,
        });
    }
    agents.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    agents
}

pub(crate) fn read_agent_events(path: &Path) -> Vec<WorkflowAgentEvent> {
    let Some(raw) = read_text_capped(path) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    let mut tool_name_by_id = HashMap::new();
    for line in raw.lines().take(MAX_AGENT_EVENTS) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            tracing::warn!(path = %path.display(), "Skipping malformed workflow agent JSONL row");
            continue;
        };
        record_tool_use_names(&value, &mut tool_name_by_id);
        for event in agent_events_from_value(&value, &tool_name_by_id) {
            events.push(event);
            if events.len() >= MAX_AGENT_EVENTS {
                return events;
            }
        }
    }
    events
}

fn agent_events_from_value(
    value: &Value,
    tool_name_by_id: &HashMap<String, String>,
) -> Vec<WorkflowAgentEvent> {
    let role = value
        .get("message")
        .and_then(|message| json_string(message, "role"))
        .or_else(|| json_string(value, "role"));
    let timestamp = timestamp_ms(value);
    let fallback_kind = json_string(value, "type").unwrap_or_else(|| "message".to_string());

    if let Some(content) = message_content(value) {
        let events = content_events(content, role.clone(), timestamp, tool_name_by_id);
        if !events.is_empty() {
            return events;
        }
    }

    let preview = value
        .get("result")
        .map(|v| preview_value(v, MAX_AGENT_EVENT_CHARS))
        .unwrap_or_else(|| preview_value(value, MAX_AGENT_EVENT_CHARS));
    vec![WorkflowAgentEvent {
        kind: fallback_kind,
        role,
        preview,
        timestamp,
        tool_use_id: None,
        tool_names: Vec::new(),
        tool_input_preview: None,
        tool_result_preview: None,
    }]
}

fn message_content(value: &Value) -> Option<&Value> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| value.get("content"))
}

fn content_events(
    content: &Value,
    role: Option<String>,
    timestamp: Option<i64>,
    tool_name_by_id: &HashMap<String, String>,
) -> Vec<WorkflowAgentEvent> {
    match content {
        Value::String(text) => vec![simple_event(
            "message",
            role,
            safe_preview(text, MAX_AGENT_EVENT_CHARS),
            timestamp,
        )],
        Value::Array(items) => content_block_events(items, role, timestamp, tool_name_by_id),
        _ => vec![simple_event(
            "message",
            role,
            preview_value(content, MAX_AGENT_EVENT_CHARS),
            timestamp,
        )],
    }
}

fn simple_event(
    kind: &str,
    role: Option<String>,
    preview: String,
    timestamp: Option<i64>,
) -> WorkflowAgentEvent {
    WorkflowAgentEvent {
        kind: kind.to_string(),
        role,
        preview,
        timestamp,
        tool_use_id: None,
        tool_names: Vec::new(),
        tool_input_preview: None,
        tool_result_preview: None,
    }
}

fn content_block_events(
    items: &[Value],
    role: Option<String>,
    timestamp: Option<i64>,
    tool_name_by_id: &HashMap<String, String>,
) -> Vec<WorkflowAgentEvent> {
    let mut events = Vec::new();

    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("tool_use") => {
                let tool_use_id = json_string(item, "id");
                let tool_names = item
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| vec![tool_name_preview(name)])
                    .unwrap_or_default();
                let summary = tool_use_summary(item);
                events.push(WorkflowAgentEvent {
                    kind: "tool_use".to_string(),
                    role: role.clone(),
                    preview: summary.clone(),
                    timestamp,
                    tool_use_id,
                    tool_names,
                    tool_input_preview: (!summary.is_empty()).then_some(summary),
                    tool_result_preview: None,
                });
            }
            Some("tool_result") => {
                let tool_use_id = json_string(item, "tool_use_id");
                let mut tool_names = Vec::new();
                if let Some(tool_use_id) = item.get("tool_use_id").and_then(Value::as_str) {
                    if let Some(name) = tool_name_by_id.get(tool_use_id) {
                        push_unique(&mut tool_names, name);
                    }
                }
                if let Some(preview) = tool_result_preview(item) {
                    events.push(WorkflowAgentEvent {
                        kind: "tool_result".to_string(),
                        role: role.clone(),
                        preview: preview.clone(),
                        timestamp,
                        tool_use_id,
                        tool_names,
                        tool_input_preview: None,
                        tool_result_preview: Some(preview),
                    });
                }
            }
            Some("thinking") | Some("redacted_thinking") => {
                if let Some(preview) = thinking_preview(item) {
                    events.push(simple_event("thinking", role.clone(), preview, timestamp));
                }
            }
            Some("text") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    events.push(simple_event(
                        "message",
                        role.clone(),
                        safe_preview(text, MAX_AGENT_EVENT_CHARS),
                        timestamp,
                    ));
                }
            }
            _ => {
                let preview = item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|text| safe_preview(text, MAX_AGENT_EVENT_CHARS))
                    .unwrap_or_else(|| preview_value(item, MAX_AGENT_EVENT_CHARS));
                events.push(simple_event("message", role.clone(), preview, timestamp));
            }
        }
    }
    events
}

fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn record_tool_use_names(value: &Value, tool_name_by_id: &mut HashMap<String, String>) {
    let Some(Value::Array(items)) = message_content(value) else {
        return;
    };
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        tool_name_by_id.insert(id.to_string(), tool_name_preview(name));
    }
}

fn tool_name_preview(name: &str) -> String {
    truncate(
        &safe_preview(name, MAX_TOOL_NAME_PREVIEW_CHARS),
        MAX_TOOL_NAME_PREVIEW_CHARS.saturating_sub(3),
    )
}

fn tool_use_summary(item: &Value) -> String {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .map(tool_name_preview)
        .unwrap_or_else(|| "Tool".to_string());
    let input = item
        .get("input")
        .and_then(summarize_tool_input)
        .unwrap_or_default();
    if input.is_empty() {
        name
    } else {
        format!("{name}: {input}")
    }
}

fn summarize_tool_input(input: &Value) -> Option<String> {
    match input {
        Value::Object(map) => {
            if map.is_empty() {
                return None;
            }
            if let Some(command) = map.get("command").and_then(Value::as_str) {
                return Some(safe_preview(command, MAX_TOOL_INPUT_PREVIEW_CHARS));
            }
            let mut parts = Vec::new();
            for (idx, (key, value)) in map.iter().enumerate() {
                if idx >= 8 {
                    parts.push("...".to_string());
                    break;
                }
                if key == "content" {
                    continue;
                }
                parts.push(format!("{key}={}", summarize_tool_value(key, value)));
            }
            let joined = parts.join(", ");
            (!joined.is_empty()).then(|| safe_preview(&joined, MAX_TOOL_INPUT_PREVIEW_CHARS))
        }
        _ => Some(preview_value(input, MAX_TOOL_INPUT_PREVIEW_CHARS)),
    }
}

fn summarize_tool_value(key: &str, value: &Value) -> String {
    if key.eq_ignore_ascii_case("content") {
        return format!("<{} chars>", value_char_len(value));
    }
    match value {
        Value::String(text) => safe_preview(text, MAX_TOOL_VALUE_PREVIEW_CHARS),
        Value::Number(_) | Value::Bool(_) | Value::Null => value.to_string(),
        _ => preview_value(value, MAX_TOOL_VALUE_PREVIEW_CHARS),
    }
}

fn tool_result_preview(item: &Value) -> Option<String> {
    item.get("content")
        .map(|content| preview_message_content(content, MAX_AGENT_EVENT_CHARS))
}

fn value_char_len(value: &Value) -> usize {
    match value {
        Value::String(text) => text.chars().count(),
        _ => serde_json::to_string(value)
            .map(|serialized| serialized.chars().count())
            .unwrap_or(0),
    }
}

fn thinking_preview(item: &Value) -> Option<String> {
    item.get("thinking")
        .or_else(|| item.get("text"))
        .or_else(|| item.get("content"))
        .map(|content| preview_message_content(content, MAX_AGENT_EVENT_CHARS))
}

fn timestamp_ms(value: &Value) -> Option<i64> {
    value
        .get("timestamp")
        .and_then(|timestamp| {
            timestamp
                .as_i64()
                .or_else(|| timestamp.as_u64().and_then(|n| i64::try_from(n).ok()))
                .or_else(|| timestamp.as_f64().map(|n| n as i64))
                .map(normalize_timestamp_ms)
        })
        .or_else(|| {
            json_string(value, "timestamp").and_then(|timestamp| {
                DateTime::parse_from_rfc3339(&timestamp)
                    .ok()
                    .map(|parsed| parsed.timestamp_millis())
            })
        })
}

fn normalize_timestamp_ms(timestamp: i64) -> i64 {
    if (-100_000_000_000..100_000_000_000).contains(&timestamp) {
        timestamp.saturating_mul(1000)
    } else {
        timestamp
    }
}

pub(crate) fn read_agent_meta_preview(run_dir: &Path, agent_id: &str) -> Option<String> {
    let stripped = agent_id.strip_prefix("agent-").unwrap_or(agent_id);
    for candidate in [
        run_dir.join(format!("agent-{stripped}.meta.json")),
        run_dir.join(format!("{agent_id}.meta.json")),
    ] {
        if candidate.is_file() {
            return read_text_capped(&candidate).map(|raw| {
                serde_json::from_str::<Value>(&raw)
                    .map(|value| preview_value(&value, MAX_AGENT_EVENT_CHARS))
                    .unwrap_or_else(|_| preview_value(&Value::String(raw), MAX_AGENT_EVENT_CHARS))
            });
        }
    }
    None
}

pub(crate) fn find_agent_jsonl(run_dir: &Path, agent_id: &str) -> Option<PathBuf> {
    let stripped = agent_id.strip_prefix("agent-").unwrap_or(agent_id);
    [
        run_dir.join(format!("agent-{stripped}.jsonl")),
        run_dir.join(format!("{agent_id}.jsonl")),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

pub(crate) fn canonical_agent_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    stem.strip_prefix("agent-").map(str::to_string)
}

pub(crate) fn first_event_preview(events: &[WorkflowAgentEvent], role: &str) -> Option<String> {
    events
        .iter()
        .find(|event| event.role.as_deref() == Some(role))
        .map(|event| event.preview.clone())
}

pub(crate) fn last_assistant_preview(events: &[WorkflowAgentEvent]) -> Option<String> {
    events
        .iter()
        .rev()
        .find(|event| event.role.as_deref() == Some("assistant"))
        .map(|event| event.preview.clone())
}
