// crates/core/src/llm/claude_cli/parsing.rs
//! Response parsing helpers for the Claude CLI provider.

use crate::llm::types::{ClassificationResponse, ClassificationUsage, LlmError};

/// Parse LLM JSON response into a ClassificationResponse.
///
/// Handles both direct JSON objects and Claude CLI's `result` wrapper format.
/// The `result` field may contain extra text (markdown, explanation) around the JSON —
/// we extract the first `{...}` block and parse that.
pub fn parse_classification_response(
    json: serde_json::Value,
) -> Result<ClassificationResponse, LlmError> {
    let wrapper_usage = extract_cli_usage(&json);
    let wrapper_total_cost_usd = json.get("total_cost_usd").and_then(|v| v.as_f64());
    let wrapper_model = json
        .get("modelUsage")
        .and_then(|v| v.as_object())
        .and_then(|m| m.keys().next().cloned());

    // Claude CLI wraps output in { "result": "..." } — check for that
    let inner = if let Some(result_str) = json.get("result").and_then(|v| v.as_str()) {
        // Try direct parse first
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(result_str) {
            v
        } else {
            // Model may have returned extra text around the JSON — extract it
            extract_json_from_text(result_str).ok_or_else(|| {
                LlmError::ParseFailed(format!(
                    "no JSON object found in CLI result: {}",
                    &result_str[..result_str.len().min(200)]
                ))
            })?
        }
    } else {
        json
    };

    let mut parsed: ClassificationResponse = serde_json::from_value(inner)
        .map_err(|e| LlmError::InvalidFormat(format!("response missing required fields: {}", e)))?;

    if parsed.usage.is_none() {
        parsed.usage = wrapper_usage;
    }
    if parsed.total_cost_usd.is_none() {
        parsed.total_cost_usd = wrapper_total_cost_usd;
    }
    if parsed.model.is_none() {
        parsed.model = wrapper_model;
    }

    Ok(parsed)
}

/// Extract the first JSON object `{...}` from a text string.
/// Handles cases where the model wraps JSON in markdown or explanation text.
fn extract_json_from_text(text: &str) -> Option<serde_json::Value> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let json_str = &text[start..end?];
    serde_json::from_str(json_str).ok()
}

fn extract_cli_usage(json: &serde_json::Value) -> Option<ClassificationUsage> {
    let usage = json.get("usage")?;
    Some(ClassificationUsage {
        input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()),
        output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()),
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64()),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64()),
    })
}
