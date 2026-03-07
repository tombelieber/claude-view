---
name: session-recap
description: "Use when the user asks to recap, summarize, or review a Claude Code session — e.g. 'recap my last session', 'what happened in that session', 'session summary'"
---

# Session Recap

Summarize a Claude Code session using the claude-view MCP tools.

## Steps

1. **Identify the session.** If the user specified a session ID, use it. Otherwise, call the `mcp__claude-view__list_sessions` tool with `limit: 5` to show recent sessions and ask which one to recap. If the user says "last session" or "most recent", use the first result.

2. **Fetch session details.** Call the `mcp__claude-view__get_session` tool with the session ID.

3. **Present the recap** in this format:

```
## Session Recap: [project] — [branch]

**Duration:** X minutes | **Model:** [model] | **Turns:** [turns]

### What was done
[2-3 sentence summary based on recent_commits and summary field]

### Commits
- `abc1234` — commit message
- `def5678` — commit message

### Metrics
- Input tokens: [input_tokens] | Output tokens: [output_tokens] | Cache hits: [cache_read_tokens]
- Cost efficiency: [from derived_metrics if available]
```

4. **Keep it concise.** The recap should fit in one screen. If there are more than 5 commits, show the top 5 and note "and N more".
