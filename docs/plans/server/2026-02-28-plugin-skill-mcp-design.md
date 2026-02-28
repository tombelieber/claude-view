# Claude View Plugin + Skill + MCP Server — Design Document

> **Date:** 2026-02-28
> **Status:** Design approved (audited 2026-02-28)
> **Scope:** Create a Claude Code plugin that bundles a skill (HTTP API interface) and MCP server (native tool access) for claude-view

---

## Problem

claude-view has a rich HTTP API (70+ endpoints) but no way for AI agents to discover or use it natively. Users must manually open the dashboard. An agent-native interface would let Claude (and Cursor, Windsurf, etc.) query session data, costs, and fluency scores directly — making claude-view the "eyes" of the AI coding workflow.

## Strategy

Three layers, one plugin:

| Layer | What | Who consumes it | Effort |
| ------- | ------ | ----------------- | -------- |
| **Skill** | Markdown file teaching Claude the HTTP API | Claude Code only | ~1 hour |
| **MCP Server** | TypeScript server exposing native tools via Model Context Protocol | Any MCP client (Claude, Cursor, Windsurf) | ~1-2 days |
| **Plugin** | Distribution wrapper bundling skill for one-step install | Claude Code marketplace | ~30 min |

```text
User installs plugin:  /plugin install claude-view
  → Gets: skill (SKILL.md) + MCP server config

Skill path:     Claude reads SKILL.md → knows to curl localhost:47892
MCP path:       Claude spawns npx -y @claude-view/mcp → native tool calls
Both require:   claude-view server running on localhost:47892
```

The skill is the fallback (works without MCP config). The MCP server is the upgrade (native tools, cross-agent). The plugin bundles both for one-step install.

---

## Package Architecture

```text
plugin.json                # Plugin manifest at install root (name, version, author)
marketplace.json           # Self-hosted marketplace entry at install root

claude-view/
  SKILL.md                 # Agent interface — teaches Claude the HTTP API
                           # (flat layout: {installPath}/claude-view/SKILL.md)

.mcp.json                  # MCP server config (flat format, no mcpServers wrapper)

packages/mcp/
  package.json             # @claude-view/mcp (npm-distributed)
  tsconfig.json            # extends ../../tsconfig.base.json (overrides noEmit)
  src/
    index.ts               # MCP server entry point (parse args, attach transport)
    server.ts              # McpServer setup, register all tools via registerTool()
    client.ts              # HTTP client wrapper for localhost:47892
    tools/
      sessions.ts          # list_sessions, get_session, search_sessions
      stats.ts             # get_stats, get_fluency_score, get_token_stats
      live.ts              # list_live_sessions, get_live_summary

test/
  skill-sync.sh            # Contract test: skill ↔ API routes stay in sync
```

---

## MCP Tool Surface (L1 — Read-Only)

8 tools covering the most valuable agent queries:

| Tool | Input | Output | Endpoint |
| ------ | ------- | -------- | ---------- |
| `list_sessions` | `?limit`, `?q`, `?filter`, `?sort`, `?offset`, `?branches`, `?models`, `?time_after`, `?time_before` | Session list (id, project, primaryModel, durationSeconds, 30+ camelCase fields) | `GET /api/sessions` |
| `get_session` | `session_id` | Full session detail with commits, derivedMetrics, token breakdown | `GET /api/sessions/{id}` |
| `search_sessions` | `?q` (required), `?limit`, `?offset`, `?scope` | Full-text search results: SessionHit objects with matchCount, bestScore, topMatch, snippet | `GET /api/search` |
| `get_stats` | `?project`, `?branch`, `?from`, `?to` | Dashboard overview: totalSessions, totalProjects, topSkills, topProjects, currentWeek, trends | `GET /api/stats/dashboard` |
| `get_fluency_score` | (none) | AI Fluency Score (0-100 composite): score, achievementRate, frictionRate, costEfficiency, satisfactionTrend, consistency, sessionsAnalyzed | `GET /api/score` |
| `get_token_stats` | (none) | Token counts: totalInputTokens, totalOutputTokens, cacheHitRatio, sessionsCount | `GET /api/stats/tokens` |
| `list_live_sessions` | (none) | Currently running sessions with agentState, cost, tokens, progressItems | `GET /api/live/sessions` |
| `get_live_summary` | (none) | Aggregate: needsYouCount, autonomousCount, deliveredCount, totalCostTodayUsd, totalTokensToday, processCount | `GET /api/live/summary` |

All tools: `readOnlyHint: true`, `destructiveHint: false`.

### V2 Control Tools (Planned, Not L1)

| Tool | Description | Safety |
| ------ | ------------- | -------- |
| `send_message` | Send message to running session | Confirmation required |
| `resume_session` | Resume paused session | Confirmation required |
| `kill_session` | Terminate session | Destructive — double confirm |

Agent-to-agent orchestration via claude-view is the killer feature for V2 — one Claude agent monitoring and controlling other Claude agents. Requires safety guardrails (confirmation prompts, rate limits) before shipping.

---

## Actual API Response Shapes

All responses use **camelCase** field names (via `#[serde(rename_all = "camelCase")]`).

### Sessions (`GET /api/sessions`)

```json
{
  "sessions": [{
    "id": "string",
    "project": "string",
    "projectPath": "string",
    "displayName": "string",
    "primaryModel": "string | null",
    "durationSeconds": 0,
    "turnCount": 0,
    "userPromptCount": 0,
    "commitCount": 0,
    "totalInputTokens": 0,
    "totalOutputTokens": 0,
    "gitBranch": "string | null",
    "preview": "string",
    "modifiedAt": 0
  }],
  "total": 0,
  "hasMore": false,
  "filter": "all",
  "sort": "recent"
}
```

Key query params: `?limit` (default 30), `?q` (text search), `?filter` (all|has_commits|high_reedit|long_session), `?sort` (recent|tokens|prompts|files_edited|duration), `?offset`, `?branches`, `?models`, `?time_after`, `?time_before`.

6 additional faceted filter params exist on the actual endpoint but are **excluded from L1** for simplicity: `?has_commits`, `?has_skills`, `?min_duration`, `?min_files`, `?min_tokens`, `?high_reedit`. These can be added in L2 if agents need fine-grained filtering.

**Note:** `?project` and `?branch` (singular) do NOT exist on this endpoint. Use `?branches` (comma-separated) for branch filtering. Project filtering is on `/api/stats/dashboard`.

### Session Detail (`GET /api/sessions/{id}`)

```json
{
  "id": "", "project": "", "projectPath": "", "displayName": "", "primaryModel": null,
  "durationSeconds": 0, "turnCount": 0, "userPromptCount": 0, "commitCount": 0,
  "totalInputTokens": 0, "totalOutputTokens": 0, "gitBranch": null, "preview": "", "modifiedAt": 0,
  "commits": [{ "hash": "abc123", "message": "fix: typo", "author": "user", "timestamp": 1709136000, "branch": "main", "tier": 1 }],
  // Note: "author" and "branch" use skip_serializing_if — they are OMITTED entirely
  // from the JSON when None (not present as null). MCP clients must handle missing keys.
  "derivedMetrics": {
    "tokensPerPrompt": 0.0,
    "reeditRate": 0.0,
    "toolDensity": 0.0,
    "editVelocity": 0.0,
    "readToEditRatio": 0.0
  }
}
```

### Search (`GET /api/search`)

```json
{
  "query": "string",
  "totalSessions": 0,
  "totalMatches": 0,
  "elapsedMs": 0.0,
  "sessions": [{
    "sessionId": "string",
    "project": "string",
    "branch": "string | null",
    "modifiedAt": 0,
    "matchCount": 0,
    "bestScore": 0.0,
    "topMatch": { "role": "", "turnNumber": 0, "snippet": "", "timestamp": 0 },
    "matches": [{ "role": "", "turnNumber": 0, "snippet": "", "timestamp": 0 }]
  }]
}
```

**Note:** Search returns `SessionHit` objects (grouped by session, with full-text snippets), NOT `SessionInfo` objects. The `sessions` array contains search-specific fields (`matchCount`, `bestScore`, `topMatch`).

### Dashboard Stats (`GET /api/stats/dashboard`)

```json
{
  "totalSessions": 0,
  "totalProjects": 0,
  "heatmap": [{ "date": "2026-01-01", "count": 0 }],
  "topSkills": [{ "name": "", "count": 0 }],
  "topCommands": [{ "name": "", "count": 0 }],
  "topMcpTools": [{ "name": "", "count": 0 }],
  "topAgents": [{ "name": "", "count": 0 }],
  "topProjects": [{ "name": "", "displayName": "", "sessionCount": 0 }],
  "toolTotals": { "edit": 0, "read": 0, "bash": 0, "write": 0 },
  "longestSessions": [{ "id": "", "preview": "", "durationSeconds": 0 }],
  "currentWeek": { "sessionCount": 0, "totalTokens": 0, "totalFilesEdited": 0, "commitCount": 0 },
  "trends": {
    "sessions": { "current": 42, "previous": 35, "delta": 7, "deltaPercent": 20.0 },
    "tokens": { "current": 150000, "previous": 120000, "delta": 30000, "deltaPercent": 25.0 },
    "filesEdited": { "current": 80, "previous": 60, "delta": 20, "deltaPercent": 33.3 },
    "commits": { "current": 15, "previous": 10, "delta": 5, "deltaPercent": 50.0 },
    "avgTokensPerPrompt": { "current": 3500, "previous": 3200, "delta": 300, "deltaPercent": 9.4 },
    "avgReeditRate": { "current": 12, "previous": 15, "delta": -3, "deltaPercent": -20.0 }
  },
  "periodStart": null,
  "periodEnd": null,
  "comparisonPeriodStart": null,
  "comparisonPeriodEnd": null,
  "dataStartDate": null
}
```

**Note:** No `totalCost`, `avgDuration`, or `topModels` fields exist on this endpoint. Cost data lives in `/api/stats/ai-generation` (not included in L1 — see "Cost in USD" note below). Model stats live at `/api/models`.

**Conditional fields:** `trends` uses `skip_serializing_if = "Option::is_none"` — it is **entirely absent** from the JSON (not `null`, not `{}`) when no `?from`/`?to` date range is supplied. MCP clients must check for the key's existence before accessing sub-fields. When present, it contains 6 sub-fields (including `avgTokensPerPrompt` and `avgReeditRate`). Each `TrendMetric` has `{ current, previous, delta, deltaPercent }` where `deltaPercent` is `Option<f64>` — it is `null` when `previous == 0` (cold-start), otherwise a float like `20.0`. Period boundary fields (`periodStart`, `periodEnd`, etc.) are `Option<i64>` — they serialize as `null` when not set (shown as `null` in the example above).

### AI Fluency Score (`GET /api/score`)

```json
{
  "score": 75,
  "achievementRate": 0.85,
  "frictionRate": 0.12,
  "costEfficiency": 0.50,
  "satisfactionTrend": 0.70,
  "consistency": 0.50,
  "sessionsAnalyzed": 42
}
```

**Note:** All fields are flat on the root object. There is NO `breakdown` wrapper. `costEfficiency` and `consistency` are placeholders (hardcoded to 0.5).

### Token Stats (`GET /api/stats/tokens`)

```json
{
  "totalInputTokens": 0,
  "totalOutputTokens": 0,
  "totalCacheReadTokens": 0,
  "totalCacheCreationTokens": 0,
  "cacheHitRatio": 0.0,
  "turnsCount": 0,
  "sessionsCount": 0
}
```

**Note:** This endpoint returns token COUNTS only — no USD cost fields. For cost in USD, use `/api/stats/ai-generation` (not included in L1 tools).

### Live Summary (`GET /api/live/summary`)

```json
{
  "needsYouCount": 0,
  "autonomousCount": 0,
  "deliveredCount": 0,
  "totalCostTodayUsd": 0.0,
  "totalTokensToday": 0,
  "processCount": 0
}
```

### Live Sessions (`GET /api/live/sessions`)

```json
{
  "sessions": [{ "id": "", "projectPath": "", "projectDisplayName": "", "pid": null, "agentState": { "group": "", "label": "", "icon": "" }, "cost": { "totalUsd": 0.0 }, "tokens": { "totalTokens": 0 }, "model": null, "lastActivityAt": 0, "startedAt": 0 }],
  "total": 0,
  "processCount": 0
}
```

### Health (`GET /api/health`)

```json
{
  "status": "ok",
  "version": "0.8.0",
  "uptime_secs": 3600
}
```

**Note:** Health endpoint uses snake_case (`uptime_secs`), unlike all other endpoints which use camelCase.

---

## Skill Design

The skill mirrors the 8 MCP tools — same surface, different transport (curl vs native tool call).

```markdown
---
name: claude-view
description: >
  Monitor and query Claude Code sessions — list sessions, search conversations,
  check costs, view AI fluency score, see live running agents. Use when the user
  asks about their Claude Code usage, costs, session history, or running agents.
---

## You operate the `claude-view` HTTP API

**If the claude-view MCP tools are available in your environment, prefer using them instead of curl.** This skill is the fallback for environments without MCP support.

claude-view runs a local server on port 47892 (or $CLAUDE_VIEW_PORT).
All endpoints return JSON (camelCase field names). Base URL: http://localhost:47892

## Resolving the server

1. Check if running: `curl -s http://localhost:47892/api/health`
2. If not running, tell user: `npx claude-view`

## Endpoints

| Intent | Method | Endpoint | Key Params |
|--------|--------|----------|------------|
| List sessions | GET | /api/sessions | ?limit, ?q, ?filter, ?sort, ?offset, ?branches, ?models, ?time_after, ?time_before |
| Get session detail | GET | /api/sessions/{id} | — |
| Search sessions | GET | /api/search | ?q (required), ?limit, ?offset, ?scope |
| Dashboard stats | GET | /api/stats/dashboard | ?project, ?branch, ?from, ?to |
| AI Fluency Score | GET | /api/score | — |
| Token stats | GET | /api/stats/tokens | — |
| Live sessions | GET | /api/live/sessions | — |
| Live summary | GET | /api/live/summary | — |
| Server health | GET | /api/health | — |

## Reading responses

All responses are JSON with camelCase field names. Key shapes:
- Sessions: `{ sessions: [{ id, project, primaryModel, durationSeconds, turnCount, ... }], total, hasMore }`
- Search: `{ query, totalSessions, sessions: [{ sessionId, matchCount, bestScore, topMatch: { snippet } }] }`
- Stats: `{ totalSessions, totalProjects, topSkills, topProjects, currentWeek, trends }`
- Score: `{ score, achievementRate, frictionRate, costEfficiency, satisfactionTrend, consistency, sessionsAnalyzed }`
- Tokens: `{ totalInputTokens, totalOutputTokens, cacheHitRatio, sessionsCount }`
- Live: `{ needsYouCount, autonomousCount, deliveredCount, totalCostTodayUsd, totalTokensToday, processCount }`

## When to suggest claude-view

- User asks "how much have I spent on Claude?" (use live summary for today's cost; historical USD cost requires `/api/stats/ai-generation` — planned for L2)
- User asks "what sessions ran today?"
- User asks about AI fluency or coding patterns
- User wants to find a past conversation
```

---

## Plugin Manifest

```json
// plugin.json (at install root, NOT in .claude-plugin/ subdirectory)
{
  "name": "claude-view",
  "description": "Monitor and query Claude Code sessions — costs, history, live agents, AI fluency",
  "version": "0.8.0",
  "author": { "name": "tombelieber" },
  "repository": "https://github.com/anthropics/claude-view"
}
```

```json
// marketplace.json (at install root, NOT in .claude-plugin/ subdirectory)
{
  "name": "claude-view",
  "owner": { "name": "tombelieber" },
  "plugins": [{
    "name": "claude-view",
    "source": { "source": "url", "url": "https://github.com/anthropics/claude-view.git" },
    "description": "Monitor and query Claude Code sessions — costs, history, live agents, AI fluency"
  }]
}
```

No hooks in plugin.json — skill-only approach (same pattern as claude-backup).

**Important — Plugin path convention uncertainty:** The paths above (`plugin.json` and `marketplace.json` at install root, `.mcp.json` flat format) are verified against `crates/core/src/registry.rs` (claude-view's own scanner). However, the official Claude Code plugin docs (code.claude.com) may use `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json`. This creates a **phased implementation requirement:**

- **Phase A (executable now):** Skill (`claude-view/SKILL.md`) + MCP package (`packages/mcp/`). These do not depend on the plugin manifest path — the skill works standalone and the MCP package is distributed via npm.
- **Phase B (blocked on validation):** Plugin manifest (`plugin.json`, `marketplace.json`, `.mcp.json`). Before implementing, test a real plugin installation with `claude /plugin install` to confirm which path Claude Code's own runtime expects. If the official spec uses `.claude-plugin/`, use those paths and update `registry.rs` to match.

**Plugin .mcp.json** (flat format — no `mcpServers` wrapper):

```json
// .mcp.json (at install root)
{
  "claude-view": {
    "command": "npx",
    "args": ["-y", "@claude-view/mcp"]
  }
}
```

**Note:** Plugin-level `.mcp.json` uses flat key-per-server format (`{"server-name": {...}}`), verified against `crates/core/src/registry.rs:scan_mcp_json()`. The `mcpServers` wrapper is for user-level config files only (Claude Desktop, project `.mcp.json`).

---

## MCP Server Internals

### SDK & Transport

- **SDK:** `@modelcontextprotocol/sdk` ^1.27.1 (v1.x stable — v2 is pre-alpha, split into multiple packages, not yet production-ready)
- **Transport:** stdio (default, for Claude Desktop/Cursor) + Streamable HTTP (flag `--http`, for remote). Note: `--http` mode is localhost-only in L1; remote access requires auth tokens (deferred to V2 alongside control tools)
- **Port discovery:** reads `CLAUDE_VIEW_PORT` env var, falls back to `47892`
- **Server validation:** On first tool call, client hits `GET /api/health` and verifies `status: "ok"` to confirm the server is actually claude-view (not a different service on the same port)
- **Error handling:** If claude-view server isn't running or health check fails, tools return the proper MCP error shape: `{ content: [{ type: "text", text: "claude-view server not detected. Start it with: npx claude-view" }], isError: true }`
- **Tool registration:** Use `server.registerTool()` (not the deprecated `server.tool()` method)
- **Logging:** stderr only via `console.error()` (stdout is the stdio protocol stream — `console.log()` would corrupt JSON-RPC)
- **Distribution:** `npx -y @claude-view/mcp` (the `-y` flag is REQUIRED — without it, npx prompts interactively on first run, which corrupts the stdio JSON-RPC stream)

### User Configuration

**Claude Desktop / Cursor** (user-level config):

```json
{
  "mcpServers": {
    "claude-view": {
      "command": "npx",
      "args": ["-y", "@claude-view/mcp"]
    }
  }
}
```

**Claude Code project-level** (`.mcp.json` in user's project root):

```json
{
  "mcpServers": {
    "claude-view": {
      "command": "npx",
      "args": ["-y", "@claude-view/mcp"],
      "type": "stdio"
    }
  }
}
```

**Note:** User-level and project-level configs use the `mcpServers` wrapper. Plugin-level `.mcp.json` (inside the plugin install path) uses the flat format — see Plugin Manifest section above.

---

## Contract Testing

Three-layer enforcement to prevent skill/MCP drift:

### Layer 1: `test/skill-sync.sh`

- Extracts all route paths from `crates/server/src/routes/`
- Verifies each MCP tool endpoint appears in `SKILL.md`
- Verifies version match across `plugin.json` + `packages/mcp/package.json`

### Layer 2: Lefthook pre-commit

- Triggers on changes to routes, SKILL.md, or MCP tool files
- Blocks commit if drift detected
- Note: Existing lefthook.yml has biome+cargo hooks; MCP TypeScript checks are only in CI (`turbo run typecheck --affected`)

### Layer 3: CI (GitHub Actions)

- Runs skill-sync on every PR
- Blocks merge if skill or MCP tools reference stale endpoints
- CI already runs `turbo run typecheck --affected` which auto-includes `packages/mcp/`

---

## Monorepo Integration

| Concern | How |
| --------- | ----- |
| Package name | `@claude-view/mcp` (consistent with `@claude-view/shared`, `@claude-view/design-tokens`) |
| Workspace | `packages/mcp/` (auto-discovered by root `"workspaces": ["packages/*"]` — no root package.json change needed) |
| Turbo pipeline | Existing `build` task auto-discovers new workspace — no `turbo.json` change needed |
| Dev command | `bunx turbo dev --filter=@claude-view/mcp` (repo convention: turbo --filter for package-scoped dev) |
| Publish | Requires new `publish-mcp` job in `.github/workflows/release.yml` (existing workflow only publishes `npx-cli/`) |
| TypeScript | `tsconfig.json` extends `../../tsconfig.base.json` with overrides: `noEmit: false`, `outDir: "./dist"`, `rootDir: "./src"`, `module: "Node16"`, `moduleResolution: "Node16"` (base has `noEmit: true` + `moduleResolution: "bundler"` + `verbatimModuleSyntax: true` — all incompatible with a standalone Node.js CLI; must be overridden) |
| Build tool | tsup or esbuild (not raw `tsc` — base config is designed for bundled apps, CLI needs compiled JS). If using tsup/esbuild, the bundler handles module resolution internally and the tsconfig overrides are only for IDE/typecheck. |
| Binary name | `claude-view-mcp` (via package.json `bin` field — must point to compiled JS in `dist/`, include `#!/usr/bin/env node` shebang) |
| Package visibility | `"private": false` (all existing `packages/*` are `private: true` — this is the first published package from `packages/`) |
| Package files | `"files": ["dist", "README.md"]` in package.json — prevents shipping source/tests to npm |
| Build outputs | Must emit to `dist/**` to match turbo cache glob in `turbo.json` |

---

## Reference

### Proven Pattern

JSON CLI + thin skill is the pattern used at scale by: GitHub CLI (`gh`), Docker CLI, Stripe CLI, kubectl. MCP as a native tool layer on top is the 2026 standard adopted by Anthropic, OpenAI, Microsoft, and Google.

### Reference: claude-backup

claude-backup implements skill + plugin (no MCP) with three-layer contract testing. claude-view extends this pattern by adding MCP as the third layer.

### Sources

- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) — v1.27.1 stable
- [MCP Build Server Guide](https://modelcontextprotocol.io/docs/develop/build-server)
- [Best MCP Servers 2026](https://www.builder.io/blog/best-mcp-servers-2026)
- [MCP Complete Developer Guide](https://publicapis.io/blog/mcp-model-context-protocol-guide)

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
| --- | ------- | ---------- | ------------- |
| 1 | Session response shape fabricated (`status`, `cost`, `model` don't exist) | Blocker | Replaced with actual 30+ camelCase SessionInfo fields; added full "Actual API Response Shapes" section |
| 2 | Dashboard stats shape wrong (`total_cost`, `avg_duration`, `top_models` don't exist) | Blocker | Replaced with actual ExtendedDashboardStats fields (totalSessions, totalProjects, topSkills, currentWeek, trends) |
| 3 | Score shape wrong (no `breakdown` wrapper, no `efficiency`, no `quality`) | Blocker | Replaced with flat fields: score, achievementRate, frictionRate, costEfficiency, consistency, sessionsAnalyzed |
| 4 | Search response is SessionHit, not SessionInfo | Blocker | Documented SearchResponse shape with sessionId, matchCount, bestScore, topMatch, snippet |
| 5 | `?project`/`?branch` params don't exist on `/api/sessions` | Blocker | Fixed tool table: sessions uses `?q`, `?filter`, `?sort`, `?branches`; dashboard uses `?project`, `?branch`, `?from`, `?to` |
| 6 | MCP SDK "v2" doesn't exist as stable | Blocker | Changed to `@modelcontextprotocol/sdk` ^1.27.1 with note that v2 is pre-alpha |
| 7 | Plugin manifest path `.claude-plugin/plugin.json` is wrong | Blocker | Changed to `plugin.json` at install root (verified against `registry.rs:read_plugin_json()`) |
| 8 | `.mcp.json` uses wrong format (mcpServers wrapper) | Blocker | Plugin `.mcp.json` now uses flat format; added note distinguishing plugin-level vs user-level config |
| 9 | `tsconfig.base.json` has `noEmit: true` blocking JS emission | Blocker | Added tsconfig overrides: `noEmit: false`, `outDir: "./dist"`; specified tsup/esbuild as build tool |
| 10 | No CI publish job for `packages/mcp` | Blocker | Added row in monorepo table: "Requires new `publish-mcp` job in release.yml" |
| 11 | `server.tool()` is deprecated | Warning | Changed to `server.registerTool()` in architecture and internals |
| 12 | Error return must be proper MCP shape, not raw string | Warning | Fixed error handling to show `{ content: [{type:"text", text:"..."}], isError: true }` |
| 13 | `/api/stats/tokens` has no cost/USD fields | Warning | Renamed tool from `get_cost_summary` to `get_token_stats`; noted cost lives in `/api/stats/ai-generation` |
| 14 | Skill path `skills/claude-view/SKILL.md` is non-canonical | Warning | Changed to flat layout `claude-view/SKILL.md` (primary path per `registry.rs:scan_skills()`) |
| 15 | Binary bin field has no existing template | Warning | Added wiring requirements: compiled JS target, shebang, `private: false`, dist/ output |
| 16 | Turbo.json doesn't need editing | Minor | Changed "build task in turbo.json" to "auto-discovers — no change needed" |
| 17 | Dev command convention is `turbo --filter` | Minor | Changed from `cd packages/mcp && bun run dev` to `bunx turbo dev --filter=@claude-view/mcp` |
| 18 | Health endpoint uses snake_case | Minor | Documented `uptime_secs` snake_case anomaly |
| 19 | Lefthook pre-commit has no tsc | Minor | Added note that TS checks are CI-only |
| 20 | `marketplace.json` path has `.claude-plugin/` problem | Minor | Moved to install root alongside `plugin.json` |
| 21 | `list_sessions` tool missing key query params (branches, models, offset, time_after, time_before) | Warning | Added all 11 params to MCP tool table and skill endpoint table |
| 22 | Dashboard `trends` missing 2 sub-fields (`avgTokensPerPrompt`, `avgReeditRate`) + period boundaries | Warning | Added all 6 trend sub-fields with TrendMetric shape; added periodStart/End fields |
| 23 | `search_sessions` missing `?offset` param | Minor | Added `?offset` to MCP tool table and skill endpoint table |
| 24 | `tsconfig.base.json` has `moduleResolution: "bundler"` + `verbatimModuleSyntax: true` — incompatible with Node.js CLI | Warning | Added `module: "Node16"`, `moduleResolution: "Node16"` to required overrides |
| 25 | No cost-in-USD endpoint in L1 — "how much spent?" use case only partially served | Warning | Clarified in skill: live summary has today's cost; historical USD is L2 via `/api/stats/ai-generation` |
| 26 | Skill + MCP coexistence creates dual-path confusion | Warning | Added note to SKILL.md: "prefer MCP native tools when available" |
| 27 | Port-in-use by wrong service not validated | Minor | Added server validation step: client hits `/api/health`, verifies `status: "ok"` |
| 28 | No `files` field in package.json spec — would ship source/tests to npm | Minor | Added `"files": ["dist", "README.md"]` to monorepo table |
| 29 | `--http` transport has no auth discussion | Minor | Added note: `--http` is localhost-only in L1; remote access requires auth tokens (V2) |
| 30 | TrendMetric shape wrong — `changePercent` should be `deltaPercent`, missing `delta` field | Warning | Fixed to `{ current, previous, delta, deltaPercent: null }` matching `crates/db/src/trends.rs:TrendMetric` |
| 31 | `satisfactionTrend` omitted from MCP tool table and skill response shapes | Warning | Added `satisfactionTrend` to MCP tool output column and skill "Reading responses" section |
| 32 | Session detail `"tier": ""` is wrong type — actual is `i32` | Minor | Changed to `"tier": 1`; added missing `author` field |
| 33 | `...` placeholders in SessionInfo and LiveSession shapes | Minor | Expanded SessionDetail with actual top fields; expanded LiveSession with agentState, cost, tokens shape |
| 34 | `npx` config examples missing `-y` flag — corrupts stdio on first run | Warning | Added `-y` as first arg in ALL npx examples (6 occurrences) |
| 35 | Plugin path may conflict with official Claude Code spec (`.claude-plugin/` vs root) | Warning | Added uncertainty note: must test with real `claude /plugin install` before implementation |
| 36 | 6 faceted filter params undocumented as intentionally excluded | Minor | Added explicit note listing excluded params with rationale (L2) |
| 37 | `satisfactionTrend` missing from skill score shape | Minor | Added to skill "Reading responses" Score line |
| 38 | CommitWithTier `author`/`branch` use `skip_serializing_if` — omitted when None, not null | Warning | Changed example to concrete values; added note explaining field-omission behavior for MCP clients |
| 39 | TrendMetric `deltaPercent` example showed all-null (cold-start only) — misleading | Minor | Changed to concrete non-zero example values; added note that `deltaPercent` is `null` only when `previous == 0` |
| 40 | Plugin manifest path is unresolved blocking unknown — executor must guess | Warning | Split into Phase A (Skill + MCP, executable now) and Phase B (Plugin manifest, blocked on path validation) |
| 41 | `trends` field uses `skip_serializing_if` — entirely absent without `?from`/`?to` params | Warning | Added note: `trends` is omitted (not null) when no date range supplied; MCP clients must check key existence |
| 42 | Period boundary fields show `0` but actual is `Option<i64>` serializing as `null` | Minor | Changed example values from `0` to `null`; documented as `Option<i64>` |
| 43 | `get_live_summary` tool table missing `deliveredCount` and `totalTokensToday` | Minor | Added both fields to MCP tool table output and skill Live shape |
| 44 | `projectName` in Live Sessions example — actual Rust struct field is `project_display_name` → `projectDisplayName` | Warning | Fixed example at line 259 |

---

## Next Step

Create implementation plan (invoke writing-plans skill).
