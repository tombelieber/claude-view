---
name: standup
description: "Use when the user asks for a standup update, work log, or activity summary — e.g. 'standup update', 'what did I work on today', 'work log', 'daily summary'"
allowed-tools:
  - mcp__claude-view__list_sessions
  - mcp__claude-view__get_session
  - mcp__claude-view__list_live_sessions
---

You have access to the claude-view MCP server which provides tools for monitoring, analyzing, and managing Claude Code sessions. The claude-view server must be running on localhost (it auto-starts via the plugin hook).

**Important:** All tool names are prefixed with `mcp__claude-view__`. When calling a tool, always use the full prefixed name.

**Error handling:** If a tool returns an error about the server not being detected, tell the user to start it with `npx claude-view`.

# Standup Update

Generate a standup-style work summary from recent Claude Code sessions.

## Steps

1. **Fetch recent sessions.** Call `mcp__claude-view__list_sessions` with:
   - `sort: "recent"`
   - `limit: 20`

   From the results, filter to sessions whose `modified` field (ISO 8601 string) is within the last 24 hours. If the user asks about a different period (e.g. "this week"), adjust the filter accordingly.

2. **For the top 3-5 sessions by duration**, call `mcp__claude-view__get_session` on each to get commit details.

3. **Check for running sessions.** Call `mcp__claude-view__list_live_sessions` to find any currently active sessions for the "In Progress" section.

4. **Present the standup** in this format:

```
## Standup — [today's date]

### Done
- **[project] ([branch])** — [1-line summary from recent_commits/summary] (Xm)
- **[project] ([branch])** — [1-line summary] (Xm)

### In Progress
- [project] — [model] — [agent_state] (from list_live_sessions)

### Metrics
- Sessions: N | Total time: Xh Ym | Commits: Z
```

5. **Keep each item to one line.** The standup should be copy-pasteable into Slack or a standup bot.

6. **If no sessions found in the time range**, say "No Claude Code sessions found in the last 24 hours."

## Available Tools

| Tool | Description |
|------|-------------|
| **Session Tools** | |
| `list_sessions` | List sessions with filters |
| `get_session` | Get session details |
| `search_sessions` | Full-text search across sessions |
| **Stats Tools** | |
| `get_stats` | Dashboard statistics |
| `get_fluency_score` | AI Fluency Score (0-100) |
| `get_token_stats` | Token usage statistics |
| **Live Tools** | |
| `list_live_sessions` | Currently running sessions |
| `get_live_summary` | Aggregate live session summary |
| **Classify Tools** | |
| `classify_start_classification` | Trigger a classification job. |
| `classify_cancel_classification` | Cancel a running classification job. |
| `classify_single_session` | Classify a single session synchronously. |
| `classify_get_classification_status` | Get classification status. |
| **Coaching Tools** | |
| `coaching_list_rules` | List all coaching rules from the rules directory. |
| `coaching_apply_rule` | Create a new coaching rule file. |
| `coaching_remove_rule` | Remove a coaching rule file. |
| **Contributions Tools** | |
| `contributions_get_contributions` | Main contributions page data. |
| `contributions_get_branch_sessions` | Sessions for a branch. |
| `contributions_get_session_contribution` | Session contribution detail. |
| **Export Tools** | |
| `export_sessions` | Export all sessions. |
| **Facets Tools** | |
| `facets_facet_badges` | Quality badges (outcome + satisfaction) for the requested session IDs |
| `facets_trigger_facet_ingest` | Start facet ingest from the Claude Code insights cache |
| `facets_pattern_alert` | Check the most recent sessions for a negative satisfaction pattern |
| `facets_facet_stats` | Aggregate statistics across all session facets. |
| **Health Tools** | |
| `health_config` | config |
| `health_check` | Health check endpoint. |
| `health_get_status` | Get index metadata and data freshness info. |
| **Ide Tools** | |
| `ide_get_detect` | `GET /api/ide/detect` — return cached list of installed IDEs. |
| `ide_post_open` | `POST /api/ide/open` — open a file in the requested IDE. |
| **Insights Tools** | |
| `insights_get_insights` | Compute and return behavioral insights. |
| `insights_get_benchmarks` | Compute personal progress benchmarks. |
| `insights_get_categories` | Returns hierarchical category data. |
| `insights_get_insights_trends` | Get time-series trend data for charts. |
| `insights_list_invocables` | List all invocables with their usage counts. |
| `insights_get_fluency_score` | Get the current AI Fluency Score. |
| **Jobs Tools** | |
| `jobs_list_jobs` | List all active jobs. |
| **Models Tools** | |
| `models_list_models` | List all known models with usage counts. |
| **Monitor Tools** | |
| `monitor_snapshot` | - One-shot JSON snapshot of current resources. |
| **Oauth Tools** | |
| `oauth_get_auth_identity` | oauth identity |
| `oauth_get_oauth_usage` | oauth usage |
| `oauth_post_oauth_usage_refresh` | oauth usage refresh |
| **Pairing Tools** | |
| `pairing_list_devices` | GET /pairing/devices — List paired devices. |
| `pairing_unpair_device` | DELETE /pairing/devices/:id — Unpair a device. |
| `pairing_generate_qr` | GET /pairing/qr — Generate QR payload for mobile pairing. |
| **Plans Tools** | |
| `plans_get_session_plans` | - returns plan documents for the session's slug. |
| **Plugins Tools** | |
| `plugins_list_plugins` | Unified view of installed + available plugins. |
| `plugins_list_marketplaces` | plugins marketplaces |
| `plugins_refresh_all` | all |
| `plugins_refresh_status` | status |
| `plugins_list_ops_handler` | List all queued/running/completed ops. |
| `plugins_enqueue_op` | Enqueue a plugin mutation, return immediately. |
| **Processes Tools** | |
| `processes_cleanup_processes` | processes cleanup |
| `processes_kill_process` | processes {pid} kill |
| **Projects Tools** | |
| `projects_list_projects` | List all projects as lightweight summaries. |
| `projects_list_project_branches` | List distinct branches with session counts. |
| `projects_list_project_sessions` | Paginated sessions for a project. |
| **Prompts Tools** | |
| `prompts_list_prompts` | List prompt history with optional search/filter. |
| `prompts_get_prompt_stats` | Aggregate prompt statistics. |
| `prompts_get_prompt_templates` | Detected prompt templates. |
| **Reports Tools** | |
| `reports_list_reports` | List all saved reports. |
| `reports_get_preview` | Aggregate preview stats for a date range. |
| `reports_get_report` | Get a single report. |
| `reports_delete_report` | Delete a report. |
| **Search Tools** | |
| `search_handler` | Full-text search across sessions. |
| **Settings Tools** | |
| `settings_get_settings` | Read current app settings. |
| `settings_update_settings` | Update app settings (partial). |
| `settings_update_git_sync_interval` | Update the git sync interval. |
| **Share Tools** | |
| `share_create_share` | sessions {session_id} share |
| `share_revoke_share` | sessions {session_id} share |
| `share_list_shares` | shares |
| **Sync Tools** | |
| `sync_indexing_status` | lightweight JSON snapshot of indexing progress. |
| `sync_trigger_deep_index` | Trigger a full deep index rebuild. |
| `sync_trigger_git_sync` | Trigger git commit scanning (A8.5). |
| **System Tools** | |
| `system_check_path` | Check whether a filesystem path still exists. |
| `system_get_system_status` | Get comprehensive system status. |
| `system_clear_cache` | Clear search index and cached data. |
| `system_trigger_git_resync` | Trigger full git re-sync. |
| `system_trigger_reindex` | Trigger a full re-index. |
| `system_reset_all` | Factory reset all data. |
| **Teams Tools** | |
| `teams_list_teams` | List all teams. |
| `teams_get_team` | Get team detail. |
| `teams_get_team_inbox` | Get team inbox messages. |
| **Telemetry Tools** | |
| `telemetry_set_consent` | Set telemetry consent preference. |
| **Turns Tools** | |
| `turns_get_session_turns` | Per-turn breakdown for a historical session. |
| **Workflows Tools** | |
| `workflows_list_workflows` | workflows |
| `workflows_create_workflow` | workflows |
| `workflows_control_run` | workflows run {run_id} control |
| `workflows_get_workflow` | workflows {id} |
| `workflows_delete_workflow` | workflows {id} |
