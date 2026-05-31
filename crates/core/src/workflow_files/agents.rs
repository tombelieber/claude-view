//! Per-agent JSONL parsing (`subagents/workflows/<run>/agent-<id>.jsonl`) and the
//! agent-id <-> file mapping. Event/meta previews are built with the redacting
//! `preview_*` helpers, so transcript content is scrubbed before it leaves here.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::fsjson::{json_i64, json_string, read_text_capped};
use super::preview::{preview_message_content, preview_value};
use super::types::{WorkflowAgentEvent, WorkflowAgentSummary};
use super::{MAX_AGENT_EVENTS, MAX_AGENT_EVENT_CHARS};

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
    for line in raw.lines().take(MAX_AGENT_EVENTS) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            tracing::warn!(path = %path.display(), "Skipping malformed workflow agent JSONL row");
            continue;
        };
        if let Some(event) = agent_event_from_value(&value) {
            events.push(event);
        }
    }
    events
}

fn agent_event_from_value(value: &Value) -> Option<WorkflowAgentEvent> {
    let role = value
        .get("message")
        .and_then(|message| json_string(message, "role"))
        .or_else(|| json_string(value, "role"));
    let kind = infer_agent_event_kind(value);
    let preview = value
        .get("message")
        .and_then(|message| message.get("content"))
        .map(|content| preview_message_content(content, MAX_AGENT_EVENT_CHARS))
        .or_else(|| {
            value
                .get("content")
                .map(|v| preview_message_content(v, MAX_AGENT_EVENT_CHARS))
        })
        .or_else(|| {
            value
                .get("result")
                .map(|v| preview_value(v, MAX_AGENT_EVENT_CHARS))
        })
        .unwrap_or_else(|| preview_value(value, MAX_AGENT_EVENT_CHARS));
    Some(WorkflowAgentEvent {
        kind,
        role,
        preview,
        timestamp: json_i64(value, "timestamp"),
    })
}

fn infer_agent_event_kind(value: &Value) -> String {
    if let Some(content) = value.get("message").and_then(|m| m.get("content")) {
        if contains_content_type(content, "tool_use") {
            return "tool_use".to_string();
        }
        if contains_content_type(content, "tool_result") {
            return "tool_result".to_string();
        }
    }
    json_string(value, "type").unwrap_or_else(|| "message".to_string())
}

fn contains_content_type(content: &Value, expected: &str) -> bool {
    content
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some(expected))
        })
        .unwrap_or(false)
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
