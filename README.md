# vibe-recall

<p align="center">
  <strong>Your AI fluency, measured.</strong>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg" alt="macOS | Linux | Windows">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/anonymous-dev/claude-view/stargazers"><img src="https://img.shields.io/github/stars/anonymous-dev/claude-view?style=social" alt="GitHub stars"></a>
</p>

<!-- TODO: demo GIF here -->

---

## The Problem

92% of developers use AI coding tools. A [METR study](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/) found they're actually **19% slower**. The problem isn't the tools — it's that nobody teaches you how to use them well.

41% of code is now AI-generated, but most developers have no idea if they're using AI tools effectively or wasting half their tokens on re-prompting.

## The Solution

**vibe-recall** is a fitness tracker for your AI coding workflow. It analyzes your Claude Code sessions, shows you your patterns, and helps you **measurably improve**.

```bash
npx claude-view
```

That's it. Opens in your browser. Your AI fluency, measured.

---

## What You'll Discover

| Insight | Example |
|---------|---------|
| **AI Fluency Score** | A single number (0-100) that tracks how effectively you use AI |
| **Token efficiency** | "You waste 34% of tokens on re-prompting" |
| **Prompt clarity** | "Your re-edit rate dropped 54% over 3 months" |
| **Model fit** | "Opus is 42% better for refactoring, but you use it for everything" |
| **Workflow patterns** | "Tuesday mornings are your most effective AI coding sessions" |
| **Skill effectiveness** | "TDD skill reduces re-edit rate by 65%" |

---

## How It's Built

| | |
|---|---|
| **Blazing fast** | Rust backend with SIMD-accelerated JSONL parsing, memory-mapped I/O — indexes thousands of sessions in seconds |
| **Tiny footprint** | Single ~15 MB binary. No runtime dependencies, no background daemons |
| **100% local** | All data stays on your machine. Zero telemetry, zero cloud, zero network requests |
| **Zero config** | `npx claude-view` and you're done. No API keys, no setup, no accounts |

---

## Quick Start

```bash
npx claude-view
```

Opens at `http://localhost:47892`.

### Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` | `47892` | Override the default port |
| `PORT` | `47892` | Alternative port override |

---

## Installation

| Method | Command |
|--------|---------|
| **npx** (recommended) | `npx claude-view` |
| **Shell script** (no Node required) | `curl -sL https://raw.githubusercontent.com/anonymous-dev/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/anonymous-dev/claude-view.git && cd claude-view && ./start.sh` |

### Requirements

- **Claude Code** installed ([get it here](https://docs.anthropic.com/en/docs/claude-code)) — this creates the session files we analyze

---

## How It Compares

Every other tool is a utility (viewer/monitor). None of them coach you to improve.

```
                    Individual ←————————→ Team
                         |                  |
            Utility      |  ccusage         |  Anthropic Analytics
            (just view)  |  History Viewer  |  GitHub Copilot Reports
                         |  Claude HUD      |
                         |                  |
            Coach        |  ★ vibe-recall   |  (coming soon)
            (improve)    |                  |
```

---

## Community

Join the [Discord server](https://discord.gg/G7wdZTpRfu) for support, feature requests, and discussion.

---

## Like this project?

If **vibe-recall** helps you level up your AI coding, consider giving it a star. It helps others discover this tool.

<p align="center">
  <a href="https://github.com/anonymous-dev/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/anonymous-dev/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
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

## Platform Roadmap

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | Available |
| macOS (Intel) | Available |
| Linux (x64) | Available |
| Windows (x64) | Available |
| Linux (ARM64) | Coming |
| Windows (ARM64) | Coming |

---

## License

MIT © 2026
