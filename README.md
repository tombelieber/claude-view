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

## 🗺️ Platform Roadmap

| Platform | Status | ETA |
|----------|--------|-----|
| macOS (Apple Silicon) | ✅ Available | Now |
| macOS (Intel) | ✅ Available | Now |
| Linux (x64) | 🔜 Coming | v2.1 |
| Linux (ARM64) | 🔜 Coming | v2.1 |
| Windows (x64) | 🔜 Coming | v2.2 |
| Windows (ARM64) | 🔜 Coming | v2.2 |

---

## 📄 License

MIT © 2026
