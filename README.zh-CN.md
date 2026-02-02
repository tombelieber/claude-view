# claude-view

<p align="center">
  <strong>浏览与导出你的 Claude Code 对话记录</strong>
</p>

<!-- TODO: 将 YOUTUBE_VIDEO_ID 替换为实际的 YouTube 视频 ID -->
<p align="center">
  <a href="https://www.youtube.com/watch?v=YOUTUBE_VIDEO_ID">
    <img src="https://img.youtube.com/vi/YOUTUBE_VIDEO_ID/maxresdefault.jpg" alt="claude-view 演示" width="800" />
  </a>
  <br/>
  <sub>点击观看演示视频</sub>
</p>

<!-- TODO: 截取应用截图并保存至 docs/screenshot.png -->
<!-- <p align="center">
  <img src="./docs/screenshot.png" alt="claude-view 截图" width="800" />
</p> -->

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg" alt="macOS | Linux | Windows">
  <a href="https://github.com/anonymous-dev/claude-view/stargazers"><img src="https://img.shields.io/github/stars/anonymous-dev/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## 😤 问题

你已经用 **Claude Code** 好几周了。几十个对话、上百次交流。但它们去哪了？

它们被埋在 `~/.claude/projects/` 里，变成一堆难以解读的 **JSONL 文件**。想找到那次 Claude 帮你解决棘手 bug 的对话？祝你好运。

## ✨ 解决方案

**claude-view** 将你的 Claude Code 对话历史变成**美观、可搜索的存档**。

```bash
npx claude-view
```

就这样。在浏览器中打开。所有对话，整理有序，随时可搜。

---

## 🎯 功能特色

| 功能 | 说明 |
|------|------|
| 📁 **按项目浏览** | 对话按工作目录分类整理 |
| 🔍 **丰富预览** | 一目了然：使用的工具、启用的技能。点入对话查看修改的文件 |
| 💬 **完整对话** | 语法高亮代码、Markdown 渲染 |
| 📤 **导出对话** | 分享或归档为 HTML、PDF 或 Markdown |
| ⌨️ **键盘优先** | `⌘K` 跨所有对话搜索 |

---

## 🚀 快速开始

```bash
npx claude-view
```

在 `http://localhost:47892` 打开 — 你的对话已准备就绪。

---

## 📦 安装方式

| 方式 | 命令 |
|------|------|
| **npx**（推荐） | `npx claude-view` |
| **Shell 脚本**（无需 Node） | `curl -sL https://raw.githubusercontent.com/anonymous-dev/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/anonymous-dev/claude-view.git && cd claude-view && ./start.sh` |
| **Homebrew**（即将推出） | `brew install claude-view` |

---

## 📋 系统要求

- 已安装 **Claude Code**（[点此获取](https://docs.anthropic.com/en/docs/claude-code)）— 本工具读取其产生的对话文件

---

## 🤔 什么是 Claude Code？

[Claude Code](https://docs.anthropic.com/en/docs/claude-code) 是 Anthropic 的 AI 编程助手，在终端中运行。你与它的每次对话都会保存在本地。**claude-view** 帮助你回顾、搜索和导出这些对话。

---

## ⭐ 喜欢这个项目？

如果 **claude-view** 节省了你的时间，请考虑给它一颗星！这有助于更多人发现这个工具。

<p align="center">
  <a href="https://github.com/anonymous-dev/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/anonymous-dev/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 🗺️ 平台支持路线图

| 平台 | 状态 |
|------|------|
| macOS (Apple Silicon) | ✅ 已支持 |
| macOS (Intel) | ✅ 已支持 |
| Linux (x64) | ✅ 已支持 |
| Windows (x64) | ✅ 已支持 |
| Linux (ARM64) | 🔜 即将推出 |
| Windows (ARM64) | 🔜 即将推出 |

---

## 📄 许可证

MIT © 2026
