// crates/core/src/parser/content.rs
//! Content extraction from JSONL message blocks.
//!
//! Handles both string and array content formats, extracts tool calls,
//! thinking text, and tool result content from various block types.

use crate::category::categorize_tool;
use crate::types::{ContentBlock, JsonlContent, ToolCall};

/// Extract readable content from tool_result array blocks.
pub(super) fn extract_tool_result_content(blocks: &[serde_json::Value]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str());
        match block_type {
            Some("tool_result") => {
                let tool_use_id = block
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                // Try to extract text content from the tool result
                match block.get("content") {
                    Some(serde_json::Value::String(s)) => {
                        let truncated = if s.len() > 200 {
                            // Find a safe char boundary at or before byte 200
                            let mut end = 200;
                            while end > 0 && !s.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...", &s[..end])
                        } else {
                            s.clone()
                        };
                        parts.push(format!("[Tool result for {}]: {}", tool_use_id, truncated));
                    }
                    Some(serde_json::Value::Array(arr)) => {
                        // Content might be array of text blocks
                        let text: String = arr
                            .iter()
                            .filter_map(|item| {
                                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            parts.push(format!("[Tool result for {}]: {}", tool_use_id, text));
                        } else {
                            parts.push(format!("[Tool result for {}]", tool_use_id));
                        }
                    }
                    _ => {
                        parts.push(format!("[Tool result for {}]", tool_use_id));
                    }
                }
            }
            Some("text") => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    parts.join("\n")
}

/// Extract text content from JSONL content, handling both string and block formats.
pub(super) fn extract_text_content(content: &JsonlContent) -> String {
    match content {
        JsonlContent::Text(text) => text.clone(),
        JsonlContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Extract text content, tool calls, and thinking text from assistant message content.
///
/// Returns `(text, tool_calls, thinking)` where thinking is the concatenated
/// content of any `Thinking` blocks in the message.
pub(super) fn extract_assistant_content(
    content: &JsonlContent,
) -> (String, Vec<ToolCall>, Option<String>) {
    match content {
        JsonlContent::Text(text) => (text.clone(), vec![], None),
        JsonlContent::Blocks(blocks) => {
            let mut text_parts: Vec<&str> = Vec::new();
            let mut thinking_parts: Vec<&str> = Vec::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        text_parts.push(text);
                    }
                    ContentBlock::Thinking { thinking } => {
                        thinking_parts.push(thinking);
                    }
                    ContentBlock::ToolUse { name, input } => {
                        tool_calls.push(ToolCall {
                            name: name.clone(),
                            count: 1,
                            input: input.clone(),
                            category: Some(categorize_tool(name).to_string()),
                        });
                    }
                    _ => {} // Ignore tool_result and other blocks
                }
            }

            let text = text_parts.join("\n");

            let thinking = if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n"))
            };

            (text, tool_calls, thinking)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_content_string() {
        let content = JsonlContent::Text("Hello world".to_string());
        assert_eq!(extract_text_content(&content), "Hello world");
    }

    #[test]
    fn test_extract_text_content_blocks() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Text {
                text: "First".to_string(),
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::Text {
                text: "Second".to_string(),
            },
        ]);
        assert_eq!(extract_text_content(&content), "First\nSecond");
    }

    #[test]
    fn test_extract_assistant_content_with_tools() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Text {
                text: "Let me help".to_string(),
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::ToolUse {
                name: "Read".to_string(),
                input: None,
            },
            ContentBlock::ToolUse {
                name: "Edit".to_string(),
                input: None,
            },
        ]);

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "Let me help");
        assert_eq!(tools.len(), 3); // Individual entries: Read, Read, Edit
        assert!(thinking.is_none());

        assert_eq!(tools[0].name, "Read");
        assert_eq!(tools[0].count, 1);
        assert_eq!(tools[1].name, "Read");
        assert_eq!(tools[1].count, 1);
        assert_eq!(tools[2].name, "Edit");
        assert_eq!(tools[2].count, 1);
    }

    #[test]
    fn test_extract_assistant_content_with_thinking() {
        let content = JsonlContent::Blocks(vec![
            ContentBlock::Thinking {
                thinking: "Let me reason about this...".to_string(),
            },
            ContentBlock::Text {
                text: "Here is the answer".to_string(),
            },
        ]);

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "Here is the answer");
        assert!(tools.is_empty());
        assert_eq!(thinking, Some("Let me reason about this...".to_string()));
    }

    #[test]
    fn test_extract_assistant_content_thinking_only() {
        let content = JsonlContent::Blocks(vec![ContentBlock::Thinking {
            thinking: "Just thinking...".to_string(),
        }]);

        let (text, tools, thinking) = extract_assistant_content(&content);
        assert_eq!(text, "");
        assert!(tools.is_empty());
        assert_eq!(thinking, Some("Just thinking...".to_string()));
    }
}
