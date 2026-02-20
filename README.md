# claude-view

<p align="center">
  <strong>Live monitor & co-pilot for Claude Code power users.</strong>
</p>

<p align="center">
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

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## The Problem

You have 3 projects open. Each project has multiple git worktrees. Each worktree has multiple Claude Code sessions running. Some are thinking, some are waiting for you, some are about to hit context limits, and one finished 10 minutes ago but you forgot about it.

You Cmd-Tab through 15 terminal windows trying to remember which session was doing what. You burn tokens because a cache expired while you weren't looking. You lose flow because there's no single place to see everything. And behind that "thinking..." spinner, Claude is spawning sub-agents, calling MCP servers, running skills, firing hooks — and you can't see any of it.

**Claude Code is incredibly powerful. But flying 10+ concurrent sessions without a dashboard is like driving without a speedometer.**

## The Solution

**claude-view** is a real-time dashboard that sits alongside your Claude Code sessions. One browser tab, every session visible, full context at a glance.

```bash
npx claude-view
```

That's it. Opens in your browser. All your sessions — live and past — in one workspace.

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
| **Command palette** | Cmd+K to jump between sessions, switch views, find anything |

### Agent Internals — See What's Hidden

Claude Code does a lot behind `"thinking..."` that never shows in your terminal. claude-view exposes all of it.

| Feature | Why it matters |
|---------|---------------|
| **Sub-agent conversations** | See the full tree of spawned agents, their prompts, and their outputs |
| **MCP server calls** | See which MCP tools are being invoked and their results |
| **Skill / hook / plugin tracking** | Know which skills fired, which hooks ran, what plugins are active |
| **Hook event recording** | Every hook event is captured and browsable — go back and check what fired and when. *(Requires claude-view to be running while sessions are active; cannot trace historical events retroactively)* |
| **Tool use timeline** | Action log of every tool_use/tool_result pair with timing |
| **Error surfacing** | Errors bubble up to the session card — no more buried failures |
| **Raw message inspector** | Drill into any message's raw JSON when you need the full picture |

### Analytics

A rich analytics suite for your Claude Code usage. Think Cursor's dashboard, but deeper.

**Dashboard Overview**

| Feature | Description |
|---------|-------------|
| **Week-over-week metrics** | Session count, token usage, cost — compared to your previous period |
| **Activity heatmap** | 90-day GitHub-style grid showing your daily Claude Code usage intensity |
| **Top skills / commands / MCP tools / agents** | Leaderboards of your most-used invocables — click any to search matching sessions |
| **Most active projects** | Bar chart of projects ranked by session count |
| **Tool usage breakdown** | Total edits, reads, and bash commands across all sessions |
| **Longest sessions** | Quick access to your marathon sessions with duration |

**AI Contributions**

| Feature | Description |
|---------|-------------|
| **Code output tracking** | Lines added/removed, files touched, commit count — across all sessions |
| **Cost ROI metrics** | Cost per commit, cost per session, cost per line of AI output — with trend charts |
| **Model comparison** | Side-by-side breakdown of output and efficiency by model (Opus, Sonnet, Haiku) |
| **Learning curve** | Re-edit rate over time — see yourself getting better at prompting |
| **Branch breakdown** | Collapsible per-branch view with session drill-down |
| **Skill effectiveness** | Which skills actually improve your output vs which ones don't |

**Insights** *(experimental)*

| Feature | Description |
|---------|-------------|
| **Pattern detection** | Behavioral patterns discovered from your session history |
| **Then vs Now benchmarks** | Compare your first month to recent usage |
| **Category breakdown** | Treemap of what you use Claude for — refactoring, features, debugging, etc. |
| **AI Fluency Score** | Single 0-100 number tracking your overall effectiveness |

> **Note:** Insights and Fluency Score are in early experimental stage. Treat as directional, not definitive.

---

## Built for Flow

claude-view is designed for the developer who:

- Runs **3+ projects simultaneously**, each with multiple worktrees
- Has **10-20 Claude Code sessions** open at any time
- Needs to context-switch fast without losing track of what's running
- Wants to **optimize token spend** by timing messages around cache windows
- Gets frustrated by Cmd-Tabbing through terminals to check on agents

One browser tab. All sessions. Stay in flow.

---

## How It's Built

| | |
|---|---|
| **Blazing fast** | Rust backend with SIMD-accelerated JSONL parsing, memory-mapped I/O — indexes thousands of sessions in seconds |
| **Real-time** | File watcher + SSE + WebSocket for sub-second live updates across all sessions |
| **Tiny footprint** | ~10 MB download, ~27 MB on disk (binary + frontend). No runtime dependencies, no background daemons |
| **100% local** | All data stays on your machine. Zero telemetry, zero cloud, zero network requests |
| **Zero config** | `npx claude-view` and you're done. No API keys, no setup, no accounts |

### Why Rust? — The Numbers

Measured on an M-series Mac with 1,493 sessions across 26 projects:

| Metric | claude-view | Typical Electron dashboard |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150–300 MB |
| **On disk** | **~27 MB** | 300–500 MB |
| **Startup (ready to serve)** | **< 500 ms** | 3–8 s |
| **RAM (full index loaded)** | **~50 MB** | 300–800 MB |
| **Deep-index 1,500 sessions** | **< 1 s** | N/A |
| **Runtime dependencies** | **0** | Node.js + Chromium |

<details>
<summary>Reproduce locally</summary>

```bash
cargo build --release
/usr/bin/time -l target/release/claude-view   # peak RSS + wall time
```
</details>

Key techniques that make this possible:

- **SIMD pre-filter** — `memchr` scans raw bytes before touching a JSON parser
- **Memory-mapped I/O** — JSONL files are mmap'd and parsed in-place, never copied
- **Tantivy search** — Same engine behind Quickwit; indexes 1,500 sessions in under a second
- **Zero-copy where it counts** — Borrowed slices from mmap through parse to response

---

## Quick Start

```bash
npx claude-view
```

Opens at `http://localhost:47892`.

### Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` or `PORT` | `47892` | Override the default port |

---

## Installation

| Method | Command |
|--------|---------|
| **npx** (recommended) | `npx claude-view` |
| **Shell script** (no Node required) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Requirements

- **Claude Code** installed ([get it here](https://docs.anthropic.com/en/docs/claude-code)) — this creates the session files we monitor

---

## How It Compares

The Claude Code ecosystem has great tools — chat UIs, history viewers, session managers. claude-view fills a different gap: **real-time monitoring + deep history + analytics in one lightweight workspace.**

### Landscape

| Tool | Category | Stack | Download | Runtime deps | Live monitor | Full-text search | Analytics |
|------|----------|-------|:--------:|:------------:|:------------:|:----------------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **None** | **Yes** | **Yes** | **Yes** |
| [opcode](https://github.com/winfunc/opcode) | GUI + session manager | Tauri 2 (Rust + React) | ~13 MB (macOS) | None | Partial | No | Yes |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI usage tracker | TypeScript | ~600 KB | Node.js | No | No | CLI-only |
| [CUI](https://github.com/wbopan/cui) | Web chat UI | TypeScript (React) | — | Node.js ≥20 | No | No | No |
| [CodePilot](https://github.com/op7418/CodePilot) | Desktop chat UI | Electron + Next.js | **~140 MB** (macOS) | Bundled Chromium | No | No | No |
| [claude-run](https://github.com/kamranahmedse/claude-run) | History viewer | TypeScript (React) | ~500 KB | Node.js ≥20 | Partial | Basic | No |
| [claude-code-webui](https://github.com/sugyan/claude-code-webui) | Web chat UI | TypeScript (React) | — | Node.js / Deno | No | No | No |

> **Note:** Chat UIs (CodePilot, CUI, claude-code-webui) solve a different problem — they're interfaces *for* Claude Code. claude-view is a dashboard that watches your existing terminal sessions. They're complementary, not competing.

### Why the size difference matters

| | claude-view | Electron app (e.g. CodePilot) |
|---|:-:|:-:|
| **Download** | ~10 MB | ~140 MB |
| **On disk** | ~27 MB | ~400 MB |
| **What's in it** | Rust server + SPA assets | Chromium + Node.js + Next.js + app code |
| **RAM at idle** | ~50 MB | ~300 MB+ |
| **Startup** | < 500 ms | 3–8 s |
| **Background cost** | Negligible | Chromium renderer process |

When you're already running 10+ Claude Code sessions eating RAM and CPU, the last thing you want is a 300 MB dashboard competing for resources.

---

## Community

Join the [Discord server](https://discord.gg/G7wdZTpRfu) for support, feature requests, and discussion.

---

## Like this project?

If **claude-view** helps you fly Claude Code, consider giving it a star. It helps others discover this tool.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Development

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

### Testing Production Distribution

```bash
bun run dist:test    # One command: build → pack → install → run
```

Or step by step:

| Command | Description |
|---------|-------------|
| `bun run dist:pack` | Package binary + frontend into tarball at `/tmp/` |
| `bun run dist:install` | Extract tarball to `~/.cache/claude-view/` (simulates first-run download) |
| `bun run dist:run` | Run the npx wrapper using the cached binary |
| `bun run dist:test` | All of the above in one shot |
| `bun run dist:clean` | Remove all dist cache and temp files |

### Releasing

```bash
bun run release          # patch bump: 0.1.0 → 0.1.1
bun run release:minor    # minor bump: 0.1.0 → 0.2.0
bun run release:major    # major bump: 0.1.0 → 1.0.0
```

This bumps the version in `npx-cli/package.json`, commits, and creates a git tag. Then:

```bash
git push origin main --tags    # triggers CI → builds all platforms → auto-publishes to npm
```

---

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | Available |
| macOS (Intel) | Available |
| Linux (x64) | Planned |
| Windows (x64) | Planned |

---

## License

MIT © 2026
