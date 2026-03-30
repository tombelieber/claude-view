//! Per-line JSONL processing logic.
//!
//! Handles token accumulation, cost calculation, sub-agent tracking,
//! tool integration tracking, progress items, and phase classification
//! for each parsed JSONL line.

use std::collections::HashMap;

use crate::local_llm::client::{ConversationTurn, Role};
use claude_view_core::live_parser::LineType;
use claude_view_core::phase::scheduler::Priority;
use claude_view_core::phase::{is_shipping_cmd, PhaseLabel, SessionPhase, MAX_PHASE_LABELS};
use claude_view_core::pricing::{calculate_cost, ModelPricing, TokenUsage};
use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

use crate::live::state::HookEvent;

use super::accumulator::SessionAccumulator;
use super::helpers::{
    make_synthesized_event, parse_timestamp_to_unix, resolve_hook_event_from_progress,
};
use super::LiveSessionManager;

impl LiveSessionManager {
    /// Process a single parsed JSONL line, updating the accumulator.
    pub(super) fn process_single_line(
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

// =============================================================================
// Pure helper functions (no &self needed)
// =============================================================================

fn accumulate_tokens(line: &claude_view_core::live_parser::LiveLine, acc: &mut SessionAccumulator) {
    if let Some(input) = line.input_tokens {
        acc.tokens.input_tokens += input;
        acc.tokens.total_tokens += input;
    }
    if let Some(output) = line.output_tokens {
        acc.tokens.output_tokens += output;
        acc.tokens.total_tokens += output;
    }
    if let Some(cache_read) = line.cache_read_tokens {
        acc.tokens.cache_read_tokens += cache_read;
        acc.tokens.total_tokens += cache_read;
    }
    if let Some(cache_creation) = line.cache_creation_tokens {
        acc.tokens.cache_creation_tokens += cache_creation;
        acc.tokens.total_tokens += cache_creation;
    }
    if let Some(tokens_5m) = line.cache_creation_5m_tokens {
        acc.tokens.cache_creation_5m_tokens += tokens_5m;
    }
    if let Some(tokens_1hr) = line.cache_creation_1hr_tokens {
        acc.tokens.cache_creation_1hr_tokens += tokens_1hr;
    }
}

fn accumulate_turn_cost(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
    pricing: &HashMap<String, ModelPricing>,
) {
    let has_tokens = line.input_tokens.is_some()
        || line.output_tokens.is_some()
        || line.cache_read_tokens.is_some()
        || line.cache_creation_tokens.is_some()
        || line.cache_creation_5m_tokens.is_some()
        || line.cache_creation_1hr_tokens.is_some();
    if !has_tokens {
        return;
    }
    let turn_tokens = TokenUsage {
        input_tokens: line.input_tokens.unwrap_or(0),
        output_tokens: line.output_tokens.unwrap_or(0),
        cache_read_tokens: line.cache_read_tokens.unwrap_or(0),
        cache_creation_tokens: line.cache_creation_tokens.unwrap_or(0),
        cache_creation_5m_tokens: line.cache_creation_5m_tokens.unwrap_or(0),
        cache_creation_1hr_tokens: line.cache_creation_1hr_tokens.unwrap_or(0),
        total_tokens: 0,
    };
    let turn_cost = calculate_cost(&turn_tokens, acc.model.as_deref(), pricing);
    acc.accumulated_cost.input_cost_usd += turn_cost.input_cost_usd;
    acc.accumulated_cost.output_cost_usd += turn_cost.output_cost_usd;
    acc.accumulated_cost.cache_read_cost_usd += turn_cost.cache_read_cost_usd;
    acc.accumulated_cost.cache_creation_cost_usd += turn_cost.cache_creation_cost_usd;
    acc.accumulated_cost.cache_savings_usd += turn_cost.cache_savings_usd;
    acc.accumulated_cost.total_usd += turn_cost.total_usd;
    acc.accumulated_cost.unpriced_input_tokens += turn_cost.unpriced_input_tokens;
    acc.accumulated_cost.unpriced_output_tokens += turn_cost.unpriced_output_tokens;
    acc.accumulated_cost.unpriced_cache_read_tokens += turn_cost.unpriced_cache_read_tokens;
    acc.accumulated_cost.unpriced_cache_creation_tokens += turn_cost.unpriced_cache_creation_tokens;
    acc.accumulated_cost.has_unpriced_usage |= turn_cost.has_unpriced_usage;
}

fn process_sub_agents(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
    last_activity_at: i64,
    pricing: &HashMap<String, ModelPricing>,
) {
    // Sub-agent spawn tracking
    for spawn in &line.sub_agent_spawns {
        if spawn.team_name.is_some() {
            continue;
        }
        if acc
            .sub_agents
            .iter()
            .any(|a| a.tool_use_id == spawn.tool_use_id)
        {
            continue;
        }
        let started_at = line
            .timestamp
            .as_deref()
            .and_then(parse_timestamp_to_unix)
            .unwrap_or(last_activity_at);
        acc.sub_agents.push(SubAgentInfo {
            tool_use_id: spawn.tool_use_id.clone(),
            agent_id: None,
            agent_type: spawn.agent_type.clone(),
            description: spawn.description.clone(),
            status: SubAgentStatus::Running,
            started_at,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            model: spawn.model.clone(),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_usd: None,
            current_activity: None,
        });
    }

    // Sub-agent completion tracking
    if let Some(ref result) = line.sub_agent_result {
        if let Some(agent) = acc
            .sub_agents
            .iter_mut()
            .find(|a| a.tool_use_id == result.tool_use_id)
        {
            let terminal_status = match result.status.as_str() {
                "completed" => Some(SubAgentStatus::Complete),
                "failed" | "killed" => Some(SubAgentStatus::Error),
                _ => None,
            };

            agent.agent_id = result.agent_id.clone();

            if let Some(status) = terminal_status {
                agent.status = status;
                agent.completed_at = line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                agent.duration_ms = result.total_duration_ms;
                agent.tool_use_count = result.total_tool_use_count;
                agent.current_activity = None;
                agent.input_tokens = result.usage_input_tokens;
                agent.output_tokens = result.usage_output_tokens;
                agent.cache_read_tokens = result.usage_cache_read_tokens;
                agent.cache_creation_tokens = result.usage_cache_creation_tokens;
                if result.model.is_some() {
                    agent.model = result.model.clone();
                }
                let pricing_model = agent.model.as_deref().or(acc.model.as_deref());
                if let Some(model) = pricing_model {
                    let sub_tokens = TokenUsage {
                        input_tokens: result.usage_input_tokens.unwrap_or(0),
                        output_tokens: result.usage_output_tokens.unwrap_or(0),
                        cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                        cache_creation_tokens: result.usage_cache_creation_tokens.unwrap_or(0),
                        cache_creation_5m_tokens: 0,
                        cache_creation_1hr_tokens: 0,
                        total_tokens: 0,
                    };
                    let sub_cost = calculate_cost(&sub_tokens, Some(model), pricing);
                    if sub_cost.total_usd > 0.0 {
                        agent.cost_usd = Some(sub_cost.total_usd);
                    }
                }
            }
        }
    }

    // Sub-agent progress tracking
    if let Some(ref progress) = line.sub_agent_progress {
        if let Some(agent) = acc
            .sub_agents
            .iter_mut()
            .find(|a| a.tool_use_id == progress.parent_tool_use_id)
        {
            if agent.agent_id.is_none() {
                agent.agent_id = Some(progress.agent_id.clone());
            }
            if agent.status == SubAgentStatus::Running {
                agent.current_activity = progress.current_tool.clone();
            }
        }
    }

    // Background agent completion via <task-notification>
    if let Some(ref notif) = line.sub_agent_notification {
        if let Some(agent) = acc
            .sub_agents
            .iter_mut()
            .find(|a| a.agent_id.as_deref() == Some(&notif.agent_id))
        {
            agent.status = if notif.status == "completed" {
                SubAgentStatus::Complete
            } else {
                SubAgentStatus::Error
            };
            agent.completed_at = line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
            agent.current_activity = None;
        }
    }
}

fn process_progress_items(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
) {
    // TodoWrite: full replacement
    if let Some(ref todos) = line.todo_write {
        use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
        acc.todo_items = todos
            .iter()
            .map(|t| {
                let status = match t.status.as_str() {
                    "in_progress" => ProgressStatus::InProgress,
                    "completed" => ProgressStatus::Completed,
                    _ => ProgressStatus::Pending,
                };
                ProgressItem {
                    id: None,
                    tool_use_id: None,
                    title: t.content.clone(),
                    status,
                    active_form: if t.active_form.is_empty() {
                        None
                    } else {
                        Some(t.active_form.clone())
                    },
                    source: ProgressSource::Todo,
                }
            })
            .collect();
    }

    // TaskCreate: append with dedup guard
    for create in &line.task_creates {
        use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
        if acc
            .task_items
            .iter()
            .any(|t| t.tool_use_id.as_deref() == Some(&create.tool_use_id))
        {
            continue;
        }
        acc.task_items.push(ProgressItem {
            id: None,
            tool_use_id: Some(create.tool_use_id.clone()),
            title: create.subject.clone(),
            status: ProgressStatus::Pending,
            active_form: if create.active_form.is_empty() {
                None
            } else {
                Some(create.active_form.clone())
            },
            source: ProgressSource::Task,
        });
    }

    // TaskIdAssignment: assign system ID
    for assignment in &line.task_id_assignments {
        if let Some(task) = acc
            .task_items
            .iter_mut()
            .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
        {
            task.id = Some(assignment.task_id.clone());
        }
    }

    // TaskUpdate: modify existing task
    for update in &line.task_updates {
        use claude_view_core::progress::ProgressStatus;
        if let Some(task) = acc
            .task_items
            .iter_mut()
            .find(|t| t.id.as_deref() == Some(&update.task_id))
        {
            if let Some(ref s) = update.status {
                task.status = match s.as_str() {
                    "in_progress" => ProgressStatus::InProgress,
                    "completed" => ProgressStatus::Completed,
                    _ => ProgressStatus::Pending,
                };
            }
            if let Some(ref subj) = update.subject {
                task.title = subj.clone();
            }
            if let Some(ref af) = update.active_form {
                task.active_form = Some(af.clone());
            }
        }
    }
}

fn emit_channel_a_events(
    line: &claude_view_core::live_parser::LiveLine,
    channel_a_events: &mut Vec<HookEvent>,
) {
    // Channel A: hook_progress events from JSONL
    if let Some(ref hp) = line.hook_progress {
        channel_a_events.push(resolve_hook_event_from_progress(hp, &line.timestamp));
    }

    // Synthesized events from existing JSONL signals
    if line.line_type == LineType::User
        && !line.is_meta
        && !line.is_tool_result_continuation
        && !line.has_system_prefix
    {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "UserPromptSubmit",
            None,
            "autonomous",
        ));
    }
    if line.is_compact_boundary {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "PreCompact",
            None,
            "autonomous",
        ));
    }
    for spawn in &line.sub_agent_spawns {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "SubagentStart",
            Some(&spawn.agent_type),
            "autonomous",
        ));
    }
    if line.sub_agent_result.is_some() {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "SubagentStop",
            None,
            "autonomous",
        ));
    }
    for tu in &line.task_updates {
        if tu.status.as_deref() == Some("completed") {
            channel_a_events.push(make_synthesized_event(
                &line.timestamp,
                "TaskCompleted",
                None,
                "autonomous",
            ));
        }
    }
}

fn process_phase_classification(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
    session_id: &str,
    dirty_tx: &tokio::sync::mpsc::Sender<(String, Priority)>,
) {
    // Deterministic shipping rule (pre-LLM shortcut)
    for cmd in &line.bash_commands {
        if is_shipping_cmd(cmd) {
            acc.stabilizer.lock_shipping();
            acc.phase_labels.push(PhaseLabel {
                phase: SessionPhase::Shipping,
                confidence: 1.0,
                scope: None,
            });
            if acc.phase_labels.len() > MAX_PHASE_LABELS {
                acc.phase_labels.remove(0);
            }
            break;
        }
    }

    // Accumulate activity signals for classify context
    for cmd in &line.bash_commands {
        if acc.recent_bash_commands.len() >= 5 {
            acc.recent_bash_commands.pop_front();
        }
        acc.recent_bash_commands.push_back(cmd.clone());
    }
    for file in &line.edited_files {
        if acc.recent_edited_files.len() >= 5 {
            acc.recent_edited_files.pop_front();
        }
        acc.recent_edited_files.push_back(file.clone());
    }

    // Accumulate conversation turn (user/assistant only)
    if line.role.as_deref() == Some("assistant") || line.role.as_deref() == Some("user") {
        let role = if line.role.as_deref() == Some("user") {
            Role::User
        } else {
            Role::Assistant
        };
        let turn = ConversationTurn {
            role,
            text: line.content_extended.clone(),
            tools: line.tool_names.clone(),
        };
        if acc.message_buf.len() >= 15 {
            acc.message_buf.pop_front();
        }
        acc.message_buf.push_back(turn);
    }

    // Mark session dirty on ANY line — drain loop handles debounce/scheduling.
    // Context is built from accumulator at drain time (freshest data).
    let priority = if acc.phase_labels.is_empty() {
        Priority::New
    } else if acc.stabilizer.displayed_phase().is_none() {
        Priority::Transition
    } else {
        Priority::Steady
    };
    let _ = dirty_tx.try_send((session_id.to_string(), priority));
}
