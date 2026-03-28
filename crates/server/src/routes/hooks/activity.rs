//! Activity label helpers for hook events.
//!
//! Derives human-readable labels from PreToolUse hook data
//! (tool name + input) for the Live Monitor timeline.

/// Derive a rich activity label from PreToolUse hook data.
pub(super) fn activity_from_pre_tool(
    tool_name: &str,
    tool_input: &Option<serde_json::Value>,
) -> String {
    let input = tool_input.as_ref();
    match tool_name {
        "Bash" => input
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .map(|cmd| {
                let truncated: String = cmd.chars().take(60).collect();
                format!("Running: {}", truncated)
            })
            .unwrap_or_else(|| "Running command".into()),
        "Read" => input
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|p| format!("Reading {}", short_path(p)))
            .unwrap_or_else(|| "Reading file".into()),
        "Edit" | "Write" => input
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|p| format!("Editing {}", short_path(p)))
            .unwrap_or_else(|| "Editing file".into()),
        "Grep" => input
            .and_then(|v| v.get("pattern"))
            .and_then(|v| v.as_str())
            .map(|pat| {
                let truncated: String = pat.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching code".into()),
        "Glob" => "Finding files".into(),
        // Claude Code renamed "Task" to "Agent" ~v0.10. Handle both.
        // Agent uses input.name as display name, Task uses input.description.
        "Task" | "Agent" => input
            .and_then(|v| v.get("name").or_else(|| v.get("description")))
            .and_then(|v| v.as_str())
            .map(|d| {
                let truncated: String = d.chars().take(50).collect();
                format!("Agent: {}", truncated)
            })
            .unwrap_or_else(|| "Dispatching agent".into()),
        "WebFetch" => "Fetching web page".into(),
        "WebSearch" => input
            .and_then(|v| v.get("query"))
            .and_then(|v| v.as_str())
            .map(|q| {
                let truncated: String = q.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching web".into()),
        _ if tool_name.starts_with("mcp__") => {
            let short = tool_name.trim_start_matches("mcp__");
            format!("MCP: {}", short)
        }
        _ => format!("Using {}", tool_name),
    }
}

/// Extract the last path component for display.
pub(super) fn short_path(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
