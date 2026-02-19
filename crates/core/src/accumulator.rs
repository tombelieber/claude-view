//! Unified session accumulator for both live (streaming) and history (batch) JSONL parsing.
//!
//! `SessionAccumulator` extracts the same rich session data regardless of whether
//! lines arrive one-at-a-time via live tailing or all-at-once via batch file reads.
//! This ensures live monitoring and historical session views produce identical data.

use std::collections::HashMap;
use std::path::Path;

use crate::live_parser::{parse_tail, LineType, TailFinders};
use crate::pricing::{calculate_cost, CacheStatus, CostBreakdown, ModelPricing, TokenUsage};
use crate::progress::{ProgressItem, ProgressSource, ProgressStatus};
use crate::subagent::{SubAgentInfo, SubAgentStatus};

/// Accumulated per-session state -- shared between live monitoring and history batch parsing.
///
/// Feed lines via [`process_line`](SessionAccumulator::process_line), then call
/// [`finish`](SessionAccumulator::finish) to produce the final [`RichSessionData`].
pub struct SessionAccumulator {
    pub tokens: TokenUsage,
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub user_turn_count: u32,
    pub first_user_message: String,
    pub last_user_message: String,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub sub_agents: Vec<SubAgentInfo>,
    pub todo_items: Vec<ProgressItem>,
    pub task_items: Vec<ProgressItem>,
    pub last_cache_hit_at: Option<i64>,
}

/// Rich session data -- output of accumulation. Same shape for live and history.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichSessionData {
    pub tokens: TokenUsage,
    pub cost: CostBreakdown,
    pub cache_status: CacheStatus,
    pub sub_agents: Vec<SubAgentInfo>,
    pub progress_items: Vec<ProgressItem>,
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub turn_count: u32,
    pub first_user_message: Option<String>,
    pub last_user_message: Option<String>,
    pub last_cache_hit_at: Option<i64>,
}

impl SessionAccumulator {
    /// Create a zero-initialized accumulator.
    pub fn new() -> Self {
        Self {
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            model: None,
            user_turn_count: 0,
            first_user_message: String::new(),
            last_user_message: String::new(),
            git_branch: None,
            started_at: None,
            sub_agents: Vec::new(),
            todo_items: Vec::new(),
            task_items: Vec::new(),
            last_cache_hit_at: None,
        }
    }

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
        // Token accumulation (cumulative, for cost calculation)
        // -----------------------------------------------------------------
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

        // -----------------------------------------------------------------
        // Track last cache hit time from Anthropic API response tokens
        // -----------------------------------------------------------------
        if line.cache_read_tokens.map(|v| v > 0).unwrap_or(false)
            || line
                .cache_creation_tokens
                .map(|v| v > 0)
                .unwrap_or(false)
        {
            if let Some(ref ts) = line.timestamp {
                self.last_cache_hit_at = parse_timestamp_to_unix(ts);
            }
        }

        // -----------------------------------------------------------------
        // Context window fill from latest assistant turn
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
        // Model tracking
        // -----------------------------------------------------------------
        if let Some(ref model) = line.model {
            self.model = Some(model.clone());
        }

        // -----------------------------------------------------------------
        // Git branch tracking (from user messages)
        // -----------------------------------------------------------------
        if let Some(ref branch) = line.git_branch {
            self.git_branch = Some(branch.clone());
        }

        // -----------------------------------------------------------------
        // User message tracking (skip meta messages for content)
        // -----------------------------------------------------------------
        if line.line_type == LineType::User {
            self.user_turn_count += 1;
            if !line.is_meta && !line.content_preview.is_empty() {
                if self.first_user_message.is_empty() {
                    self.first_user_message = line.content_preview.clone();
                }
                self.last_user_message = line.content_preview.clone();
            }
        }

        // -----------------------------------------------------------------
        // Session start time from first timestamp
        // -----------------------------------------------------------------
        if self.started_at.is_none() {
            if let Some(ref ts) = line.timestamp {
                self.started_at = parse_timestamp_to_unix(ts);
            }
        }

        // -----------------------------------------------------------------
        // Sub-agent spawn tracking
        // -----------------------------------------------------------------
        for spawn in &line.sub_agent_spawns {
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
                cost_usd: None,
                current_activity: None,
            });
        }

        // -----------------------------------------------------------------
        // Sub-agent completion tracking
        // -----------------------------------------------------------------
        if let Some(ref result) = line.sub_agent_result {
            if let Some(agent) = self
                .sub_agents
                .iter_mut()
                .find(|a| a.tool_use_id == result.tool_use_id)
            {
                agent.status = if result.status == "completed" {
                    SubAgentStatus::Complete
                } else {
                    SubAgentStatus::Error
                };
                agent.agent_id = result.agent_id.clone();
                agent.completed_at = line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                agent.duration_ms = result.total_duration_ms;
                agent.tool_use_count = result.total_tool_use_count;
                agent.current_activity = None;

                // Compute cost from sub-agent token usage via pricing table
                if let Some(model) = self.model.as_deref() {
                    let sub_tokens = TokenUsage {
                        input_tokens: result.usage_input_tokens.unwrap_or(0),
                        output_tokens: result.usage_output_tokens.unwrap_or(0),
                        cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                        cache_creation_tokens: result.usage_cache_creation_tokens.unwrap_or(0),
                        cache_creation_5m_tokens: 0,
                        cache_creation_1hr_tokens: 0,
                        total_tokens: 0, // not used by calculate_cost
                    };
                    let sub_cost = calculate_cost(&sub_tokens, Some(model), pricing);
                    if sub_cost.total_usd > 0.0 {
                        agent.cost_usd = Some(sub_cost.total_usd);
                    }
                }
            }
            // If no matching spawn found, ignore gracefully (orphaned tool_result)
        }

        // -----------------------------------------------------------------
        // Sub-agent progress tracking (early agentId + current activity)
        // -----------------------------------------------------------------
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

        // -----------------------------------------------------------------
        // TodoWrite: full replacement
        // -----------------------------------------------------------------
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

        // -----------------------------------------------------------------
        // TaskCreate: append with dedup guard
        // -----------------------------------------------------------------
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

        // -----------------------------------------------------------------
        // TaskIdAssignment: assign system ID
        // -----------------------------------------------------------------
        for assignment in &line.task_id_assignments {
            if let Some(task) = self
                .task_items
                .iter_mut()
                .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
            {
                task.id = Some(assignment.task_id.clone());
            }
        }

        // -----------------------------------------------------------------
        // TaskUpdate: modify existing task
        // -----------------------------------------------------------------
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

    /// Finalize accumulation: calculate cost, derive cache status, combine
    /// progress items, and return the complete [`RichSessionData`].
    pub fn finish(&self, pricing: &HashMap<String, ModelPricing>) -> RichSessionData {
        let cost = calculate_cost(&self.tokens, self.model.as_deref(), pricing);
        let cache_status = derive_cache_status(self.last_cache_hit_at);

        let mut progress_items = self.todo_items.clone();
        progress_items.extend(self.task_items.clone());

        RichSessionData {
            tokens: self.tokens.clone(),
            cost,
            cache_status,
            sub_agents: self.sub_agents.clone(),
            progress_items,
            context_window_tokens: self.context_window_tokens,
            model: self.model.clone(),
            git_branch: self.git_branch.clone(),
            turn_count: self.user_turn_count,
            first_user_message: if self.first_user_message.is_empty() {
                None
            } else {
                Some(self.first_user_message.clone())
            },
            last_user_message: if self.last_user_message.is_empty() {
                None
            } else {
                Some(self.last_user_message.clone())
            },
            last_cache_hit_at: self.last_cache_hit_at,
        }
    }

    /// Convenience: read a JSONL file from offset 0, parse all lines, and
    /// return the accumulated [`RichSessionData`].
    ///
    /// Uses synchronous I/O internally (suitable for `spawn_blocking`).
    pub fn from_file(
        path: &Path,
        pricing: &HashMap<String, ModelPricing>,
    ) -> std::io::Result<RichSessionData> {
        let finders = TailFinders::new();
        let (lines, _offset) = parse_tail(path, 0, &finders)?;

        // Derive a fallback activity timestamp from the file's mtime
        let last_activity_at = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0);

        let mut acc = Self::new();
        for line in &lines {
            acc.process_line(line, last_activity_at, pricing);
        }
        Ok(acc.finish(pricing))
    }
}

impl Default for SessionAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Private helpers
// =============================================================================

/// Derive cache status from the last cache hit timestamp.
///
/// Warm if the last cache activity was within 5 minutes, Cold otherwise.
fn derive_cache_status(last_cache_hit_at: Option<i64>) -> CacheStatus {
    match last_cache_hit_at {
        Some(ts) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            if (now - ts) < 300 {
                CacheStatus::Warm
            } else {
                CacheStatus::Cold
            }
        }
        None => CacheStatus::Unknown,
    }
}

/// Parse an ISO 8601 / RFC 3339 timestamp to a Unix epoch second.
fn parse_timestamp_to_unix(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp())
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc().timestamp())
        })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_parser::LiveLine;

    /// Helper: build a minimal LiveLine with defaults.
    fn empty_line() -> LiveLine {
        LiveLine {
            line_type: LineType::Other,
            role: None,
            content_preview: String::new(),
            tool_names: Vec::new(),
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cache_creation_5m_tokens: None,
            cache_creation_1hr_tokens: None,
            timestamp: None,
            stop_reason: None,
            git_branch: None,
            is_meta: false,
            is_tool_result_continuation: false,
            has_system_prefix: false,
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
            sub_agent_progress: None,
            todo_write: None,
            task_creates: Vec::new(),
            task_updates: Vec::new(),
            task_id_assignments: Vec::new(),
        }
    }

    #[test]
    fn test_accumulate_empty() {
        let acc = SessionAccumulator::new();
        let pricing = HashMap::new();
        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.total_tokens, 0);
        assert_eq!(data.turn_count, 0);
        assert!(data.cost.total_usd == 0.0);
        assert_eq!(data.cache_status, CacheStatus::Unknown);
        assert!(data.first_user_message.is_none());
        assert!(data.last_user_message.is_none());
        assert!(data.model.is_none());
        assert!(data.sub_agents.is_empty());
        assert!(data.progress_items.is_empty());
    }

    #[test]
    fn test_token_accumulation() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(1000);
        line1.output_tokens = Some(500);
        line1.cache_read_tokens = Some(200);
        line1.cache_creation_tokens = Some(50);

        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(2000);
        line2.output_tokens = Some(800);

        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.input_tokens, 3000);
        assert_eq!(data.tokens.output_tokens, 1300);
        assert_eq!(data.tokens.cache_read_tokens, 200);
        assert_eq!(data.tokens.cache_creation_tokens, 50);
        assert_eq!(data.tokens.total_tokens, 3000 + 1300 + 200 + 50);
    }

    #[test]
    fn test_cache_creation_split_accumulation() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.cache_creation_5m_tokens = Some(100);
        line.cache_creation_1hr_tokens = Some(500);

        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.cache_creation_5m_tokens, 100);
        assert_eq!(data.tokens.cache_creation_1hr_tokens, 500);
    }

    #[test]
    fn test_model_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.model = Some("claude-sonnet-4-6".to_string());
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.model = Some("claude-opus-4-6".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Last model seen wins
        assert_eq!(data.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn test_user_message_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Meta message should be skipped for content
        let mut meta = empty_line();
        meta.line_type = LineType::User;
        meta.is_meta = true;
        meta.content_preview = "system stuff".to_string();
        acc.process_line(&meta, 0, &pricing);

        // First real user message
        let mut first = empty_line();
        first.line_type = LineType::User;
        first.content_preview = "Fix the bug".to_string();
        acc.process_line(&first, 0, &pricing);

        // Second real user message
        let mut second = empty_line();
        second.line_type = LineType::User;
        second.content_preview = "Now add tests".to_string();
        acc.process_line(&second, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.turn_count, 3); // meta counts as a turn
        assert_eq!(data.first_user_message.as_deref(), Some("Fix the bug"));
        assert_eq!(data.last_user_message.as_deref(), Some("Now add tests"));
    }

    #[test]
    fn test_git_branch_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.git_branch = Some("feature/auth".to_string());
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.git_branch.as_deref(), Some("feature/auth"));
    }

    #[test]
    fn test_context_window_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // First assistant turn
        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(5000);
        line1.cache_read_tokens = Some(3000);
        line1.cache_creation_tokens = Some(1000);
        acc.process_line(&line1, 0, &pricing);

        assert_eq!(acc.context_window_tokens, 9000); // 5000+3000+1000

        // Second assistant turn with larger context
        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(10000);
        line2.cache_read_tokens = Some(5000);
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Last assistant turn's context window
        assert_eq!(data.context_window_tokens, 15000); // 10000+5000
    }

    #[test]
    fn test_started_at_from_first_timestamp() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.timestamp = Some("2026-02-20T10:00:00Z".to_string());
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.timestamp = Some("2026-02-20T10:05:00Z".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Should capture the first timestamp only
        assert!(data.tokens.total_tokens == 0); // no tokens in these lines
        let started = acc.started_at.unwrap();
        assert!(started > 0);
        // Second line should not overwrite started_at
        let ts1 = parse_timestamp_to_unix("2026-02-20T10:00:00Z").unwrap();
        assert_eq!(started, ts1);
    }

    #[test]
    fn test_sub_agent_spawn_and_completion() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line.timestamp = Some("2026-02-20T10:00:00Z".to_string());
        spawn_line.sub_agent_spawns.push(crate::live_parser::SubAgentSpawn {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_type: "Explore".to_string(),
            description: "Search codebase".to_string(),
        });
        acc.process_line(&spawn_line, 0, &pricing);

        assert_eq!(acc.sub_agents.len(), 1);
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);

        // Completion
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.timestamp = Some("2026-02-20T10:01:00Z".to_string());
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: Some("a951849".to_string()),
            status: "completed".to_string(),
            total_duration_ms: Some(60000),
            total_tool_use_count: Some(15),
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.sub_agents.len(), 1);
        assert_eq!(data.sub_agents[0].status, SubAgentStatus::Complete);
        assert_eq!(data.sub_agents[0].agent_id.as_deref(), Some("a951849"));
        assert_eq!(data.sub_agents[0].duration_ms, Some(60000));
        assert_eq!(data.sub_agents[0].tool_use_count, Some(15));
    }

    #[test]
    fn test_sub_agent_dedup() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.sub_agent_spawns.push(crate::live_parser::SubAgentSpawn {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_type: "Explore".to_string(),
            description: "Search".to_string(),
        });

        // Process same spawn twice (replay resilience)
        acc.process_line(&line, 0, &pricing);
        acc.process_line(&line, 0, &pricing);

        assert_eq!(acc.sub_agents.len(), 1);
    }

    #[test]
    fn test_sub_agent_progress() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line.sub_agent_spawns.push(crate::live_parser::SubAgentSpawn {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_type: "Explore".to_string(),
            description: "Search".to_string(),
        });
        acc.process_line(&spawn_line, 0, &pricing);

        // Progress event
        let mut progress_line = empty_line();
        progress_line.line_type = LineType::Progress;
        progress_line.sub_agent_progress = Some(crate::live_parser::SubAgentProgress {
            parent_tool_use_id: "toolu_01ABC".to_string(),
            agent_id: "a951849".to_string(),
            current_tool: Some("Read".to_string()),
        });
        acc.process_line(&progress_line, 0, &pricing);

        assert_eq!(acc.sub_agents[0].agent_id.as_deref(), Some("a951849"));
        assert_eq!(
            acc.sub_agents[0].current_activity.as_deref(),
            Some("Read")
        );
    }

    #[test]
    fn test_todo_write_full_replacement() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTodoItem;

        // First TodoWrite
        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.todo_write = Some(vec![
            RawTodoItem {
                content: "Step 1".to_string(),
                status: "completed".to_string(),
                active_form: String::new(),
            },
            RawTodoItem {
                content: "Step 2".to_string(),
                status: "pending".to_string(),
                active_form: "Working on step 2".to_string(),
            },
        ]);
        acc.process_line(&line1, 0, &pricing);
        assert_eq!(acc.todo_items.len(), 2);

        // Second TodoWrite replaces all
        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.todo_write = Some(vec![RawTodoItem {
            content: "New plan".to_string(),
            status: "in_progress".to_string(),
            active_form: String::new(),
        }]);
        acc.process_line(&line2, 0, &pricing);
        assert_eq!(acc.todo_items.len(), 1);
        assert_eq!(acc.todo_items[0].title, "New plan");
        assert_eq!(acc.todo_items[0].status, ProgressStatus::InProgress);
    }

    #[test]
    fn test_task_create_and_update() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate};

        // TaskCreate
        let mut create_line = empty_line();
        create_line.line_type = LineType::Assistant;
        create_line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_task1".to_string(),
            subject: "Write tests".to_string(),
            description: "Add unit tests".to_string(),
            active_form: "Writing tests".to_string(),
        });
        acc.process_line(&create_line, 0, &pricing);
        assert_eq!(acc.task_items.len(), 1);
        assert!(acc.task_items[0].id.is_none()); // no ID yet

        // TaskIdAssignment
        let mut assign_line = empty_line();
        assign_line.line_type = LineType::User;
        assign_line.task_id_assignments.push(RawTaskIdAssignment {
            tool_use_id: "toolu_task1".to_string(),
            task_id: "1".to_string(),
        });
        acc.process_line(&assign_line, 0, &pricing);
        assert_eq!(acc.task_items[0].id.as_deref(), Some("1"));

        // TaskUpdate
        let mut update_line = empty_line();
        update_line.line_type = LineType::Assistant;
        update_line.task_updates.push(RawTaskUpdate {
            task_id: "1".to_string(),
            status: Some("completed".to_string()),
            subject: None,
            active_form: None,
        });
        acc.process_line(&update_line, 0, &pricing);
        assert_eq!(acc.task_items[0].status, ProgressStatus::Completed);
    }

    #[test]
    fn test_task_create_dedup() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTaskCreate;

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_task1".to_string(),
            subject: "Task 1".to_string(),
            description: "".to_string(),
            active_form: "".to_string(),
        });

        acc.process_line(&line, 0, &pricing);
        acc.process_line(&line, 0, &pricing);

        assert_eq!(acc.task_items.len(), 1);
    }

    #[test]
    fn test_finish_combines_todo_and_task_items() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTaskCreate;
        use crate::progress::RawTodoItem;

        // Add a todo
        let mut todo_line = empty_line();
        todo_line.line_type = LineType::Assistant;
        todo_line.todo_write = Some(vec![RawTodoItem {
            content: "Todo item".to_string(),
            status: "pending".to_string(),
            active_form: String::new(),
        }]);
        acc.process_line(&todo_line, 0, &pricing);

        // Add a task
        let mut task_line = empty_line();
        task_line.line_type = LineType::Assistant;
        task_line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_1".to_string(),
            subject: "Task item".to_string(),
            description: "".to_string(),
            active_form: "".to_string(),
        });
        acc.process_line(&task_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.progress_items.len(), 2);
        assert_eq!(data.progress_items[0].source, ProgressSource::Todo);
        assert_eq!(data.progress_items[1].source, ProgressSource::Task);
    }

    #[test]
    fn test_cost_calculation_with_known_model() {
        let mut acc = SessionAccumulator::new();
        let pricing = crate::pricing::default_pricing();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.model = Some("claude-sonnet-4-6".to_string());
        line.input_tokens = Some(1_000_000);
        line.output_tokens = Some(100_000);
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert!(!data.cost.is_estimated);
        assert!(data.cost.total_usd > 0.0);
    }

    #[test]
    fn test_cost_calculation_with_unknown_model() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new(); // no pricing data

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.model = Some("unknown-model-xyz".to_string());
        line.input_tokens = Some(1000);
        line.output_tokens = Some(500);
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert!(data.cost.is_estimated);
        assert!(data.cost.total_usd > 0.0);
    }

    #[test]
    fn test_cache_status_unknown_no_activity() {
        let acc = SessionAccumulator::new();
        let pricing = HashMap::new();
        let data = acc.finish(&pricing);
        assert_eq!(data.cache_status, CacheStatus::Unknown);
    }

    #[test]
    fn test_cache_status_cold_old_timestamp() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Use a timestamp far in the past
        let mut line = empty_line();
        line.cache_read_tokens = Some(100);
        line.timestamp = Some("2020-01-01T00:00:00Z".to_string());
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.cache_status, CacheStatus::Cold);
    }

    #[test]
    fn test_parse_timestamp_to_unix_rfc3339() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00Z");
        assert!(ts.is_some());
        assert!(ts.unwrap() > 0);
    }

    #[test]
    fn test_parse_timestamp_to_unix_with_offset() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00+08:00");
        assert!(ts.is_some());
    }

    #[test]
    fn test_parse_timestamp_to_unix_fallback_format() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00");
        assert!(ts.is_some());
    }

    #[test]
    fn test_parse_timestamp_to_unix_invalid() {
        let ts = parse_timestamp_to_unix("not-a-timestamp");
        assert!(ts.is_none());
    }

    #[test]
    fn test_derive_cache_status_none() {
        assert_eq!(derive_cache_status(None), CacheStatus::Unknown);
    }

    #[test]
    fn test_derive_cache_status_recent() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 30 seconds ago = warm
        assert_eq!(derive_cache_status(Some(now - 30)), CacheStatus::Warm);
    }

    #[test]
    fn test_derive_cache_status_old() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 10 minutes ago = cold
        assert_eq!(derive_cache_status(Some(now - 600)), CacheStatus::Cold);
    }

    #[test]
    fn test_from_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::File::create(&path).unwrap();

        let pricing = HashMap::new();
        let data = SessionAccumulator::from_file(&path, &pricing).unwrap();
        assert_eq!(data.tokens.total_tokens, 0);
        assert_eq!(data.turn_count, 0);
    }

    #[test]
    fn test_from_file_with_content() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello"}},"gitBranch":"main","timestamp":"2026-02-20T10:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"Hi there","model":"claude-sonnet-4-6","usage":{{"input_tokens":1000,"output_tokens":200}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let pricing = crate::pricing::default_pricing();
        let data = SessionAccumulator::from_file(&path, &pricing).unwrap();
        assert_eq!(data.turn_count, 1);
        assert_eq!(data.first_user_message.as_deref(), Some("Hello"));
        assert_eq!(data.git_branch.as_deref(), Some("main"));
        assert_eq!(data.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(data.tokens.input_tokens, 1000);
        assert_eq!(data.tokens.output_tokens, 200);
        assert!(!data.cost.is_estimated);
    }

    #[test]
    fn test_sub_agent_cost_calculation() {
        let mut acc = SessionAccumulator::new();
        let mut pricing = crate::pricing::default_pricing();
        crate::pricing::fill_tiering_gaps(&mut pricing);

        // Set model first
        let mut model_line = empty_line();
        model_line.model = Some("claude-sonnet-4-6".to_string());
        acc.process_line(&model_line, 0, &pricing);

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line.sub_agent_spawns.push(crate::live_parser::SubAgentSpawn {
            tool_use_id: "toolu_cost".to_string(),
            agent_type: "code".to_string(),
            description: "Write code".to_string(),
        });
        acc.process_line(&spawn_line, 0, &pricing);

        // Complete with token usage
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_cost".to_string(),
            agent_id: Some("abc1234".to_string()),
            status: "completed".to_string(),
            total_duration_ms: Some(5000),
            total_tool_use_count: Some(3),
            usage_input_tokens: Some(100_000),
            usage_output_tokens: Some(10_000),
            usage_cache_read_tokens: Some(50_000),
            usage_cache_creation_tokens: Some(5_000),
        });
        acc.process_line(&result_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.sub_agents.len(), 1);
        assert!(
            data.sub_agents[0].cost_usd.unwrap() > 0.0,
            "Sub-agent should have a non-zero cost"
        );
    }
}
