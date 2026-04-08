// crates/core/src/category.rs

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Action category — 8 values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    Skill,
    Mcp,
    Builtin,
    Agent,
    Hook,
    System,
    Snapshot,
    Queue,
}

impl ActionCategory {
    pub fn from_str_category(s: &str) -> Self {
        match s {
            "skill" => Self::Skill,
            "mcp" => Self::Mcp,
            "agent" => Self::Agent,
            "hook" => Self::Hook,
            "system" => Self::System,
            "snapshot" => Self::Snapshot,
            "queue" => Self::Queue,
            _ => Self::Builtin,
        }
    }
}

/// Categorize a tool call by its name.
///
/// Maps tool names to categories:
/// - "Skill" → "skill"
/// - "mcp__*" / "mcp_*" → "mcp"
/// - "Task" / "Agent" → "agent"  (Claude Code renamed Task to Agent ~v0.10)
/// - everything else → "builtin"
pub fn categorize_tool(name: &str) -> &'static str {
    if name == "Skill" {
        "skill"
    } else if name.starts_with("mcp__") || name.starts_with("mcp_") {
        "mcp"
    } else if name == "Task" || name == "Agent" {
        "agent"
    } else {
        "builtin"
    }
}

/// Categorize a progress message by its `data.type` subfield.
///
/// Maps progress subtypes to categories:
/// - "hook_progress" → "hook"
/// - "agent_progress" → "agent"
/// - "bash_progress" → "builtin"
/// - "mcp_progress" → "mcp"
/// - "waiting_for_task" → "agent"
/// - anything else → None (uncategorized)
pub fn categorize_progress(data_type: &str) -> Option<&'static str> {
    match data_type {
        "hook_progress" => Some("hook"),
        "agent_progress" => Some("agent"),
        "bash_progress" => Some("builtin"),
        "mcp_progress" => Some("mcp"),
        "waiting_for_task" => Some("agent"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_tool_skill() {
        assert_eq!(categorize_tool("Skill"), "skill");
    }

    #[test]
    fn test_categorize_tool_mcp() {
        assert_eq!(categorize_tool("mcp__chrome-devtools__click"), "mcp");
        assert_eq!(categorize_tool("mcp_playwright"), "mcp");
    }

    #[test]
    fn test_categorize_tool_agent() {
        assert_eq!(categorize_tool("Task"), "agent");
        assert_eq!(categorize_tool("Agent"), "agent");
    }

    #[test]
    fn test_categorize_tool_builtin() {
        assert_eq!(categorize_tool("Read"), "builtin");
        assert_eq!(categorize_tool("Bash"), "builtin");
        assert_eq!(categorize_tool("Edit"), "builtin");
        assert_eq!(categorize_tool("Write"), "builtin");
        assert_eq!(categorize_tool("Grep"), "builtin");
        assert_eq!(categorize_tool("Glob"), "builtin");
    }

    #[test]
    fn test_categorize_progress_hook() {
        assert_eq!(categorize_progress("hook_progress"), Some("hook"));
    }

    #[test]
    fn test_categorize_progress_agent() {
        assert_eq!(categorize_progress("agent_progress"), Some("agent"));
        assert_eq!(categorize_progress("waiting_for_task"), Some("agent"));
    }

    #[test]
    fn test_categorize_progress_builtin() {
        assert_eq!(categorize_progress("bash_progress"), Some("builtin"));
    }

    #[test]
    fn test_categorize_progress_mcp() {
        assert_eq!(categorize_progress("mcp_progress"), Some("mcp"));
    }

    #[test]
    fn test_categorize_progress_unknown() {
        assert_eq!(categorize_progress("unknown_progress"), None);
    }
}
