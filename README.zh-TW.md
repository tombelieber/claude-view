# claude-view

<p align="center">
  <img src="./docs/screenshot.png" alt="claude-view" width="800" />
</p>

<p align="center">
  <strong>瀏覽與匯出你的 Claude Code 對話紀錄</strong>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Node-18+-green.svg" alt="Node 18+">
  <a href="https://github.com/myorg/claude-view/stargazers"><img src="https://img.shields.io/github/stars/myorg/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## 😤 問題

你已經用 **Claude Code** 好幾週了。幾十個對話、上百次交流。但它們去哪了？

它們被埋在 `~/.claude/projects/` 裡，變成一堆難以解讀的 **JSONL 檔案**。想找到那次 Claude 幫你解決棘手 bug 的對話？祝你好運。

## ✨ 解決方案

**claude-view** 將你的 Claude Code 對話歷史變成**美觀、可搜尋的存檔**。

```bash
npx claude-view
```

就這樣。在瀏覽器中開啟。所有對話，整理有序，隨時可搜。

---

## 🎯 功能特色

| 功能 | 說明 |
|------|------|
| 📁 **依專案瀏覽** | 對話依工作目錄分類整理 |
| 🔍 **豐富預覽** | 一目瞭然：修改的檔案、使用的工具、啟用的技能 |
| 💬 **完整對話** | 語法高亮程式碼、Markdown 渲染 |
| 📤 **匯出 HTML** | 分享或封存為獨立檔案 |
| ⌨️ **鍵盤優先** | `⌘K` 跨所有對話搜尋 |

---

## 🚀 快速開始

### 方法一：直接執行（推薦）

```bash
npx claude-view
```

### 方法二：全域安裝

```bash
npm install -g claude-view
claude-view
```

在 `http://localhost:3000` 開啟 — 你的對話已準備就緒。

---

## 📋 系統需求

- **Node.js 18+**
- 已安裝 **Claude Code**（[點此取得](https://docs.anthropic.com/en/docs/claude-code)）— 本工具讀取其產生的對話檔案

---

## 🤔 什麼是 Claude Code？

[Claude Code](https://docs.anthropic.com/en/docs/claude-code) 是 Anthropic 的 AI 程式設計助手，在終端機中運行。你與它的每次對話都會儲存在本地。**claude-view** 幫助你回顧、搜尋和匯出這些對話。

---

## ⭐ 喜歡這個專案？

如果 **claude-view** 節省了你的時間，請考慮給它一顆星！這有助於更多人發現這個工具。

<p align="center">
  <a href="https://github.com/myorg/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/myorg/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 📄 授權條款

MIT © 2026
