//! Sub-agent spawn, completion, progress, and notification tracking.

use std::collections::HashMap;

use claude_view_core::pricing::{calculate_cost, ModelPricing, TokenUsage};
use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

use super::super::accumulator::SessionAccumulator;
use super::super::helpers::parse_timestamp_to_unix;

pub(super) fn process_sub_agents(
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
                    // Threads nested cache_creation.{5m,1h} breakdown through so
                    // 1hr-caching costs are charged at the 1hr rate.
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
