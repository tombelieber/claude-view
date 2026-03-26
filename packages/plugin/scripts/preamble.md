You have access to the claude-view MCP server which provides tools for monitoring, analyzing, and managing Claude Code sessions. The claude-view server must be running on localhost (it auto-starts via the plugin hook).

**Important:** All tool names are prefixed with `mcp__claude-view__`. When calling a tool, always use the full prefixed name.

**Error handling:** If a tool returns an error about the server not being detected, tell the user to start it with `npx claude-view`.
