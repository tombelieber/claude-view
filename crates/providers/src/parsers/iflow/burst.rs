// crates/providers/src/parsers/iflow/burst.rs
//
// Streaming-burst dedup. iFlow's uuid/parentUuid DAG encodes sliding-window
// snapshots of one in-flight assistant turn, not conversation forks: each
// snapshot repeats the currently-active tool calls while text appears only
// in the first. Merging keeps transcripts truthful without duplication.

use super::Entry;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Max gap between assistant snapshots of one streaming burst. iFlow emits
/// incremental updates within milliseconds; >1s signals a new turn.
const STREAMING_GAP_SECS: f64 = 1.0;

/// Merge redundant assistant streaming snapshots. Strictly-adjacent
/// assistant entries (consecutive valid-line positions, same parentUuid,
/// <1s apart) form a burst: earlier snapshots are dropped and their content
/// blocks fold into the last entry, deduping tool_use blocks by id
/// (first occurrence wins). User entries always survive.
pub(super) fn merge_streaming_bursts(mut entries: Vec<Entry>) -> Vec<Entry> {
    let mut groups: HashMap<String, Vec<Vec<usize>>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if !e.is_assistant || e.parent_uuid.is_empty() {
            continue;
        }
        let runs = groups.entry(e.parent_uuid.clone()).or_default();
        let extend = runs
            .last()
            .and_then(|run| run.last())
            .is_some_and(|&prev_idx| {
                let prev = &entries[prev_idx];
                prev.line_index + 1 == e.line_index
                    && match (prev.timestamp, e.timestamp) {
                        (Some(p), Some(t)) => t - p < STREAMING_GAP_SECS,
                        _ => false,
                    }
            });
        match runs.last_mut().filter(|_| extend) {
            Some(run) => run.push(i),
            None => runs.push(vec![i]),
        }
    }

    let mut dropped: HashSet<usize> = HashSet::new();
    let mut merged: HashMap<usize, Value> = HashMap::new();
    for runs in groups.values() {
        for run in runs.iter().filter(|r| r.len() > 1) {
            dropped.extend(run[..run.len() - 1].iter().copied());
            merged.insert(run[run.len() - 1], merged_content(&entries, run));
        }
    }
    for (idx, content) in merged {
        // Splice into the last snapshot only where message.content exists
        // (mirrors the Go splice guard).
        if let Some(slot) = entries[idx].value.pointer_mut("/message/content") {
            *slot = content;
        }
    }
    entries
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !dropped.contains(i))
        .map(|(_, e)| e)
        .collect()
}

/// Combined content array of a burst: every block in order, tool_use
/// deduped by id (first wins; id-less tool_use blocks are never deduped).
fn merged_content(entries: &[Entry], run: &[usize]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for &idx in run {
        let Some(items) = entries[idx]
            .value
            .pointer("/message/content")
            .and_then(Value::as_array)
        else {
            continue;
        };
        for block in items {
            if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                if let Some(id) = block.get("id").and_then(Value::as_str) {
                    if !id.is_empty() && !seen.insert(id) {
                        continue;
                    }
                }
            }
            out.push(block.clone());
        }
    }
    Value::Array(out)
}
