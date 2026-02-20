# claude-view

<p align="center">
  <strong>Claude Code 電源用戶的即時監控與副駕駛。</strong>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a> ·
  <a href="./README.ja.md">日本語</a> ·
  <a href="./README.es.md">Español</a> ·
  <a href="./README.fr.md">Français</a> ·
  <a href="./README.de.md">Deutsch</a> ·
  <a href="./README.pt.md">Português</a> ·
  <a href="./README.it.md">Italiano</a> ·
  <a href="./README.ko.md">한국어</a> ·
  <a href="./README.nl.md">Nederlands</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## 問題

你已經開啟了 3 個專案。每個專案有多個 git 工作樹。每個工作樹上有多個 Claude Code 會話運行。有的在思考，有的在等你，有的快要達到上下文限制，還有一個 10 分鐘前就完成了但你忘了。

你用 Cmd-Tab 在 15 個終端視窗間切換，試著記起每個會話在做什麼。你浪費了 tokens，因為一個快取在你沒注意時過期了。你失去了工作流，因為沒有一個地方能看到所有東西。而在那個「思考中...」的轉輪後面，Claude 正在生成子代理、調用 MCP 伺服器、執行技能、觸發鉤子——而你看不到任何一個。

**Claude Code 非常強大。但在沒有儀表板的情況下駕馭 10+ 個併發會話，就像開著沒有時速表的車一樣。**

## 解決方案

**claude-view** 是一個即時儀表板，與你的 Claude Code 會話並肩運行。一個瀏覽器標籤頁，每個會話都可見，一目瞭然的完整上下文。

```bash
npx claude-view
```

就這樣。在瀏覽器中開啟。你的所有會話——即時的和過去的——都在一個工作區裡。

---

## 你會得到什麼

### 即時監控

| 功能 | 為什麼重要 |
|---------|------------------|
| **會話卡片及最後訊息** | 立即回憶起每個長時間運行的會話在做什麼 |
| **通知音效** | 當會話完成或需要你的輸入時收到提醒——停止輪詢終端 |
| **上下文量表** | 每個會話的實時上下文視窗使用情況——看出哪些在危險區域 |
| **快取預熱倒計時** | 準確知道何時提示快取過期，以便你能在節省 tokens 的時候安排下一則訊息 |
| **成本追蹤** | 每個會話和總體支出，以及快取節省的分解 |
| **子代理可視化** | 看到完整的代理樹——子代理、它們的狀態，以及它們調用的工具 |
| **多個檢視** | 網格、列表或監控模式（即時聊天網格）——選擇最適合你工作流的 |

### 豐富的聊天歷史

| 功能 | 為什麼重要 |
|---------|------------------|
| **完整對話瀏覽器** | 每個會話、每條訊息，完整呈現 markdown 和程式碼區塊 |
| **工具調用可視化** | 看到檔案讀取、編輯、bash 命令、MCP 調用、技能調用——不只是文字 |
| **精簡/詳細切換** | 快速瀏覽對話或深入查看每個工具調用 |
| **執行緒檢視** | 跟蹤代理對話與子代理層級結構 |
| **匯出** | Markdown 匯出用於上下文恢復或共享 |

### 進階搜尋

| 功能 | 為什麼重要 |
|---------|------------------|
| **全文搜尋** | 跨所有會話搜尋——訊息、工具調用、檔案路徑 |
| **專案與分支篩選** | 範圍限定到你現在正在處理的專案 |
| **命令調色板** | Cmd+K 在會話間跳躍、切換檢視、尋找任何東西 |

### 代理內部——看到隱藏的東西

Claude Code 在「思考中...」後面做了很多東西，這些從不會在你的終端顯示。claude-view 揭露了所有這些。

| 功能 | 為什麼重要 |
|---------|------------------|
| **子代理對話** | 看到完整的生成代理樹、它們的提示和輸出 |
| **MCP 伺服器調用** | 看到哪些 MCP 工具被調用及其結果 |
| **技能/鉤子/外掛追蹤** | 知道哪些技能觸發了、哪些鉤子執行了、什麼外掛處於活動狀態 |
| **鉤子事件記錄** | 每個鉤子事件都被捕捉和可瀏覽——回頭檢查什麼觸發了及何時觸發。*(需要 claude-view 在會話活動時運行；無法追溯歷史事件)* |
| **工具使用時間線** | 每個 tool_use/tool_result 配對及其時間的動作日誌 |
| **錯誤浮現** | 錯誤浮現到會話卡片——不再有埋藏的失敗 |
| **原始訊息檢查器** | 當你需要完整圖像時，深入查看任何訊息的原始 JSON |

### 分析

一套豐富的分析套件用於你的 Claude Code 使用。想像 Cursor 的儀表板，但更深入。

**儀表板概觀**

| 功能 | 描述 |
|---------|--------|
| **周對周指標** | 會話計數、tokens 使用、成本——與你之前的期間比較 |
| **活動熱圖** | 90 天 GitHub 風格的網格顯示你的日常 Claude Code 使用強度 |
| **排名前列的技能/命令/MCP 工具/代理** | 你最常用的調用項的排行榜——點擊任何一個來搜尋匹配的會話 |
| **最活躍的專案** | 按會話計數排名的專案的長條圖 |
| **工具使用分解** | 跨所有會話的總編輯、讀取和 bash 命令 |
| **最長會話** | 快速存取你的馬拉松會話及其持續時間 |

**AI 貢獻**

| 功能 | 描述 |
|---------|--------|
| **程式碼輸出追蹤** | 行數新增/刪除、受影響的檔案、跨所有會話的提交計數 |
| **成本 ROI 指標** | 每次提交的成本、每個會話的成本、每行 AI 輸出的成本——帶趨勢圖表 |
| **模型比較** | 依模型（Opus、Sonnet、Haiku）的輸出和效率的並排分解 |
| **學習曲線** | 隨著時間推移的重新編輯率——看到自己在提示方面變得更好 |
| **分支分解** | 可摺疊的逐分支檢視及會話深入探討 |
| **技能效能** | 哪些技能實際上改進了你的輸出，哪些沒有 |

**深入分析** *(實驗性)*

| 功能 | 描述 |
|---------|--------|
| **模式檢測** | 從你的會話歷史發現的行為模式 |
| **當時 vs 現在基準** | 將你的第一個月與最近的使用進行比較 |
| **類別分解** | Treemap 顯示你使用 Claude 做什麼——重構、功能、除錯等 |
| **AI 流利度分數** | 追蹤你的整體效能的單一 0-100 數字 |

> **注意:** 深入分析和流利度分數處於早期實驗階段。當作方向性的，不是確定的。

---

## 為流程而打造

claude-view 是為以下開發者設計的：

- 同時運行 **3+ 個專案**，每個都有多個工作樹
- 任何時候都有 **10-20 個 Claude Code 會話**開啟
- 需要快速上下文切換而不失去追蹤
- 想要**優化 tokens 支出**，通過在快取視窗周圍安排訊息
- 對 Cmd-Tabbing 通過終端以檢查代理而感到沮喪

一個瀏覽器標籤頁。所有會話。保持流程狀態。

---

## 如何打造的

| | |
|---|---|
| **快如閃電** | Rust 後端，SIMD 加速的 JSONL 解析、記憶體映射 I/O——在幾秒內索引數千個會話 |
| **即時** | 檔案監視器 + SSE + WebSocket，所有會話的亞秒級即時更新 |
| **足跡很小** | 單一 ~15 MB 二進制。無執行時依賴、無背景守護進程 |
| **100% 本地** | 所有資料保留在你的機器上。零遙測、零雲端、零網路請求 |
| **零配置** | `npx claude-view`，完成。無 API 金鑰、無設置、無帳戶 |

---

## 快速開始

```bash
npx claude-view
```

在 `http://localhost:47892` 開啟。

### 配置

| 環境變數 | 預設 | 描述 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` 或 `PORT` | `47892` | 覆蓋預設埠 |

---

## 安裝

| 方式 | 命令 |
|--------|---------|
| **npx**（推薦） | `npx claude-view` |
| **Shell 指令碼**（無需 Node） | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### 需求

- **Claude Code** 已安裝（[在此取得](https://docs.anthropic.com/en/docs/claude-code)）——這建立了我們監控的會話檔案

---

## 比較方式

其他工具要麼是檢視器（瀏覽歷史）要麼是簡單的監控器。沒有任何工具在單一工作區中結合即時監控、豐富的聊天歷史、除錯工具和進階搜尋。

```
                    被動 ←————————————→ 主動
                         |                  |
            僅檢視    |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            僅監控    |  claude-code-ui  |
                         |  Agent Sessions  |
                         |                  |
            完整      |  ★ claude-view   |
            工作區    |                  |
```

---

## 社群

加入 [Discord 伺服器](https://discord.gg/G7wdZTpRfu)尋求支援、功能請求和討論。

---

## 喜歡這個專案嗎？

如果 **claude-view** 幫助你駕馭 Claude Code，請考慮給它一顆星。這有助於其他人發現這個工具。

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 開發

前置條件：[Rust](https://rustup.rs/)、[Bun](https://bun.sh/)、`cargo install cargo-watch`

```bash
bun install        # 安裝前端依賴
bun dev            # 啟動全棧開發（Rust + Vite 含熱重載）
```

| 命令 | 描述 |
|---------|-------------|
| `bun dev` | 全棧開發——Rust 在變更時自動重啟、Vite HMR |
| `bun dev:server` | 僅 Rust 後端（含 cargo-watch） |
| `bun dev:client` | 僅 Vite 前端（假設後端運行） |
| `bun run build` | 為生產構建前端 |
| `bun run preview` | 構建 + 通過發布二進制提供服務 |
| `bun run lint` | 檢查前端（ESLint）和後端（Clippy） |
| `bun run fmt` | 格式化 Rust 程式碼 |
| `bun run check` | 類型檢查 + 檢查 + 測試（提交前閘門） |
| `bun test` | 執行 Rust 測試套件（`cargo test --workspace`） |
| `bun test:client` | 執行前端測試（vitest） |
| `bun run test:e2e` | 執行 Playwright 端對端測試 |

### 測試生產分發

```bash
bun run dist:test    # 一個命令：構建 → 打包 → 安裝 → 執行
```

或一步步進行：

| 命令 | 描述 |
|---------|-------------|
| `bun run dist:pack` | 將二進制 + 前端打包到 `/tmp/` 的 tarball |
| `bun run dist:install` | 提取 tarball 到 `~/.cache/claude-view/`（模擬首次運行下載） |
| `bun run dist:run` | 使用快取二進制執行 npx 包裝器 |
| `bun run dist:test` | 上述全部於一個命令中 |
| `bun run dist:clean` | 移除所有 dist 快取和臨時檔案 |

### 發佈

```bash
bun run release          # 補丁凹凸：0.1.0 → 0.1.1
bun run release:minor    # 次要凹凸：0.1.0 → 0.2.0
bun run release:major    # 主要凹凸：0.1.0 → 1.0.0
```

這在 `npx-cli/package.json` 中凹凸版本、提交並建立一個 git 標籤。然後：

```bash
git push origin main --tags    # 觸發 CI → 構建所有平台 → 自動發佈到 npm
```

---

## 平台支援

| 平台 | 狀態 |
|----------|--------|
| macOS (Apple Silicon) | 可用 |
| macOS (Intel) | 可用 |
| Linux (x64) | 計畫中 |
| Windows (x64) | 計畫中 |

---

## 授權條款

MIT © 2026
