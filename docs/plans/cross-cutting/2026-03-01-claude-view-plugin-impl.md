# Claude View Plugin — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a Claude Code plugin (`@claude-view/plugin`) that auto-starts the web dashboard, bundles 8 MCP tools, and adds 3 skills — replacing the standalone `@claude-view/mcp` distribution.

**Architecture:** Plugin lives in `packages/plugin/` as a new Turborepo workspace. It bundles the MCP server from `packages/mcp/` (now private) into its own `dist/`. A `SessionStart` hook auto-starts the Rust server. Three skills (`/session-recap`, `/daily-cost`, `/standup`) provide guided queries.

**Tech Stack:** Claude Code Plugin format, shell scripts (hooks), Markdown (skills), Node.js (bundled MCP server), Turborepo build pipeline.

**Design doc:** `docs/plans/cross-cutting/2026-03-01-claude-view-plugin-design.md`

---

### Task 1: Demote `packages/mcp/` to private workspace

**Files:**
- Modify: `packages/mcp/package.json`

**Step 1: Set private and remove bin**

In `packages/mcp/package.json`, set `"private": true` and remove the `"bin"` field. The package is now only consumed as a workspace dependency by `packages/plugin/`.

```json
{
  "name": "@claude-view/mcp",
  "version": "0.8.0",
  "description": "MCP server for claude-view — internal package, bundled into @claude-view/plugin",
  "private": true,
  "type": "module",
  "main": "./dist/index.js",
  "files": ["dist"],
  "scripts": {
    "build": "tsc",
    "dev": "tsc --watch",
    "typecheck": "tsc --noEmit",
    "lint": "biome check .",
    "test": "bun test"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.27.1",
    "zod": "^3.24"
  },
  "devDependencies": {
    "typescript": "^5.7",
    "@types/node": "^22"
  }
}
```

**Step 2: Verify MCP package still builds**

Run: `cd packages/mcp && bun run build`
Expected: Clean compile, `dist/` generated.

**Step 3: Run existing MCP tests**

Run: `cd packages/mcp && bun test`
Expected: All tests pass (no behavior change).

**Step 4: Commit**

```bash
git add packages/mcp/package.json
git commit -m "chore(mcp): demote to private workspace — bundled into plugin"
```

---

### Task 2: Create plugin scaffold

**Files:**
- Create: `packages/plugin/package.json`
- Create: `packages/plugin/.claude-plugin/plugin.json`
- Create: `packages/plugin/README.md`

**Step 1: Create directory structure**

```bash
mkdir -p packages/plugin/.claude-plugin
mkdir -p packages/plugin/skills/session-recap
mkdir -p packages/plugin/skills/daily-cost
mkdir -p packages/plugin/skills/standup
mkdir -p packages/plugin/hooks
mkdir -p packages/plugin/dist
```

**Step 2: Create `packages/plugin/package.json`**

```json
{
  "name": "@claude-view/plugin",
  "version": "0.8.0",
  "description": "Claude Code plugin for claude-view — auto-starts web dashboard, provides session/cost/fluency tools and skills",
  "private": false,
  "type": "module",
  "files": [
    ".claude-plugin/",
    ".mcp.json",
    "skills/",
    "hooks/",
    "dist/",
    "README.md"
  ],
  "scripts": {
    "build": "node scripts/bundle-mcp.mjs",
    "typecheck": "echo 'no TS in plugin'",
    "lint": "echo 'markdown + json only'",
    "test": "bash scripts/validate-plugin.sh"
  },
  "dependencies": {
    "@claude-view/mcp": "workspace:*"
  },
  "keywords": [
    "claude-code",
    "claude-code-plugin",
    "claude-view",
    "mission-control",
    "session-analytics",
    "cost-tracking"
  ],
  "repository": {
    "type": "git",
    "url": "https://github.com/tombelieber/claude-view",
    "directory": "packages/plugin"
  },
  "license": "MIT"
}
```

**Step 3: Create `packages/plugin/.claude-plugin/plugin.json`**

```json
{
  "name": "claude-view",
  "description": "Mission Control for Claude Code — auto-starts a web dashboard, provides session/cost/fluency tools, and adds /session-recap, /daily-cost, /standup skills.",
  "version": "0.8.0",
  "author": {
    "name": "tombelieber",
    "email": "nicholasgee1997@gmail.com"
  },
  "homepage": "https://github.com/tombelieber/claude-view",
  "repository": "https://github.com/tombelieber/claude-view",
  "license": "MIT",
  "keywords": [
    "claude-code",
    "mission-control",
    "session-analytics",
    "cost-tracking",
    "fluency-score",
    "dashboard"
  ]
}
```

**Step 4: Create stub `packages/plugin/README.md`**

```markdown
# claude-view plugin

Mission Control for Claude Code. Auto-starts a web dashboard, provides 8 session/cost/fluency
tools, and adds `/session-recap`, `/daily-cost`, `/standup` skills.

## Install

\`\`\`bash
claude plugin add @claude-view/plugin
\`\`\`

## Prerequisites

The plugin auto-starts the claude-view server, but the binary must be available:

\`\`\`bash
npx claude-view   # downloads the pre-built Rust binary on first run
\`\`\`

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
| `get_fluency_score` | AI Fluency Score (0–100) |
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

## License

MIT
```

**Step 5: Verify workspace resolution**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.claude/worktrees/monorepo-expo && bun install`
Expected: `packages/plugin` appears in workspace list, `@claude-view/mcp` resolves.

**Step 6: Commit**

```bash
git add packages/plugin/package.json packages/plugin/.claude-plugin/plugin.json packages/plugin/README.md
git commit -m "feat(plugin): scaffold @claude-view/plugin package"
```

---

### Task 3: Create MCP bundle script and `.mcp.json`

**Files:**
- Create: `packages/plugin/scripts/bundle-mcp.mjs`
- Create: `packages/plugin/.mcp.json`

**Step 1: Create the bundle script**

This script copies the built MCP server entry point from `packages/mcp/dist/` into `packages/plugin/dist/`. Turborepo guarantees `packages/mcp` builds first (via `^build` dependency).

Create `packages/plugin/scripts/bundle-mcp.mjs`:

```javascript
import { cpSync, mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const pluginRoot = resolve(__dirname, '..')
const mcpDist = resolve(pluginRoot, '..', 'mcp', 'dist')
const pluginDist = resolve(pluginRoot, 'dist')

mkdirSync(pluginDist, { recursive: true })

// Copy entire MCP dist — preserves internal imports (server.js, client.js, tools/)
cpSync(mcpDist, pluginDist, { recursive: true })

console.log(`Bundled MCP server → ${pluginDist}/`)
```

**Step 2: Create `packages/plugin/.mcp.json`**

```json
{
  "mcpServers": {
    "claude-view": {
      "type": "stdio",
      "command": "node",
      "args": ["${CLAUDE_PLUGIN_ROOT}/dist/index.js"],
      "env": {}
    }
  }
}
```

Note: `${CLAUDE_PLUGIN_ROOT}` is the standard plugin path variable. The MCP server's `ClaudeViewClient` reads `CLAUDE_VIEW_PORT` from env (default 47892).

**Step 3: Test the build pipeline**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.claude/worktrees/monorepo-expo && bun run build --filter=@claude-view/plugin`
Expected: `packages/mcp` builds first (tsc), then `packages/plugin` copies dist. `packages/plugin/dist/index.js` exists.

**Step 4: Verify the bundled server runs standalone**

Run: `node packages/plugin/dist/index.js 2>&1 | head -1`
Expected: Output contains `claude-view MCP server starting` (it will fail to connect to stdio transport in this context, but the import/startup works).

**Step 5: Commit**

```bash
git add packages/plugin/scripts/bundle-mcp.mjs packages/plugin/.mcp.json
git commit -m "feat(plugin): add MCP bundle script and .mcp.json config"
```

---

### Task 4: Create SessionStart hook

**Files:**
- Create: `packages/plugin/hooks/hooks.json`
- Create: `packages/plugin/hooks/start-server.sh`

**Step 1: Create `packages/plugin/hooks/hooks.json`**

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/start-server.sh\""
          }
        ]
      }
    ]
  }
}
```

Matcher `startup|resume` — fires on new sessions and resumed sessions. Does NOT fire on `clear` or `compact` (server is already running in those cases).

**Step 2: Create `packages/plugin/hooks/start-server.sh`**

```bash
#!/usr/bin/env bash
# claude-view SessionStart hook — ensure server is running
# Non-blocking: exits 0 even if server fails to start

set -euo pipefail

PORT="${CLAUDE_VIEW_PORT:-47892}"
HEALTH_URL="http://localhost:${PORT}/api/health"

# Check if already running
if curl -sf --max-time 2 "${HEALTH_URL}" >/dev/null 2>&1; then
  exit 0
fi

# Not running — start in background
# Use npx claude-view which downloads/runs the pre-built Rust binary
nohup npx claude-view >/dev/null 2>&1 &

# Wait up to 3 seconds for server to become healthy
for i in 1 2 3; do
  sleep 1
  if curl -sf --max-time 1 "${HEALTH_URL}" >/dev/null 2>&1; then
    exit 0
  fi
done

# Server didn't start in time — don't block the session
# It may still be starting up; tools will retry on their own
exit 0
```

**Step 3: Make the script executable**

```bash
chmod +x packages/plugin/hooks/start-server.sh
```

**Step 4: Test the hook script manually**

Run the script to verify it works (assumes server is not running):

```bash
bash packages/plugin/hooks/start-server.sh && echo "Hook exited 0"
```

Expected: Either server starts (check `curl localhost:47892/api/health`) or exits 0 regardless. Hook must never block.

**Step 5: Commit**

```bash
git add packages/plugin/hooks/
git commit -m "feat(plugin): add SessionStart hook — auto-start claude-view server"
```

---

### Task 5: Create `/session-recap` skill

**Files:**
- Create: `packages/plugin/skills/session-recap/SKILL.md`

**Step 1: Write the skill**

Create `packages/plugin/skills/session-recap/SKILL.md`:

```markdown
---
name: session-recap
description: "Use when the user asks to recap, summarize, or review a Claude Code session — e.g. 'recap my last session', 'what happened in that session', 'session summary'"
---

# Session Recap

Summarize a Claude Code session using claude-view data.

## Steps

1. **Identify the session.** If the user specified a session ID, use it. Otherwise, call `list_sessions` with `limit: 5` to show recent sessions and ask which one to recap. If the user says "last session" or "most recent", use the first result.

2. **Fetch session details.** Call `get_session` with the session ID.

3. **Present the recap** in this format:

```
## Session Recap: [project name] — [branch]

**Duration:** X minutes | **Model:** [model] | **Turns:** [count]

### What was done
[2-3 sentence summary based on commits and session preview]

### Commits
- `abc1234` — commit message
- `def5678` — commit message

### Metrics
- Input tokens: X | Output tokens: Y | Cache hits: Z
- Cost efficiency: [from derived_metrics if available]
```

4. **Keep it concise.** The recap should fit in one screen. If there are more than 5 commits, show the top 5 and note "and N more".
```

**Step 2: Commit**

```bash
git add packages/plugin/skills/session-recap/
git commit -m "feat(plugin): add /session-recap skill"
```

---

### Task 6: Create `/daily-cost` skill

**Files:**
- Create: `packages/plugin/skills/daily-cost/SKILL.md`

**Step 1: Write the skill**

Create `packages/plugin/skills/daily-cost/SKILL.md`:

```markdown
---
name: daily-cost
description: "Use when the user asks about cost, spending, or budget — e.g. 'how much did I spend today', 'daily cost', 'cost report', 'what's my spend'"
---

# Daily Cost Report

Show the user's Claude Code spending for today using claude-view data.

## Steps

1. **Get live summary.** Call `get_live_summary` to get today's aggregate cost and active session count.

2. **Get dashboard stats.** Call `get_stats` with `from` set to today's date (ISO 8601, e.g. `2026-03-01`) to get today's session breakdown.

3. **Present the cost report** in this format:

```
## Daily Cost Report — [today's date]

**Total spent today:** $X.XX USD
**Sessions today:** N | **Currently running:** M

### Running sessions
- [project] — [model] — $X.XX — [agent state]
(from get_live_summary data)

### Token usage today
- Input: X tokens | Output: Y tokens
- Cache read: Z tokens
```

4. **If total cost is $0.00**, say "No Claude Code usage detected today" and suggest checking if the claude-view server has indexed recent sessions.

5. **If the user asks about a different time range** (e.g. "this week", "last month"), use the `from` and `to` parameters on `get_stats` accordingly.
```

**Step 2: Commit**

```bash
git add packages/plugin/skills/daily-cost/
git commit -m "feat(plugin): add /daily-cost skill"
```

---

### Task 7: Create `/standup` skill

**Files:**
- Create: `packages/plugin/skills/standup/SKILL.md`

**Step 1: Write the skill**

Create `packages/plugin/skills/standup/SKILL.md`:

```markdown
---
name: standup
description: "Use when the user asks for a standup update, work log, or activity summary — e.g. 'standup update', 'what did I work on today', 'work log', 'daily summary'"
---

# Standup Update

Generate a standup-style work summary from recent Claude Code sessions.

## Steps

1. **Fetch recent sessions.** Call `list_sessions` with:
   - `sort: "recent"`
   - `limit: 20`
   - `time_after`: Unix timestamp for 24 hours ago (calculate from current time)

   If the user asks about a different period (e.g. "this week"), adjust `time_after` accordingly.

2. **For the top 3-5 sessions by duration**, call `get_session` on each to get commit details.

3. **Present the standup** in this format:

```
## Standup — [today's date]

### Done
- **[project] ([branch])** — [1-line summary from commits/preview] (Xm, $Y.YY)
- **[project] ([branch])** — [1-line summary] (Xm, $Y.YY)

### In Progress
- [any sessions still running, from list_live_sessions if available]

### Metrics
- Sessions: N | Total time: Xh Ym | Commits: Z
```

4. **Keep each item to one line.** The standup should be copy-pasteable into Slack or a standup bot.

5. **If no sessions found in the time range**, say "No Claude Code sessions found in the last 24 hours."
```

**Step 2: Commit**

```bash
git add packages/plugin/skills/standup/
git commit -m "feat(plugin): add /standup skill"
```

---

### Task 8: Create plugin validation script

**Files:**
- Create: `packages/plugin/scripts/validate-plugin.sh`

**Step 1: Create the validation script**

This is the `test` script for the plugin package. It validates structure, JSON syntax, and required files.

Create `packages/plugin/scripts/validate-plugin.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERRORS=0

check() {
  if [ ! -e "$PLUGIN_ROOT/$1" ]; then
    echo "FAIL: missing $1"
    ERRORS=$((ERRORS + 1))
  else
    echo "  OK: $1"
  fi
}

check_json() {
  if ! node -e "JSON.parse(require('fs').readFileSync('$PLUGIN_ROOT/$1','utf8'))" 2>/dev/null; then
    echo "FAIL: invalid JSON in $1"
    ERRORS=$((ERRORS + 1))
  else
    echo "  OK: $1 (valid JSON)"
  fi
}

echo "=== claude-view plugin validation ==="
echo ""

echo "--- Required files ---"
check ".claude-plugin/plugin.json"
check ".mcp.json"
check "hooks/hooks.json"
check "hooks/start-server.sh"
check "dist/index.js"
check "skills/session-recap/SKILL.md"
check "skills/daily-cost/SKILL.md"
check "skills/standup/SKILL.md"
check "README.md"

echo ""
echo "--- JSON validation ---"
check_json ".claude-plugin/plugin.json"
check_json ".mcp.json"
check_json "hooks/hooks.json"

echo ""
echo "--- Executable check ---"
if [ -x "$PLUGIN_ROOT/hooks/start-server.sh" ]; then
  echo "  OK: hooks/start-server.sh is executable"
else
  echo "FAIL: hooks/start-server.sh is not executable"
  ERRORS=$((ERRORS + 1))
fi

echo ""
if [ "$ERRORS" -eq 0 ]; then
  echo "All checks passed."
else
  echo "$ERRORS check(s) failed."
  exit 1
fi
```

**Step 2: Make executable and test**

```bash
chmod +x packages/plugin/scripts/validate-plugin.sh
```

Run: `cd packages/plugin && bun test`
Expected: After a full build (`bun run build --filter=@claude-view/plugin`), all checks pass. Before build, `dist/index.js` check will fail (expected — that's the build artifact).

**Step 3: Commit**

```bash
git add packages/plugin/scripts/validate-plugin.sh
git commit -m "feat(plugin): add validation script for plugin structure"
```

---

### Task 9: Wire into Turborepo and verify full build

**Files:**
- Modify: `turbo.json` (only if pipeline changes needed)

**Step 1: Verify Turbo picks up the new workspace**

The root `package.json` already has `"workspaces": ["apps/*", "packages/*"]`, so `packages/plugin` is automatically a workspace. The `turbo.json` `build` task has `"dependsOn": ["^build"]` which means `packages/plugin` build will wait for `packages/mcp` build. No turbo.json change needed.

Run: `bun run build --filter=@claude-view/plugin`
Expected: `packages/mcp` builds (tsc), then `packages/plugin` builds (copies dist). Both succeed.

**Step 2: Run the validation**

Run: `cd packages/plugin && bun test`
Expected: All checks pass (files exist, JSON valid, script executable).

**Step 3: Verify the full workspace still builds**

Run: `bun run build`
Expected: All apps and packages build successfully, including the new plugin.

**Step 4: Commit (if any turbo.json changes were needed)**

```bash
git add turbo.json
git commit -m "chore: wire @claude-view/plugin into Turborepo pipeline"
```

---

### Task 10: Local plugin testing with Claude Code

**No files to create — manual testing.**

**Step 1: Ensure the Rust server is built**

Run: `cargo build -p claude-view-server`

**Step 2: Build the full plugin**

Run: `bun run build --filter=@claude-view/plugin`

**Step 3: Test plugin installation locally**

Run: `claude plugin add /Users/TBGor/dev/@vicky-ai/claude-view/.claude/worktrees/monorepo-expo/packages/plugin`

Expected: Plugin installs. Claude Code reports the plugin name and components found.

**Step 4: Verify in a new Claude Code session**

Start a new `claude` session. Verify:

1. **Hook fires:** The SessionStart hook runs `start-server.sh`. Check that the Rust server starts (or is already running).
2. **MCP tools available:** Ask Claude "what tools do you have from claude-view?" — should list the 8 tools.
3. **Skills available:** Type `/session-recap` — should trigger the skill.
4. **Tool works:** Ask "how much did I spend today?" — should call `get_live_summary` and return data.

**Step 5: Commit any fixes found during testing**

```bash
git add -A packages/plugin/
git commit -m "fix(plugin): adjustments from local testing"
```

---

### Task 11: Final cleanup and integration commit

**Files:**
- Modify: `docs/plans/PROGRESS.md` (update status)

**Step 1: Update PROGRESS.md**

Add entry for the plugin work.

**Step 2: Final commit**

```bash
git add docs/plans/PROGRESS.md
git commit -m "docs: update progress — @claude-view/plugin complete"
```
