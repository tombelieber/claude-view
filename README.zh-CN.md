<div align="center">

# claude-view

**Claude Code 的任务控制中心**

你有 10 个 AI 代理在运行。一个在 12 分钟前就完成了。另一个触及了上下文限制。第三个需要工具授权。你在终端之间疯狂 <kbd>Cmd</kbd>+<kbd>Tab</kbd> 切换，每月盲烧 $200。

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/docs-claudeview.ai-orange" alt="Website"></a>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg?label=plugin" alt="plugin version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

<p>
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

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

**一条命令。所有会话尽收眼底。实时更新。**

</div>

---

## claude-view 是什么？

claude-view 是一个开源仪表板，可以监控你机器上每一个 Claude Code 会话——运行中的代理、历史对话、费用、子代理、hooks、工具调用——全部集中在一个界面。Rust 后端，React 前端，约 10 MB 的二进制文件。零配置，零账户，100% 本地运行。

**30 个版本。86 个 MCP 工具。9 个技能。一个 `npx claude-view` 搞定。**

---

## 实时监控

一目了然地查看每个正在运行的会话。再也不用在终端标签页之间来回切换。

| 功能 | 描述 |
|---------|-------------|
| **会话卡片** | 每张卡片显示最新消息、模型、费用和状态——即时了解每个代理正在做什么 |
| **多会话聊天** | 以 VS Code 风格的标签页（dockview）并排打开会话。拖拽可水平或垂直分屏 |
| **上下文仪表** | 每个会话的上下文窗口实时填充量——在代理触及限制前发现哪些处于危险区 |
| **缓存倒计时** | 精确掌握提示缓存何时过期，以便合理安排消息发送时机来节省 token |
| **费用追踪** | 单个会话和累计花费，附带 token 明细——悬停可按模型查看输入/输出/缓存分布 |
| **子代理树** | 查看所有派生代理的完整树形结构、状态、费用，以及正在调用的工具 |
| **通知音效** | 当会话完成、出错或需要你输入时发出提醒——不用再轮询终端 |
| **多种视图** | 网格、列表、看板或监控模式——选择最适合你工作流的方式 |
| **看板泳道** | 按项目或分支对会话分组——多项目工作流的可视化泳道布局 |
| **最近关闭** | 结束的会话出现在"最近关闭"区域而非直接消失——服务器重启后仍然保留 |
| **排队消息** | 队列中等待的消息以待处理气泡显示，附带"排队中"标记 |
| **SSE 驱动** | 所有实时数据通过 Server-Sent Events 推送——完全消除缓存过期的风险 |

---

## 聊天与对话

读取、搜索任意会话并与之交互——无论是实时还是历史会话。

| 功能 | 描述 |
|---------|-------------|
| **统一实时聊天** | 历史消息和实时消息在同一个可滚动的对话中——无需切换标签页 |
| **开发者模式** | 为每个会话切换聊天视图和开发者视图。开发者模式显示工具卡片、事件卡片、hook 元数据，以及带过滤筛选的完整执行追踪 |
| **完整对话浏览器** | 每个会话、每条消息，完整渲染 markdown 和代码块 |
| **工具调用可视化** | 查看文件读取、编辑、bash 命令、MCP 调用、技能调用——不仅仅是文本 |
| **简洁/详细切换** | 快速浏览对话或深入每个工具调用的细节 |
| **线程视图** | 以子代理层级和缩进线程方式跟踪代理对话 |
| **Hook 事件内联** | Pre/post 工具 hook 作为对话块渲染——在对话旁边查看 hook 的触发情况 |
| **导出** | Markdown 导出，用于上下文恢复或分享 |
| **批量选择与归档** | 选择多个会话进行批量归档，并保持过滤状态 |
| **加密分享** | 通过端到端加密链接分享任意会话——AES-256-GCM，零服务器信任，密钥仅存在于 URL 片段中 |

---

## 代理内部机制

Claude Code 在 `"thinking..."` 背后做了很多你在终端中看不到的事情。claude-view 让这一切全部可见。

| 功能 | 描述 |
|---------|-------------|
| **子代理对话** | 派生代理的完整树形结构，包括提示、输出和每个代理的费用/token 明细 |
| **MCP 服务器调用** | 查看正在调用哪些 MCP 工具及其结果 |
| **技能 / hook / 插件追踪** | 哪些技能被触发、哪些 hook 被执行、哪些插件处于活跃状态 |
| **Hook 事件记录** | 双通道 hook 捕获（实时 WebSocket + JSONL 回填）——每个事件都被记录且可浏览，包括历史会话 |
| **会话来源标记** | 每个会话显示其启动方式：Terminal、VS Code、Agent SDK 或其他入口 |
| **Worktree 分支偏移** | 检测 git worktree 分支何时出现分歧——在实时监控和历史记录中均有显示 |
| **@文件引用标签** | `@filename` 引用被提取并显示为标签——悬停查看完整路径 |
| **工具使用时间线** | 每个 tool_use/tool_result 对的操作日志，附带计时信息 |
| **错误浮现** | 错误冒泡到会话卡片——不再有被埋没的故障 |
| **原始消息检查器** | 当你需要完整信息时，可以深入查看任何消息的原始 JSON |

---

## 搜索

| 功能 | 描述 |
|---------|-------------|
| **全文搜索** | 跨所有会话搜索——消息、工具调用、文件路径。由 Tantivy 驱动（Rust 原生，Lucene 级别） |
| **统一搜索引擎** | Tantivy 全文搜索 + SQLite 预过滤并行运行——一个端点，50 毫秒内返回结果 |
| **项目与分支过滤** | 限定到你当前正在操作的项目或分支 |
| **命令面板** | <kbd>Cmd</kbd>+<kbd>K</kbd> 在会话间跳转、切换视图、查找任何内容 |

---

## 分析

为你的 Claude Code 使用情况提供完整的分析套件。类似 Cursor 的仪表板，但更深入。

<details>
<summary><strong>仪表板</strong></summary>
<br>

| 功能 | 描述 |
|---------|-------------|
| **周环比指标** | 会话数、token 用量、费用——与上一周期对比 |
| **活动热力图** | 90 天 GitHub 风格网格，显示每日使用强度 |
| **热门技能 / 命令 / MCP 工具 / 代理** | 你最常使用的可调用项排行榜——点击任意项可搜索匹配的会话 |
| **最活跃项目** | 按会话数排名的项目柱状图 |
| **工具使用明细** | 所有会话中的编辑、读取和 bash 命令总计 |
| **最长会话** | 快速访问你的马拉松式会话，附带持续时间 |

</details>

<details>
<summary><strong>AI 贡献</strong></summary>
<br>

| 功能 | 描述 |
|---------|-------------|
| **代码产出追踪** | 新增/删除行数、涉及文件数、提交数——跨所有会话统计 |
| **成本投资回报率** | 每次提交成本、每次会话成本、每行 AI 输出成本——附带趋势图 |
| **模型对比** | 按模型（Opus、Sonnet、Haiku）并排展示产出和效率 |
| **学习曲线** | 随时间变化的重新编辑率——直观感受你的提示技巧在进步 |
| **分支明细** | 可折叠的分支视图，支持下钻到具体会话 |
| **技能效果** | 哪些技能真正提升了你的产出，哪些没有 |

</details>

<details>
<summary><strong>洞察</strong> <em>（实验性）</em></summary>
<br>

| 功能 | 描述 |
|---------|-------------|
| **模式检测** | 从你的会话历史中发现行为模式 |
| **今昔对比** | 比较你第一个月和近期的使用情况 |
| **分类明细** | 你使用 Claude 做什么的树状图——重构、新功能、调试等 |
| **AI 流利度评分** | 单一的 0-100 分数，追踪你的整体效率 |

> 洞察和流利度评分为实验性功能。仅供参考，不作定论。

</details>

---

## 计划、提示与团队

| 功能 | 描述 |
|---------|-------------|
| **计划浏览器** | 直接在会话详情中查看你的 `.claude/plans/`——不用再翻找文件 |
| **提示历史** | 跨所有已发送提示的全文搜索，支持模板聚类和意图分类 |
| **团队面板** | 查看团队负责人、收件箱消息、团队任务和所有团队成员的文件变更 |
| **提示分析** | 提示模板排行榜、意图分布和使用统计 |

---

## 系统监控

| 功能 | 描述 |
|---------|-------------|
| **实时 CPU / 内存 / 磁盘仪表** | 通过 SSE 流式传输的实时系统指标，带平滑动画过渡 |
| **组件面板** | 查看 sidecar 和本地 AI 指标：显存使用、CPU、内存和每个组件的会话数 |
| **进程列表** | 按名称分组、按 CPU 排序的进程列表——查看代理运行时你的机器实际在做什么 |

---

## 本地 AI

运行本地 LLM 进行会话阶段分类——无需 API 调用，无额外费用。

| 功能 | 描述 |
|---------|-------------|
| **与供应商无关** | 连接任何 OpenAI 兼容的端点——oMLX、Ollama、LM Studio 或你自己的服务器 |
| **模型选择器** | 从精选模型注册表中选择，显示内存需求 |
| **阶段分类** | 使用置信度门控显示，为会话标记当前阶段（编码、调试、规划等） |
| **智能资源管理** | EMA 稳定分类与指数退避——相比朴素轮询减少 93% 的 GPU 浪费 |

---

## 插件

`@claude-view/plugin` 让 Claude 原生访问你的仪表板数据——86 个 MCP 工具、9 个技能，自动启动。

```bash
claude plugin add @claude-view/plugin
```

### 自动启动

每个 Claude Code 会话自动启动仪表板。无需手动运行 `npx claude-view`。

### 86 个 MCP 工具

8 个精心打造的工具，为 Claude 优化输出：

| 工具 | 描述 |
|------|-------------|
| `list_sessions` | 使用过滤器浏览会话 |
| `get_session` | 完整的会话详情，包含消息和指标 |
| `search_sessions` | 跨所有对话的全文搜索 |
| `get_stats` | 仪表板概览——总会话数、费用、趋势 |
| `get_fluency_score` | AI 流利度评分（0-100），附带明细 |
| `get_token_stats` | Token 用量及缓存命中率 |
| `list_live_sessions` | 当前正在运行的代理（实时） |
| `get_live_summary` | 今日累计费用和状态 |

另有 **78 个自动生成的工具**，基于 OpenAPI 规范，涵盖 27 个类别（贡献、洞察、辅导、导出、工作流等）。

### 9 个技能

| 技能 | 描述 |
|-------|-------------|
| `/session-recap` | 总结特定会话——提交、指标、持续时间 |
| `/daily-cost` | 今日花费、运行中的会话、token 用量 |
| `/standup` | 多会话工作日志，用于站会更新 |
| `/coaching` | AI 辅导建议和自定义规则管理 |
| `/insights` | 行为模式分析 |
| `/project-overview` | 跨会话的项目总结 |
| `/search` | 自然语言搜索 |
| `/export-data` | 将会话导出为 CSV/JSON |
| `/team-status` | 团队活动概览 |

---

## 工作流

| 功能 | 描述 |
|---------|-------------|
| **工作流构建器** | 创建多阶段工作流，支持 VS Code 风格布局、Mermaid 图表预览和 YAML 编辑器 |
| **流式 LLM 聊天导轨** | 通过内嵌聊天实时生成工作流定义 |
| **阶段运行器** | 在工作流执行时可视化阶段列、尝试卡片和进度条 |
| **内置种子工作流** | Plan Polisher 和 Plan Executor 开箱即用 |

---

## 在 IDE 中打开

| 功能 | 描述 |
|---------|-------------|
| **一键打开文件** | 会话中引用的文件直接在编辑器中打开 |
| **自动检测编辑器** | VS Code、Cursor、Zed 等——无需配置 |
| **随处可用** | 按钮出现在变更标签页、文件头部和看板项目标题中 |
| **偏好记忆** | 跨会话记住你首选的编辑器 |

---

## 技术架构

| | |
|---|---|
| **极致性能** | Rust 后端，SIMD 加速 JSONL 解析，内存映射 I/O——数千个会话秒级完成索引 |
| **实时推送** | 文件监听 + SSE + 多路复用 WebSocket，支持心跳、事件回放和崩溃恢复 |
| **极致轻量** | 约 10 MB 下载，约 27 MB 磁盘占用。无运行时依赖，无后台守护进程 |
| **100% 本地** | 所有数据留在你的机器上。默认零遥测，零必需账户 |
| **零配置** | `npx claude-view` 即可上手。无需 API 密钥、无需设置、无需账户 |
| **FSM 驱动** | 聊天会话运行在有限状态机上，具有明确的阶段和类型化事件——确定性、无竞态 |

<details>
<summary><strong>性能数据</strong></summary>
<br>

在 M 系列 Mac 上测量，涵盖 26 个项目中的 1,493 个会话：

| 指标 | claude-view | 典型 Electron 仪表板 |
|--------|:-----------:|:--------------------------:|
| **下载大小** | **约 10 MB** | 150-300 MB |
| **磁盘占用** | **约 27 MB** | 300-500 MB |
| **启动时间** | **< 500 毫秒** | 3-8 秒 |
| **内存（完整索引）** | **约 50 MB** | 300-800 MB |
| **索引 1,500 个会话** | **< 1 秒** | 不适用 |
| **运行时依赖** | **0** | Node.js + Chromium |

关键技术：SIMD 预过滤（`memchr`）、内存映射 JSONL 解析、Tantivy 全文搜索、从 mmap 到解析再到响应的零拷贝切片。

</details>

---

## 横向对比

| 工具 | 类别 | 技术栈 | 大小 | 实时监控 | 多会话聊天 | 搜索 | 分析 | MCP 工具 |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | 监控 + 工作区 | Rust | **约 10 MB** | **是** | **是** | **是** | **是** | **85** |
| [opcode](https://github.com/winfunc/opcode) | GUI + 会话管理 | Tauri 2 | 约 13 MB | 部分 | 否 | 否 | 是 | 否 |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI 使用追踪 | TypeScript | 约 600 KB | 否 | 否 | 否 | CLI | 否 |
| [CodePilot](https://github.com/op7418/CodePilot) | 桌面聊天 UI | Electron | 约 140 MB | 否 | 否 | 否 | 否 | 否 |
| [claude-run](https://github.com/kamranahmedse/claude-run) | 历史查看器 | TypeScript | 约 500 KB | 部分 | 否 | 基础 | 否 | 否 |

> 聊天 UI（CodePilot、CUI、claude-code-webui）是 Claude Code 的*界面*。claude-view 是监控你现有终端会话的仪表板。它们是互补关系。

---

## 安装

| 方式 | 命令 |
|--------|---------|
| **Shell**（推荐） | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **插件**（自动启动） | `claude plugin add @claude-view/plugin` |

Shell 安装器下载预编译二进制文件（约 10 MB），安装到 `~/.claude-view/bin`，并添加到你的 PATH。然后只需运行 `claude-view`。

**唯一要求：** 已安装 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)。

<details>
<summary><strong>配置</strong></summary>
<br>

| 环境变量 | 默认值 | 描述 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` 或 `PORT` | `47892` | 覆盖默认端口 |

</details>

<details>
<summary><strong>自托管与本地开发</strong></summary>
<br>

预编译二进制文件内置了认证、分享和移动端中继功能。从源码构建？这些功能**通过环境变量选择性启用**——不设置任何变量，对应功能就不会启用。

| 环境变量 | 功能 | 不设置时 |
|-------------|---------|------------|
| `SUPABASE_URL` | 登录 / 认证 | 认证禁用——完全本地运行，零账户模式 |
| `RELAY_URL` | 移动端配对 | 二维码配对不可用 |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | 加密分享 | 分享按钮隐藏 |

```bash
bun dev    # 完全本地运行，无云端依赖
```

</details>

<details>
<summary><strong>企业 / 沙盒环境</strong></summary>
<br>

如果你的机器限制写入（DataCloak、CrowdStrike、企业 DLP）：

```bash
cp crates/server/.env.example .env
# 取消注释 CLAUDE_VIEW_DATA_DIR
```

这会将数据库、搜索索引和锁文件保存在仓库内部。设置 `CLAUDE_VIEW_SKIP_HOOKS=1` 可在只读环境中跳过 hook 注册。

</details>

---

## 常见问题

<details>
<summary><strong>即使已登录仍显示"未登录"横幅</strong></summary>
<br>

claude-view 通过读取 `~/.claude/.credentials.json`（带 macOS 钥匙串回退）来检查你的 Claude 凭据。请尝试以下步骤：

1. **验证 Claude CLI 认证状态：** `claude auth status`
2. **检查凭据文件：** `cat ~/.claude/.credentials.json` —— 应包含 `claudeAiOauth` 部分及 `accessToken`
3. **检查 macOS 钥匙串：** `security find-generic-password -s "Claude Code-credentials" -w`
4. **检查令牌过期时间：** 查看凭据 JSON 中的 `expiresAt` —— 如果已过期，运行 `claude auth login`
5. **检查 HOME 目录：** `echo $HOME` —— 服务器从 `$HOME/.claude/.credentials.json` 读取

如果所有检查通过但横幅仍然存在，请在 [Discord](https://discord.gg/G7wdZTpRfu) 上反馈。

</details>

<details>
<summary><strong>claude-view 访问哪些数据？</strong></summary>
<br>

claude-view 读取 Claude Code 写入 `~/.claude/projects/` 的 JSONL 会话文件。使用 SQLite 和 Tantivy 在本地建立索引。**除非你主动使用加密分享功能，否则没有数据会离开你的机器。** 遥测功能为可选项，默认关闭。

</details>

<details>
<summary><strong>是否支持 VS Code / Cursor / IDE 扩展中的 Claude Code？</strong></summary>
<br>

是的。claude-view 监控所有 Claude Code 会话，无论其启动方式——终端 CLI、VS Code 扩展、Cursor 或 Agent SDK。每个会话都显示来源标记（Terminal、VS Code、SDK），你可以按启动方式过滤。

</details>

---

## 社区

- **官方网站：** [claudeview.ai](https://claudeview.ai) —— 文档、更新日志、博客
- **Discord：** [加入服务器](https://discord.gg/G7wdZTpRfu) —— 支持、功能请求、讨论
- **插件：** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) —— 86 个 MCP 工具、9 个技能、自动启动

---

<details>
<summary><strong>开发</strong></summary>
<br>

前置要求：[Rust](https://rustup.rs/)、[Bun](https://bun.sh/)、`cargo install cargo-watch`

```bash
bun install        # 安装所有工作区依赖
bun dev            # 启动全栈开发（Rust + Web + Sidecar，支持热重载）
```

### 工作区结构

| 路径 | 包名 | 用途 |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA（Vite）—— 主 Web 前端 |
| `apps/share/` | `@claude-view/share` | 分享查看器 SPA —— Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo 原生应用 |
| `apps/landing/` | `@claude-view/landing` | Astro 5 落地页（零客户端 JS） |
| `packages/shared/` | `@claude-view/shared` | 共享类型和主题 token |
| `packages/design-tokens/` | `@claude-view/design-tokens` | 颜色、间距、字体排版 |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code 插件（MCP 服务器 + 工具 + 技能） |
| `crates/` | — | Rust 后端（Axum） |
| `sidecar/` | — | Node.js sidecar（Agent SDK 桥接） |
| `infra/share-worker/` | — | Cloudflare Worker —— 分享 API（R2 + D1） |
| `infra/install-worker/` | — | Cloudflare Worker —— 安装脚本与下载追踪 |

### 开发命令

| 命令 | 描述 |
|---------|-------------|
| `bun dev` | 全栈开发 —— Rust + Web + Sidecar，支持热重载 |
| `bun run dev:web` | 仅 Web 前端 |
| `bun run dev:server` | 仅 Rust 后端 |
| `bun run build` | 构建所有工作区 |
| `bun run preview` | 构建 Web + 通过 release 二进制文件提供服务 |
| `bun run lint:all` | 检查 JS/TS + Rust（Clippy） |
| `bun run typecheck` | TypeScript 类型检查 |
| `bun run test` | 运行所有测试（Turbo） |
| `bun run test:rust` | 运行 Rust 测试 |
| `bun run storybook` | 启动 Storybook 进行组件开发 |
| `bun run dist:test` | 构建 + 打包 + 安装 + 运行（完整发布测试） |

### 发布

```bash
bun run release          # 补丁版本号升级
bun run release:minor    # 次版本号升级
git push origin main --tags    # 触发 CI → 构建 → 自动发布到 npm
```

</details>

---

## 平台支持

| 平台 | 状态 |
|----------|--------|
| macOS (Apple Silicon) | 已支持 |
| macOS (Intel) | 已支持 |
| Linux (x64) | 计划中 |
| Windows (x64) | 计划中 |

---

## 相关项目

- **[claudeview.ai](https://claudeview.ai)** —— 官方网站、文档和更新日志
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** —— Claude Code 插件，包含 86 个 MCP 工具和 9 个技能。`claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** —— Claude Code 会在 30 天后删除你的会话。这个工具帮你保存它们。`npx claude-backup`

---

<div align="center">

如果 **claude-view** 帮助你看到 AI 代理正在做什么，请考虑给它一个 star。

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
