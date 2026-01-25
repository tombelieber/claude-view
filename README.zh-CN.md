# claude-view

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Node](https://img.shields.io/badge/Node-18+-green.svg)](https://nodejs.org)

[English](./README.md) · [繁體中文](./README.zh-TW.md) · [简体中文]

<p align="center">
  <img src="./docs/screenshot.png" alt="claude-view" width="800" />
</p>

<p align="center">
  浏览与导出你的 Claude Code 对话记录
</p>

## 这是什么？

**claude-view** 是一个本地网页界面，用于浏览你的 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 对话历史。Claude Code 将对话存储为 `~/.claude/projects/` 中的 JSONL 文件 — 这个工具将它们转换为可搜索、可浏览的存档，并支持导出功能。

如果你使用 Claude Code（Anthropic 的 AI 编程助手）并想回顾过去的对话、跨对话搜索，或将它们导出为可分享的 HTML 文件，这个工具就是为你打造的。

## 快速开始

```bash
npx claude-view
```

自动在浏览器打开 `http://localhost:3000`

## 功能特色

- **按项目浏览** — 对话按工作目录分类整理
- **丰富预览** — 一目了然：修改的文件、使用的工具、启用的技能
- **完整对话** — 语法高亮的代码块、Markdown 渲染
- **导出 HTML** — 分享或归档对话为独立文件
- **键盘优先** — `⌘K` 跨所有对话搜索

## 系统要求

- Node.js 18+
- 已安装 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)（本工具读取其产生的对话文件）

## 许可证

MIT
