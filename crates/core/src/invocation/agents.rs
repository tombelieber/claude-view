// crates/core/src/invocation/agents.rs
//
// Built-in agent allowlist for Task/Agent tool classification.

/// Known built-in agent types used by the Task/Agent tool in Claude Code.
/// Built-in agent types that get classified as `builtin:{name}`.
/// Public so the registry can seed these into the invocables table.
pub const BUILTIN_AGENT_NAMES: &[&str] = &[
    "Bash",
    "general-purpose",
    "Explore",
    "Plan",
    "statusline-setup",
    "claude-code-guide",
];

/// Check if an agent type string is a built-in agent.
/// Built-in agents are either in the allowlist or do NOT contain ":"
/// (plugin agents always use the "plugin:agent" format).
pub(crate) fn is_builtin_agent(agent_type: &str) -> bool {
    BUILTIN_AGENT_NAMES.contains(&agent_type)
}
