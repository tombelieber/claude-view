// crates/core/src/invocation/classify.rs
//
// Classify tool_use calls from JSONL lines against a Registry to determine
// which invocable was called (skill, command, agent, MCP tool, or built-in).

use crate::registry::{InvocableKind, Registry, BUILTIN_TOOLS};

use super::agents::is_builtin_agent;
use super::mcp_parser::parse_mcp_tool_name;
use super::types::ClassifyResult;

/// Classify a tool_use call against the registry to determine what invocable was called.
///
/// - `name`: The tool name from the JSONL `tool_use` event (e.g. "Skill", "Bash", "mcp__plugin_...")
/// - `input`: The optional JSON input from the tool_use event
/// - `registry`: The invocable registry to look up against
pub fn classify_tool_use(
    name: &str,
    input: &Option<serde_json::Value>,
    registry: &Registry,
) -> ClassifyResult {
    match name {
        // ---- Skill tool: extract skill name from input.skill ----
        "Skill" => {
            let skill_name = input
                .as_ref()
                .and_then(|v| v.get("skill"))
                .and_then(|v| v.as_str());

            match skill_name {
                Some(s) if BUILTIN_TOOLS.contains(&s) => ClassifyResult::Rejected {
                    raw_value: s.into(),
                    reason: "builtin_misroute".into(),
                },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid {
                        invocable_id: info.id.clone(),
                        kind: info.kind,
                    },
                    None => ClassifyResult::Rejected {
                        raw_value: s.into(),
                        reason: "not_in_registry".into(),
                    },
                },
                None => ClassifyResult::Ignored,
            }
        }

        // ---- Task/Agent tool: extract agent type from input.subagent_type ----
        // Claude Code renamed "Task" to "Agent" ~v0.10. Both extract subagent_type.
        "Task" | "Agent" => {
            let agent_type = input
                .as_ref()
                .and_then(|v| v.get("subagent_type"))
                .and_then(|v| v.as_str());

            match agent_type {
                Some(s) if is_builtin_agent(s) => ClassifyResult::Valid {
                    invocable_id: format!("builtin:{s}"),
                    kind: InvocableKind::BuiltinTool,
                },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid {
                        invocable_id: info.id.clone(),
                        kind: info.kind,
                    },
                    None => ClassifyResult::Rejected {
                        raw_value: s.into(),
                        reason: "not_in_registry".into(),
                    },
                },
                None => ClassifyResult::Ignored,
            }
        }

        // ---- MCP plugin tools: parse structured name ----
        n if n.starts_with("mcp__plugin_") => match parse_mcp_tool_name(n) {
            Some((plugin, tool)) => match registry.lookup_mcp(plugin, tool) {
                Some(info) => ClassifyResult::Valid {
                    invocable_id: info.id.clone(),
                    kind: InvocableKind::McpTool,
                },
                None => ClassifyResult::Rejected {
                    raw_value: n.into(),
                    reason: "not_in_registry".into(),
                },
            },
            None => ClassifyResult::Ignored,
        },

        // ---- Built-in tools: Bash, Read, Write, etc. ----
        n if BUILTIN_TOOLS.contains(&n) => ClassifyResult::Valid {
            invocable_id: format!("builtin:{n}"),
            kind: InvocableKind::BuiltinTool,
        },

        // ---- Unknown tool: silently ignore ----
        _ => ClassifyResult::Ignored,
    }
}
