# claude-view plugin

<p>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/Website-claudeview.ai-orange" alt="Website"></a>
  <a href="https://github.com/tombelieber/claude-view/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
</p>

Mission Control plugin for [Claude Code](https://docs.anthropic.com/en/docs/claude-code). Auto-starts the [claude-view](https://claudeview.ai) web dashboard, provides 8 session/cost/fluency MCP tools, and adds `/session-recap`, `/daily-cost`, `/standup` skills.

## Install

```bash
claude plugin add @claude-view/plugin
```

## Prerequisites

- **Node.js >= 18** (required by MCP SDK)
- The plugin auto-starts the claude-view server, but the binary must be available:

```bash
npx claude-view   # downloads the pre-built Rust binary on first run (~5-15s first time)
```

> **First run note:** The very first session after install may take 5-30 seconds to download the
> Rust binary. The hook will time out gracefully — the server will start in the background and be
> ready for your next tool call or next session.

## What You Get

### Auto-start (SessionStart hook)
Every time you start a Claude Code session, the plugin checks if the claude-view server
is running. If not, it starts it in the background. Web dashboard appears at
`http://localhost:47892`.

### 8 MCP Tools (available to Claude)
| Tool | Purpose |
|------|---------|
| `list_sessions` | List/filter/paginate sessions |
| `get_session` | Full session detail + commits |
| `search_sessions` | Full-text search across sessions |
| `get_stats` | Dashboard overview: projects, skills, trends |
| `get_fluency_score` | AI Fluency Score (0-100) |
| `get_token_stats` | Token usage breakdown |
| `list_live_sessions` | Currently running sessions |
| `get_live_summary` | Aggregate: cost today, attention count |

### 3 Skills
| Skill | Trigger |
|-------|---------|
| `/session-recap` | "recap my last session", "summarize session" |
| `/daily-cost` | "how much did I spend today", "cost report" |
| `/standup` | "standup update", "what did I work on" |

## Configuration

Set `CLAUDE_VIEW_PORT` to override the default port (47892).

## Links

- [claudeview.ai](https://claudeview.ai) — Website, docs, changelog
- [claude-view on npm](https://www.npmjs.com/package/claude-view) — The dashboard binary (`npx claude-view`)
- [GitHub](https://github.com/tombelieber/claude-view) — Source code, issues, discussions
- [Discord](https://discord.gg/G7wdZTpRfu) — Community support

## License

MIT
