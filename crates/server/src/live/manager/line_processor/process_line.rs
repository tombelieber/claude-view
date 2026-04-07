//! Main `process_single_line` method on `LiveSessionManager`.

use claude_view_core::live_parser::LineType;

use crate::live::state::HookEvent;

use super::super::accumulator::SessionAccumulator;
use super::super::helpers::parse_timestamp_to_unix;
use super::super::LiveSessionManager;
use super::channel_a::emit_channel_a_events;
use super::phase::process_phase_classification;
use super::progress::process_progress_items;
use super::sub_agents::process_sub_agents;
use super::tokens::{accumulate_tokens, accumulate_turn_cost};

impl LiveSessionManager {
    /// Process a single parsed JSONL line, updating the accumulator.
    pub(in crate::live::manager) fn process_single_line(
        &self,
        line: &claude_view_core::live_parser::LiveLine,
        acc: &mut SessionAccumulator,
        last_activity_at: i64,
        session_id: &str,
        channel_a_events: &mut Vec<HookEvent>,
    ) {
        // Content-block dedup
        let has_measurement_data = line.input_tokens.is_some()
            || line.output_tokens.is_some()
            || line.cache_read_tokens.is_some()
            || line.cache_creation_tokens.is_some()
            || line.cache_creation_5m_tokens.is_some()
            || line.cache_creation_1hr_tokens.is_some();
        let should_count_block = match (line.message_id.as_deref(), line.request_id.as_deref()) {
            (Some(msg_id), Some(req_id)) => {
                if has_measurement_data {
                    let key = format!("{}:{}", msg_id, req_id);
                    acc.seen_api_calls.insert(key)
                } else {
                    false
                }
            }
            _ => true,
        };

        if should_count_block {
            accumulate_tokens(line, acc);
        }

        // Track last cache hit time
        if line.cache_read_tokens.map(|v| v > 0).unwrap_or(false)
            || line.cache_creation_tokens.map(|v| v > 0).unwrap_or(false)
        {
            if let Some(ref ts) = line.timestamp {
                acc.last_cache_hit_at = parse_timestamp_to_unix(ts);
            }
        }

        // Accumulate tool counts
        for name in &line.tool_names {
            match name.as_str() {
                "Edit" => acc.tool_counts_edit += 1,
                "Read" => acc.tool_counts_read += 1,
                "Bash" => acc.tool_counts_bash += 1,
                "Write" => acc.tool_counts_write += 1,
                _ => {}
            }
        }

        // Track context window fill
        if line.line_type == LineType::Assistant {
            let turn_input = line.input_tokens.unwrap_or(0)
                + line.cache_read_tokens.unwrap_or(0)
                + line.cache_creation_tokens.unwrap_or(0);
            if turn_input > 0 {
                acc.context_window_tokens = turn_input;
            }
        }

        // Track model (before per-turn cost)
        if let Some(ref model) = line.model {
            acc.model = Some(model.clone());
        }

        // Per-turn cost accumulation
        if should_count_block {
            accumulate_turn_cost(line, acc, &self.pricing);
        }

        // Track git branch, cwd, slug
        if let Some(ref branch) = line.git_branch {
            acc.git_branch = Some(branch.clone());
        }
        if let Some(ref cwd) = line.cwd {
            acc.latest_cwd = Some(cwd.clone());
        }
        if acc.slug.is_none() {
            if let Some(ref s) = line.slug {
                acc.slug = Some(s.clone());
            }
        }
        if acc.entrypoint.is_none() {
            if let Some(ref ep) = line.entrypoint {
                acc.entrypoint = Some(ep.clone());
            }
        }

        // Track AI-generated title
        if let Some(ref title) = line.ai_title {
            acc.ai_title = Some(title.clone());
        }

        // Track user messages
        if line.line_type == LineType::User {
            acc.user_turn_count += 1;
            if !line.is_meta && !line.content_preview.is_empty() {
                if acc.first_user_message.is_empty() {
                    acc.first_user_message = line.content_preview.clone();
                }
                acc.last_user_message = line.content_preview.clone();
            }
            if let Some(ref ide_file) = line.ide_file {
                if acc.at_files.len() < 10 {
                    acc.at_files.insert(ide_file.clone());
                }
            }
            for file in &line.at_files {
                if acc.at_files.len() < 10 {
                    acc.at_files.insert(file.clone());
                }
            }
            for path in &line.pasted_paths {
                if acc.pasted_paths.len() < 10 {
                    acc.pasted_paths.insert(path.clone());
                }
            }
        }

        // Track current turn start time
        if line.line_type == LineType::User
            && !line.is_meta
            && !line.is_tool_result_continuation
            && !line.has_system_prefix
        {
            if let Some(ref ts) = line.timestamp {
                acc.current_turn_started_at = parse_timestamp_to_unix(ts);
            }
        }

        // Track session start time
        if acc.started_at.is_none() {
            if let Some(ref ts) = line.timestamp {
                acc.started_at = parse_timestamp_to_unix(ts);
            }
        }

        // Team name tracking
        if acc.team_name.is_none() {
            if let Some(ref tn) = line.team_name {
                acc.team_name = Some(tn.clone());
            }
        }

        // Sub-agent tracking
        process_sub_agents(line, acc, last_activity_at, &self.pricing);

        // Tool integration tracking (MCP servers + skills)
        for tool_name in &line.tool_names {
            if let Some(rest) = tool_name.strip_prefix("mcp__") {
                if let Some(idx) = rest.find("__") {
                    let server = &rest[..idx];
                    acc.mcp_servers.insert(server.to_string());
                }
            }
        }
        for skill_name in &line.skill_names {
            if !skill_name.is_empty() {
                acc.skills.insert(skill_name.clone());
            }
        }

        // Track compaction events
        if line.is_compact_boundary {
            acc.compact_count += 1;
        }

        // Progress items (TodoWrite, TaskCreate, TaskIdAssignment, TaskUpdate)
        process_progress_items(line, acc);

        // Channel A events
        emit_channel_a_events(line, channel_a_events);

        // Phase classification
        process_phase_classification(line, acc, session_id, &self.dirty_tx);
    }
}
