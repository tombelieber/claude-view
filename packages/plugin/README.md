# claude-view plugin

<p>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/Website-claudeview.ai-orange" alt="Website"></a>
  <a href="https://github.com/tombelieber/claude-view/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
</p>

Mission Control plugin for [Claude Code](https://docs.anthropic.com/en/docs/claude-code). Auto-starts the [claude-view](https://claudeview.ai) web dashboard, provides 86 MCP tools across 30 categories, and adds 9 skills for common workflows.

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

## Tools (86)

### Hand-written (curated output) — 8 tools

These tools have hand-crafted response shaping for optimal Claude consumption.

| Tool | Description |
|------|-------------|
| `list_sessions` | List/filter/paginate sessions with summaries |
| `get_session` | Full session detail + commits + derived metrics |
| `search_sessions` | Full-text search across all sessions |
| `get_stats` | Dashboard overview: projects, skills, trends |
| `get_fluency_score` | AI Fluency Score (0-100 composite) |
| `get_token_stats` | Token usage breakdown (input/output/cache) |
| `list_live_sessions` | Currently running sessions with real-time state |
| `get_live_summary` | Aggregate: cost today, attention count, tokens |

### Auto-generated (JSON passthrough) — 78 tools

Generated from the OpenAPI spec. 78 tools across 27 categories.

<details>
<summary>Show all generated tools</summary>

| Tag | Tools | Example |
|-----|-------|---------|
| classify | 4 | `classify_start_classification` |
| coaching | 3 | `coaching_list_rules` |
| contributions | 3 | `contributions_get_contributions` |
| export | 1 | `export_sessions` |
| facets | 4 | `facets_facet_badges` |
| health | 3 | `health_config` |
| ide | 2 | `ide_get_detect` |
| insights | 5 | `insights_get_insights` |
| jobs | 1 | `jobs_list_jobs` |
| models | 1 | `models_list_models` |
| monitor | 1 | `monitor_snapshot` |
| oauth | 3 | `oauth_get_auth_identity` |
| pairing | 3 | `pairing_list_devices` |
| plans | 1 | `plans_get_session_plans` |
| plugins | 6 | `plugins_list_plugins` |
| processes | 2 | `processes_cleanup_processes` |
| projects | 3 | `projects_list_projects` |
| prompts | 3 | `prompts_list_prompts` |
| reports | 4 | `reports_list_reports` |
| settings | 3 | `settings_get_settings` |
| share | 3 | `share_create_share` |
| sync | 3 | `sync_indexing_status` |
| system | 6 | `system_check_path` |
| teams | 3 | `teams_list_teams` |
| telemetry | 1 | `telemetry_set_consent` |
| turns | 1 | `turns_get_session_turns` |
| workflows | 5 | `workflows_list_workflows` |

</details>

### 9 Skills

| Skill | Trigger |
|-------|---------|
| `/session-recap` | "recap my last session", "session summary" |
| `/daily-cost` | "how much did I spend today", "cost report" |
| `/standup` | "standup update", "what did I work on today" |
| `/coaching` | "how can I improve", "coaching tips", "add a coaching rule" |
| `/insights` | "what patterns do you see", "behavioral analysis" |
| `/project-overview` | "show me project X", "project summary" |
| `/search` | "find where I discussed X", "search for Y" |
| `/export-data` | "export my sessions", "download data", "export to CSV" |
| `/team-status` | "team status", "what's the team doing", "who's active" |

## Configuration

Set `CLAUDE_VIEW_PORT` to override the default port (47892).

## Links

- [claudeview.ai](https://claudeview.ai) — Website, docs, changelog
- [claude-view on npm](https://www.npmjs.com/package/claude-view) — The dashboard binary (`npx claude-view`)
- [GitHub](https://github.com/tombelieber/claude-view) — Source code, issues, discussions
- [Discord](https://discord.gg/G7wdZTpRfu) — Community support

## License

MIT
