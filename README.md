<div align="center">

# claude-view

**You have 10 Claude sessions running right now. What are they doing?**

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
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

</div>

Behind every "thinking..." spinner, Claude is spawning sub-agents, calling MCP servers, running skills, firing hooks — and you can't see any of it. You <kbd>Cmd</kbd>+<kbd>Tab</kbd> through 15 terminals trying to remember which session was doing what. A cache expired while you weren't looking. A session finished 10 minutes ago and you didn't notice. Another one hit its context limit and you're burning tokens on a dead conversation.

**You're paying $200/mo for Claude Code. You deserve a dashboard.**

<div align="center">

```bash
npx claude-view
```

**One command. Every session visible. Real-time.**

</div>

---

## What You Get

### Live Monitor

| Feature | Why it matters |
|---------|---------------|
| **Session cards with last message** | Instantly remember what each long-running session is working on |
| **Notification sounds** | Get pinged when a session finishes or needs your input — stop polling terminals |
| **Context gauge** | Real-time context window usage per session — see which ones are in the danger zone |
| **Cache warm countdown** | Know exactly when prompt cache expires so you can time your next message to save tokens |
| **Cost tracking** | Per-session and aggregate spend with cache savings breakdown |
| **Sub-agent visualization** | See the full agent tree — sub-agents, their status, and what tools they're calling |
| **Multiple views** | Grid, List, or Monitor mode (live chat grid) — pick what fits your workflow |

### Rich Chat History

| Feature | Why it matters |
|---------|---------------|
| **Full conversation browser** | Every session, every message, fully rendered with markdown and code blocks |
| **Tool call visualization** | See file reads, edits, bash commands, MCP calls, skill invocations — not just text |
| **Compact / verbose toggle** | Skim the conversation or drill into every tool call |
| **Thread view** | Follow agent conversations with sub-agent hierarchies |
| **Export** | Markdown export for context resumption or sharing |

### Advanced Search

| Feature | Why it matters |
|---------|---------------|
| **Full-text search** | Search across all sessions — messages, tool calls, file paths |
| **Project & branch filters** | Scope to the project you're working on right now |
| **Command palette** | <kbd>Cmd</kbd>+<kbd>K</kbd> to jump between sessions, switch views, find anything |

---

### Agent Internals — See What's Hidden

Claude Code does a lot behind `"thinking..."` that never shows in your terminal. claude-view exposes all of it.

| Feature | Why it matters |
|---------|---------------|
| **Sub-agent conversations** | See the full tree of spawned agents, their prompts, and their outputs |
| **MCP server calls** | See which MCP tools are being invoked and their results |
| **Skill / hook / plugin tracking** | Know which skills fired, which hooks ran, what plugins are active |
| **Hook event recording** | Every hook event captured and browsable *(requires claude-view running during session)* |
| **Tool use timeline** | Action log of every tool_use/tool_result pair with timing |
| **Error surfacing** | Errors bubble up to the session card — no more buried failures |
| **Raw message inspector** | Drill into any message's raw JSON when you need the full picture |

---

### Analytics

A rich analytics suite for your Claude Code usage. Think Cursor's dashboard, but deeper.

<details>
<summary><strong>Dashboard Overview</strong></summary>
<br>

| Feature | Description |
|---------|-------------|
| **Week-over-week metrics** | Session count, token usage, cost — compared to your previous period |
| **Activity heatmap** | 90-day GitHub-style grid showing your daily Claude Code usage intensity |
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
| **Skill effectiveness** | Which skills actually improve your output vs which ones don't |

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

> Insights and Fluency Score are in early experimental stage. Treat as directional, not definitive.

</details>

---

## Built for the Power User

claude-view is for the developer who:

- Runs **3+ projects simultaneously**, each with multiple worktrees
- Has **10-20 Claude Code sessions** open at any time
- Needs to context-switch fast without losing track of what's running
- Wants to **optimize token spend** by timing messages around cache windows
- Gets frustrated by <kbd>Cmd</kbd>+<kbd>Tab</kbd>-bing through terminals to check on agents

---

## How It's Built

| | |
|---|---|
| **Blazing fast** | Rust backend with SIMD-accelerated JSONL parsing, memory-mapped I/O — indexes thousands of sessions in seconds |
| **Real-time** | File watcher + SSE + WebSocket for sub-second live updates across all sessions |
| **Tiny footprint** | ~10 MB download, ~27 MB on disk. No runtime dependencies, no background daemons |
| **100% local** | All data stays on your machine. Zero telemetry, zero cloud, zero network requests |
| **Zero config** | `npx claude-view` and you're done. No API keys, no setup, no accounts |

<details>
<summary><strong>Why Rust? — The Numbers</strong></summary>
<br>

Measured on an M-series Mac with 1,493 sessions across 26 projects:

| Metric | claude-view | Typical Electron dashboard |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150–300 MB |
| **On disk** | **~27 MB** | 300–500 MB |
| **Startup (ready to serve)** | **< 500 ms** | 3–8 s |
| **RAM (full index loaded)** | **~50 MB** | 300–800 MB |
| **Deep-index 1,500 sessions** | **< 1 s** | N/A |
| **Runtime dependencies** | **0** | Node.js + Chromium |

Key techniques:

- **SIMD pre-filter** — `memchr` scans raw bytes before touching a JSON parser
- **Memory-mapped I/O** — JSONL files are mmap'd and parsed in-place, never copied
- **Tantivy search** — Same engine behind Quickwit; indexes 1,500 sessions in under a second
- **Zero-copy where it counts** — Borrowed slices from mmap through parse to response

```bash
cargo build --release
/usr/bin/time -l target/release/claude-view   # peak RSS + wall time
```

</details>

---

## How It Compares

The Claude Code ecosystem has great tools — chat UIs, history viewers, session managers. claude-view fills a different gap: **real-time monitoring + deep history + analytics in one lightweight workspace.**

| Tool | Category | Stack | Size | Live monitor | Search | Analytics |
|------|----------|-------|:----:|:------------:|:------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **Yes** | **Yes** | **Yes** |
| [opcode](https://github.com/winfunc/opcode) | GUI + session manager | Tauri 2 | ~13 MB | Partial | No | Yes |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI usage tracker | TypeScript | ~600 KB | No | No | CLI |
| [CodePilot](https://github.com/op7418/CodePilot) | Desktop chat UI | Electron | ~140 MB | No | No | No |
| [claude-run](https://github.com/kamranahmedse/claude-run) | History viewer | TypeScript | ~500 KB | Partial | Basic | No |

> Chat UIs (CodePilot, CUI, claude-code-webui) are interfaces *for* Claude Code. claude-view is a dashboard that watches your existing terminal sessions. They're complementary, not competing.

<details>
<summary><strong>Why the size difference matters</strong></summary>
<br>

| | claude-view | Electron app |
|---|:-:|:-:|
| **Download** | ~10 MB | ~140 MB |
| **On disk** | ~27 MB | ~400 MB |
| **What's in it** | Rust server + SPA assets | Chromium + Node.js + Next.js + app code |
| **RAM at idle** | ~50 MB | ~300 MB+ |
| **Startup** | < 500 ms | 3–8 s |
| **Background cost** | Negligible | Chromium renderer process |

When you're already running 10+ Claude Code sessions eating RAM and CPU, the last thing you want is a 300 MB dashboard competing for resources.

</details>

---

## Installation

```bash
npx claude-view
```

Opens at `http://localhost:47892`.

| Method | Command |
|--------|---------|
| **npx** (recommended) | `npx claude-view` |
| **Shell script** (no Node required) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

**Only requirement:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed — this creates the session files we monitor.

<details>
<summary><strong>Configuration</strong></summary>
<br>

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` or `PORT` | `47892` | Override the default port |

</details>

<details>
<summary><strong>Corporate / Sandbox Environments</strong></summary>
<br>

If your machine restricts writes to `~/Library/Caches/` (e.g., DataCloak, CrowdStrike, corporate DLP):

```bash
cp .env.example .env
# Uncomment the CLAUDE_VIEW_DATA_DIR line
```

This keeps the database, search index, and lock files in `.data/` inside the repo — no writes outside the project directory.

</details>

---

## Community

Join the [Discord server](https://discord.gg/G7wdZTpRfu) for support, feature requests, and discussion.

---

<details>
<summary><strong>Development</strong></summary>
<br>

Prerequisites: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Install frontend dependencies
bun dev            # Start full-stack dev (Rust + Vite with hot reload)
```

| Command | Description |
|---------|-------------|
| `bun dev` | Full-stack dev — Rust auto-restarts on changes, Vite HMR |
| `bun dev:server` | Rust backend only (with cargo-watch) |
| `bun dev:client` | Vite frontend only (assumes backend running) |
| `bun run build` | Build frontend for production |
| `bun run preview` | Build + serve via release binary |
| `bun run lint` | Lint both frontend (ESLint) and backend (Clippy) |
| `bun run fmt` | Format Rust code |
| `bun run check` | Typecheck + lint + test (pre-commit gate) |
| `bun test` | Run Rust test suite (`cargo test --workspace`) |
| `bun test:client` | Run frontend tests (vitest) |
| `bun run test:e2e` | Run Playwright end-to-end tests |

**Testing Production Distribution:**

```bash
bun run dist:test    # One command: build → pack → install → run
```

| Command | Description |
|---------|-------------|
| `bun run dist:pack` | Package binary + frontend into tarball at `/tmp/` |
| `bun run dist:install` | Extract tarball to `~/.cache/claude-view/` (simulates first-run download) |
| `bun run dist:run` | Run the npx wrapper using the cached binary |
| `bun run dist:clean` | Remove all dist cache and temp files |

**Releasing:**

```bash
bun run release          # patch bump: 0.1.0 → 0.1.1
bun run release:minor    # minor bump: 0.1.0 → 0.2.0
bun run release:major    # major bump: 0.1.0 → 1.0.0
```

```bash
git push origin main --tags    # triggers CI → builds all platforms → auto-publishes to npm
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

- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code deletes your sessions after 30 days. This saves them. `npx claude-backup`

---

<div align="center">

If **claude-view** helps you fly Claude Code, consider giving it a star.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT © 2026

</div>
