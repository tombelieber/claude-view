//! Core state machine: processing a single JSONL line and accumulating session data.

use std::collections::HashMap;

use crate::live_parser::LineType;
use crate::pricing::{calculate_cost, ModelPricing, TokenUsage};
use crate::progress::{ProgressItem, ProgressSource, ProgressStatus};
use crate::subagent::{SubAgentInfo, SubAgentStatus};

use super::helpers::parse_timestamp_to_unix;
use super::types::SessionAccumulator;

impl SessionAccumulator {
    /// Process a single parsed JSONL line, accumulating tokens, sub-agents,
    /// progress items, and other session metadata.
    ///
    /// `last_activity_at` is a fallback Unix timestamp (e.g. file mtime) used
    /// when the line itself has no timestamp.
    pub fn process_line(
        &mut self,
        line: &crate::live_parser::LiveLine,
        last_activity_at: i64,
        pricing: &HashMap<String, ModelPricing>,
    ) {
        // -----------------------------------------------------------------
        // Content-block dedup: Claude Code writes one JSONL line per content
        // block (thinking, text, tool_use), each carrying the full message-level
        // usage. Count tokens/cost only once per API response. Important: do NOT
        // mark a response as seen if this block has no usage/cost fields, so a
        // later block with actual usage is still counted.
        // -----------------------------------------------------------------
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
                    self.seen_api_calls.insert(key) // true if newly inserted
                } else {
                    false
                }
            }
            _ => true, // no IDs available -- count it (legacy data / live streaming)
        };

        // -----------------------------------------------------------------
        // Model tracking (must happen BEFORE per-turn cost so model is current,
        // and outside the dedup guard since we want the latest model always)
        // -----------------------------------------------------------------
        if let Some(ref model) = line.model {
            self.model = Some(model.clone());
        }

        // -----------------------------------------------------------------
        // Context window fill from latest assistant turn (always update,
        // even on duplicate blocks -- we want the latest context window size)
        // -----------------------------------------------------------------
        if line.line_type == LineType::Assistant {
            let turn_input = line.input_tokens.unwrap_or(0)
                + line.cache_read_tokens.unwrap_or(0)
                + line.cache_creation_tokens.unwrap_or(0);
            if turn_input > 0 {
                self.context_window_tokens = turn_input;
            }
        }

        // -----------------------------------------------------------------
        // Track last cache hit time (always update for accurate cache status)
        // -----------------------------------------------------------------
        if line.cache_read_tokens.map(|v| v > 0).unwrap_or(false)
            || line.cache_creation_tokens.map(|v| v > 0).unwrap_or(false)
        {
            if let Some(ref ts) = line.timestamp {
                self.last_cache_hit_at = parse_timestamp_to_unix(ts);
            }
        }

        // -----------------------------------------------------------------
        // Token + cost accumulation -- guarded by dedup
        // -----------------------------------------------------------------
        if should_count_block {
            self.accumulate_tokens(line);
            self.accumulate_cost(line, pricing);
        }

        // -----------------------------------------------------------------
        // Metadata tracking
        // -----------------------------------------------------------------
        self.track_metadata(line);

        // -----------------------------------------------------------------
        // Sub-agent tracking
        // -----------------------------------------------------------------
        self.track_sub_agents(line, last_activity_at, pricing);

        // -----------------------------------------------------------------
        // Progress tracking (TodoWrite + Tasks)
        // -----------------------------------------------------------------
        self.track_progress(line);

        // TODO: wire oMLX classification (Task 8)
    }

    /// Accumulate raw token counts from a line.
    fn accumulate_tokens(&mut self, line: &crate::live_parser::LiveLine) {
        if let Some(input) = line.input_tokens {
            self.tokens.input_tokens += input;
            self.tokens.total_tokens += input;
        }
        if let Some(output) = line.output_tokens {
            self.tokens.output_tokens += output;
            self.tokens.total_tokens += output;
        }
        if let Some(cache_read) = line.cache_read_tokens {
            self.tokens.cache_read_tokens += cache_read;
            self.tokens.total_tokens += cache_read;
        }
        if let Some(cache_creation) = line.cache_creation_tokens {
            self.tokens.cache_creation_tokens += cache_creation;
            self.tokens.total_tokens += cache_creation;
        }
        // Split cache creation by TTL
        if let Some(tokens_5m) = line.cache_creation_5m_tokens {
            self.tokens.cache_creation_5m_tokens += tokens_5m;
        }
        if let Some(tokens_1hr) = line.cache_creation_1hr_tokens {
            self.tokens.cache_creation_1hr_tokens += tokens_1hr;
        }
    }

    /// Accumulate per-turn cost from a line.
    fn accumulate_cost(
        &mut self,
        line: &crate::live_parser::LiveLine,
        pricing: &HashMap<String, ModelPricing>,
    ) {
        let has_tokens = line.input_tokens.is_some()
            || line.output_tokens.is_some()
            || line.cache_read_tokens.is_some()
            || line.cache_creation_tokens.is_some()
            || line.cache_creation_5m_tokens.is_some()
            || line.cache_creation_1hr_tokens.is_some();
        if has_tokens {
            let turn_tokens = TokenUsage {
                input_tokens: line.input_tokens.unwrap_or(0),
                output_tokens: line.output_tokens.unwrap_or(0),
                cache_read_tokens: line.cache_read_tokens.unwrap_or(0),
                cache_creation_tokens: line.cache_creation_tokens.unwrap_or(0),
                cache_creation_5m_tokens: line.cache_creation_5m_tokens.unwrap_or(0),
                cache_creation_1hr_tokens: line.cache_creation_1hr_tokens.unwrap_or(0),
                total_tokens: 0, // unused by calculate_cost
            };
            let turn_cost = calculate_cost(&turn_tokens, self.model.as_deref(), pricing);
            self.accumulated_cost.input_cost_usd += turn_cost.input_cost_usd;
            self.accumulated_cost.output_cost_usd += turn_cost.output_cost_usd;
            self.accumulated_cost.cache_read_cost_usd += turn_cost.cache_read_cost_usd;
            self.accumulated_cost.cache_creation_cost_usd += turn_cost.cache_creation_cost_usd;
            self.accumulated_cost.cache_savings_usd += turn_cost.cache_savings_usd;
            self.accumulated_cost.total_usd += turn_cost.total_usd;
            self.accumulated_cost.unpriced_input_tokens += turn_cost.unpriced_input_tokens;
            self.accumulated_cost.unpriced_output_tokens += turn_cost.unpriced_output_tokens;
            self.accumulated_cost.unpriced_cache_read_tokens +=
                turn_cost.unpriced_cache_read_tokens;
            self.accumulated_cost.unpriced_cache_creation_tokens +=
                turn_cost.unpriced_cache_creation_tokens;
            self.accumulated_cost.has_unpriced_usage |= turn_cost.has_unpriced_usage;
        }
    }

    /// Track git branch, slug, user messages, started_at, and team_name.
    fn track_metadata(&mut self, line: &crate::live_parser::LiveLine) {
        // Git branch tracking (from user messages)
        if let Some(ref branch) = line.git_branch {
            self.git_branch = Some(branch.clone());
        }

        // Slug tracking (set-once: first line wins)
        if self.slug.is_none() {
            if let Some(ref s) = line.slug {
                self.slug = Some(s.clone());
            }
        }

        // User message tracking (skip meta messages for content)
        if line.line_type == LineType::User {
            self.user_turn_count += 1;
            if !line.is_meta && !line.content_preview.is_empty() {
                if self.first_user_message.is_empty() {
                    self.first_user_message = line.content_preview.clone();
                }
                self.last_user_message = line.content_preview.clone();
            }
        }

        // Session start time from first timestamp
        if self.started_at.is_none() {
            if let Some(ref ts) = line.timestamp {
                self.started_at = parse_timestamp_to_unix(ts);
            }
        }

        // Team name tracking (from top-level `teamName` JSONL field)
        if self.team_name.is_none() {
            if let Some(ref tn) = line.team_name {
                self.team_name = Some(tn.clone());
            }
        }
    }

    /// Track sub-agent spawn, completion, progress, and notification events.
    fn track_sub_agents(
        &mut self,
        line: &crate::live_parser::LiveLine,
        last_activity_at: i64,
        pricing: &HashMap<String, ModelPricing>,
    ) {
        // Sub-agent spawn tracking
        for spawn in &line.sub_agent_spawns {
            // Team spawns are NOT sub-agents -- their lifecycle is managed by
            // ~/.claude/teams/, not the JSONL sub-agent tracking system.
            if spawn.team_name.is_some() {
                continue;
            }

            // Dedup guard: skip if we already know this tool_use_id
            if self
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

            self.sub_agents.push(SubAgentInfo {
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
            if let Some(agent) = self
                .sub_agents
                .iter_mut()
                .find(|a| a.tool_use_id == result.tool_use_id)
            {
                // Whitelist known terminal statuses. Everything else
                // (async_launched, teammate_spawned, queued, or any future
                // non-terminal status) means the agent is still running --
                // just capture the agentId and keep Running.
                let terminal_status = match result.status.as_str() {
                    "completed" => Some(SubAgentStatus::Complete),
                    "failed" | "killed" => Some(SubAgentStatus::Error),
                    _ => None, // non-terminal: still running
                };

                agent.agent_id = result.agent_id.clone();

                if let Some(status) = terminal_status {
                    agent.status = status;
                    agent.completed_at =
                        line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                    agent.duration_ms = result.total_duration_ms;
                    agent.tool_use_count = result.total_tool_use_count;
                    agent.current_activity = None;

                    // Compute cost from sub-agent token usage via pricing table.
                    // Threads nested cache_creation.{5m,1h} breakdown through so
                    // 1hr-caching costs are charged at the 1hr rate.
                    if let Some(model) = self.model.as_deref() {
                        let sub_tokens = TokenUsage {
                            input_tokens: result.usage_input_tokens.unwrap_or(0),
                            output_tokens: result.usage_output_tokens.unwrap_or(0),
                            cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                            cache_creation_tokens: result.usage_cache_creation_tokens.unwrap_or(0),
                            cache_creation_5m_tokens: result
                                .usage_cache_creation_5m_tokens
                                .unwrap_or(0),
                            cache_creation_1hr_tokens: result
                                .usage_cache_creation_1hr_tokens
                                .unwrap_or(0),
                            total_tokens: 0, // not used by calculate_cost
                        };
                        let sub_cost = calculate_cost(&sub_tokens, Some(model), pricing);
                        if sub_cost.total_usd > 0.0 {
                            agent.cost_usd = Some(sub_cost.total_usd);
                        }
                    }
                }
            }
            // If no matching spawn found, ignore gracefully (orphaned tool_result)
        }

        // Sub-agent progress tracking (early agentId + current activity)
        if let Some(ref progress) = line.sub_agent_progress {
            if let Some(agent) = self
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
            if let Some(agent) = self
                .sub_agents
                .iter_mut()
                .find(|a| a.agent_id.as_deref() == Some(&notif.agent_id))
            {
                agent.status = if notif.status == "completed" {
                    SubAgentStatus::Complete
                } else {
                    // "failed", "killed", or any other terminal status
                    SubAgentStatus::Error
                };
                agent.completed_at = line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                agent.current_activity = None;
            }
        }
    }

    /// Track TodoWrite full replacement and Task create/update/id-assignment.
    fn track_progress(&mut self, line: &crate::live_parser::LiveLine) {
        // TodoWrite: full replacement
        if let Some(ref todos) = line.todo_write {
            self.todo_items = todos
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
            if self
                .task_items
                .iter()
                .any(|t| t.tool_use_id.as_deref() == Some(&create.tool_use_id))
            {
                continue; // Already seen this create (replay resilience)
            }
            self.task_items.push(ProgressItem {
                id: None, // Assigned later by TaskIdAssignment
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
            if let Some(task) = self
                .task_items
                .iter_mut()
                .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
            {
                task.id = Some(assignment.task_id.clone());
            }
        }

        // TaskUpdate: modify existing task
        for update in &line.task_updates {
            if let Some(task) = self
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
}
