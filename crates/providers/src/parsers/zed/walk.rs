// crates/providers/src/parsers/zed/walk.rs
//
// Recursive extraction over Zed's Rust-enum-style content blocks
// (ported from agentsview's zedWalk and its extractors).

use serde_json::{Map, Value};

/// `obj.content` when present and non-null, else the value itself
/// (ported from zedMessageContent).
pub(super) fn message_content(v: &Value) -> &Value {
    match v.as_object().and_then(|o| o.get("content")) {
        Some(content) if !content.is_null() => content,
        _ => v,
    }
}

/// Whether v contains any non-empty content structure — distinguishes
/// "no content" (drop) from "content without Text blocks" (keep).
pub(super) fn has_content(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
        _ => true,
    }
}

/// Recursive walk over arbitrary nesting (ported from zedWalk): visit every
/// JSON object, depth-first, including arrays.
fn zed_walk<'a, F: FnMut(&'a Map<String, Value>)>(v: &'a Value, visit: &mut F) {
    match v {
        Value::Object(obj) => {
            visit(obj);
            for child in obj.values() {
                zed_walk(child, visit);
            }
        }
        Value::Array(items) => {
            for child in items {
                zed_walk(child, visit);
            }
        }
        _ => {}
    }
}

/// All 'Text' leaves — plain string or `{text}` object — joined by newline.
pub(super) fn extract_text(v: &Value) -> String {
    let mut parts: Vec<&str> = Vec::new();
    zed_walk(v, &mut |obj| match obj.get("Text") {
        Some(Value::String(s)) => parts.push(s),
        Some(Value::Object(inner)) => {
            if let Some(Value::String(s)) = inner.get("text") {
                parts.push(s);
            }
        }
        _ => {}
    });
    parts.join("\n")
}

/// All 'Thinking':{text} leaves joined by newline.
pub(super) fn extract_thinking(v: &Value) -> String {
    let mut parts: Vec<&str> = Vec::new();
    zed_walk(v, &mut |obj| {
        if let Some(Value::String(s)) = obj
            .get("Thinking")
            .and_then(Value::as_object)
            .and_then(|t| t.get("text"))
        {
            parts.push(s);
        }
    });
    parts.join("\n")
}

/// All 'ToolUse' blocks as (id, name, input). `input` falls back to
/// `raw_input` when null/absent (streaming-interrupted calls).
pub(super) fn extract_tool_calls(v: &Value) -> Vec<(String, String, Value)> {
    let mut calls = Vec::new();
    zed_walk(v, &mut |obj| {
        let Some(tool) = obj.get("ToolUse").and_then(Value::as_object) else {
            return;
        };
        let input = tool
            .get("input")
            .filter(|i| !i.is_null())
            .or_else(|| tool.get("raw_input"))
            .cloned()
            .unwrap_or(Value::Null);
        let id = tool.get("id").and_then(Value::as_str).unwrap_or_default();
        let name = tool.get("name").and_then(Value::as_str).unwrap_or_default();
        calls.push((id.to_string(), name.to_string(), input));
    });
    calls
}

/// The `tool_results` map as (tool_use_id, text) pairs, sorted by key for
/// determinism (matches the Go port).
pub(super) fn extract_tool_results(agent: &Value) -> Vec<(String, String)> {
    let Some(results) = agent
        .as_object()
        .and_then(|o| o.get("tool_results"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = results.keys().collect();
    keys.sort();
    keys.into_iter()
        .map(|key| (key.clone(), tool_result_text(&results[key])))
        .collect()
}

/// Plain text from a tool result entry: an `output` string, or a `content`
/// array of Text blocks.
fn tool_result_text(v: &Value) -> String {
    let Some(entry) = v.as_object() else {
        return String::new();
    };
    if let Some(output) = entry.get("output").and_then(Value::as_str) {
        if !output.is_empty() {
            return output.to_string();
        }
    }
    if let Some(content) = entry.get("content") {
        let text = extract_text(content);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}
