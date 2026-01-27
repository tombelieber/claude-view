# claude-view

<p align="center">
  <img src="./docs/screenshot.png" alt="claude-view" width="800" />
</p>

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
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://github.com/vicky-ai/claude-view/stargazers"><img src="https://img.shields.io/github/stars/vicky-ai/claude-view?style=social" alt="GitHub stars"></a>
</p>

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
| 🔍 **Rich previews** | See files touched, tools used, skills invoked — at a glance |
| 💬 **Full conversations** | Syntax-highlighted code, rendered markdown |
| 📤 **Export to HTML** | Share or archive as standalone files |
| ⌨️ **Keyboard-first** | `⌘K` to search across all sessions |

---

## 🚀 Quick Start

```bash
npx claude-view
```

Opens at `http://localhost:3000` — your sessions are waiting.

---

## 📦 Installation

| Method | Command |
|--------|---------|
| **npx** (recommended) | `npx claude-view` |
| **Homebrew** | `brew install claude-view` |

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
  <a href="https://github.com/vicky-ai/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/vicky-ai/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
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
| `bun test` | Run Rust test suite |
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
