// crates/core/src/block_accumulator/handlers.rs
//
// JSONL entry-type handlers for BlockAccumulator.
// Each method handles one `type` value from the JSONL stream:
// user, assistant, system, progress, and the simple system-like entries
// (queue-operation, file-history-snapshot, ai-title, etc.).

use serde_json::Value;

use super::BlockAccumulator;
use crate::block_accumulator::assistant::AssistantBlockBuilder;
use crate::block_accumulator::boundary::{
    detect_notice_from_assistant_error, detect_notice_from_system,
};
use crate::block_accumulator::content::{extract_content_blocks, extract_tool_results};
use crate::block_accumulator::transcript_builder;
use crate::block_types::*;

impl BlockAccumulator {
    // ── User handlers ────────────────────────────────────────────

    pub(super) fn handle_user(&mut self, entry: &Value) {
        let content = entry.pointer("/message/content");

        // String content: Claude CLI sometimes writes content as a plain string
        // instead of the array-of-blocks format. Handle it directly.
        if let Some(text) = content.and_then(|c| c.as_str()) {
            // Feed teammate messages to active transcript builder
            if text.contains("<teammate-message") {
                if let Some(builder) = self.transcript_builders.last_mut() {
                    builder.add_teammate_messages(text, self.line_index);
                }
            }
            self.flush_current_assistant();
            self.blocks.push(ConversationBlock::User(UserBlock {
                id: entry
                    .get("uuid")
                    .and_then(|u| u.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| self.make_id("user")),
                text: text.to_string(),
                timestamp: self.extract_timestamp(entry).unwrap_or(0.0),
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
                is_sidechain: entry.get("isSidechain").and_then(|v| v.as_bool()),
                agent_id: entry
                    .get("agentId")
                    .and_then(|a| a.as_str())
                    .map(String::from),
                images: vec![],
                raw_json: None,
            }));
            return;
        }

        // Array content: standard Claude API format
        let Some(arr) = content.and_then(|c| c.as_array()) else {
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
            is_sidechain: entry.get("isSidechain").and_then(|v| v.as_bool()),
            agent_id: entry
                .get("agentId")
                .and_then(|a| a.as_str())
                .map(String::from),
            images: blocks.images,
            raw_json: None,
        }));
    }

    // ── Assistant handler ────────────────────────────────────────

    pub(super) fn handle_assistant(&mut self, entry: &Value) {
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

        // Accumulate usage. Extract typed TokenUsage (including nested 5m/1h
        // cache_creation breakdown) so cost calculation gets the full picture.
        if let Some(usage_val) = usage {
            let tokens = crate::pricing::extract_usage_tokens(usage_val);
            self.boundary_acc.add_usage(model, &tokens);
        }

        // messageId flush guard -- new message_id means new assistant block
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
            if let Some(sc) = entry.get("isSidechain").and_then(|v| v.as_bool()) {
                builder.set_is_sidechain(sc);
            }
            if let Some(aid) = entry.get("agentId").and_then(|a| a.as_str()) {
                builder.set_agent_id(aid.to_string());
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

        // ── Team transcript wiring ──────────────────────────────────
        if let Some(arr) = content {
            self.process_assistant_transcript_wiring(arr);
        }

        // If stop_reason != "tool_use", flush (turn complete from assistant side)
        if stop_reason.is_some() && stop_reason != Some("tool_use") {
            self.flush_current_assistant();
        }
    }

    /// Process team transcript tool_use and narration from assistant content.
    fn process_assistant_transcript_wiring(&mut self, arr: &[Value]) {
        let cb = extract_content_blocks(arr);

        for tu in &cb.tool_uses {
            match tu.name.as_str() {
                "TeamCreate" => {
                    let team_name = tu
                        .input
                        .get("team_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let description = tu
                        .input
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.transcript_builders
                        .push(transcript_builder::TranscriptBuilder::new(
                            team_name,
                            description,
                        ));
                }
                "Agent" => {
                    if tu.input.get("team_name").is_some() {
                        if let Some(b) = self.transcript_builders.last_mut() {
                            let name = tu.input.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let desc = tu
                                .input
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let model = tu.input.get("model").and_then(|v| v.as_str());
                            b.add_speaker_from_spawn(name, None, desc, model);
                        }
                    }
                }
                "SendMessage" => {
                    if let Some(b) = self.transcript_builders.last_mut() {
                        if let (Some(to), Some(msg)) = (
                            tu.input.get("to").and_then(|v| v.as_str()),
                            tu.input.get("message").and_then(|v| v.as_str()),
                        ) {
                            b.add_moderator_relay(to.into(), msg.into(), self.line_index);
                        }
                    }
                }
                "TaskCreate" => {
                    if let Some(b) = self.transcript_builders.last_mut() {
                        b.add_task_event(
                            tu.input
                                .get("subject")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            None,
                            None,
                            self.line_index,
                        );
                    }
                }
                "TaskUpdate" => {
                    if let Some(b) = self.transcript_builders.last_mut() {
                        b.add_task_event(
                            tu.input
                                .get("subject")
                                .and_then(|v| v.as_str())
                                .unwrap_or("task")
                                .into(),
                            tu.input
                                .get("status")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            tu.input
                                .get("owner")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            self.line_index,
                        );
                    }
                }
                "TeamDelete" => {
                    if let Some(b) = self.transcript_builders.last_mut() {
                        b.mark_verdict();
                    }
                }
                _ => {}
            }
        }

        // Moderator narration from text segments
        if !self.transcript_builders.is_empty() {
            for seg in &cb.text_segments {
                if !seg.text.is_empty() {
                    if let Some(b) = self.transcript_builders.last_mut() {
                        b.add_moderator_narration(seg.text.clone(), self.line_index);
                    }
                }
            }
        }
    }

    // ── System handler ───────────────────────────────────────────

    pub(super) fn handle_system(&mut self, entry: &Value) {
        let subtype = entry.get("subtype").and_then(|s| s.as_str()).unwrap_or("");

        match subtype {
            "turn_duration" => {
                let duration = entry
                    .get("durationMs")
                    .and_then(|d| d.as_u64())
                    .unwrap_or(0);
                self.boundary_acc.set_duration(duration);
            }
            "stop_hook_summary" => {
                self.boundary_acc.set_hook_summary(entry);
                if let Some(block) = self.boundary_acc.build(self.make_id("tb")) {
                    self.blocks.push(ConversationBlock::TurnBoundary(block));
                }
                self.boundary_acc.reset();
            }
            "compact_boundary" => {
                if let Some(notice) = detect_notice_from_system("compact_boundary", entry) {
                    self.blocks.push(ConversationBlock::Notice(notice));
                }
            }
            "microcompact_boundary" => {
                if let Some(notice) = detect_notice_from_system("microcompact_boundary", entry) {
                    self.blocks.push(ConversationBlock::Notice(notice));
                }
            }
            "api_error" => {
                self.blocks.push(ConversationBlock::Notice(NoticeBlock {
                    id: self.make_id("notice"),
                    variant: NoticeVariant::Error,
                    data: entry.clone(),
                    retry_in_ms: None,
                    retry_attempt: None,
                    max_retries: None,
                }));
            }
            "local_command" => {
                self.blocks.push(ConversationBlock::System(SystemBlock {
                    id: self.make_id("sys"),
                    variant: SystemVariant::LocalCommand,
                    data: entry.clone(),
                    raw_json: None,
                }));
            }
            "informational" => {
                self.blocks.push(ConversationBlock::System(SystemBlock {
                    id: self.make_id("sys"),
                    variant: SystemVariant::Informational,
                    data: entry.clone(),
                    raw_json: None,
                }));
            }
            "scheduled_task_fire" => {
                self.blocks.push(ConversationBlock::System(SystemBlock {
                    id: self.make_id("sys"),
                    variant: SystemVariant::ScheduledTaskFire,
                    data: entry.clone(),
                    raw_json: None,
                }));
            }
            "away_summary" => {
                self.blocks.push(ConversationBlock::System(SystemBlock {
                    id: self.make_id("sys"),
                    variant: SystemVariant::AwaySummary,
                    data: entry.clone(),
                    raw_json: None,
                }));
            }
            // Fallback: field-sniffing for entries without subtype (older JSONL files)
            _ => self.handle_system_by_fields(entry),
        }
    }

    /// Fallback system handler that uses field-sniffing for older JSONL files
    /// that don't have a `subtype` field.
    fn handle_system_by_fields(&mut self, entry: &Value) {
        if entry.get("durationMs").is_some() {
            let duration = entry
                .get("durationMs")
                .and_then(|d| d.as_u64())
                .unwrap_or(0);
            self.boundary_acc.set_duration(duration);
        } else if entry.get("stopReason").is_some() && entry.get("hookInfos").is_some() {
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
                retry_in_ms: None,
                retry_attempt: None,
                max_retries: None,
            }));
        } else if entry.get("planContent").is_some() {
            self.blocks.push(ConversationBlock::System(SystemBlock {
                id: self.make_id("sys"),
                variant: SystemVariant::PlanContent,
                data: entry.clone(),
                raw_json: None,
            }));
        } else {
            let variant = SystemVariant::Informational;
            self.blocks.push(ConversationBlock::System(SystemBlock {
                id: self.make_id("sys"),
                variant,
                data: entry.clone(),
                raw_json: None,
            }));
        }
    }

    // ── Simple system-like entry handlers ────────────────────────

    pub(super) fn handle_pr_link(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::PrLink,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_custom_title(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::CustomTitle,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_queue_operation(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::QueueOperation,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_file_history_snapshot(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::FileHistorySnapshot,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_ai_title(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::AiTitle,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_last_prompt(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::LastPrompt,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_agent_name(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::AgentName,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_worktree_state(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::WorktreeState,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_attachment(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::Attachment,
            data: entry.clone(),
            raw_json: None,
        }));
    }

    pub(super) fn handle_permission_mode(&mut self, entry: &Value) {
        self.blocks.push(ConversationBlock::System(SystemBlock {
            id: self.make_id("sys"),
            variant: SystemVariant::PermissionModeChange,
            data: entry.clone(),
            raw_json: None,
        }));
    }
}
