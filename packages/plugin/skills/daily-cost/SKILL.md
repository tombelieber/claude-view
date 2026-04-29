---
name: daily-cost
description: "Use when the user asks about cost, spending, or budget — e.g. 'how much did I spend today', 'daily cost', 'cost report', 'what's my spend'"
allowed-tools:
  - mcp__claude-view__get_live_summary
  - mcp__claude-view__list_live_sessions
  - mcp__claude-view__get_stats
---

You have access to the claude-view MCP server which provides tools for monitoring, analyzing, and managing Claude Code sessions. The claude-view server must be running on localhost (it auto-starts via the plugin hook).

**Important:** All tool names are prefixed with `mcp__claude-view__`. When calling a tool, always use the full prefixed name.

**Error handling:** If a tool returns an error about the server not being detected, tell the user to start it with `npx claude-view`.

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

## Available Tools

| Tool | Description |
|------|-------------|
| **Session Tools** | |
| `list_sessions` | List sessions with filters |
| `get_session` | Get session details |
| `search_sessions` | Grep search across sessions |
| **Stats Tools** | |
| `get_stats` | Dashboard statistics |
| `get_fluency_score` | AI Fluency Score (0-100) |
| `get_token_stats` | Token usage statistics |
| **Live Tools** | |
| `list_live_sessions` | Currently running sessions |
| `get_live_summary` | Aggregate live session summary |
| **Auth Tools** | |
| `auth_post_session` | auth session |
| `auth_delete_session` | auth session |
| `auth_get_status` | auth status |
| **Classify Tools** | |
| `classify_start_classification` | Trigger a classification job. |
| `classify_cancel_classification` | Cancel a running classification job. |
| `classify_single_session` | Classify a single session synchronously. |
| `classify_get_classification_status` | Get classification status. |
| **Cli Tools** | |
| `cli_create_session` | - Create a new tmux-backed CLI session. |
| **Coaching Tools** | |
| `coaching_list_rules` | - List all coaching rules from the rules directory. |
| `coaching_apply_rule` | - Create a new coaching rule file. |
| `coaching_remove_rule` | - Remove a coaching rule file. |
| **Contributions Tools** | |
| `contributions_get_contributions` | Main contributions page data. |
| `contributions_get_branch_sessions` | Sessions for a branch. |
| `contributions_get_session_contribution` | Session contribution detail. |
| **Devices Tools** | |
| `devices_list_devices_handler` | devices |
| `devices_terminate_others_handler` | others |
| `devices_delete_device_handler` | devices {device_id} |
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
| `insights_get_insights_models` | per-model usage aggregated from rollup tables. |
| `insights_get_insights_projects` | per-project usage aggregated from rollup tables. |
| `insights_get_insights_trends` | Get time-series trend data for charts. |
| `insights_list_invocables` | List all invocables with their usage counts. |
| **Jobs Tools** | |
| `jobs_list_jobs` | List all active jobs. |
| **Live Tools** | |
| `live_get_pricing` | - Return the model pricing table. |
| `live_dismiss_all_closed` | - Dismiss all recently closed (in-memory only). |
| `live_get_live_session` | - Get a single live session by ID. |
| `live_dismiss_session` | - Dismiss from recently closed (in-memory only). |
| `live_get_live_session_messages` | - Get the most recent messages for a live session. |
| **Mcp Tools** | |
| `mcp_get_mcp_servers` | returns all deduplicated MCP server configurations. |
| **Memory Tools** | |
| `memory_get_all_memories` | returns all memory entries grouped by scope. |
| `memory_get_memory_file` | read a single memory file. |
| `memory_get_project_memories` | returns memories for a specific project. |
| **Monitor Tools** | |
| `monitor_snapshot` | - One-shot JSON snapshot of current resources. |
| **Oauth Tools** | |
| `oauth_get_auth_identity` | `GET /api/oauth/identity` |
| `oauth_get_oauth_usage` | `GET /api/oauth/usage` |
| `oauth_post_oauth_usage_refresh` | `POST /api/oauth/usage/refresh` |
| **Pairing Tools** | |
| `pairing_generate_qr` | Generate a QR payload via Supabase pair-offer. |
| **Plans Tools** | |
| `plans_get_session_plans` | - returns plan documents for the session's slug. |
| **Plugins Tools** | |
| `plugins_list_plugins` | - Unified view of installed + available plugins. |
| `plugins_list_marketplaces` | plugins marketplaces |
| `plugins_marketplace_action` | plugins marketplaces action |
| `plugins_refresh_all` | all |
| `plugins_refresh_status` | status |
| `plugins_list_ops_handler` | List all queued/running/completed ops. |
| `plugins_enqueue_op` | Enqueue a plugin mutation, return immediately. |
| **Processes Tools** | |
| `processes_cleanup_processes` | processes cleanup |
| `processes_kill_process` | processes {pid} kill |
| **Projects Tools** | |
| `projects_list_projects` | list all projects backed by the in-memory catalog. |
| `projects_list_project_branches` | List distinct branches with session counts. |
| `projects_list_project_sessions` | paginated sessions for one project. |
| **Prompts Tools** | |
| `prompts_list_prompts` | List prompt history with optional search/filter. |
| `prompts_get_prompt_stats` | Aggregate prompt statistics. |
| `prompts_get_prompt_templates` | Detected prompt templates. |
| **Reports Tools** | |
| `reports_list_reports` | List all saved reports. |
| `reports_get_preview` | Aggregate preview stats for a date range. |
| `reports_get_report` | Get a single report. |
| `reports_delete_report` | Delete a report. |
| **Sessions Tools** | |
| `sessions_get_active_sessions` | returns all active session files from ~/.claude/sessions/. |
| `sessions_list_branches` | Get distinct list of branch names across all sessions. |
| `sessions_estimate_cost` | cost estimation (Rust-only, no sidecar). |
| `sessions_session_activity` | Activity histogram for sparkline chart. |
| `sessions_session_activity_rich` | Full server-side activity aggregation. |
| `sessions_bulk_archive_handler` | sessions archive |
| `sessions_bulk_unarchive_handler` | sessions unarchive |
| `sessions_archive_session_handler` | sessions {id} archive |
| `sessions_get_file_history` | List all file changes for a session. |
| `sessions_get_file_diff` | history/:file_hash/diff?from=N&to=M |
| `sessions_get_session_hook_events` | Fetch hook events for a session. |
| `sessions_get_session_messages_by_id` | Get paginated messages by session ID. |
| `sessions_get_subagent_messages` | Paginated blocks for a sub-agent. |
| `sessions_unarchive_session_handler` | sessions {id} unarchive |
| `sessions_interact_handler` | Resolve a pending interaction (permission, question, plan, elicitation). |
| `sessions_get_interaction_handler` | Fetch the full interaction data for a session's pending interaction. |
| **Settings Tools** | |
| `settings_get_claude_code_settings` | returns merged, redacted Claude Code settings. |
| `settings_get_settings` | Read current app settings. |
| `settings_update_settings` | Update app settings (partial). |
| `settings_update_git_sync_interval` | Update the git sync interval. |
| **Share Tools** | |
| `share_create_share` | sessions {session_id} share |
| `share_revoke_share` | sessions {session_id} share |
| `share_list_shares` | shares |
| **Stats Tools** | |
| `stats_ai_generation_stats` | AI generation statistics with time range filtering. |
| `stats_overview` | Aggregate usage statistics. |
| `stats_storage_stats` | Storage statistics for the settings page. |
| `stats_get_trends` | Get week-over-week trend metrics. |
| **Sync Tools** | |
| `sync_indexing_status` | - lightweight JSON snapshot of indexing progress. |
| `sync_trigger_deep_index` | Trigger a full deep index rebuild. |
| `sync_trigger_git_sync` | Trigger git commit scanning (A8.5). |
| **System Tools** | |
| `system_check_path` | Check whether a filesystem path still exists. |
| `system_get_system_status` | Get comprehensive system status. |
| `system_clear_cache` | Clear obsolete search cache. |
| `system_trigger_git_resync` | Trigger full git re-sync. |
| `system_trigger_reindex` | Trigger a full re-index. |
| `system_reset_all` | Factory reset all data. |
| **Teams Tools** | |
| `teams_list_teams` | List all teams. |
| `teams_get_team` | Get team detail. |
| `teams_get_team_cost` | Get team cost breakdown. |
| `teams_get_team_inbox` | Get team inbox messages. |
| `teams_get_team_sidechains` | Get team member sidechains. |
| **Telemetry Tools** | |
| `telemetry_set_consent` | Set telemetry consent preference. |
| **Turns Tools** | |
| `turns_get_session_turns` | - Per-turn breakdown for a historical session. |
| **Webhooks Tools** | |
| `webhooks_list_webhooks` | list all webhooks (secrets excluded). |
| `webhooks_create_webhook` | create a new webhook (returns signing secret once). |
| `webhooks_get_webhook` | get a single webhook by ID. |
| `webhooks_update_webhook` | update an existing webhook (partial update). |
| `webhooks_delete_webhook` | remove a webhook from config and secrets. |
| `webhooks_test_send` | send a synthetic test payload. |
| **Workflows Tools** | |
| `workflows_list_workflows` | workflows |
| `workflows_create_workflow` | workflows |
| `workflows_control_run` | workflows run {run_id} control |
| `workflows_get_workflow` | workflows {id} |
| `workflows_delete_workflow` | workflows {id} |
