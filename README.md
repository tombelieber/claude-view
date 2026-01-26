# claude-view

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Node](https://img.shields.io/badge/Node-18+-green.svg)](https://nodejs.org)

[English] · [繁體中文](./README.zh-TW.md) · [简体中文](./README.zh-CN.md)

<p align="center">
  <img src="./docs/screenshot.png" alt="claude-view" width="800" />
</p>

<p align="center">
  Browse and export your Claude Code sessions.
</p>

## What is this?

**claude-view** is a local web UI for browsing your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) conversation history. Claude Code stores sessions as JSONL files in `~/.claude/projects/` — this tool turns them into a searchable, browsable archive with export capabilities.

If you use Claude Code (Anthropic's AI coding assistant) and want to revisit past conversations, search across sessions, or export them as shareable HTML files, this is for you.

## Quick Start

```bash
npx claude-view
```

Opens in your browser at `http://localhost:3000`

## Features

- **Browse by project** — Sessions organized by working directory
- **Rich previews** — See files touched, tools used, and skills invoked at a glance
- **Full conversations** — Syntax-highlighted code blocks, markdown rendering
- **Export to HTML** — Share or archive sessions as standalone files
- **Keyboard-first** — `⌘K` to search across all sessions

## Requirements

- Node.js 18+
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed (creates the session files this tool reads)

## License

MIT
