// ── Per-line invariant checks ──

use crate::live_parser::{LineType, LiveLine};

use super::types::{CheckAccum, PipelineSignals};

/// Check 7: Every assistant line with `usage` in raw JSON -> parsed tokens are non-zero.
pub fn check_token_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_raw_usage = raw
        .get("message")
        .and_then(|m| m.get("usage"))
        .and_then(|u| u.as_object())
        .is_some_and(|u| {
            u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) > 0
                || u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) > 0
        });

    if !has_raw_usage {
        return;
    }

    let parsed_has_tokens =
        parsed.input_tokens.unwrap_or(0) > 0 || parsed.output_tokens.unwrap_or(0) > 0;

    if parsed_has_tokens {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has usage (input/output > 0) but parsed tokens are all None/0",
        );
    }
}

/// Check 8: Every assistant line with `model` in raw JSON -> parsed model is Some.
pub fn check_model_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_model = raw
        .get("message")
        .and_then(|m| m.get("model"))
        .and_then(|v| v.as_str());
    if raw_model.is_none() {
        return;
    }
    if parsed.model.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!("raw has model={:?} but parsed model is None", raw_model),
        );
    }
}

/// Check 9: Every assistant line with tool_use blocks -> parsed tool_names non-empty.
pub fn check_tool_name_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_tool_use = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .is_some_and(|blocks| {
            blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        });
    if !has_tool_use {
        return;
    }
    if !parsed.tool_names.is_empty() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has tool_use blocks but parsed tool_names is empty",
        );
    }
}

/// Check 10: Every Read/Edit/Write tool_use with file_path -> parser tool_names includes it.
pub fn check_file_path_tool_presence(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let file_tools = ["Read", "Edit", "Write"];
    let has_file_path_tool = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .is_some_and(|blocks| {
            blocks.iter().any(|b| {
                let is_tool_use = b.get("type").and_then(|t| t.as_str()) == Some("tool_use");
                if !is_tool_use {
                    return false;
                }
                let tool_name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if !file_tools.contains(&tool_name) {
                    return false;
                }
                b.get("input")
                    .and_then(|i| i.get("file_path"))
                    .and_then(|fp| fp.as_str())
                    .is_some_and(|path| !path.is_empty())
            })
        });
    if !has_file_path_tool {
        return;
    }
    let has_file_tool_in_parsed = parsed
        .tool_names
        .iter()
        .any(|t| file_tools.contains(&t.as_str()));
    if has_file_tool_in_parsed {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has file-path tool (Read/Edit/Write with file_path) but parsed tool_names missing it",
        );
    }
}

/// Check 11: Every user/assistant line with text content -> content_preview non-empty.
///
/// Correctly handles system-injected noise tags (`<command-message>`, `<task-notification>`,
/// etc.) which `strip_noise_tags()` intentionally removes. If the raw text is entirely
/// noise tags, empty `content_preview` is correct parser behavior.
pub fn check_content_preview(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    use crate::live_parser::strip_noise_tags;

    let raw_type = raw.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if raw_type != "user" && raw_type != "assistant" {
        return;
    }

    // Collect raw text content, checking if ANY meaningful text survives noise-tag stripping.
    let has_meaningful_text = match raw.get("message").and_then(|m| m.get("content")) {
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                false
            } else {
                let (stripped, _) = strip_noise_tags(s);
                !stripped.is_empty()
            }
        }
        Some(serde_json::Value::Array(blocks)) => blocks.iter().any(|b| {
            b.get("type").and_then(|t| t.as_str()) == Some("text")
                && b.get("text").and_then(|t| t.as_str()).is_some_and(|text| {
                    if text.is_empty() {
                        return false;
                    }
                    let (stripped, _) = strip_noise_tags(text);
                    !stripped.is_empty()
                })
        }),
        _ => false,
    };
    if !has_meaningful_text {
        return;
    }
    if !parsed.content_preview.is_empty() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            "raw has text content (after noise-tag stripping) but parsed content_preview is empty",
        );
    }
}

/// Check 12: Every line with timestamp string -> parsed timestamp is Some.
pub fn check_timestamp_extraction(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_ts = raw.get("timestamp").and_then(|v| v.as_str());
    if raw_ts.is_none() || raw_ts.is_some_and(|s| s.is_empty()) {
        return;
    }
    if parsed.timestamp.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "raw has timestamp={:?} but parsed timestamp is None",
                raw_ts
            ),
        );
    }
}

/// Check 13: Cache creation 5m + 1hr == total when both splits are present.
///
/// Handles early API data quirk where `cache_creation` split object exists with
/// both values at 0 while `cache_creation_input_tokens` total is non-zero.
/// In this case the split data is unreliable — skip the check for that line.
pub fn check_cache_token_split(
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let total = match parsed.cache_creation_tokens {
        Some(t) if t > 0 => t,
        _ => return,
    };
    let t5m = parsed.cache_creation_5m_tokens;
    let t1hr = parsed.cache_creation_1hr_tokens;
    if t5m.is_none() && t1hr.is_none() {
        return; // No split data (older API)
    }
    let sum = t5m.unwrap_or(0) + t1hr.unwrap_or(0);
    if sum == 0 && total > 0 {
        return; // Early API data: split object exists but values not populated
    }
    if sum == total {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "cache split mismatch: 5m({}) + 1hr({}) = {} != total({})",
                t5m.unwrap_or(0),
                t1hr.unwrap_or(0),
                sum,
                total
            ),
        );
    }
}

/// Check 16: Every line with tokens > 0 should have a model.
pub fn check_cost_requires_model(
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let has_tokens = parsed.input_tokens.unwrap_or(0) > 0 || parsed.output_tokens.unwrap_or(0) > 0;
    if !has_tokens {
        return;
    }
    if parsed.model.is_some() {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "has tokens (in={}, out={}) but no model",
                parsed.input_tokens.unwrap_or(0),
                parsed.output_tokens.unwrap_or(0)
            ),
        );
    }
}

/// Check 18: Raw type field -> parser LineType must match.
pub fn check_role_classification(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    accum: &mut CheckAccum,
) {
    let raw_type = raw.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let (expected_line_type, type_name) = match raw_type {
        "assistant" => (LineType::Assistant, "Assistant"),
        "user" => (LineType::User, "User"),
        "system" => (LineType::System, "System"),
        "progress" => (LineType::Progress, "Progress"),
        _ => return,
    };
    if parsed.line_type == expected_line_type {
        accum.record_pass();
    } else {
        accum.record_violation(
            file,
            line_num,
            &format!(
                "raw type={:?} but parser classified as {:?} (expected {:?})",
                raw_type, parsed.line_type, type_name
            ),
        );
    }
}

/// Run all per-line invariant checks on a single parsed line.
pub fn run_per_line_checks(
    raw: &serde_json::Value,
    parsed: &LiveLine,
    file: &str,
    line_num: usize,
    signals: &mut PipelineSignals,
) {
    check_token_extraction(raw, parsed, file, line_num, &mut signals.token_extraction);
    check_model_extraction(raw, parsed, file, line_num, &mut signals.model_extraction);
    check_tool_name_extraction(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.tool_name_extraction,
    );
    check_file_path_tool_presence(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.file_path_tool_presence,
    );
    check_content_preview(raw, parsed, file, line_num, &mut signals.content_preview);
    check_timestamp_extraction(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.timestamp_extraction,
    );
    check_cache_token_split(parsed, file, line_num, &mut signals.cache_token_split);
    check_cost_requires_model(parsed, file, line_num, &mut signals.cost_requires_model);
    check_role_classification(
        raw,
        parsed,
        file,
        line_num,
        &mut signals.role_classification,
    );
}
