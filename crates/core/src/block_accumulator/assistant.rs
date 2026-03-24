// crates/core/src/block_accumulator/assistant.rs
//
// Builder for accumulating an AssistantBlock from multiple JSONL entries.
// An AssistantBlock spans multiple JSONL lines: the `assistant` entry (with
// tool_use + text content blocks), followed by `user` entries (with tool_result
// content). The builder correlates tool_use <-> tool_result by `tool_use_id`.

use crate::block_types::*;
use crate::category::{categorize_tool, ActionCategory};

/// Builder for accumulating an AssistantBlock from multiple JSONL entries.
#[derive(Clone)]
pub struct AssistantBlockBuilder {
    message_id: String,
    segments: Vec<AssistantSegment>,
    thinking: Option<String>,
    timestamp: Option<f64>,
    parent_uuid: Option<String>,
    is_sidechain: Option<bool>,
    agent_id: Option<String>,
    raw_json: Option<serde_json::Value>,
    has_ask_question: bool,
}

impl AssistantBlockBuilder {
    pub fn new(message_id: String, timestamp: Option<f64>) -> Self {
        Self {
            message_id,
            segments: Vec::new(),
            thinking: None,
            timestamp,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            raw_json: None,
            has_ask_question: false,
        }
    }

    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    /// Add a text segment.
    pub fn add_text(&mut self, text: String, parent_tool_use_id: Option<String>) {
        self.segments.push(AssistantSegment::Text {
            text,
            parent_tool_use_id,
        });
    }

    /// Add a tool_use segment (initially status=Running, no result).
    pub fn add_tool_use(
        &mut self,
        tool_use_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
        parent_tool_use_id: Option<String>,
    ) {
        if tool_name == "AskUserQuestion" {
            self.has_ask_question = true;
        }

        let category_str = categorize_tool(&tool_name);
        let category = Some(ActionCategory::from_str_category(category_str));

        self.segments.push(AssistantSegment::Tool {
            execution: ToolExecution {
                tool_name,
                tool_input,
                tool_use_id,
                parent_tool_use_id,
                result: None,
                progress: None,
                summary: None,
                status: ToolStatus::Running,
                category,
                live_output: None,
                duration: None,
            },
        });
    }

    /// Attach a tool_result to the matching ToolExecution by tool_use_id.
    /// Returns `Some(&ToolExecution)` if matched, `None` if orphaned.
    pub fn attach_tool_result(
        &mut self,
        tool_use_id: &str,
        output: &str,
        is_error: bool,
    ) -> Option<&ToolExecution> {
        // Iterate in reverse — most recent tool_use is most likely match
        for segment in self.segments.iter_mut().rev() {
            if let AssistantSegment::Tool { execution } = segment {
                if execution.tool_use_id == tool_use_id {
                    execution.result = Some(ToolResult {
                        output: output.to_string(),
                        is_error,
                        is_replay: false,
                    });
                    execution.status = if is_error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Complete
                    };
                    return Some(execution);
                }
            }
        }
        None
    }

    /// Set the thinking/reasoning text for the block.
    pub fn set_thinking(&mut self, thinking: String) {
        self.thinking = Some(thinking);
    }

    /// Returns `true` if `new_message_id` differs from current (signals need to flush).
    pub fn should_flush(&self, new_message_id: &str) -> bool {
        self.message_id != new_message_id
    }

    /// Returns `true` if any tool_use is AskUserQuestion.
    pub fn has_ask_user_question(&self) -> bool {
        self.has_ask_question
    }

    /// Set the parent UUID for message threading.
    pub fn set_parent_uuid(&mut self, parent_uuid: String) {
        self.parent_uuid = Some(parent_uuid);
    }

    /// Set whether this message is on a sidechain (conversation branch).
    pub fn set_is_sidechain(&mut self, is_sidechain: bool) {
        self.is_sidechain = Some(is_sidechain);
    }

    /// Set the agent ID that produced this message.
    pub fn set_agent_id(&mut self, agent_id: String) {
        self.agent_id = Some(agent_id);
    }

    /// Store the raw JSON for the block (for debugging / pass-through).
    pub fn set_raw_json(&mut self, raw: serde_json::Value) {
        self.raw_json = Some(raw);
    }

    /// Finalize into an `AssistantBlock`. Consumes the builder.
    pub fn finalize(self) -> AssistantBlock {
        AssistantBlock {
            id: self.message_id,
            segments: self.segments,
            thinking: self.thinking,
            streaming: false,
            timestamp: self.timestamp,
            parent_uuid: self.parent_uuid,
            is_sidechain: self.is_sidechain,
            agent_id: self.agent_id,
            raw_json: self.raw_json,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_creates_text_segment() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.add_text("Hello world".into(), None);
        let block = builder.finalize();
        assert_eq!(block.segments.len(), 1);
        match &block.segments[0] {
            AssistantSegment::Text { text, .. } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected text segment"),
        }
    }

    #[test]
    fn builder_creates_tool_segment() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.add_tool_use(
            "tu-1".into(),
            "Bash".into(),
            serde_json::json!({"command": "ls"}),
            None,
        );
        let block = builder.finalize();
        assert_eq!(block.segments.len(), 1);
        match &block.segments[0] {
            AssistantSegment::Tool { execution } => {
                assert_eq!(execution.tool_name, "Bash");
                assert_eq!(execution.tool_use_id, "tu-1");
                assert_eq!(execution.status, ToolStatus::Running);
            }
            _ => panic!("Expected tool segment"),
        }
    }

    #[test]
    fn builder_attaches_tool_result() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.add_tool_use("tu-1".into(), "Read".into(), serde_json::json!({}), None);
        let result = builder.attach_tool_result("tu-1", "file contents", false);
        assert!(result.is_some()); // matched
        let block = builder.finalize();
        match &block.segments[0] {
            AssistantSegment::Tool { execution } => {
                assert!(execution.result.is_some());
                assert_eq!(execution.result.as_ref().unwrap().output, "file contents");
                assert_eq!(execution.status, ToolStatus::Complete);
            }
            _ => panic!("Expected tool segment"),
        }
    }

    #[test]
    fn builder_orphaned_tool_result_returns_none() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.add_tool_use("tu-1".into(), "Bash".into(), serde_json::json!({}), None);
        // Try to attach with a DIFFERENT tool_use_id
        let result = builder.attach_tool_result("tu-999", "output", false);
        assert!(result.is_none()); // no match
    }

    #[test]
    fn builder_detects_ask_user_question() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.add_tool_use(
            "tu-1".into(),
            "AskUserQuestion".into(),
            serde_json::json!({"questions": []}),
            None,
        );
        assert!(builder.has_ask_user_question());
    }

    #[test]
    fn builder_sets_thinking() {
        let mut builder = AssistantBlockBuilder::new("msg-1".into(), None);
        builder.set_thinking("Let me think about this...".into());
        let block = builder.finalize();
        assert_eq!(
            block.thinking,
            Some("Let me think about this...".to_string())
        );
    }

    #[test]
    fn builder_flush_on_message_id_change() {
        let builder = AssistantBlockBuilder::new("msg-1".into(), None);
        assert!(!builder.should_flush("msg-1")); // same id, don't flush
        assert!(builder.should_flush("msg-2")); // different id, flush
    }
}
