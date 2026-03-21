/// Extracted content blocks from a JSONL message.content[] array.
pub struct ContentBlocks {
    pub text_segments: Vec<TextSegment>,
    pub tool_uses: Vec<ToolUseBlock>,
    pub thinking: Option<String>,
}

pub struct TextSegment {
    pub text: String,
    pub parent_tool_use_id: Option<String>,
}

pub struct ToolUseBlock {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub parent_tool_use_id: Option<String>,
}

pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub output: String,
    pub is_error: bool,
}

/// Extract typed content blocks from a JSONL `message.content[]` array.
///
/// Handles `text`, `tool_use`, and `thinking` block types.
/// Unknown types are silently skipped for forward compatibility.
pub fn extract_content_blocks(content: &[serde_json::Value]) -> ContentBlocks {
    let mut blocks = ContentBlocks {
        text_segments: Vec::new(),
        tool_uses: Vec::new(),
        thinking: None,
    };

    for item in content {
        let block_type = match item.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };

        match block_type {
            "text" => {
                let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let parent = item
                    .get("parent_tool_use_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                blocks.text_segments.push(TextSegment {
                    text: text.to_string(),
                    parent_tool_use_id: parent,
                });
            }
            "tool_use" => {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let input = item
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let parent = item
                    .get("parent_tool_use_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                blocks.tool_uses.push(ToolUseBlock {
                    id: id.to_string(),
                    name: name.to_string(),
                    input,
                    parent_tool_use_id: parent,
                });
            }
            "thinking" => {
                let text = item.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                blocks.thinking = Some(text.to_string());
            }
            _ => {
                // Unknown block type — skip silently for forward compatibility
            }
        }
    }

    blocks
}

/// Extract tool result blocks from a JSONL `message.content[]` array.
///
/// Handles `tool_result` entries. The `content` field can be either a plain
/// string or an array of content blocks (in which case text blocks are joined).
pub fn extract_tool_results(content: &[serde_json::Value]) -> Vec<ToolResultBlock> {
    let mut results = Vec::new();

    for item in content {
        if item.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
            continue;
        }

        let tool_use_id = item
            .get("tool_use_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let output = match item.get("content") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        block.get("text").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        };

        let is_error = item
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        results.push(ToolResultBlock {
            tool_use_id,
            output,
            is_error,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_blocks() {
        let content = serde_json::json!([
            {"type": "text", "text": "Hello world"}
        ]);
        let blocks = extract_content_blocks(content.as_array().unwrap());
        assert_eq!(blocks.text_segments.len(), 1);
        assert_eq!(blocks.text_segments[0].text, "Hello world");
    }

    #[test]
    fn extract_tool_use_blocks() {
        let content = serde_json::json!([
            {"type": "tool_use", "id": "tu-1", "name": "Bash", "input": {"command": "ls"}}
        ]);
        let blocks = extract_content_blocks(content.as_array().unwrap());
        assert_eq!(blocks.tool_uses.len(), 1);
        assert_eq!(blocks.tool_uses[0].id, "tu-1");
        assert_eq!(blocks.tool_uses[0].name, "Bash");
    }

    #[test]
    fn extract_thinking_blocks() {
        let content = serde_json::json!([
            {"type": "thinking", "thinking": "Let me consider..."}
        ]);
        let blocks = extract_content_blocks(content.as_array().unwrap());
        assert_eq!(blocks.thinking, Some("Let me consider...".to_string()));
    }

    #[test]
    fn extract_tool_result_blocks() {
        let content = serde_json::json!([
            {"type": "tool_result", "tool_use_id": "tu-1", "content": "output text", "is_error": false}
        ]);
        let results = extract_tool_results(content.as_array().unwrap());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_use_id, "tu-1");
        assert_eq!(results[0].output, "output text");
        assert!(!results[0].is_error);
    }

    #[test]
    fn mixed_content_blocks() {
        let content = serde_json::json!([
            {"type": "thinking", "thinking": "hmm"},
            {"type": "text", "text": "I'll help"},
            {"type": "tool_use", "id": "tu-1", "name": "Read", "input": {"file_path": "/tmp/x"}},
            {"type": "text", "text": " with that"}
        ]);
        let blocks = extract_content_blocks(content.as_array().unwrap());
        assert_eq!(blocks.text_segments.len(), 2);
        assert_eq!(blocks.tool_uses.len(), 1);
        assert_eq!(blocks.thinking, Some("hmm".to_string()));
    }

    #[test]
    fn unknown_content_type_skipped_gracefully() {
        let content = serde_json::json!([
            {"type": "text", "text": "Hello"},
            {"type": "future_block_type", "data": "something"},
            {"type": "text", "text": "World"}
        ]);
        let blocks = extract_content_blocks(content.as_array().unwrap());
        assert_eq!(blocks.text_segments.len(), 2);
        // No panic, unknown type silently skipped
    }
}
