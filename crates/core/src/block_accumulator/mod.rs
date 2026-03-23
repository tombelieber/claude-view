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

    /// Reset the accumulator to its initial empty state.
    /// Used when the JSONL file is truncated (e.g., context compaction).
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.current_assistant = None;
        self.boundary_acc = TurnBoundaryAccumulator::new();
        self.forked_from = None;
        self.line_index = 0;
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
            "worktree-state" => self.handle_worktree_state(entry),
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

    /// Non-consuming snapshot of current blocks, including any in-progress assistant.
    ///
    /// Used by the terminal WS block-mode stream to emit incremental updates
    /// without resetting the accumulator. The persistent accumulator correlates
    /// multi-line constructs (e.g., assistant + tool_result) that the per-line
    /// approach cannot.
    pub fn snapshot(&self) -> Vec<ConversationBlock> {
        let mut blocks = self.blocks.clone();
        if let Some(ref builder) = self.current_assistant {
            blocks.push(ConversationBlock::Assistant(builder.clone().finalize()));
        }
        blocks
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
            parent_uuid: entry
                .get("parentUuid")
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
            let mut builder = AssistantBlockBuilder::new(message_id.to_string(), ts);
            if let Some(pu) = entry.get("parentUuid").and_then(|p| p.as_str()) {
                builder.set_parent_uuid(pu.to_string());
            }
            self.current_assistant = Some(builder);
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

    fn handle_worktree_state(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::WorktreeState,
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

/// Session-level metadata extracted alongside blocks.
pub struct ParsedSession {
    pub blocks: Vec<ConversationBlock>,
    pub forked_from: Option<Value>,
}

/// Convenience function for batch processing — returns blocks + session metadata.
pub fn parse_session(content: &str) -> ParsedSession {
    let mut acc = BlockAccumulator::new();
    acc.process_all(content);
    let forked_from = acc.forked_from().cloned();
    let blocks = acc.finalize();
    ParsedSession {
        blocks,
        forked_from,
    }
}

/// Convenience function for batch processing (blocks only, legacy).
pub fn parse_session_as_blocks(content: &str) -> Vec<ConversationBlock> {
    parse_session(content).blocks
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

    /// Regression test: CC CLI writes incremental assistant entries with the
    /// same message.id (thinking, text, tool_use as separate lines). The
    /// persistent accumulator must merge them into ONE AssistantBlock with
    /// all segments, not produce separate blocks that replace each other.
    #[test]
    fn incremental_assistant_entries_merge_into_one_block() {
        let fixture =
            std::fs::read_to_string(fixtures_path().join("incremental_assistant.jsonl")).unwrap();
        let blocks = parse_session_as_blocks(&fixture);

        // Should have: User, Assistant(msg-inc-001), Assistant(msg-inc-002), TurnBoundary
        let assistants: Vec<_> = blocks
            .iter()
            .filter(|b| matches!(b, ConversationBlock::Assistant(_)))
            .collect();
        assert_eq!(
            assistants.len(),
            2,
            "Expected 2 AssistantBlocks (msg-inc-001 merged + msg-inc-002), got {}",
            assistants.len()
        );

        // The first assistant block (msg-inc-001) must have ALL segments from the
        // three incremental entries: thinking + text + tool_use
        if let ConversationBlock::Assistant(a) = assistants[0] {
            assert_eq!(a.id, "msg-inc-001");
            assert!(
                a.thinking.is_some(),
                "msg-inc-001 should have thinking from first incremental entry"
            );

            let text_segs: Vec<_> = a
                .segments
                .iter()
                .filter(|s| matches!(s, AssistantSegment::Text { .. }))
                .collect();
            assert!(
                !text_segs.is_empty(),
                "msg-inc-001 should have text segment from second entry"
            );

            let tool_segs: Vec<_> = a
                .segments
                .iter()
                .filter(|s| matches!(s, AssistantSegment::Tool { .. }))
                .collect();
            assert!(
                !tool_segs.is_empty(),
                "msg-inc-001 should have tool segment from third entry"
            );

            // Tool result should be attached (from the user tool_result entry)
            if let AssistantSegment::Tool { execution } = tool_segs[0] {
                assert!(
                    execution.result.is_some(),
                    "Tool should have result attached from user tool_result"
                );
                assert_eq!(execution.status, ToolStatus::Complete);
            }
        } else {
            panic!("Expected AssistantBlock");
        }
    }

    /// Test that snapshot() returns in-progress assistant blocks without
    /// consuming the accumulator state.
    #[test]
    fn snapshot_returns_in_progress_assistant() {
        let mut acc = BlockAccumulator::new();

        // Feed first incremental entry (thinking only)
        let entry1 = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-snap-001",
                "content": [{"type": "thinking", "thinking": "Let me think..."}]
            },
            "timestamp": "2026-03-23T01:00:00.000Z"
        });
        acc.process_line(&entry1);

        // Snapshot should show the in-progress assistant
        let snap1 = acc.snapshot();
        assert_eq!(snap1.len(), 1);
        if let ConversationBlock::Assistant(a) = &snap1[0] {
            assert_eq!(a.id, "msg-snap-001");
            assert!(a.thinking.is_some());
        } else {
            panic!("Expected AssistantBlock in snapshot");
        }

        // Feed second entry (text) — same message.id, should accumulate
        let entry2 = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-snap-001",
                "content": [{"type": "text", "text": "I'll read the file"}]
            }
        });
        acc.process_line(&entry2);

        // Second snapshot should have BOTH thinking and text
        let snap2 = acc.snapshot();
        assert_eq!(snap2.len(), 1);
        if let ConversationBlock::Assistant(a) = &snap2[0] {
            assert!(a.thinking.is_some());
            assert_eq!(a.segments.len(), 1); // one text segment
        } else {
            panic!("Expected AssistantBlock in second snapshot");
        }

        // Feed third entry (tool_use) — same message.id
        let entry3 = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-snap-001",
                "content": [{"type": "tool_use", "id": "tu-1", "name": "Read", "input": {}}],
                "stop_reason": "tool_use"
            }
        });
        acc.process_line(&entry3);

        let snap3 = acc.snapshot();
        assert_eq!(snap3.len(), 1);
        if let ConversationBlock::Assistant(a) = &snap3[0] {
            assert!(a.thinking.is_some());
            assert_eq!(a.segments.len(), 2); // text + tool
        } else {
            panic!("Expected AssistantBlock in third snapshot");
        }

        // Accumulator should still be usable (not consumed)
        let entry4 = serde_json::json!({
            "type": "user",
            "uuid": "u-1",
            "message": {"content": [{"type": "tool_result", "tool_use_id": "tu-1", "content": "result", "is_error": false}]}
        });
        acc.process_line(&entry4);
        let snap4 = acc.snapshot();
        // Still 1 block — tool result attached to existing assistant
        assert_eq!(snap4.len(), 1);
        if let ConversationBlock::Assistant(a) = &snap4[0] {
            let tool_seg = a
                .segments
                .iter()
                .find(|s| matches!(s, AssistantSegment::Tool { .. }));
            if let Some(AssistantSegment::Tool { execution }) = tool_seg {
                assert!(execution.result.is_some(), "Tool result should be attached");
            }
        }
    }

    #[test]
    fn reset_clears_accumulator_state() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-reset",
                "content": [{"type": "text", "text": "before reset"}]
            }
        });
        acc.process_line(&entry);
        assert_eq!(acc.snapshot().len(), 1);

        acc.reset();
        assert!(acc.snapshot().is_empty());
        assert!(acc.finalize().is_empty());
    }

    #[test]
    fn worktree_state_creates_system_block() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "worktree-state",
            "worktreeSession": {
                "originalCwd": "/Users/test/project",
                "worktreePath": "/Users/test/project/.claude/worktrees/feature",
                "worktreeName": "feature",
                "worktreeBranch": "worktree-feature",
                "originalBranch": "main",
                "originalHeadCommit": "abc123"
            },
            "sessionId": "sess-wt-1"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        assert_eq!(blocks.len(), 1);
        if let ConversationBlock::System(s) = &blocks[0] {
            assert_eq!(s.variant, SystemVariant::WorktreeState);
            assert_eq!(s.data["worktreeSession"]["worktreeName"], "feature");
        } else {
            panic!("Expected SystemBlock with WorktreeState variant");
        }
    }

    #[test]
    fn parse_session_returns_forked_from() {
        let content = r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hi"}]},"forkedFrom":{"sessionId":"parent-abc","messageUuid":"msg-xyz"},"timestamp":"2026-03-24T01:00:00.000Z"}"#;
        let parsed = super::parse_session(content);
        assert!(!parsed.blocks.is_empty());
        assert!(parsed.forked_from.is_some());
        let fk = parsed.forked_from.unwrap();
        assert_eq!(fk["sessionId"], "parent-abc");
    }

    #[test]
    fn parent_uuid_propagated_to_user_block() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "user",
            "uuid": "u-child",
            "parentUuid": "u-parent",
            "message": {"content": [{"type": "text", "text": "sub-agent message"}]},
            "timestamp": "2026-03-24T01:00:00.000Z"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        assert_eq!(blocks.len(), 1);
        if let ConversationBlock::User(u) = &blocks[0] {
            assert_eq!(u.parent_uuid, Some("u-parent".to_string()));
        } else {
            panic!("Expected UserBlock");
        }
    }

    #[test]
    fn parent_uuid_propagated_to_assistant_block() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "assistant",
            "parentUuid": "u-parent",
            "message": {
                "id": "msg-child",
                "content": [{"type": "text", "text": "sub-agent reply"}],
                "stop_reason": "end_turn"
            },
            "timestamp": "2026-03-24T01:00:01.000Z"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        assert_eq!(blocks.len(), 1);
        if let ConversationBlock::Assistant(a) = &blocks[0] {
            assert_eq!(a.parent_uuid, Some("u-parent".to_string()));
        } else {
            panic!("Expected AssistantBlock");
        }
    }

    #[test]
    fn parent_uuid_none_when_absent_from_jsonl() {
        let mut acc = BlockAccumulator::new();
        let entry = serde_json::json!({
            "type": "user",
            "uuid": "u-top",
            "message": {"content": [{"type": "text", "text": "top-level message"}]},
            "timestamp": "2026-03-24T01:00:00.000Z"
        });
        acc.process_line(&entry);
        let blocks = acc.finalize();
        if let ConversationBlock::User(u) = &blocks[0] {
            assert_eq!(u.parent_uuid, None);
        } else {
            panic!("Expected UserBlock");
        }
    }
}
