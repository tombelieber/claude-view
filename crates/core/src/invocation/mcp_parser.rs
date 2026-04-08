// crates/core/src/invocation/mcp_parser.rs
//
// MCP tool name parser for extracting plugin and tool names from structured
// tool_use names like `mcp__plugin_playwright_playwright__browser_navigate`.

/// Parse an MCP tool name like `mcp__plugin_playwright_playwright__browser_navigate`
/// into (plugin_name, tool_name).
///
/// Format: `mcp__plugin_{plugin}_{server}__{tool}`
///
/// Returns `None` if the name doesn't match the expected pattern.
pub(crate) fn parse_mcp_tool_name(name: &str) -> Option<(&str, &str)> {
    // Strip the "mcp__plugin_" prefix
    let rest = name.strip_prefix("mcp__plugin_")?;

    // Split on double underscore "__" to separate "{plugin}_{server}" from "{tool}"
    let dunder_pos = rest.find("__")?;
    let plugin_server = &rest[..dunder_pos];
    let tool = &rest[dunder_pos + 2..];

    if tool.is_empty() || plugin_server.is_empty() {
        return None;
    }

    // From "{plugin}_{server}", extract the plugin name.
    // The plugin name is everything before the LAST underscore.
    // Examples:
    //   "playwright_playwright" -> "playwright"
    //   "supabase_supabase"     -> "supabase"
    //   "Notion_notion"         -> "Notion"
    //   "claude-mem_mcp-search" -> "claude-mem"
    let plugin = match plugin_server.rfind('_') {
        Some(pos) => &plugin_server[..pos],
        None => plugin_server, // no underscore, whole thing is the plugin name
    };

    Some((plugin, tool))
}
