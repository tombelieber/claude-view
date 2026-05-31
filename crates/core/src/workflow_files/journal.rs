//! Workflow journal parsing (`subagents/workflows/<run>/journal.jsonl`). Each row
//! is a runtime event; previews are redacted via `preview_value`.

use std::path::Path;

use serde_json::Value;

use super::fsjson::{json_i64, json_string, read_text_capped};
use super::preview::preview_value;
use super::types::WorkflowJournalEvent;
use super::{MAX_AGENT_EVENT_CHARS, MAX_JOURNAL_EVENTS};

pub(crate) fn read_journal_events(path: &Path) -> Vec<WorkflowJournalEvent> {
    let Some(raw) = read_text_capped(path) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for line in raw.lines().take(MAX_JOURNAL_EVENTS) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            tracing::warn!(path = %path.display(), "Skipping malformed workflow journal JSONL row");
            continue;
        };
        events.push(WorkflowJournalEvent {
            kind: json_string(&value, "type").unwrap_or_else(|| "event".to_string()),
            agent_id: json_string(&value, "agentId"),
            preview: value
                .get("result")
                .or_else(|| value.get("message"))
                .or_else(|| value.get("payload"))
                .map(|v| preview_value(v, MAX_AGENT_EVENT_CHARS)),
            timestamp: json_i64(&value, "timestamp"),
        });
    }
    events
}
