<div align="center">

# claude-view

**Mission Control for Claude Code**

You have 10 AI agents running. One finished 12 minutes ago. Another hit its context limit. A third needs tool approval. You're <kbd>Cmd</kbd>+<kbd>Tab</kbd>-bing through terminals, burning $200/mo blind.

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/docs-claudeview.ai-orange" alt="Website"></a>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg?label=plugin" alt="plugin version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

<p>
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a> ·
  <a href="./README.ja.md">日本語</a> ·
  <a href="./README.es.md">Español</a> ·
  <a href="./README.fr.md">Français</a> ·
  <a href="./README.de.md">Deutsch</a> ·
  <a href="./README.pt.md">Português</a> ·
  <a href="./README.it.md">Italiano</a> ·
  <a href="./README.ko.md">한국어</a> ·
  <a href="./README.nl.md">Nederlands</a>
</p>

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

**One command. Every session visible. Real-time.**

</div>

---

## What Is claude-view?

claude-view is an open-source dashboard that monitors every Claude Code session on your machine — live agents, past conversations, costs, sub-agents, hooks, tool calls — in one place. Rust backend, React frontend, ~10 MB binary. Zero config, zero accounts, 100% local.

**50+ releases. 85 MCP tools. 9 skills. One `npx claude-view`.**

---

## Live Monitor

See every running session at a glance. No more terminal tab-switching.

| Feature | What it does |
|---------|-------------|
| **Session cards** | Each card shows the last message, model, cost, and status — instantly know what every agent is working on |
| **Multi-session chat** | Open sessions side-by-side in VS Code-style tabs (dockview). Drag to split horizontally or vertically |
| **Context gauge** | Real-time context window fill per session — see which agents are in the danger zone before they hit the limit |
| **Cache countdown** | Know exactly when prompt cache expires so you can time messages to save tokens |
| **Cost tracking** | Per-session and aggregate spend with token breakdown — hover for input/output/cache split by model |
| **Sub-agent tree** | See the full tree of spawned agents, their status, costs, and what tools they're calling |
| **Notification sounds** | Get pinged when a session finishes, errors, or needs your input — stop polling terminals |
| **Multiple views** | Grid, List, Kanban, Monitor, or Harness mode — pick what fits your workflow |
| **Kanban swimlanes** | Group sessions by project or branch — visual swimlane layout for multi-project workflows |
| **Recently closed** | Sessions that end appear in "Recently Closed" instead of vanishing — persists across server restarts |
| **Queued messages** | Messages waiting in the queue show as pending bubbles with a "Queued" badge |
| **SSE-driven** | All live data pushed via Server-Sent Events — eliminates stale-cache risks entirely |

---

## Chat & Conversation

Read, search, and interact with any session — live or historical.

| Feature | What it does |
|---------|-------------|
| **Unified live chat** | History and real-time messages in a single scrollable conversation — no tab-switching |
| **Developer mode** | Toggle between Chat and Developer views per session. Developer mode shows tool cards, event cards, hook metadata, and the full execution trace with filter chips |
| **Full conversation browser** | Every session, every message, fully rendered with markdown and code blocks |
| **Tool call visualization** | See file reads, edits, bash commands, MCP calls, skill invocations — not just text |
| **Compact / verbose toggle** | Skim the conversation or drill into every tool call |
| **Thread view** | Follow agent conversations with sub-agent hierarchies and indented threading |
| **Hook events inline** | Pre/post tool hooks rendered as conversation blocks — see hooks firing alongside the conversation |
| **Export** | Markdown export for context resumption or sharing |
| **Bulk select & archive** | Select multiple sessions for batch archiving with persistent filter state |
| **Encrypted sharing** | Share any session via E2E encrypted link — AES-256-GCM, zero server trust, key lives only in the URL fragment |

---

## Agent Internals

Claude Code does a lot behind `"thinking..."` that never shows in your terminal. claude-view exposes all of it.

| Feature | What it does |
|---------|-------------|
| **Sub-agent conversations** | Full tree of spawned agents, their prompts, outputs, and per-agent cost/token breakdown |
| **MCP server calls** | Which MCP tools are being invoked and their results |
| **Skill / hook / plugin tracking** | Which skills fired, which hooks ran, what plugins are active |
| **Hook event recording** | Dual-channel hook capture (live WebSocket + JSONL backfill) — every event recorded and browsable, even for past sessions |
| **Session source badges** | Each session shows how it was started: Terminal, VS Code, Agent SDK, or other entrypoints |
| **Worktree branch drift** | Detects when git worktree branches diverge — shown in live monitor and history |
| **@File mention chips** | `@filename` references extracted and shown as chips — hover for full path |
| **Tool use timeline** | Action log of every tool_use/tool_result pair with timing |
| **Error surfacing** | Errors bubble up to the session card — no buried failures |
| **Raw message inspector** | Drill into any message's raw JSON when you need the full picture |

---

## Search

| Feature | What it does |
|---------|-------------|
| **Full-text search** | Search across all sessions — messages, tool calls, file paths. Powered by Tantivy (Rust-native, Lucene-class) |
| **Unified search engine** | Tantivy full-text + SQLite pre-filter run in parallel — one endpoint, sub-50ms results |
| **Project & branch filters** | Scope to the project or branch you're working on right now |
| **Command palette** | <kbd>Cmd</kbd>+<kbd>K</kbd> to jump between sessions, switch views, find anything |

---

## Analytics

A full analytics suite for your Claude Code usage. Think Cursor's dashboard, but deeper.

<details>
<summary><strong>Dashboard</strong></summary>
<br>

| Feature | Description |
|---------|-------------|
| **Week-over-week metrics** | Session count, token usage, cost — compared to your previous period |
| **Activity heatmap** | 90-day GitHub-style grid showing daily usage intensity |
| **Top skills / commands / MCP tools / agents** | Leaderboards of your most-used invocables — click any to search matching sessions |
| **Most active projects** | Bar chart of projects ranked by session count |
| **Tool usage breakdown** | Total edits, reads, and bash commands across all sessions |
| **Longest sessions** | Quick access to your marathon sessions with duration |

</details>

<details>
<summary><strong>AI Contributions</strong></summary>
<br>

| Feature | Description |
|---------|-------------|
| **Code output tracking** | Lines added/removed, files touched, commit count — across all sessions |
| **Cost ROI metrics** | Cost per commit, cost per session, cost per line of AI output — with trend charts |
| **Model comparison** | Side-by-side breakdown of output and efficiency by model (Opus, Sonnet, Haiku) |
| **Learning curve** | Re-edit rate over time — see yourself getting better at prompting |
| **Branch breakdown** | Collapsible per-branch view with session drill-down |
| **Skill effectiveness** | Which skills actually improve your output vs which don't |

</details>

<details>
<summary><strong>Insights</strong> <em>(experimental)</em></summary>
<br>

| Feature | Description |
|---------|-------------|
| **Pattern detection** | Behavioral patterns discovered from your session history |
| **Then vs Now benchmarks** | Compare your first month to recent usage |
| **Category breakdown** | Treemap of what you use Claude for — refactoring, features, debugging, etc. |
| **AI Fluency Score** | Single 0-100 number tracking your overall effectiveness |

> Insights and Fluency Score are experimental. Treat as directional, not definitive.

</details>

---

## Plans, Prompts & Teams

| Feature | What it does |
|---------|-------------|
| **Plan browser** | View your `.claude/plans/` directly in session detail — no more hunting through files |
| **Prompt history** | Full-text search across all prompts you've sent with template clustering and intent classification |
| **Teams dashboard** | See team leads, inbox messages, team tasks, and file changes across all team members |
| **Prompt analytics** | Leaderboards of prompt templates, intent distribution, and usage statistics |

---

## System Monitor

| Feature | What it does |
|---------|-------------|
| **Live CPU / RAM / Disk gauges** | Real-time system metrics streaming via SSE with smooth animated transitions |
| **Component dashboard** | See sidecar and on-device AI metrics: VRAM usage, CPU, RAM, and session count per component |
| **Process list** | Processes grouped by name, sorted by CPU — see what your machine is actually doing while agents run |

---

## On-Device AI

Run a local LLM for session phase classification — no API calls, no extra cost.

| Feature | What it does |
|---------|-------------|
| **Provider-agnostic** | Connect to any OpenAI-compatible endpoint — oMLX, Ollama, LM Studio, or your own server |
| **Model selector** | Choose from a curated model registry with RAM requirements shown |
| **Phase classification** | Sessions tagged with their current phase (coding, debugging, planning, etc.) using confidence-gated display |
| **Smart resource management** | EMA-stabilized classification with exponential backoff — 93% GPU waste reduction vs naive polling |

---

## Plugin

`@claude-view/plugin` gives Claude native access to your dashboard data — 85 MCP tools, 9 skills, and auto-start.

```bash
claude plugin marketplace add tombelieber/claude-view
claude plugin install claude-view
```

### Auto-start

Every Claude Code session automatically starts the dashboard. No manual `npx claude-view` needed.

### 85 MCP tools

8 hand-crafted tools with optimized output for Claude:

| Tool | Description |
|------|-------------|
| `list_sessions` | Browse sessions with filters |
| `get_session` | Full session detail with messages and metrics |
| `search_sessions` | Full-text search across all conversations |
| `get_stats` | Dashboard overview — total sessions, costs, trends |
| `get_fluency_score` | AI Fluency Score (0-100) with breakdown |
| `get_token_stats` | Token usage with cache hit ratio |
| `list_live_sessions` | Currently running agents (real-time) |
| `get_live_summary` | Aggregate cost and status for today |

Plus **77 auto-generated tools** from the OpenAPI spec across 26 categories (contributions, insights, coaching, exports, workflows, and more).

### 9 Skills

| Skill | What it does |
|-------|-------------|
| `/session-recap` | Summarize a specific session — commits, metrics, duration |
| `/daily-cost` | Today's spending, running sessions, token usage |
| `/standup` | Multi-session work log for standup updates |
| `/coaching` | AI coaching tips and custom rule management |
| `/insights` | Behavioral pattern analysis |
| `/project-overview` | Project summary across sessions |
| `/search` | Natural language search |
| `/export-data` | Export sessions to CSV/JSON |
| `/team-status` | Team activity overview |

---

## Workflows

| Feature | What it does |
|---------|-------------|
| **Workflow builder** | Create multi-stage workflows with VS Code-style layout, Mermaid diagram preview, and YAML editor |
| **Streaming LLM chat rail** | Generate workflow definitions in real time via embedded chat |
| **Stage runner** | Visualize stage columns, attempt cards, and progress bar as your workflow executes |
| **Built-in seed workflows** | Plan Polisher and Plan Executor ship out of the box |

---

## Open in IDE

| Feature | What it does |
|---------|-------------|
| **One-click file open** | Files referenced in sessions open directly in your editor |
| **Auto-detects your editor** | VS Code, Cursor, Zed, and others — no configuration needed |
| **Everywhere it matters** | Button appears in Changes tab, file headers, and Kanban project headers |
| **Preference memory** | Your preferred editor is remembered across sessions |

---

## How It's Built

| | |
|---|---|
| **Fast** | Rust backend with SIMD-accelerated JSONL parsing, memory-mapped I/O — indexes thousands of sessions in seconds |
| **Real-time** | File watcher + SSE + multiplexed WebSocket with heartbeat, event replay, and crash recovery |
| **Tiny** | ~10 MB download, ~27 MB on disk. No runtime dependencies, no background daemons |
| **100% local** | All data stays on your machine. Zero telemetry by default, zero required accounts |
| **Zero config** | `npx claude-view` and you're done. No API keys, no setup, no accounts |
| **FSM-driven** | Chat sessions run on a finite state machine with explicit phases and typed events — deterministic, race-free |

<details>
<summary><strong>The Numbers</strong></summary>
<br>

Measured on an M-series Mac with 1,493 sessions across 26 projects:

| Metric | claude-view | Typical Electron dashboard |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150-300 MB |
| **On disk** | **~27 MB** | 300-500 MB |
| **Startup** | **< 500 ms** | 3-8 s |
| **RAM (full index)** | **~50 MB** | 300-800 MB |
| **Index 1,500 sessions** | **< 1 s** | N/A |
| **Runtime deps** | **0** | Node.js + Chromium |

Key techniques: SIMD pre-filter (`memchr`), memory-mapped JSONL parsing, Tantivy full-text search, zero-copy slices from mmap through parse to response.

</details>

---

## How It Compares

| Tool | Category | Stack | Size | Live monitor | Multi-session chat | Search | Analytics | MCP tools |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **Yes** | **Yes** | **Yes** | **Yes** | **85** |
| [opcode](https://github.com/winfunc/opcode) | GUI + session manager | Tauri 2 | ~13 MB | Partial | No | No | Yes | No |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI usage tracker | TypeScript | ~600 KB | No | No | No | CLI | No |
| [CodePilot](https://github.com/op7418/CodePilot) | Desktop chat UI | Electron | ~140 MB | No | No | No | No | No |
| [claude-run](https://github.com/kamranahmedse/claude-run) | History viewer | TypeScript | ~500 KB | Partial | No | Basic | No | No |

> Chat UIs (CodePilot, CUI, claude-code-webui) are interfaces *for* Claude Code. claude-view is a dashboard that watches your existing terminal sessions. They're complementary.

---

## Installation

| Method | Command |
|--------|---------|
| **Shell** (recommended) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (auto-start) | `claude plugin marketplace add tombelieber/claude-view && claude plugin install claude-view` |

The shell installer downloads a pre-built binary (~10 MB), installs to `~/.claude-view/bin`, and adds it to your PATH. Then just run `claude-view`.

**Only requirement:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed.

<details>
<summary><strong>Configuration</strong></summary>
<br>

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` or `PORT` | `47892` | Override the default port |

</details>

<details>
<summary><strong>Self-Hosting & Local Dev</strong></summary>
<br>

The pre-built binary ships with auth, sharing, and mobile relay baked in. Building from source? These features are **opt-in via environment variables** — omit any and that feature is simply disabled.

| Env Variable | Feature | Without it |
|-------------|---------|------------|
| `SUPABASE_URL` | Login / auth | Auth disabled — fully local, zero-account mode |
| `RELAY_URL` | Mobile pairing | QR pairing unavailable |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Encrypted sharing | Share button hidden |

```bash
bun dev    # fully local, no cloud dependencies
```

</details>

<details>
<summary><strong>Enterprise / Sandbox Environments</strong></summary>
<br>

If your machine restricts writes (DataCloak, CrowdStrike, corporate DLP):

```bash
cp crates/server/.env.example .env
# Uncomment CLAUDE_VIEW_DATA_DIR
```

This keeps database, search index, and lock files inside the repo. Set `CLAUDE_VIEW_SKIP_HOOKS=1` to skip hook registration in read-only environments.

</details>

---

## FAQ

<details>
<summary><strong>"Not signed in" banner showing even though I'm logged in</strong></summary>
<br>

claude-view checks your Claude credentials by reading `~/.claude/.credentials.json` (with macOS Keychain fallback). Try these steps:

1. **Verify Claude CLI auth:** `claude auth status`
2. **Check credentials file:** `cat ~/.claude/.credentials.json` — should have a `claudeAiOauth` section with an `accessToken`
3. **Check macOS Keychain:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Check token expiry:** Look at `expiresAt` in the credentials JSON — if past, run `claude auth login`
5. **Check HOME:** `echo $HOME` — the server reads from `$HOME/.claude/.credentials.json`

If all checks pass and the banner persists, report it on [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>What data does claude-view access?</strong></summary>
<br>

claude-view reads the JSONL session files that Claude Code writes to `~/.claude/projects/`. It indexes them locally using SQLite and Tantivy. **No data leaves your machine** unless you explicitly use the encrypted sharing feature. Telemetry is opt-in and off by default.

</details>

<details>
<summary><strong>Does it work with Claude Code in VS Code / Cursor / IDE extensions?</strong></summary>
<br>

Yes. claude-view monitors all Claude Code sessions regardless of how they were started — terminal CLI, VS Code extension, Cursor, or Agent SDK. Each session shows a source badge (Terminal, VS Code, SDK) so you can filter by launch method.

</details>

---

## Community

- **Website:** [claudeview.ai](https://claudeview.ai) — docs, changelog, blog
- **Discord:** [Join the server](https://discord.gg/G7wdZTpRfu) — support, feature requests, discussion
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 85 MCP tools, 9 skills, auto-start

---

<details>
<summary><strong>Development</strong></summary>
<br>

Prerequisites: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Install all workspace dependencies
bun dev            # Start full-stack dev (Rust + Web + Sidecar with hot reload)
```

### Workspace Layout

| Path | Package | Purpose |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA (Vite) — main web frontend |
| `apps/share/` | `@claude-view/share` | Share viewer SPA — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo native app |
| `apps/landing/` | `@claude-view/landing` | Astro 5 landing page (zero client-side JS) |
| `packages/shared/` | `@claude-view/shared` | Shared types & theme tokens |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Colors, spacing, typography |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code plugin (MCP server + tools + skills) |
| `crates/` | — | Rust backend (Axum) |
| `sidecar/` | — | Node.js sidecar (Agent SDK bridge) |
| `infra/share-worker/` | — | Cloudflare Worker — share API (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — install script with download tracking |

### Dev Commands

| Command | Description |
|---------|-------------|
| `bun dev` | Full-stack dev — Rust + Web + Sidecar with hot reload |
| `bun run dev:web` | Web frontend only |
| `bun run dev:server` | Rust backend only |
| `bun run build` | Build all workspaces |
| `bun run preview` | Build web + serve via release binary |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | TypeScript type checking |
| `bun run test` | Run all tests (Turbo) |
| `bun run test:rust` | Run Rust tests |
| `bun run storybook` | Launch Storybook for component development |
| `bun run dist:test` | Build + pack + install + run (full dist test) |

### Releasing

```bash
bun run release          # patch bump
bun run release:minor    # minor bump
git push origin main --tags    # triggers CI → builds → auto-publishes to npm
```

</details>

---

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | Available |
| macOS (Intel) | Available |
| Linux (x64) | Planned |
| Windows (x64) | Planned |

---

## Related

- **[claudeview.ai](https://claudeview.ai)** — Official website, docs, and changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Claude Code plugin with 85 MCP tools and 9 skills. `claude plugin marketplace add tombelieber/claude-view && claude plugin install claude-view`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code deletes your sessions after 30 days. This saves them. `npx claude-backup`

---

<div align="center">

If **claude-view** helps you see what your AI agents are doing, consider giving it a star.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
