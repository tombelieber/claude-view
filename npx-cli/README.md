<div align="center">

# claude-view

**You have 10 Claude sessions running right now. What are they doing?**

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/Website-claudeview.ai-orange" alt="Website"></a>
  <a href="https://github.com/tombelieber/claude-view/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

</div>

Behind every "thinking..." spinner, Claude is spawning sub-agents, calling MCP servers, running skills, firing hooks — and you can't see any of it.

**You're paying $200/mo for Claude Code. You deserve a dashboard.**

<div align="center">

```bash
curl -fsSL https://raw.githubusercontent.com/tombelieber/claude-view/main/install.sh | sh
```

**One command. Every session visible. Real-time.**

</div>

---

## Install

| Method | Command |
|--------|---------|
| **Shell** (recommended) | `curl -fsSL https://raw.githubusercontent.com/tombelieber/claude-view/main/install.sh \| sh` |
| **npx** | `npx claude-view` |

---

## What You Get

- **Live session cards** — see what every session is working on, right now
- **Notification sounds** — get pinged when a session finishes or needs input
- **Context gauge** — real-time context window usage per session
- **Cache warm countdown** — time your messages to save tokens
- **Cost tracking** — per-session and aggregate spend with cache savings
- **Sub-agent visualization** — see the full agent tree, tool calls, MCP invocations
- **Full-text search** — search across all sessions, messages, tool calls, file paths
- **Analytics** — activity heatmap, cost ROI, model comparison, AI Fluency Score
- **Rich chat history** — every conversation rendered with markdown, code blocks, tool calls

---

## How It Works

On first run, `npx claude-view` downloads a platform-specific Rust binary (~10 MB) from GitHub Releases. The binary is cached at `~/.cache/claude-view/` so subsequent runs start instantly.

Everything stays on your machine. Zero telemetry, zero cloud, zero network requests.

---

## Configuration

| Env Variable | Default | Description |
| --- | --- | --- |
| `CLAUDE_VIEW_PORT` | `47892` | Port for the local server |
| `PORT` | `47892` | Alternative port override |

## Supported Platforms

| OS | Architecture |
| --- | --- |
| macOS | Apple Silicon (arm64), Intel (x64) |
| Linux | x64 |
| Windows | x64 |

---

## Links

- [Website](https://claudeview.ai) — docs, changelog, blog
- [GitHub Repository](https://github.com/tombelieber/claude-view) — full feature list, comparison table, architecture details
- [@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin) — Claude Code plugin with 8 MCP tools and 3 skills
- [claude-backup](https://github.com/tombelieber/claude-backup) — Claude Code deletes your sessions after 30 days. This saves them.
- [Report an Issue](https://github.com/tombelieber/claude-view/issues)
- [Discord](https://discord.gg/G7wdZTpRfu)

---

<div align="center">

MIT

</div>
