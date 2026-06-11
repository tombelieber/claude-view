// crates/providers/src/util/blocks.rs
//
// ConversationBlock builders for provider parsers. Foreign sessions use the
// provider-neutral core of the union (User / Assistant / Tool segments) plus
// SystemVariant::Informational and NoticeVariant::ContextCompacted; every
// CC-specific field stays None so the existing renderers degrade gracefully.

use claude_view_types::block_types::{
    AssistantBlock, AssistantSegment, ConversationBlock, NoticeBlock, NoticeVariant, SystemBlock,
    SystemVariant, ToolExecution, ToolResult, ToolStatus, UserBlock,
};

/// A user prompt block.
pub fn user(id: String, text: String, timestamp: Option<f64>) -> ConversationBlock {
    ConversationBlock::User(UserBlock {
        id,
        text,
        timestamp: timestamp.unwrap_or(0.0),
        status: None,
        local_id: None,
        pending: None,
        permission_mode: None,
        parent_uuid: None,
        is_sidechain: None,
        agent_id: None,
        prompt_source: None,
        images: Vec::new(),
        raw_json: None,
    })
}

/// An assistant block from pre-built segments.
pub fn assistant(
    id: String,
    segments: Vec<AssistantSegment>,
    thinking: Option<String>,
    timestamp: Option<f64>,
) -> ConversationBlock {
    ConversationBlock::Assistant(AssistantBlock {
        id,
        segments,
        thinking: thinking.filter(|t| !t.is_empty()),
        streaming: false,
        timestamp,
        parent_uuid: None,
        is_sidechain: None,
        agent_id: None,
        raw_json: None,
    })
}

/// A text segment.
pub fn text_segment(text: String) -> AssistantSegment {
    AssistantSegment::Text {
        text,
        parent_tool_use_id: None,
    }
}

/// A tool-call segment without a result yet (status defaults to Complete for
/// historical transcripts — foreign history has no in-flight tools).
pub fn tool_segment(
    tool_name: String,
    tool_input: serde_json::Value,
    tool_use_id: String,
) -> AssistantSegment {
    AssistantSegment::Tool {
        execution: ToolExecution {
            tool_name,
            tool_input,
            tool_use_id,
            parent_tool_use_id: None,
            result: None,
            progress: None,
            summary: None,
            status: ToolStatus::Complete,
            category: None,
            live_output: None,
            duration: None,
        },
    }
}

/// Attach a result to the matching tool segment (searched from the END of
/// the accumulated blocks — results follow their calls). Returns true when
/// a matching call was found.
pub fn attach_tool_result(
    blocks: &mut [ConversationBlock],
    tool_use_id: &str,
    output: String,
    is_error: bool,
) -> bool {
    for block in blocks.iter_mut().rev() {
        let ConversationBlock::Assistant(a) = block else {
            continue;
        };
        for seg in a.segments.iter_mut().rev() {
            let AssistantSegment::Tool { execution } = seg else {
                continue;
            };
            if execution.tool_use_id == tool_use_id {
                execution.result = Some(ToolResult {
                    output,
                    is_error,
                    is_replay: false,
                });
                execution.status = if is_error {
                    ToolStatus::Error
                } else {
                    ToolStatus::Complete
                };
                return true;
            }
        }
    }
    false
}

/// A low-emphasis system/info line (renders as a small gray info row).
pub fn system_info(id: String, text: String) -> ConversationBlock {
    ConversationBlock::System(SystemBlock {
        id,
        variant: SystemVariant::Informational,
        data: serde_json::json!({ "content": text }),
        raw_json: None,
    })
}

/// A context-compaction marker.
pub fn compaction_notice(id: String, summary: Option<String>) -> ConversationBlock {
    ConversationBlock::Notice(NoticeBlock {
        id,
        variant: NoticeVariant::ContextCompacted,
        data: match summary {
            Some(s) => serde_json::json!({ "summary": s }),
            None => serde_json::json!({}),
        },
        retry_in_ms: None,
        retry_attempt: None,
        max_retries: None,
    })
}

/// Deterministic per-session block id: `<session-raw-id>:<ordinal>`.
/// Foreign formats rarely carry stable per-message uuids; ordinals are
/// stable for an immutable historical file.
pub fn block_id(session_raw_id: &str, ordinal: usize) -> String {
    format!("{session_raw_id}:{ordinal}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_result_finds_latest_matching_call() {
        let mut blocks = vec![assistant(
            "a1".into(),
            vec![tool_segment(
                "Read".into(),
                serde_json::json!({"file_path": "/x"}),
                "t1".into(),
            )],
            None,
            None,
        )];
        assert!(attach_tool_result(&mut blocks, "t1", "contents".into(), false));
        assert!(!attach_tool_result(&mut blocks, "missing", String::new(), false));
        let ConversationBlock::Assistant(a) = &blocks[0] else {
            panic!()
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!()
        };
        assert_eq!(execution.result.as_ref().unwrap().output, "contents");
        assert_eq!(execution.status, ToolStatus::Complete);
    }

    #[test]
    fn error_results_set_error_status() {
        let mut blocks = vec![assistant(
            "a1".into(),
            vec![tool_segment("Bash".into(), serde_json::json!({}), "t9".into())],
            None,
            None,
        )];
        attach_tool_result(&mut blocks, "t9", "boom".into(), true);
        let ConversationBlock::Assistant(a) = &blocks[0] else {
            panic!()
        };
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!()
        };
        assert_eq!(execution.status, ToolStatus::Error);
    }
}
