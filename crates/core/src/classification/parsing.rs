// crates/core/src/classification/parsing.rs
//! Response parsing and validation for batch classification LLM output.

use super::taxonomy::{CategoryL1, CategoryL2, CategoryL3};
use super::types::{BatchClassificationResponse, ValidatedClassification};

/// Default batch size for classification (sessions per LLM call).
pub const BATCH_SIZE: usize = 5;

/// Parse and validate a batch classification response from the LLM.
///
/// Handles:
/// - Direct JSON objects
/// - Claude CLI `{ "result": "..." }` wrapper
/// - Markdown code blocks around JSON
/// - Category strings like "code/feature/new-component"
pub fn parse_batch_response(raw: &str) -> Result<Vec<ValidatedClassification>, String> {
    // Strip markdown code blocks if present
    let cleaned = strip_markdown_json(raw);

    // Try to parse as the batch response format
    let response: BatchClassificationResponse = serde_json::from_str(&cleaned)
        .or_else(|_| {
            // Try Claude CLI wrapper format: { "result": "..." }
            let wrapper: serde_json::Value =
                serde_json::from_str(&cleaned).map_err(|e| format!("JSON parse failed: {}", e))?;
            if let Some(result_str) = wrapper.get("result").and_then(|v| v.as_str()) {
                let inner_cleaned = strip_markdown_json(result_str);
                serde_json::from_str(&inner_cleaned)
                    .map_err(|e| format!("Inner JSON parse failed: {}", e))
            } else {
                Err("Not a valid classification response".to_string())
            }
        })
        .map_err(|e| format!("Failed to parse classification response: {}", e))?;

    let mut results = Vec::new();
    for item in response.classifications {
        match parse_category_string(&item.category) {
            Some((l1, l2, l3)) => {
                let confidence = item.confidence.clamp(0.0, 1.0);
                results.push(ValidatedClassification {
                    session_id: item.session_id,
                    l1: l1.to_string(),
                    l2: l2.to_string(),
                    l3: l3.to_string(),
                    confidence,
                    reasoning: item.reasoning,
                });
            }
            None => {
                // Skip invalid categories but log them
                tracing::warn!(
                    session_id = %item.session_id,
                    category = %item.category,
                    "Skipping invalid category string"
                );
            }
        }
    }

    Ok(results)
}

/// Parse a category string like "code/feature/new-component" into (l1, l2, l3) components.
pub fn parse_category_string(category: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = category.split('/').collect();
    if parts.len() != 3 {
        return None;
    }

    let l1 = parts[0];
    let l2 = parts[1];
    let l3 = parts[2];

    // Validate L1
    CategoryL1::parse(l1)?;

    // Validate L2
    CategoryL2::parse(l2)?;

    // Validate L3
    CategoryL3::parse(l3)?;

    Some((l1, l2, l3))
}

/// Strip markdown code block fences from a string containing JSON.
fn strip_markdown_json(s: &str) -> String {
    let trimmed = s.trim();

    // Strip ```json ... ``` or ``` ... ```
    if trimmed.starts_with("```") {
        let start = if let Some(newline_pos) = trimmed.find('\n') {
            newline_pos + 1
        } else {
            return trimmed.to_string();
        };

        let end = trimmed.rfind("```").unwrap_or(trimmed.len());
        if end > start {
            return trimmed[start..end].trim().to_string();
        }
    }

    trimmed.to_string()
}
