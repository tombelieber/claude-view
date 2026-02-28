# Claude View Plugin — Design Doc

**Date:** 2026-03-01
**Status:** Done (implemented 2026-03-01)
**Scope:** New `packages/plugin/` workspace; demote `packages/mcp/` to private

---

## Problem

Claude View currently has two disconnected distribution channels:

1. `npx claude-view` — starts the Rust server + web UI
2. `@claude-view/mcp` — standalone MCP server (manual `settings.json` config)

Users must discover both, install both, and wire them together. The MCP package is undiscoverable — no skills, no hooks, no plugin ecosystem presence.

Meanwhile, Claude Code plugins offer single-command install with bundled tools, skills, hooks, and automatic discovery. This is the 2026 SOTA distribution for Claude Code extensions.

## Decision

**Ship a single Claude Code plugin (`@claude-view/plugin`) that:**

1. Auto-starts the Rust server via a `SessionStart` hook (zero-config companion)
2. Bundles the MCP tools (8 tools, read-only, proxied to localhost Rust API)
3. Adds 3 skills for common queries (`/session-recap`, `/daily-cost`, `/standup`)
4. Drops `@claude-view/mcp` as a published npm package (demote to private workspace dep)

**npm packages after this change:**

| Package | Purpose | Published? |
|---------|---------|-----------|
| `claude-view` (npx-cli/) | Rust binary wrapper | Yes (unchanged) |
| `@claude-view/plugin` (packages/plugin/) | Plugin — hooks + MCP tools + skills | Yes (NEW) |
| `@claude-view/mcp` (packages/mcp/) | MCP server source code | No — private, bundled into plugin |

## Architecture

### Two UX Surfaces

The plugin serves two complementary surfaces:

1. **Web dashboard** (browser at `localhost:47892`) — Cursor-like monitoring, session list, cost tracking, fluency score. Visual, always-on, non-blocking.
2. **Agent tools** (MCP, in-terminal) — Claude can query session data, costs, and live status conversationally. "How much did I spend today?" → `get_live_summary` tool call.

Both surfaces talk to the same Rust server API. The plugin makes both available with zero setup.

### Data Flow

```
Claude Code session starts
  → Plugin SessionStart hook fires
  → Hook: curl localhost:47892/api/health
    → Running? Skip.
    → Not running? Spawn: npx claude-view (background)
  → MCP server (bundled in plugin) registered via .mcp.json
  → Claude has 8 tools available + 3 skills
  → User has web dashboard at localhost:47892
```

### Component Map

```
packages/plugin/                    # @claude-view/plugin
  .claude-plugin/
    plugin.json                     # Manifest: name, version, description
  skills/
    session-recap/SKILL.md          # /session-recap — summarize a session
    daily-cost/SKILL.md             # /daily-cost — today's spend + live sessions
    standup/SKILL.md                # /standup — multi-session work log
  hooks/
    hooks.json                      # SessionStart → start-server.sh
    start-server.sh                 # Health check + background spawn
  .mcp.json                         # MCP config → ${CLAUDE_PLUGIN_ROOT}/dist/index.js
  dist/
    index.js                        # Bundled from packages/mcp/dist at build time
  package.json                      # deps: @modelcontextprotocol/sdk, zod
  README.md

packages/mcp/                       # @claude-view/mcp (PRIVATE)
  package.json                      # "private": true
  src/
    index.ts                        # Entry point (stdio transport)
    server.ts                       # MCP server setup, tool registration
    client.ts                       # HTTP client → localhost:47892
    tools/
      sessions.ts                   # list_sessions, get_session, search_sessions
      stats.ts                      # get_stats, get_fluency_score, get_token_stats
      live.ts                       # list_live_sessions, get_live_summary
```

## Plugin Components — Detail

### 1. SessionStart Hook (`hooks/`)

**Purpose:** Auto-start the claude-view Rust server when a Claude Code session begins.

**`hooks.json`:**
```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume|clear|compact",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/start-server.sh",
            "async": false
          }
        ]
      }
    ]
  }
}
```

**`start-server.sh` logic:**
1. `curl -sf http://localhost:${CLAUDE_VIEW_PORT:-47892}/api/health` (2s timeout)
2. If healthy → exit 0 (already running)
3. If not → `npx claude-view &` (background, detached)
4. Wait up to 3s for health check to pass
5. Exit 0 (non-blocking — don't hold up the session)

**Key constraints:**
- Hook must be fast (< 5s total). Rust server starts in ~50ms, so 3s wait is generous.
- Hook must not block the session if server fails to start.
- Uses `CLAUDE_VIEW_PORT` env var for custom port (default 47892).

### 2. MCP Tools (`.mcp.json` + bundled server)

**`.mcp.json`:**
```json
{
  "mcpServers": {
    "claude-view": {
      "type": "stdio",
      "command": "node",
      "args": ["${CLAUDE_PLUGIN_ROOT}/dist/index.js"]
    }
  }
}
```

**8 tools (all read-only, proxied to Rust API):**

| Tool | Endpoint | Purpose |
|------|----------|---------|
| `list_sessions` | `/api/sessions` | List/filter/paginate sessions |
| `get_session` | `/api/sessions/:id` | Full session detail + commits |
| `search_sessions` | `/api/search` | Full-text search across sessions |
| `get_stats` | `/api/stats/dashboard` | Overview: projects, skills, trends |
| `get_fluency_score` | `/api/score` | AI Fluency Score (0–100) |
| `get_token_stats` | `/api/stats/tokens` | Token usage breakdown |
| `list_live_sessions` | `/api/live/sessions` | Currently running sessions |
| `get_live_summary` | `/api/live/summary` | Aggregate: cost today, attention count |

All tools have `readOnlyHint: true`, `destructiveHint: false`.

### 3. Skills (`skills/`)

**`/session-recap`** — Summarize a specific session.
- Trigger: "recap my last session", "what happened in session X", "summarize session"
- Uses: `get_session` tool, formats commits + metrics + duration
- Output: Markdown summary with key decisions, files touched, cost

**`/daily-cost`** — Today's spending and activity.
- Trigger: "how much did I spend today", "daily cost", "cost report"
- Uses: `get_live_summary` + `get_stats` tools
- Output: Cost breakdown, session count, running sessions, comparison to yesterday

**`/standup`** — Multi-session work log for standup updates.
- Trigger: "standup update", "what did I work on", "work log"
- Uses: `list_sessions` (last 24h) + `get_session` for top 3-5
- Output: Bullet points per session — project, branch, commits, duration

### 4. Plugin Manifest

**`.claude-plugin/plugin.json`:**
```json
{
  "name": "claude-view",
  "version": "0.8.0",
  "description": "Mission Control for Claude Code — auto-starts a web dashboard, provides session/cost/fluency tools, and adds /session-recap, /daily-cost, /standup skills.",
  "author": {
    "name": "tombelieber",
    "url": "https://github.com/tombelieber/claude-view"
  }
}
```

## Build Pipeline

Turborepo build order:

```
packages/mcp (build)  →  packages/plugin (build)
```

**`packages/plugin/` build script:**
1. `tsc` (if any TS in plugin — likely not needed for v1)
2. Copy `packages/mcp/dist/*` → `packages/plugin/dist/` (preserves internal imports)
3. Ensure `hooks/start-server.sh` is executable

**npm publish:** Only `packages/plugin/` publishes. `files` in package.json:
```json
{
  "files": [
    ".claude-plugin/",
    "skills/",
    "hooks/",
    ".mcp.json",
    "dist/",
    "README.md"
  ]
}
```

## Migration Path

### What changes in `packages/mcp/`

1. Set `"private": true` in `package.json`
2. Remove `"bin"` field (no longer a standalone binary)
3. Keep all source code, tests, build scripts unchanged
4. Turbo still builds it (plugin depends on its output)

### What's new (`packages/plugin/`)

1. New Turborepo workspace in `package.json` and `turbo.json`
2. All files listed in Component Map above
3. Build script that bundles MCP dist

### What's removed

1. `@claude-view/mcp` is unpublished from npm (or deprecated with message pointing to plugin)
2. Any docs referencing standalone MCP installation

## Prerequisites / Constraints

- **Rust server must be installed:** `npx claude-view` must work (downloads pre-built binary). The plugin hook calls this.
- **Port 47892:** Default, overridable via `CLAUDE_VIEW_PORT`. Both hook and MCP client respect this.
- **Node.js >= 18:** Required for the MCP server (bundled JS).
- **Claude Code:** This plugin is Claude Code-only. Codex support is planned for a future release.

## Not In Scope (v1)

- Hooks for cost alerts (PostToolUse) — add in v2 once usage patterns emerge
- Codex integration — next release
- Auto-open browser — server already does this on first start
- Plugin settings / `.local.md` — not needed for v1 (port override is env var)

## Success Criteria

1. `claude plugin add @claude-view/plugin` installs cleanly
2. Starting a Claude Code session auto-launches the web dashboard
3. All 8 MCP tools work inside Claude Code conversations
4. `/session-recap`, `/daily-cost`, `/standup` skills trigger correctly
5. Zero manual configuration required (no editing `settings.json`)
