# claude-view

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Node](https://img.shields.io/badge/Node-18+-green.svg)](https://nodejs.org)

[English](./README.md) · [繁體中文] · [简体中文](./README.zh-CN.md)

<p align="center">
  <img src="./docs/screenshot.png" alt="claude-view" width="800" />
</p>

<p align="center">
  瀏覽與匯出你的 Claude Code 對話紀錄
</p>

## 這是什麼？

**claude-view** 是一個本地網頁介面，用於瀏覽你的 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 對話歷史。Claude Code 將對話存儲為 `~/.claude/projects/` 中的 JSONL 檔案 — 這個工具將它們轉換為可搜尋、可瀏覽的存檔，並支援匯出功能。

如果你使用 Claude Code（Anthropic 的 AI 編程助手）並想回顧過去的對話、跨對話搜尋，或將它們匯出為可分享的 HTML 檔案，這個工具就是為你打造的。

## 快速開始

```bash
npx claude-view
```

自動在瀏覽器開啟 `http://localhost:3000`

## 功能特色

- **依專案瀏覽** — 對話依工作目錄分類整理
- **豐富預覽** — 一目瞭然：修改的檔案、使用的工具、啟用的技能
- **完整對話** — 語法高亮的程式碼區塊、Markdown 渲染
- **匯出 HTML** — 分享或封存對話為獨立檔案
- **鍵盤優先** — `⌘K` 跨所有對話搜尋

## 系統需求

- Node.js 18+
- 已安裝 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)（本工具讀取其產生的對話檔案）

## 授權條款

MIT
