pub mod assistant;
pub mod boundary;
pub mod content;
mod handlers;
pub mod interactions;
mod progress;
pub mod transcript_builder;

use serde_json::Value;

use self::boundary::TurnBoundaryAccumulator;
use crate::block_types::*;

/// Sequential JSONL-to-ConversationBlock accumulator.
///
/// Processes JSONL entries one at a time, accumulating multi-line constructs:
/// - AssistantBlock spans assistant + user(tool_result) entries
/// - TurnBoundaryBlock assembles from turn_duration + stop_hook_summary + usage
pub struct BlockAccumulator {
    pub(crate) blocks: Vec<ConversationBlock>,
    pub(crate) current_assistant: Option<assistant::AssistantBlockBuilder>,
    pub(crate) boundary_acc: TurnBoundaryAccumulator,
    forked_from: Option<Value>,
    entrypoint: Option<String>,
    pub(crate) line_index: usize,
    pub(crate) transcript_builders: Vec<transcript_builder::TranscriptBuilder>,
}

impl BlockAccumulator {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_assistant: None,
            boundary_acc: TurnBoundaryAccumulator::new(),
            forked_from: None,
            entrypoint: None,
            line_index: 0,
            transcript_builders: Vec::new(),
        }
    }

    pub fn forked_from(&self) -> Option<&Value> {
        self.forked_from.as_ref()
    }

    pub fn entrypoint(&self) -> Option<&str> {
        self.entrypoint.as_deref()
    }

    /// Reset the accumulator to its initial empty state.
    /// Used when the JSONL file is truncated (e.g., context compaction).
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.current_assistant = None;
        self.boundary_acc = TurnBoundaryAccumulator::new();
        self.forked_from = None;
        self.entrypoint = None;
        self.line_index = 0;
        self.transcript_builders = Vec::new();
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

        // Extract entrypoint from first entry that has it
        if self.entrypoint.is_none() {
            if let Some(ep) = entry.get("entrypoint").and_then(|e| e.as_str()) {
                self.entrypoint = Some(ep.to_string());
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
            "pr-link" => self.handle_pr_link(entry),
            "custom-title" => self.handle_custom_title(entry),
            "agent-name" => self.handle_agent_name(entry),
            "attachment" => self.handle_attachment(entry),
            "permission-mode" => self.handle_permission_mode(entry),
            _ => {} // unknown entry type -- skip silently
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

        // Emit transcript blocks from all builders
        let builders: Vec<_> = self.transcript_builders.drain(..).collect();
        for (i, builder) in builders.into_iter().enumerate() {
            if !builder.is_empty() {
                let id = format!("transcript-{}-{}", self.line_index, i);
                self.blocks
                    .push(ConversationBlock::TeamTranscript(builder.build(id)));
            }
        }

        // Post-processing: synthesise historical InteractionBlocks for Plan
        // and Question patterns. Live sidecar InteractionBlocks never flow
        // through this accumulator, so no dedup is needed here -- per the
        // "Separate Channels = Separate Data" rule in CLAUDE.md.
        let mut blocks = std::mem::take(&mut self.blocks);
        interactions::synthesize_historical_interactions(&mut blocks);
        blocks
    }

    /// Non-consuming snapshot of current blocks, including any in-progress assistant.
    ///
    /// Used by the terminal WS block-mode stream to emit incremental updates
    /// without resetting the accumulator. The persistent accumulator correlates
    /// multi-line constructs (e.g., assistant + tool_result) that the per-line
    /// approach cannot.
    ///
    /// Historical InteractionBlocks are synthesised on every snapshot call
    /// (deterministic -- same input -> same IDs) so the live-monitor replay
    /// gets the same plan/question context that finalize() would produce.
    /// This does NOT overlap with the sidecar's live InteractionBlocks --
    /// terminal WS (block stream) and sidecar WS (chat FSM) are separate
    /// UI surfaces per CLAUDE.md's two-WS architecture rule.
    pub fn snapshot(&self) -> Vec<ConversationBlock> {
        let mut blocks = self.blocks.clone();
        if let Some(ref builder) = self.current_assistant {
            blocks.push(ConversationBlock::Assistant(builder.clone().finalize()));
        }

        // Include in-progress transcript blocks (non-consuming clone + build)
        for (i, builder) in self.transcript_builders.iter().enumerate() {
            if !builder.is_empty() {
                let id = format!("transcript-snap-{}-{}", self.line_index, i);
                blocks.push(ConversationBlock::TeamTranscript(builder.clone().build(id)));
            }
        }

        interactions::synthesize_historical_interactions(&mut blocks);
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

    // ── Helpers ──────────────────────────────────────────

    pub(crate) fn flush_current_assistant(&mut self) {
        if let Some(builder) = self.current_assistant.take() {
            self.blocks
                .push(ConversationBlock::Assistant(builder.finalize()));
        }
    }

    pub(crate) fn make_id(&self, prefix: &str) -> String {
        format!("{}-{}", prefix, self.line_index)
    }

    pub(crate) fn extract_timestamp(&self, entry: &Value) -> Option<f64> {
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
    pub entrypoint: Option<String>,
}

/// Convenience function for batch processing -- returns blocks + session metadata.
pub fn parse_session(content: &str) -> ParsedSession {
    let mut acc = BlockAccumulator::new();
    acc.process_all(content);
    let forked_from = acc.forked_from().cloned();
    let entrypoint = acc.entrypoint().map(String::from);
    let blocks = acc.finalize();
    ParsedSession {
        blocks,
        forked_from,
        entrypoint,
    }
}

/// Convenience function for batch processing (blocks only, legacy).
pub fn parse_session_as_blocks(content: &str) -> Vec<ConversationBlock> {
    parse_session(content).blocks
}

#[cfg(test)]
mod tests;
