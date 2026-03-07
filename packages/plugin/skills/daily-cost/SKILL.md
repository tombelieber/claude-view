---
name: daily-cost
description: "Use when the user asks about cost, spending, or budget — e.g. 'how much did I spend today', 'daily cost', 'cost report', 'what's my spend'"
---

# Daily Cost Report

Show the user's Claude Code spending for today using the claude-view MCP tools.

## Steps

1. **Get live summary.** Call `mcp__claude-view__get_live_summary` to get today's aggregate cost and active session counts.

2. **Get per-session details.** If there are running sessions (needs_attention > 0 or autonomous > 0), call `mcp__claude-view__list_live_sessions` to get per-session project, model, cost, and agent state.

3. **Get dashboard stats.** Call `mcp__claude-view__get_stats` with `from` set to today's date (e.g. `2026-03-01`) to get today's session breakdown.

4. **Present the cost report** in this format:

```
## Daily Cost Report — [today's date]

**Total spent today:** $X.XX USD
**Sessions today:** N | **Currently running:** M

### Running sessions
- [project] — [model] — $X.XX — [agent_state]
(per-session data from list_live_sessions)

### Token usage today
- Input: X tokens | Output: Y tokens
- Cache read: Z tokens
```

5. **If total cost is $0.00**, say "No Claude Code usage detected today" and suggest checking if the claude-view server has indexed recent sessions.

6. **If the user asks about a different time range** (e.g. "this week", "last month"), use the `from` and `to` parameters on `mcp__claude-view__get_stats` accordingly.
