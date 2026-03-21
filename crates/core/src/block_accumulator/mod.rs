pub mod assistant;
pub mod boundary;
pub mod content;

use serde_json::Value;

use self::assistant::AssistantBlockBuilder;
use self::boundary::{
    detect_notice_from_assistant_error, detect_notice_from_system, TurnBoundaryAccumulator,
};
use self::content::{extract_content_blocks, extract_tool_results};
use crate::block_types::*;
use crate::category::ActionCategory;

/// Sequential JSONL-to-ConversationBlock accumulator.
///
/// Processes JSONL entries one at a time, accumulating multi-line constructs:
/// - AssistantBlock spans assistant + user(tool_result) entries
/// - TurnBoundaryBlock assembles from turn_duration + stop_hook_summary + usage
pub struct BlockAccumulator {
    blocks: Vec<ConversationBlock>,
    current_assistant: Option<AssistantBlockBuilder>,
    boundary_acc: TurnBoundaryAccumulator,
    forked_from: Option<Value>,
    line_index: usize,
}

impl BlockAccumulator {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_assistant: None,
            boundary_acc: TurnBoundaryAccumulator::new(),
            forked_from: None,
            line_index: 0,
        }
    }

    pub fn forked_from(&self) -> Option<&Value> {
        self.forked_from.as_ref()
    }

    /// Process a single JSONL entry.
    pub fn process_line(&mut self, entry: &Value) {
        self.line_index += 1;

        // Extract forkedFrom from first entry that has it
        if self.forked_from.is_none() {
            if let Some(fk) = entry.get("forkedFrom") {
                self.forked_from = Some(fk.clone());
            }
        }

        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match entry_type {
            "user" => self.handle_user(entry),
            "assistant" => self.handle_assistant(entry),
            "progress" => self.handle_progress(entry),
            "system" => self.handle_system(entry),
            "queue-operation" => self.handle_queue_operation(entry),
            "file-history-snapshot" => self.handle_file_history_snapshot(entry),
            "ai-title" => self.handle_ai_title(entry),
            "last-prompt" => self.handle_last_prompt(entry),
            _ => {} // unknown entry type — skip silently
        }
    }

    /// Finalize: flush any pending state, return all accumulated blocks.
    pub fn finalize(&mut self) -> Vec<ConversationBlock> {
        self.flush_current_assistant();

        // Emit partial TurnBoundary if we have duration but no hook summary
        if self.boundary_acc.has_duration() {
            if let Some(block) = self.boundary_acc.build_partial(self.make_id("tb")) {
                self.blocks.push(ConversationBlock::TurnBoundary(block));
            }
        }

        std::mem::take(&mut self.blocks)
    }

    /// Process all lines from a string (convenience for batch mode).
    pub fn process_all(&mut self, content: &str) {
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<Value>(line) {
                self.process_line(&entry);
            }
            // Malformed JSON lines are silently skipped
        }
    }

    // ── Private handlers ──────────────────────────────────────────

    fn handle_user(&mut self, entry: &Value) {
        let content_arr = entry.pointer("/message/content").and_then(|c| c.as_array());

        let Some(arr) = content_arr else {
            return;
        };

        let is_tool_result = arr
            .iter()
            .any(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_result"));

        if is_tool_result {
            self.handle_user_tool_result(arr, entry);
        } else {
            self.handle_user_message(arr, entry);
        }
    }

    fn handle_user_tool_result(&mut self, arr: &[Value], entry: &Value) {
        let tool_results = extract_tool_results(arr);
        for tr in tool_results {
            let matched = self
                .current_assistant
                .as_mut()
                .and_then(|b| b.attach_tool_result(&tr.tool_use_id, &tr.output, tr.is_error));

            if matched.is_none() {
                // Orphaned tool_result
                self.blocks.push(ConversationBlock::System(SystemBlock {
                    id: self.make_id("sys"),
                    variant: SystemVariant::Unknown,
                    data: entry.clone(),
                    raw_json: Some(entry.clone()),
                }));
            }
        }
    }

    fn handle_user_message(&mut self, arr: &[Value], entry: &Value) {
        self.flush_current_assistant();

        let blocks = extract_content_blocks(arr);
        let text = blocks
            .text_segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let timestamp = self.extract_timestamp(entry).unwrap_or(0.0);

        self.blocks.push(ConversationBlock::User(UserBlock {
            id: entry
                .get("uuid")
                .and_then(|u| u.as_str())
                .map(String::from)
                .unwrap_or_else(|| self.make_id("user")),
            text,
            timestamp,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: entry
                .get("permissionMode")
                .and_then(|p| p.as_str())
                .map(String::from),
            raw_json: None,
        }));
    }

    fn handle_assistant(&mut self, entry: &Value) {
        let message = entry.get("message");
        let message_id = message
            .and_then(|m| m.get("id"))
            .and_then(|i| i.as_str())
            .unwrap_or("");
        let model = message
            .and_then(|m| m.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let usage = message.and_then(|m| m.get("usage"));
        let stop_reason = message
            .and_then(|m| m.get("stop_reason"))
            .and_then(|s| s.as_str());
        let content = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array());

        // Check for error/retry entries first
        if let Some(notice) = detect_notice_from_assistant_error(entry) {
            self.blocks.push(ConversationBlock::Notice(notice));
            return;
        }

        // Accumulate usage
        if let Some(usage_val) = usage {
            self.boundary_acc.add_usage(model, usage_val);
        }

        // messageId flush guard — new message_id means new assistant block
        if let Some(ref builder) = self.current_assistant {
            if !message_id.is_empty() && builder.should_flush(message_id) {
                self.flush_current_assistant();
            }
        }

        // Start or continue assistant builder
        if self.current_assistant.is_none() {
            let ts = self.extract_timestamp(entry);
            self.current_assistant = Some(AssistantBlockBuilder::new(message_id.to_string(), ts));
        }

        // Process content blocks
        if let Some(arr) = content {
            let blocks = extract_content_blocks(arr);

            if let Some(thinking) = blocks.thinking {
                if let Some(ref mut builder) = self.current_assistant {
                    builder.set_thinking(thinking);
                }
            }

            for seg in blocks.text_segments {
                if let Some(ref mut builder) = self.current_assistant {
                    builder.add_text(seg.text, seg.parent_tool_use_id);
                }
            }

            for tu in blocks.tool_uses {
                if let Some(ref mut builder) = self.current_assistant {
                    builder.add_tool_use(tu.id, tu.name, tu.input, tu.parent_tool_use_id);
                }
            }
        }

        // If stop_reason != "tool_use", flush (turn complete from assistant side)
        if stop_reason.is_some() && stop_reason != Some("tool_use") {
            self.flush_current_assistant();
        }
    }

    fn handle_progress(&mut self, entry: &Value) {
        let data = entry.get("data");
        let data_type = data
            .and_then(|d| d.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let ts = self.extract_timestamp(entry).unwrap_or(0.0);
        let parent_tool_use_id = entry
            .get("parentToolUseID")
            .and_then(|p| p.as_str())
            .map(String::from);

        let Some((variant, category, progress_data)) =
            self.build_progress_data(data_type, data, ts)
        else {
            return; // Unknown progress type — skip
        };

        self.blocks.push(ConversationBlock::Progress(ProgressBlock {
            id: self.make_id("prog"),
            variant,
            category,
            data: progress_data,
            ts,
            parent_tool_use_id,
        }));
    }

    fn build_progress_data(
        &self,
        data_type: &str,
        data: Option<&Value>,
        _ts: f64,
    ) -> Option<(ProgressVariant, ActionCategory, ProgressData)> {
        let d = data?;
        match data_type {
            "bash_progress" => Some((
                ProgressVariant::Bash,
                ActionCategory::Builtin,
                ProgressData::Bash(BashProgress {
                    output: str_field(d, "output").into(),
                    full_output: str_field(d, "fullOutput").into(),
                    elapsed_time_seconds: d
                        .get("elapsedTimeSeconds")
                        .and_then(|e| e.as_f64())
                        .unwrap_or(0.0),
                    total_lines: d.get("totalLines").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                    total_bytes: d.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0),
                    task_id: d.get("taskId").and_then(|t| t.as_str()).map(String::from),
                }),
            )),
            "agent_progress" => Some((
                ProgressVariant::Agent,
                ActionCategory::Agent,
                ProgressData::Agent(AgentProgress {
                    prompt: str_field(d, "prompt").into(),
                    agent_id: str_field(d, "agentId").into(),
                    message: d.get("message").cloned(),
                }),
            )),
            "mcp_progress" => Some((
                ProgressVariant::Mcp,
                ActionCategory::Mcp,
                ProgressData::Mcp(McpProgress {
                    status: str_field(d, "status").into(),
                    server_name: str_field(d, "serverName").into(),
                    tool_name: str_field(d, "toolName").into(),
                }),
            )),
            "hook_progress" => Some((
                ProgressVariant::Hook,
                ActionCategory::Hook,
                ProgressData::Hook(HookProgress {
                    hook_event: str_field(d, "hookEvent").into(),
                    hook_name: str_field(d, "hookName").into(),
                    command: str_field(d, "command").into(),
                    status_message: str_field(d, "statusMessage").into(),
                }),
            )),
            "waiting_for_task" => Some((
                ProgressVariant::TaskQueue,
                ActionCategory::Agent,
                ProgressData::TaskQueue(TaskQueueProgress {
                    task_description: str_field(d, "taskDescription").into(),
                    task_type: str_field(d, "taskType").into(),
                }),
            )),
            "search_results_received" => Some((
                ProgressVariant::Search,
                ActionCategory::Builtin,
                ProgressData::Search(SearchProgress {
                    result_count: d.get("resultCount").and_then(|r| r.as_u64()).unwrap_or(0) as u32,
                    query: str_field(d, "query").into(),
                }),
            )),
            "query_update" => Some((
                ProgressVariant::Query,
                ActionCategory::Builtin,
                ProgressData::Query(QueryProgress {
                    query: str_field(d, "query").into(),
                }),
            )),
            _ => None,
        }
    }

    fn handle_system(&mut self, entry: &Value) {
        if entry.get("durationMs").is_some() {
            // turn_duration
            let duration = entry
                .get("durationMs")
                .and_then(|d| d.as_u64())
                .unwrap_or(0);
            self.boundary_acc.set_duration(duration);
        } else if entry.get("stopReason").is_some() && entry.get("hookInfos").is_some() {
            // stop_hook_summary — build and emit TurnBoundaryBlock
            self.boundary_acc.set_hook_summary(entry);
            if let Some(block) = self.boundary_acc.build(self.make_id("tb")) {
                self.blocks.push(ConversationBlock::TurnBoundary(block));
            }
            self.boundary_acc.reset();
        } else if entry.get("compactMetadata").is_some() {
            if let Some(notice) = detect_notice_from_system("compact_boundary", entry) {
                self.blocks.push(ConversationBlock::Notice(notice));
            }
        } else if entry.get("microcompactMetadata").is_some() {
            if let Some(notice) = detect_notice_from_system("microcompact_boundary", entry) {
                self.blocks.push(ConversationBlock::Notice(notice));
            }
        } else if entry.get("error").is_some() && entry.get("isApiErrorMessage").is_some() {
            self.blocks.push(ConversationBlock::Notice(NoticeBlock {
                id: self.make_id("notice"),
                variant: NoticeVariant::Error,
                data: entry.clone(),
            }));
        } else {
            // Other system entries
            let variant = SystemVariant::Informational;
            self.blocks.push(ConversationBlock::System(SystemBlock {
                id: self.make_id("sys"),
                variant,
                data: entry.clone(),
                raw_json: None,
            }));
        }
    }

    fn handle_queue_operation(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::QueueOperation,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    fn handle_file_history_snapshot(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::FileHistorySnapshot,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    fn handle_ai_title(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::AiTitle,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    fn handle_last_prompt(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::LastPrompt,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    // ── Helpers ──────────────────────────────────────────

    fn flush_current_assistant(&mut self) {
        if let Some(builder) = self.current_assistant.take() {
            self.blocks
                .push(ConversationBlock::Assistant(builder.finalize()));
        }
    }

    fn make_id(&self, prefix: &str) -> String {
        format!("{}-{}", prefix, self.line_index)
    }

    fn extract_timestamp(&self, entry: &Value) -> Option<f64> {
        entry.get("timestamp").and_then(|t| {
            if let Some(s) = t.as_str() {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp() as f64 + dt.timestamp_subsec_millis() as f64 / 1000.0)
            } else {
                t.as_f64()
            }
        })
    }
}

impl Default for BlockAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function for batch processing.
pub fn parse_session_as_blocks(content: &str) -> Vec<ConversationBlock> {
    let mut acc = BlockAccumulator::new();
    acc.process_all(content);
    acc.finalize()
}

/// Helper: extract a string field with fallback to "".
fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|s| s.as_str()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/block_accumulator")
    }

    #[test]
    fn simple_turn_produces_correct_blocks() {
        let fixture = std::fs::read_to_string(fixtures_path().join("simple_turn.jsonl")).unwrap();
        let mut acc = BlockAccumulator::new();
        for line in fixture.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            acc.process_line(&entry);
        }
        let blocks = acc.finalize();

        // Expected: UserBlock, AssistantBlock (tool_use text), ProgressBlock,
        //           AssistantBlock (final text), TurnBoundaryBlock
        assert!(
            blocks.len() >= 3,
            "Expected at least 3 blocks, got {}",
            blocks.len()
        );

        // First block should be UserBlock
        assert!(matches!(&blocks[0], ConversationBlock::User(_)));

        // Should contain a ProgressBlock
        assert!(blocks
            .iter()
            .any(|b| matches!(b, ConversationBlock::Progress(_))));

        // Should contain an AssistantBlock with segments
        let assistant = blocks
            .iter()
            .find(|b| matches!(b, ConversationBlock::Assistant(_)));
        assert!(assistant.is_some());
        if let ConversationBlock::Assistant(a) = assistant.unwrap() {
            assert!(!a.segments.is_empty());
        }

        // Last block should be TurnBoundaryBlock
        assert!(matches!(
            blocks.last().unwrap(),
            ConversationBlock::TurnBoundary(_)
        ));
    }

    #[test]
    fn empty_file_produces_no_blocks() {
        let mut acc = BlockAccumulator::new();
        let blocks = acc.finalize();
        assert!(blocks.is_empty());
    }

    #[test]
    fn system_only_session() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "ai-title",
            "sessionId": "sess-1",
            "aiTitle": "Test Session"
        });
        acc.process_line(&entry);
        let entry = serde_json::json!({
            "type": "last-prompt",
            "sessionId": "sess-1",
            "lastPrompt": "hello"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        assert_eq!(blocks.len(), 2);
        assert!(blocks
            .iter()
            .all(|b| matches!(b, ConversationBlock::System(_))));
    }

    #[test]
    fn standalone_progress_entries() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "progress",
            "data": {
                "type": "hook_progress",
                "hookEvent": "PreToolUse",
                "hookName": "live-monitor",
                "command": "echo test",
                "statusMessage": "running"
            },
            "timestamp": "2026-03-21T01:00:00.000Z"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ConversationBlock::Progress(_)));
    }

    #[test]
    fn forked_from_extraction() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "user",
            "uuid": "u-1",
            "message": {"content": [{"type": "text", "text": "hello"}]},
            "forkedFrom": {"sessionId": "parent-sess", "messageUuid": "parent-msg"},
            "timestamp": "2026-03-21T01:00:00.000Z"
        });
        acc.process_line(&entry);
        assert!(acc.forked_from().is_some());
        let fk = acc.forked_from().unwrap();
        assert_eq!(fk["sessionId"], "parent-sess");
    }

    // ── Extended integration tests (fixture-based) ───────────────

    #[test]
    fn multi_turn_produces_two_boundaries() {
        let fixture = std::fs::read_to_string(fixtures_path().join("multi_turn.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);
        let boundaries: Vec<_> = blocks
            .iter()
            .filter(|b| matches!(b, ConversationBlock::TurnBoundary(_)))
            .collect();
        assert_eq!(
            boundaries.len(),
            2,
            "Expected 2 TurnBoundaryBlocks for 2 turns"
        );
    }

    #[test]
    fn ask_user_question_creates_assistant_with_tool() {
        let fixture =
            std::fs::read_to_string(fixtures_path().join("with_interactions.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);
        let assistant = blocks
            .iter()
            .find(|b| matches!(b, ConversationBlock::Assistant(_)));
        assert!(assistant.is_some(), "Should have an AssistantBlock");
        if let ConversationBlock::Assistant(a) = assistant.unwrap() {
            let has_ask = a.segments.iter().any(|s| {
                if let AssistantSegment::Tool { execution } = s {
                    execution.tool_name == "AskUserQuestion"
                } else {
                    false
                }
            });
            assert!(has_ask, "Should have AskUserQuestion tool");
        }
    }

    #[test]
    fn notices_from_compact_and_errors() {
        let fixture = std::fs::read_to_string(fixtures_path().join("with_notices.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);
        let notices: Vec<_> = blocks
            .iter()
            .filter(|b| matches!(b, ConversationBlock::Notice(_)))
            .collect();
        assert!(
            notices.len() >= 2,
            "Expected at least 2 notices (rate_limit + context_compacted), got {}",
            notices.len()
        );
    }

    #[test]
    fn forked_from_extracted_from_fixture() {
        let fixture =
            std::fs::read_to_string(fixtures_path().join("with_forked_from.jsonl")).unwrap();
        let mut acc = BlockAccumulator::new();
        acc.process_all(&fixture);
        assert!(acc.forked_from().is_some());
        let fk = acc.forked_from().unwrap();
        assert!(fk.get("sessionId").is_some());
        assert!(fk.get("messageUuid").is_some());
        assert_eq!(fk["sessionId"], "parent-session-abc");
        assert_eq!(fk["messageUuid"], "parent-msg-xyz");
    }

    #[test]
    fn orphaned_tool_result_emits_system_block() {
        let fixture =
            std::fs::read_to_string(fixtures_path().join("orphaned_tool_result.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);
        let unknown_systems: Vec<_> = blocks
            .iter()
            .filter(|b| {
                if let ConversationBlock::System(s) = b {
                    matches!(s.variant, SystemVariant::Unknown)
                } else {
                    false
                }
            })
            .collect();
        assert!(
            !unknown_systems.is_empty(),
            "Expected at least 1 SystemBlock(Unknown) for orphaned tool_result"
        );
    }

    #[test]
    fn system_only_session_no_crash() {
        let fixture = std::fs::read_to_string(fixtures_path().join("system_only.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);
        assert_eq!(
            blocks.len(),
            4,
            "Expected 4 SystemBlocks, got {}",
            blocks.len()
        );
        for block in &blocks {
            assert!(
                matches!(block, ConversationBlock::System(_)),
                "Expected all SystemBlocks"
            );
        }
        let variants: Vec<_> = blocks
            .iter()
            .map(|b| {
                if let ConversationBlock::System(s) = b {
                    s.variant
                } else {
                    panic!("Expected SystemBlock")
                }
            })
            .collect();
        assert!(variants.contains(&SystemVariant::AiTitle));
        assert!(variants.contains(&SystemVariant::LastPrompt));
        assert!(variants.contains(&SystemVariant::QueueOperation));
        assert!(variants.contains(&SystemVariant::FileHistorySnapshot));
    }
}
