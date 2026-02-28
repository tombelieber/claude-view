# Claude View Plugin — Implementation Plan

> **Status:** DONE (2026-03-01) — all 11 tasks implemented, shippable audit passed (SHIP IT)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a Claude Code plugin (`@claude-view/plugin`) that auto-starts the web dashboard, bundles 8 MCP tools, and adds 3 skills — replacing the standalone `@claude-view/mcp` distribution.

**Architecture:** Plugin lives in `packages/plugin/` as a new Turborepo workspace. It bundles the MCP server from `packages/mcp/` (now private) into its own `dist/`. A `SessionStart` hook auto-starts the Rust server. Three skills (`/session-recap`, `/daily-cost`, `/standup`) provide guided queries.

**Tech Stack:** Claude Code Plugin format, shell scripts (hooks), Markdown (skills), Node.js >= 18 (bundled MCP server), Turborepo build pipeline.

**Design doc:** `docs/plans/cross-cutting/2026-03-01-claude-view-plugin-design.md`

**Prerequisites:** Node.js >= 18 (required by `@modelcontextprotocol/sdk`).

### Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `1e11f29d` | Demote `packages/mcp/` to private workspace |
| 2 | `c1abe375` | Scaffold `@claude-view/plugin` package |
| 3 | `7a1b0171` | Add MCP bundle script and `.mcp.json` config |
| 4 | `b502105b` | Add SessionStart hook — auto-start server |
| 5-7 | `83287118` | Add `/session-recap`, `/daily-cost`, `/standup` skills |
| 8 | `cd717b19` | Add validation script for plugin structure |
| 9 | — | Verified Turborepo wiring (no changes needed) |
| 10 | — | Local testing passed (build, npm pack, validation) |
| 11 | `43594da0` | Sync design doc with implementation |

**Shippable audit:** 4/4 passes green. MCP 24/24 tests pass, plugin 13/13 validation checks pass, Rust compiles clean, npm pack 33 files (11.5kB). All 7 wiring paths verified. 0 blockers.

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
mkdir -p packages/plugin/scripts
mkdir -p packages/plugin/dist
```

**Step 2: Create `packages/plugin/package.json`**

Note: `@claude-view/mcp` is NOT listed as a dependency (it is private and won't exist on npm). Instead, the MCP server's runtime deps (`@modelcontextprotocol/sdk`, `zod`) are listed directly — these are the bare-specifier imports that the bundled `dist/` files need at runtime.

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
    "prepublishOnly": "node scripts/bundle-mcp.mjs",
    "typecheck": "echo 'no TS in plugin'",
    "lint": "echo 'markdown + json only'",
    "test": "node scripts/bundle-mcp.mjs && bash scripts/validate-plugin.sh"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.27.1",
    "zod": "^3.24"
  },
  "engines": {
    "node": ">=18"
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

- **Node.js >= 18** (required by MCP SDK)
- The plugin auto-starts the claude-view server, but the binary must be available:

\`\`\`bash
npx claude-view   # downloads the pre-built Rust binary on first run (~5-15s first time)
\`\`\`

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
Expected: `packages/plugin` appears in workspace list. `@modelcontextprotocol/sdk` and `zod` resolve.

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

This script copies the built MCP server entry point from `packages/mcp/dist/` into `packages/plugin/dist/`. Turborepo guarantees `packages/mcp` builds first (via `^build` dependency). Includes an existence guard for clear error messages.

Create `packages/plugin/scripts/bundle-mcp.mjs`:

```javascript
import { cpSync, existsSync, mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const pluginRoot = resolve(__dirname, '..')
const mcpDist = resolve(pluginRoot, '..', 'mcp', 'dist')
const pluginDist = resolve(pluginRoot, 'dist')

// Guard: ensure packages/mcp was built first
if (!existsSync(mcpDist)) {
  console.error(`ERROR: packages/mcp/dist not found at ${mcpDist}`)
  console.error('Run: bun run build --filter=@claude-view/mcp first')
  process.exit(1)
}

mkdirSync(pluginDist, { recursive: true })

// Copy entire MCP dist — preserves internal imports (server.js, client.js, tools/)
cpSync(mcpDist, pluginDist, { recursive: true })

console.log(`Bundled MCP server → ${pluginDist}/`)
```

**Step 2: Create `packages/plugin/.mcp.json`**

Note: The `.mcp.json` format for Claude Code plugins uses the same schema as user MCP config. `${CLAUDE_PLUGIN_ROOT}` is the standard env var for resolving paths relative to the plugin directory. If Claude Code doesn't expand this in `.mcp.json` args, test with the flat format (no `mcpServers` wrapper) as a fallback — see Task 10 for verification.

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

**Fallback if `mcpServers` wrapper doesn't work in plugins** (test in Task 10):
```json
{
  "claude-view": {
    "type": "stdio",
    "command": "node",
    "args": ["${CLAUDE_PLUGIN_ROOT}/dist/index.js"],
    "env": {}
  }
}
```

**Step 3: Test the build pipeline**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.claude/worktrees/monorepo-expo && bun run build --filter=@claude-view/plugin`
Expected: `packages/mcp` builds first (tsc), then `packages/plugin` copies dist. `packages/plugin/dist/index.js` exists.

**Step 4: Verify the bundled server runs standalone**

Run: `node packages/plugin/dist/index.js 2>&1 | head -1`
Expected: Output contains `claude-view MCP server starting` (it will fail to connect to stdio transport in this context, but the import/startup works).

**Important:** This test passes in the monorepo because Bun's hoisted linker puts `@modelcontextprotocol/sdk` and `zod` in the root `node_modules/`. When published to npm, these resolve from the plugin's own `node_modules/` because they are listed as direct dependencies in `package.json`. This local test does not prove the published version works — Task 10 verifies that.

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

Format matches the canonical superpowers plugin structure. Key details:
- `matcher: "startup|resume|clear|compact"` — fires on all session lifecycle events. The health check exits in ~2ms if the server is already running, so including `clear|compact` has zero UX cost but catches crash-restart scenarios.
- `async: false` — hook runs synchronously so the 3s wait loop completes before the session starts (MCP tools need the server up).
- No `bash` wrapper — the shebang `#!/usr/bin/env bash` handles interpreter selection. Direct reference avoids quoting issues if `CLAUDE_PLUGIN_ROOT` contains spaces.

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

**Step 2: Create `packages/plugin/hooks/start-server.sh`**

Key design decisions:
- `set -uo pipefail` (no `-e`) — we want the script to always reach `exit 0`. Using `-e` would abort on `nohup` failure (which is outside an `if` guard).
- `npx` PATH resolution — searches common Node.js version manager paths (nvm, volta, fnm, asdf, Homebrew, system) before invoking. Under Claude Code hooks, shell init files (`.bashrc`, `.zshrc`) are not sourced, so version-managed `npx` may not be in `$PATH`.
- `CLAUDE_VIEW_NO_OPEN=1` — suppresses browser tab opening when started from a hook. Without this, every cold session start pops a browser tab (the Rust server calls `open::that()` in `main.rs`).

```bash
#!/usr/bin/env bash
# claude-view SessionStart hook — ensure server is running
# Non-blocking: ALWAYS exits 0, even if server fails to start.
# Uses set -uo pipefail (no -e) so the script never aborts early.

set -uo pipefail

PORT="${CLAUDE_VIEW_PORT:-47892}"
HEALTH_URL="http://localhost:${PORT}/api/health"

# Check if already running (curl inside if-guard: safe with any set flags)
if curl -sf --max-time 2 "${HEALTH_URL}" >/dev/null 2>&1; then
  exit 0
fi

# Locate npx — version managers (nvm, volta, fnm, asdf) put it outside system PATH.
# Claude Code hooks run with limited env, so we search common locations.
find_npx() {
  # 1. Already in PATH?
  command -v npx 2>/dev/null && return
  # 2. Common version manager locations
  for candidate in \
    "${HOME}/.volta/bin/npx" \
    "${HOME}/.local/share/fnm/aliases/default/bin/npx" \
    "/usr/local/bin/npx" \
    "/opt/homebrew/bin/npx"; do
    [ -x "$candidate" ] && echo "$candidate" && return
  done
  # 3. nvm — glob for the default version
  for candidate in "${HOME}"/.nvm/versions/node/*/bin/npx; do
    [ -x "$candidate" ] && echo "$candidate" && return
  done
}

NPX="$(find_npx)"
if [ -z "$NPX" ]; then
  # npx not found — cannot auto-start server. Tools will show a clear error.
  exit 0
fi

# Not running — start in background.
# CLAUDE_VIEW_NO_OPEN=1 suppresses browser tab open from the hook context.
# || true ensures nohup failure (e.g. binary download timeout) never propagates.
CLAUDE_VIEW_NO_OPEN=1 nohup "$NPX" claude-view >/dev/null 2>&1 &

# Wait up to 3 seconds for server to become healthy.
# On first-ever run, binary download takes 5-30s — this will time out. That's fine:
# the server starts in the background and will be ready for the next tool call or session.
for _ in 1 2 3; do
  sleep 1
  if curl -sf --max-time 1 "${HEALTH_URL}" >/dev/null 2>&1; then
    exit 0
  fi
done

# Server didn't start in time — don't block the session.
exit 0
```

**Step 3: Make the script executable**

```bash
chmod +x packages/plugin/hooks/start-server.sh
```

**Step 4: Add `CLAUDE_VIEW_NO_OPEN` support to the Rust server**

In `crates/server/src/main.rs`, find the `open::that(&browse_url)` call and guard it:

```rust
// Only open browser if not suppressed (hook starts set CLAUDE_VIEW_NO_OPEN=1)
if std::env::var("CLAUDE_VIEW_NO_OPEN").unwrap_or_default() != "1" {
    let _ = open::that(&browse_url);
}
```

Run: `cargo test -p claude-view-server`
Expected: Tests pass (this is a trivial env var guard).

**Step 5: Test the hook script manually**

```bash
CLAUDE_VIEW_NO_OPEN=1 bash packages/plugin/hooks/start-server.sh && echo "Hook exited 0"
```

Expected: Either server starts (check `curl localhost:47892/api/health`) or exits 0 regardless. Hook must never block or pop a browser.

**Step 6: Commit**

```bash
git add packages/plugin/hooks/ crates/server/src/main.rs
git commit -m "feat(plugin): add SessionStart hook — auto-start claude-view server"
```

---

### Task 5: Create `/session-recap` skill

**Files:**
- Create: `packages/plugin/skills/session-recap/SKILL.md`

**Step 1: Write the skill**

Note: Tool names are qualified as `mcp__claude-view__<tool>` — this is the MCP namespace format Claude Code uses when multiple MCP servers are registered. Skills must use the full qualified name so Claude unambiguously finds the right tool.

Create `packages/plugin/skills/session-recap/SKILL.md`:

```markdown
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

Note: `mcp__claude-view__get_live_summary` returns aggregate data only (needs_attention count, autonomous count, total_cost_today_usd). For per-session details in the "Running sessions" section, the skill must also call `mcp__claude-view__list_live_sessions`.

Create `packages/plugin/skills/daily-cost/SKILL.md`:

```markdown
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

Note: Instead of asking Claude to compute a Unix timestamp for `time_after` (unreliable), the skill uses `sort: "recent"` with `limit: 20` and filters by the `modified` field (ISO 8601 string) in the response. This avoids timestamp arithmetic entirely.

Create `packages/plugin/skills/standup/SKILL.md`:

```markdown
---
name: standup
description: "Use when the user asks for a standup update, work log, or activity summary — e.g. 'standup update', 'what did I work on today', 'work log', 'daily summary'"
---

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

Note: The `check_json` function uses `--input-type=commonjs` because `packages/plugin` has `"type": "module"` — without this flag, `node -e` evaluates as ESM where `require()` is undefined.

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
  # --input-type=commonjs: required because package.json has "type": "module"
  # which makes node -e evaluate as ESM where require() is undefined
  if ! node --input-type=commonjs -e "JSON.parse(require('fs').readFileSync('$PLUGIN_ROOT/$1','utf8'))" 2>/dev/null; then
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
Expected: The `test` script first runs the bundle step, then the validation. All checks pass.

**Step 3: Commit**

```bash
git add packages/plugin/scripts/validate-plugin.sh
git commit -m "feat(plugin): add validation script for plugin structure"
```

---

### Task 9: Wire into Turborepo and verify full build

**Files:**
- Verify: `turbo.json` (no change needed)

**Step 1: Verify Turbo picks up the new workspace**

The root `package.json` already has `"workspaces": ["apps/*", "packages/*"]`, so `packages/plugin` is automatically a workspace. The `turbo.json` `build` task has `"dependsOn": ["^build"]` — since `packages/plugin` lists no workspace deps in its published `dependencies`, Turbo won't auto-order mcp before plugin via `^build`. However, the `test` script in `packages/plugin/package.json` runs `node scripts/bundle-mcp.mjs` (which copies from `packages/mcp/dist/`) as its first step, ensuring the bundle is fresh before validation.

For CI, add a build step that ensures mcp builds before plugin:

Run: `bun run build --filter=@claude-view/mcp && bun run build --filter=@claude-view/plugin`
Expected: MCP compiles (tsc), then plugin copies dist. Both succeed.

**Step 2: Run the validation**

Run: `cd packages/plugin && bun test`
Expected: All checks pass (files exist, JSON valid, script executable).

**Step 3: Verify the full workspace still builds**

Run: `bun run build`
Expected: All apps and packages build successfully, including the new plugin.

**Step 4: Commit (only if changes were needed)**

```bash
git add turbo.json
git commit -m "chore: verify @claude-view/plugin wired into Turborepo pipeline"
```

---

### Task 10: Local plugin testing with Claude Code

**No files to create — manual testing.**

**Step 1: Ensure the Rust server is built**

Run: `cargo build -p claude-view-server`

**Step 2: Build the full plugin**

Run: `bun run build --filter=@claude-view/mcp && bun run build --filter=@claude-view/plugin`

**Step 3: Test plugin installation locally**

Run: `claude plugin add /Users/TBGor/dev/@vicky-ai/claude-view/.claude/worktrees/monorepo-expo/packages/plugin`

Expected: Plugin installs. Claude Code reports the plugin name and components found.

**Step 4: Verify `.mcp.json` format**

If the MCP server does NOT appear in the plugin's tool list:
1. Try the fallback `.mcp.json` format (flat, no `mcpServers` wrapper — see Task 3 Step 2)
2. Verify `${CLAUDE_PLUGIN_ROOT}` is expanded correctly in the args
3. Check `claude --debug` output for MCP server startup errors

**Step 5: Verify in a new Claude Code session**

Start a new `claude` session. Verify:

1. **Hook fires:** The SessionStart hook runs `start-server.sh`. Check that the Rust server starts (or is already running). No browser tab should open.
2. **MCP tools available:** Ask Claude "what tools do you have from claude-view?" — should list the 8 tools (prefixed as `mcp__claude-view__*`).
3. **Skills available:** Type `/session-recap` — should trigger the skill.
4. **Tool works:** Ask "how much did I spend today?" — should call `mcp__claude-view__get_live_summary` and return data.

**Step 6: Commit any fixes found during testing**

```bash
git add -A packages/plugin/
git commit -m "fix(plugin): adjustments from local testing"
```

---

### Task 11: Final cleanup and integration commit

**Files:**
- Modify: `docs/plans/PROGRESS.md` (update status)
- Modify: `docs/plans/cross-cutting/2026-03-01-claude-view-plugin-design.md` (fix design doc discrepancy)

**Step 1: Fix design doc filename discrepancy**

The design doc references `dist/mcp-server.js` but the actual bundle script produces `dist/index.js`. Update the design doc to match.

**Step 2: Update PROGRESS.md**

Add entry for the plugin work.

**Step 3: Final commit**

```bash
git add docs/plans/PROGRESS.md docs/plans/cross-cutting/2026-03-01-claude-view-plugin-design.md
git commit -m "docs: update progress — @claude-view/plugin complete"
```

---

## How to Undo

If the plugin causes issues:
1. `claude plugin remove claude-view` — removes the plugin from Claude Code
2. `git revert <commit-range>` — reverts all plugin commits
3. Set `"private": false` back in `packages/mcp/package.json` and restore `"bin"` if re-publishing standalone

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `workspace:*` dep on private `@claude-view/mcp` → 404 on npm install | Blocker | Removed `@claude-view/mcp` from deps. Added `@modelcontextprotocol/sdk` + `zod` directly. |
| 2 | Missing runtime deps (`@modelcontextprotocol/sdk`, `zod`) → `ERR_MODULE_NOT_FOUND` | Blocker | Listed both as direct dependencies in plugin's `package.json`. |
| 3 | `require()` in `validate-plugin.sh` fails under ESM `"type": "module"` | Blocker | Added `--input-type=commonjs` flag to `node -e` invocations. |
| 4 | `npx` not in PATH under nvm/volta/fnm/asdf | Blocker | Added `find_npx()` function that searches common version manager paths. |
| 5 | `.mcp.json` may need flat format (no `mcpServers` wrapper) | Blocker | Provided both formats; Task 10 Step 4 tests and selects the correct one. |
| 6 | Skills use bare tool names (`get_session`) not MCP-namespaced | Warning | Changed all skills to use `mcp__claude-view__<tool_name>` qualified names. |
| 7 | `npx claude-view` opens browser tab from hook | Warning | Added `CLAUDE_VIEW_NO_OPEN=1` env var in hook; Task 4 Step 4 adds Rust guard. |
| 8 | hooks.json missing `"async": false` | Warning | Added `"async": false` to hook entry, matching superpowers canonical format. |
| 9 | hooks.json uses `bash "..."` wrapper | Warning | Changed to bare `${CLAUDE_PLUGIN_ROOT}/hooks/start-server.sh` with shebang. |
| 10 | Matcher `startup\|resume` excludes `clear\|compact` | Warning | Changed to `startup\|resume\|clear\|compact` to catch crash-restart. |
| 11 | `/daily-cost` formats per-session data from `get_live_summary` (aggregates only) | Warning | Added `list_live_sessions` call for per-session details in the Running sessions section. |
| 12 | `/standup` asks Claude to compute Unix timestamp | Warning | Changed to `sort: "recent"` + `limit: 20` with `modified` field filtering (ISO 8601 string). |
| 13 | `set -euo pipefail` + `nohup npx` — needs error guard | Warning | Changed to `set -uo pipefail` (no `-e`) + `\|\| true` after nohup. |
| 14 | No `prepublishOnly` safety net for build | Warning | Added `"prepublishOnly": "node scripts/bundle-mcp.mjs"` to package.json. |
| 15 | Node minimum should be 18, not 16 | Warning | Added `"engines": { "node": ">=18" }` to package.json. Updated plan header. |
| 16 | Bundle script has no existence guard before `cpSync` | Warning | Added `existsSync` check with clear error message. |
| 17 | Turbo test doesn't run plugin's own build first | Warning | Changed `test` script to `"node scripts/bundle-mcp.mjs && bash scripts/validate-plugin.sh"`. |
| 18 | Hoisted linker masks missing deps in local test | Warning | Added documentation in Task 3 Step 4 explaining the false-positive risk. |
| 19 | Design doc says `dist/mcp-server.js`, impl says `dist/index.js` | Minor | Task 11 Step 1 fixes the design doc. |
| 20 | First-run download takes >3s, hook times out | Minor | Documented in README and as comments in `start-server.sh`. |
