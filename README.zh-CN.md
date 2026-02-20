# claude-view

<p align="center">
  <strong>Claude Code 电源用户的实时监控与副驾驶。</strong>
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

## 问题

你已经打开了 3 个项目。每个项目有多个 git 工作树。每个工作树上有多个 Claude Code 会话运行。有的在思考，有的在等你，有的快要达到上下文限制，还有一个 10 分钟前就完成了但你忘了。

你用 Cmd-Tab 在 15 个终端窗口间切换，试着记起每个会话在做什么。你浪费了 tokens，因为一个缓存在你没注意时过期了。你失去了工作流，因为没有一个地方能看到所有东西。而在那个「思考中...」的转轮后面，Claude 正在生成子代理、调用 MCP 服务器、执行技能、触发钩子——而你看不到任何一个。

**Claude Code 非常强大。但在没有仪表板的情况下驾驭 10+ 个并发会话，就像开着没有时速表的车一样。**

## 解决方案

**claude-view** 是一个实时仪表板，与你的 Claude Code 会话并肩运行。一个浏览器标签页，每个会话都可见，一目了然的完整上下文。

```bash
npx claude-view
```

就这样。在浏览器中打开。你的所有会话——实时的和过去的——都在一个工作区里。

---

## 你会得到什么

### 实时监控

| 功能 | 为什么重要 |
|---------|------------------|
| **会话卡片及最后消息** | 立即回忆起每个长时间运行的会话在做什么 |
| **通知音效** | 当会话完成或需要你的输入时收到提醒——停止轮询终端 |
| **上下文量表** | 每个会话的实时上下文窗口使用情况——看出哪些在危险区域 |
| **缓存预热倒计时** | 准确知道何时提示缓存过期，以便你能在节省 tokens 的时候安排下一则消息 |
| **成本追踪** | 每个会话和总体支出，以及缓存节省的分解 |
| **子代理可视化** | 看到完整的代理树——子代理、它们的状态，以及它们调用的工具 |
| **多个视图** | 网格、列表或监控模式（实时聊天网格）——选择最适合你工作流的 |

### 丰富的聊天历史

| 功能 | 为什么重要 |
|---------|------------------|
| **完整对话浏览器** | 每个会话、每条消息，完整呈现 markdown 和代码块 |
| **工具调用可视化** | 看到文件读取、编辑、bash 命令、MCP 调用、技能调用——不只是文字 |
| **精简/详细切换** | 快速浏览对话或深入查看每个工具调用 |
| **执行线程视图** | 跟踪代理对话与子代理层级结构 |
| **导出** | Markdown 导出用于上下文恢复或共享 |

### 进阶搜索

| 功能 | 为什么重要 |
|---------|------------------|
| **全文搜索** | 跨所有会话搜索——消息、工具调用、文件路径 |
| **项目与分支筛选** | 范围限定到你现在正在处理的项目 |
| **命令调色板** | Cmd+K 在会话间跳跃、切换视图、寻找任何东西 |

### 代理内部——看到隐藏的东西

Claude Code 在「思考中...」后面做了很多东西，这些从不会在你的终端显示。claude-view 揭露了所有这些。

| 功能 | 为什么重要 |
|---------|------------------|
| **子代理对话** | 看到完整的生成代理树、它们的提示和输出 |
| **MCP 服务器调用** | 看到哪些 MCP 工具被调用及其结果 |
| **技能/钩子/插件追踪** | 知道哪些技能触发了、哪些钩子执行了、什么插件处于活动状态 |
| **钩子事件记录** | 每个钩子事件都被捕捉和可浏览——回头检查什么触发了及何时触发。*(需要 claude-view 在会话活动时运行；无法追溯历史事件)* |
| **工具使用时间线** | 每个 tool_use/tool_result 配对及其时间的动作日志 |
| **错误浮现** | 错误浮现到会话卡片——不再有埋藏的失败 |
| **原始消息检查器** | 当你需要完整图像时，深入查看任何消息的原始 JSON |

### 分析

一套丰富的分析套件用于你的 Claude Code 使用。想象 Cursor 的仪表板，但更深入。

**仪表板概览**

| 功能 | 描述 |
|---------|--------|
| **周对周指标** | 会话计数、tokens 使用、成本——与你之前的期间比较 |
| **活动热图** | 90 天 GitHub 风格的网格显示你的日常 Claude Code 使用强度 |
| **排名前列的技能/命令/MCP 工具/代理** | 你最常用的调用项的排行榜——点击任何一个来搜索匹配的会话 |
| **最活跃的项目** | 按会话计数排名的项目的柱状图 |
| **工具使用分解** | 跨所有会话的总编辑、读取和 bash 命令 |
| **最长会话** | 快速存取你的马拉松会话及其持续时间 |

**AI 贡献**

| 功能 | 描述 |
|---------|--------|
| **代码输出追踪** | 行数新增/删除、受影响的文件、跨所有会话的提交计数 |
| **成本 ROI 指标** | 每次提交的成本、每个会话的成本、每行 AI 输出的成本——带趋势图表 |
| **模型比较** | 依模型（Opus、Sonnet、Haiku）的输出和效率的并排分解 |
| **学习曲线** | 随着时间推移的重新编辑率——看到自己在提示方面变得更好 |
| **分支分解** | 可折叠的逐分支视图及会话深入探讨 |
| **技能效能** | 哪些技能实际上改进了你的输出，哪些没有 |

**深入分析** *(实验性)*

| 功能 | 描述 |
|---------|--------|
| **模式检测** | 从你的会话历史发现的行为模式 |
| **当时 vs 现在基准** | 将你的第一个月与最近的使用进行比较 |
| **类别分解** | Treemap 显示你使用 Claude 做什么——重构、功能、调试等 |
| **AI 流利度分数** | 追踪你的整体效能的单一 0-100 数字 |

> **注意:** 深入分析和流利度分数处于早期实验阶段。当作方向性的，不是确定的。

---

## 为流程而打造

claude-view 是为以下开发者设计的：

- 同时运行 **3+ 个项目**，每个都有多个工作树
- 任何时候都有 **10-20 个 Claude Code 会话**打开
- 需要快速上下文切换而不失去追踪
- 想要**优化 tokens 支出**，通过在缓存窗口周围安排消息
- 对 Cmd-Tabbing 通过终端以检查代理而感到沮丧

一个浏览器标签页。所有会话。保持流程状态。

---

## 如何打造的

| | |
|---|---|
| **快如闪电** | Rust 后端，SIMD 加速的 JSONL 解析、内存映射 I/O——在几秒内索引数千个会话 |
| **实时** | 文件监视器 + SSE + WebSocket，所有会话的亚秒级实时更新 |
| **足迹很小** | 单一 ~15 MB 二进制。无运行时依赖、无后台守护进程 |
| **100% 本地** | 所有数据保留在你的机器上。零遥测、零云端、零网络请求 |
| **零配置** | `npx claude-view`，完成。无 API 密钥、无设置、无账户 |

---

## 快速开始

```bash
npx claude-view
```

在 `http://localhost:47892` 打开。

### 配置

| 环境变量 | 默认 | 描述 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` 或 `PORT` | `47892` | 覆盖默认端口 |

---

## 安装

| 方式 | 命令 |
|--------|---------|
| **npx**（推荐） | `npx claude-view` |
| **Shell 脚本**（无需 Node） | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### 需求

- **Claude Code** 已安装（[在此获取](https://docs.anthropic.com/en/docs/claude-code)）——这建立了我们监控的会话文件

---

## 比较方式

其他工具要么是查看器（浏览历史）要么是简单的监控器。没有任何工具在单一工作区中结合实时监控、丰富的聊天历史、调试工具和进阶搜索。

```
                    被动 ←————————————→ 主动
                         |                  |
            仅查看    |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            仅监控    |  claude-code-ui  |
                         |  Agent Sessions  |
                         |                  |
            完整      |  ★ claude-view   |
            工作区    |                  |
```

---

## 社群

加入 [Discord 服务器](https://discord.gg/G7wdZTpRfu)寻求支持、功能请求和讨论。

---

## 喜欢这个项目吗？

如果 **claude-view** 帮助你驾驭 Claude Code，请考虑给它一颗星。这有助于其他人发现这个工具。

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 开发

前置条件：[Rust](https://rustup.rs/)、[Bun](https://bun.sh/)、`cargo install cargo-watch`

```bash
bun install        # 安装前端依赖
bun dev            # 启动全栈开发（Rust + Vite 含热重载）
```

| 命令 | 描述 |
|---------|-------------|
| `bun dev` | 全栈开发——Rust 在变更时自动重启、Vite HMR |
| `bun dev:server` | 仅 Rust 后端（含 cargo-watch） |
| `bun dev:client` | 仅 Vite 前端（假设后端运行） |
| `bun run build` | 为生产构建前端 |
| `bun run preview` | 构建 + 通过发布二进制提供服务 |
| `bun run lint` | 检查前端（ESLint）和后端（Clippy） |
| `bun run fmt` | 格式化 Rust 代码 |
| `bun run check` | 类型检查 + 检查 + 测试（提交前闸门） |
| `bun test` | 执行 Rust 测试套件（`cargo test --workspace`） |
| `bun test:client` | 执行前端测试（vitest） |
| `bun run test:e2e` | 执行 Playwright 端对端测试 |

### 测试生产分发

```bash
bun run dist:test    # 一个命令：构建 → 打包 → 安装 → 执行
```

或一步步进行：

| 命令 | 描述 |
|---------|-------------|
| `bun run dist:pack` | 将二进制 + 前端打包到 `/tmp/` 的 tarball |
| `bun run dist:install` | 提取 tarball 到 `~/.cache/claude-view/`（模拟首次运行下载） |
| `bun run dist:run` | 使用缓存二进制执行 npx 包装器 |
| `bun run dist:test` | 上述全部于一个命令中 |
| `bun run dist:clean` | 移除所有 dist 缓存和临时文件 |

### 发布

```bash
bun run release          # 补丁凹凸：0.1.0 → 0.1.1
bun run release:minor    # 次要凹凸：0.1.0 → 0.2.0
bun run release:major    # 主要凹凸：0.1.0 → 1.0.0
```

这在 `npx-cli/package.json` 中凹凸版本、提交并创建一个 git 标签。然后：

```bash
git push origin main --tags    # 触发 CI → 构建所有平台 → 自动发布到 npm
```

---

## 平台支持

| 平台 | 状态 |
|----------|--------|
| macOS (Apple Silicon) | 可用 |
| macOS (Intel) | 可用 |
| Linux (x64) | 计划中 |
| Windows (x64) | 计划中 |

---

## 许可证

MIT © 2026
