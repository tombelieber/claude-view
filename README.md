# claude-view

<p align="center">
  <strong>Browse and export your Claude Code sessions</strong>
</p>


<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg" alt="macOS | Linux | Windows">
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

<p align="center">
  <b>Fast</b> · <b>Lightweight</b> · <b>Private</b> · <b>Zero Config</b>
</p>

---

## Why claude-view?

| | |
|---|---|
| **Blazing fast** | Rust-powered backend with SIMD-accelerated JSONL parsing, memory-mapped I/O, and instant startup — indexes thousands of sessions in seconds |
| **Tiny footprint** | Single ~15 MB binary. No runtime dependencies, no background daemons, no bloat |
| **100% private** | All data stays on your machine. No telemetry, no cloud, no network requests. Read-only access to your session files |
| **Memory safe** | Written in Rust — no buffer overflows, no data races, no GC pauses |
| **Full-text search** | Powered by Tantivy (Rust search engine) and SQLite — find any conversation instantly |
| **Zero config** | `npx claude-view` and you're done. No API keys, no setup, no accounts |

---

## 😤 The Problem

You've been using **Claude Code** for weeks. Dozens of sessions. Hundreds of conversations. But where did they go?

They're buried in `~/.claude/projects/` as cryptic **JSONL files**. Good luck finding that one conversation where Claude helped you fix that tricky bug.

## ✨ The Solution

**claude-view** turns your Claude Code session history into a **beautiful, searchable archive**.

```bash
npx claude-view
```

That's it. Opens in your browser. All your sessions, organized and searchable.

---

## 🎯 Features

| Feature | Description |
|---------|-------------|
| 📁 **Browse by project** | Sessions organized by working directory |
| 🔍 **Rich previews** | See tools used, skills invoked — at a glance. Drill into sessions for files touched |
| 💬 **Full conversations** | Syntax-highlighted code, rendered markdown |
| 📤 **Export conversations** | Share or archive as HTML, PDF, or Markdown |
| ⌨️ **Keyboard-first** | `⌘K` to search across all sessions |

---

## 🚀 Quick Start

```bash
npx claude-view
```

Opens at `http://localhost:47892` — your sessions are waiting.

### Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` | `47892` | Override the default port |
| `PORT` | `47892` | Alternative port override |

---

## 📦 Installation

| Method | Command |
|--------|---------|
| **npx** (recommended) | `npx claude-view` |
| **Shell script** (no Node required) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

---

## 📋 Requirements

- **Claude Code** installed ([get it here](https://docs.anthropic.com/en/docs/claude-code)) — this creates the session files we read

---

## 🤔 What is Claude Code?

[Claude Code](https://docs.anthropic.com/en/docs/claude-code) is Anthropic's AI coding assistant that runs in your terminal. Every conversation you have with it is saved locally. **claude-view** helps you revisit, search, and export those conversations.

---

## ⭐ Like this project?

If **claude-view** saves you time, consider giving it a star! It helps others discover this tool.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 🛠️ Development

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

These commands simulate the full `npx claude-view` experience locally:

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

## 🗺️ Platform Roadmap

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | ✅ Available |
| macOS (Intel) | ✅ Available |
| Linux (x64) | ✅ Available |
| Windows (x64) | ✅ Available |
| Linux (ARM64) | 🔜 Coming |
| Windows (ARM64) | 🔜 Coming |

---

## 📄 License

MIT © 2026
