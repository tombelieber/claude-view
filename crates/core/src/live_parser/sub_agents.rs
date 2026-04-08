//! Sub-agent extraction: spawn, progress, notification, result, and tool-use result parsing.

use super::types::{
    increment_progress_message_content_fallback_count, SubAgentProgress, SubAgentResult,
    SubAgentSpawn,
};

/// Internal representation of a parsed toolUseResult payload.
#[derive(Debug, Default, Clone)]
pub(crate) struct ToolUseResultPayload {
    pub agent_id: Option<String>,
    pub status: Option<String>,
    pub total_duration_ms: Option<u64>,
    pub total_tool_use_count: Option<u32>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
    pub usage_cache_read_tokens: Option<u64>,
    pub usage_cache_creation_tokens: Option<u64>,
    pub usage_cache_creation_5m_tokens: Option<u64>,
    pub usage_cache_creation_1hr_tokens: Option<u64>,
    pub model: Option<String>,
}

impl ToolUseResultPayload {
    fn has_data(&self) -> bool {
        self.agent_id.is_some()
            || self.status.is_some()
            || self.total_duration_ms.is_some()
            || self.total_tool_use_count.is_some()
            || self.usage_input_tokens.is_some()
            || self.usage_output_tokens.is_some()
            || self.usage_cache_read_tokens.is_some()
            || self.usage_cache_creation_tokens.is_some()
            || self.usage_cache_creation_5m_tokens.is_some()
            || self.usage_cache_creation_1hr_tokens.is_some()
            || self.model.is_some()
    }

    fn merge(&mut self, other: ToolUseResultPayload) {
        if other.agent_id.is_some() && self.agent_id.is_none() {
            self.agent_id = other.agent_id;
        }
        if other.status.is_some() {
            self.status = other.status;
        }
        if other.total_duration_ms.is_some() && self.total_duration_ms.is_none() {
            self.total_duration_ms = other.total_duration_ms;
        }
        if other.total_tool_use_count.is_some() && self.total_tool_use_count.is_none() {
            self.total_tool_use_count = other.total_tool_use_count;
        }
        if other.usage_input_tokens.is_some() && self.usage_input_tokens.is_none() {
            self.usage_input_tokens = other.usage_input_tokens;
        }
        if other.usage_output_tokens.is_some() && self.usage_output_tokens.is_none() {
            self.usage_output_tokens = other.usage_output_tokens;
        }
        if other.usage_cache_read_tokens.is_some() && self.usage_cache_read_tokens.is_none() {
            self.usage_cache_read_tokens = other.usage_cache_read_tokens;
        }
        if other.usage_cache_creation_tokens.is_some() && self.usage_cache_creation_tokens.is_none()
        {
            self.usage_cache_creation_tokens = other.usage_cache_creation_tokens;
        }
        if other.usage_cache_creation_5m_tokens.is_some()
            && self.usage_cache_creation_5m_tokens.is_none()
        {
            self.usage_cache_creation_5m_tokens = other.usage_cache_creation_5m_tokens;
        }
        if other.usage_cache_creation_1hr_tokens.is_some()
            && self.usage_cache_creation_1hr_tokens.is_none()
        {
            self.usage_cache_creation_1hr_tokens = other.usage_cache_creation_1hr_tokens;
        }
        if other.model.is_some() && self.model.is_none() {
            self.model = other.model;
        }
    }
}

pub(crate) fn parse_tool_use_result_payload(
    tur: &serde_json::Value,
) -> Option<ToolUseResultPayload> {
    match tur {
        serde_json::Value::Object(obj) => {
            let has_known_key = obj.contains_key("status")
                || obj.contains_key("agentId")
                || obj.contains_key("totalDurationMs")
                || obj.contains_key("totalToolUseCount")
                || obj.contains_key("usage")
                || obj.contains_key("model");
            if !has_known_key {
                return None;
            }

            let status = obj
                .get("status")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| Some("completed".to_string()));
            let usage = obj.get("usage");

            // Extract nested cache_creation ephemeral breakdown if present.
            // Sub-agent toolUseResult.usage mirrors the SDK message.usage shape,
            // including cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}.
            let (usage_cache_creation_5m_tokens, usage_cache_creation_1hr_tokens) = usage
                .and_then(|u| u.get("cache_creation"))
                .and_then(|cc| cc.as_object())
                .map(|cc_obj| {
                    let t5m = cc_obj
                        .get("ephemeral_5m_input_tokens")
                        .and_then(|v| v.as_u64());
                    let t1h = cc_obj
                        .get("ephemeral_1h_input_tokens")
                        .and_then(|v| v.as_u64());
                    (t5m, t1h)
                })
                .unwrap_or((None, None));

            Some(ToolUseResultPayload {
                agent_id: obj
                    .get("agentId")
                    .or_else(|| obj.get("agent_id")) // team spawns use snake_case
                    .and_then(|v| v.as_str())
                    .map(String::from),
                status,
                total_duration_ms: obj.get("totalDurationMs").and_then(|v| v.as_u64()),
                total_tool_use_count: obj
                    .get("totalToolUseCount")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                usage_input_tokens: usage
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_output_tokens: usage
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_cache_read_tokens: usage
                    .and_then(|u| u.get("cache_read_input_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_cache_creation_tokens: usage
                    .and_then(|u| u.get("cache_creation_input_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_cache_creation_5m_tokens,
                usage_cache_creation_1hr_tokens,
                model: obj.get("model").and_then(|v| v.as_str()).map(String::from),
            })
        }
        serde_json::Value::String(status) => {
            let status = status.trim();
            if status.is_empty() {
                None
            } else {
                Some(ToolUseResultPayload {
                    status: Some(status.to_string()),
                    ..ToolUseResultPayload::default()
                })
            }
        }
        serde_json::Value::Array(items) => {
            let mut merged = ToolUseResultPayload::default();
            for item in items {
                if let Some(parsed) = parse_tool_use_result_payload(item) {
                    merged.merge(parsed);
                }
            }
            if merged.has_data() {
                Some(merged)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn json_value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn progress_content_blocks(data: &serde_json::Value) -> (Option<&Vec<serde_json::Value>>, bool) {
    let primary = data
        .get("message")
        .and_then(|m| m.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());
    if primary.is_some() {
        return (primary, false);
    }

    let fallback = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());
    (fallback, fallback.is_some())
}

/// Extract sub-agent spawn info from assistant content blocks.
pub(crate) fn extract_sub_agent_spawns(msg: Option<&serde_json::Value>) -> Vec<SubAgentSpawn> {
    let mut spawns = Vec::new();
    if let Some(content) = msg
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if tool_name != "Task" && tool_name != "Agent" {
                    continue;
                }
                let tool_use_id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input");
                // Agent tool uses input.name as display name, Task uses input.description
                let description = input
                    .and_then(|i| i.get("name").or_else(|| i.get("description")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let agent_type = input
                    .and_then(|i| i.get("subagent_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(tool_name)
                    .to_string();
                let spawn_team_name = input
                    .and_then(|i| i.get("team_name"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let spawn_model = input
                    .and_then(|i| i.get("model"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                if !tool_use_id.is_empty() {
                    spawns.push(SubAgentSpawn {
                        tool_use_id,
                        agent_type,
                        description,
                        team_name: spawn_team_name,
                        model: spawn_model,
                    });
                }
            }
        }
    }
    spawns
}

/// Extract sub-agent completion result from a user line with toolUseResult.
pub(crate) fn extract_sub_agent_result(
    parsed: &serde_json::Value,
    msg: Option<&serde_json::Value>,
) -> Option<SubAgentResult> {
    parsed.get("toolUseResult").and_then(|tur| {
        let parsed_tur = match parse_tool_use_result_payload(tur) {
            Some(parsed) => parsed,
            None => {
                tracing::debug!(
                    kind = json_value_kind(tur),
                    "Ignoring unsupported toolUseResult variant"
                );
                return None;
            }
        };

        // Find the matching tool_use_id from the tool_result block in content
        let tool_use_id = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        b.get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            })?;
        Some(SubAgentResult {
            tool_use_id,
            agent_id: parsed_tur.agent_id,
            status: parsed_tur.status.unwrap_or_else(|| "completed".to_string()),
            total_duration_ms: parsed_tur.total_duration_ms,
            total_tool_use_count: parsed_tur.total_tool_use_count,
            usage_input_tokens: parsed_tur.usage_input_tokens,
            usage_output_tokens: parsed_tur.usage_output_tokens,
            usage_cache_read_tokens: parsed_tur.usage_cache_read_tokens,
            usage_cache_creation_tokens: parsed_tur.usage_cache_creation_tokens,
            usage_cache_creation_5m_tokens: parsed_tur.usage_cache_creation_5m_tokens,
            usage_cache_creation_1hr_tokens: parsed_tur.usage_cache_creation_1hr_tokens,
            model: parsed_tur.model,
        })
    })
}

/// Extract sub-agent progress from a progress line with agent_progress data.
pub(crate) fn extract_sub_agent_progress(parsed: &serde_json::Value) -> Option<SubAgentProgress> {
    parsed.get("data").and_then(|data| {
        if data.get("type").and_then(|t| t.as_str()) != Some("agent_progress") {
            return None;
        }
        let parent_tool_use_id = parsed
            .get("parentToolUseID")
            .and_then(|v| v.as_str())
            .map(String::from)?;
        let agent_id = data
            .get("agentId")
            .and_then(|v| v.as_str())
            .map(String::from)?;
        // Primary: data.message.message.content[*]
        // Fallback: data.message.content[*]
        let (progress_blocks, used_fallback) = progress_content_blocks(data);
        if used_fallback {
            increment_progress_message_content_fallback_count();
        }

        let current_tool = progress_blocks.and_then(|blocks| {
            blocks.iter().rev().find_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    b.get("name").and_then(|n| n.as_str()).map(String::from)
                } else {
                    None
                }
            })
        });
        Some(SubAgentProgress {
            parent_tool_use_id,
            agent_id,
            current_tool,
        })
    })
}
