---
status: approved
date: 2026-02-10
feature: mission-control
---

# Mission Control - Feature Progress

> Central dashboard for monitoring and managing all active Claude Code sessions across the machine.

## At a Glance

| Phase | Name | Status | Description |
|-------|------|--------|-------------|
| A | Read-Only Monitoring | `pending` | JSONL file watching, session state machine, cost calculator, SSE, Grid view |
| B | Views & Layout | `pending` | List/Kanban views, view switcher, keyboard shortcuts, mobile responsive |
| C | Monitor Mode | `pending` | Live terminal grid, WebSocket + xterm.js, responsive pane grid |
| D | Sub-Agent Visualization | `pending` | Swim lanes, sub-agent extraction, compact pills, timeline view |
| E | Custom Layout | `pending` | react-mosaic drag-and-drop, layout save/load, presets |
| F | Interactive Control | `pending` | Node.js sidecar, Agent SDK resume, dashboard chat, bidirectional WebSocket |

## Dependencies

```
Phase A ──► Phase B ──► Phase C ──► Phase D
                                       │
                              Phase E ◄─┘
                              Phase F (independent of E, depends on A)
```

- **A** is the foundation - everything depends on it
- **B** depends on A (needs session data to display in different views)
- **C** depends on B (Monitor is a view mode, needs the view switcher infrastructure)
- **D** depends on C (sub-agent viz appears inside Monitor panes and session cards)
- **E** depends on C (custom layout applies to Monitor mode panes)
- **F** depends on A only (Agent SDK sidecar just needs the session list to know what to resume)

**External dependents:**
- **Mobile PWA** (M1: Status Monitor) depends on **Phase A** — needs JSONL file watching + session state machine for remote status
- **Mobile PWA** (M3: Interactive Control) depends on **Phase F** — needs Agent SDK sidecar for remote session resume/control

## Plan File Index

| File | Phase | Status |
|------|-------|--------|
| [`design.md`](design.md) | All | `approved` |
| [`phase-a-monitoring.md`](phase-a-monitoring.md) | A | `pending` |
| [`phase-b-views-layout.md`](phase-b-views-layout.md) | B | `pending` |
| [`phase-c-monitor-mode.md`](phase-c-monitor-mode.md) | C | `pending` |
| [`phase-d-subagent-viz.md`](phase-d-subagent-viz.md) | D | `pending` |
| [`phase-e-custom-layout.md`](phase-e-custom-layout.md) | E | `pending` |
| [`phase-f-interactive.md`](phase-f-interactive.md) | F | `pending` |

## Key Decisions Log

| Date | Decision | Context |
|------|----------|---------|
| 2026-02-10 | Read-only monitoring for terminal sessions, no PTY attachment | macOS blocks TIOCSTI, reptyr not supported. Zero-friction > bidirectional control. |
| 2026-02-10 | Agent SDK for "Resume in Dashboard" (spawns new process with conversation history) | SDK cannot attach to existing sessions. Resume loads JSONL history into new subprocess. |
| 2026-02-10 | No tmux prerequisite | Too much friction for users. File watching is zero-setup. |
| 2026-02-10 | SSE for structured data, WebSocket for terminal streams | SSE simpler + auto-reconnect for status/cost. WebSocket needed for xterm.js binary stream. |
| 2026-02-10 | In-memory state for live sessions, SQLite for historical only | Live data changes every 1-5s, only ~20-50 sessions. SQLite adds unnecessary latency. |
| 2026-02-10 | Tailscale/Cloudflare for mobile access, not built into app | Document in README. User brings their own tunnel. |
| 2026-02-10 | Dark Mode OLED theme with green=working, amber=waiting status colors | Design system generated via UI/UX Pro Max. |
| 2026-02-10 | Prompt caching education via contextual tooltips, not docs pages | "Saved you $X" messaging at decision points. |
| 2026-02-10 | 4 view modes: Grid, List, Kanban, Monitor | Progressive complexity. Grid default for few sessions, List for many. |
| 2026-02-10 | Swim lanes for sub-agent visualization, timeline for history | Most intuitive for real-time parallel work. Node graph deferred to Teams/Swarm mode. |
| 2026-02-10 | react-mosaic for custom layout (Phase E) | Lightweight (8KB), React-native, used by Palantir. Over GoldenLayout (jQuery-era). |
| 2026-02-10 | Node.js sidecar for Agent SDK (Phase F) | SDK is npm-only. Rust handles HTTP/SSE/WebSocket/SQLite. Node handles Agent SDK IPC. |

## Research Artifacts

Key findings from brainstorming research (2026-02-10):

- **Claude Agent SDK** cannot attach to existing sessions - only spawns new subprocesses
- **PTY attachment** not feasible on macOS (TIOCSTI blocked, reptyr unsupported)
- **Prompt caching** makes resume cost-effective: 90% discount on cached tokens, 5-min TTL
- **Anthropic API is stateless** - every request sends full conversation history
- **Auto-compaction** triggers at ~75-95% context window usage
- **Existing tools**: claude-code-ui (XState), claude-code-monitor (hooks), clog (web viewer) - none combine monitoring + control + cost tracking

### OpenClaw Ecosystem Research (2026-02-10, corrected)

**Previous assumption was wrong.** OpenClaw IS built on top of Claude Code CLI subscription:

| Component | What it does | Relevance to us |
|-----------|-------------|-----------------|
| **OpenClaw core** (180k+ stars) | Personal AI agent on any messaging platform. WebSocket Gateway, Pi agent RPC, skill system via ClawHub. Built on Node.js ≥22. | Architecture reference for multi-agent orchestration. NOT relevant for session monitoring (different use case). |
| **claude-max-api-proxy** | Routes OpenAI-format requests (`localhost:3456/v1/chat/completions`) through Claude Code CLI using existing subscription auth. No separate API key needed. | Potential fallback for users who want OpenAI-compatible LLM access without an API key. Deferred — CLI Direct is simpler. |
| **OpenClaw Dashboard** | Scrapes CLI `/usage` via persistent tmux session. Reads `~/.openclaw/agents/` session dirs. 5s polling. Rate limit monitoring with "time to limit" predictions. Glassmorphic dark UI. | UI patterns to study: rate limit visualization, burn rate predictions, glassmorphic cards. But their session monitoring approach (tmux scraping) is inferior to our JSONL file watching. |
| **openclaw-claude-code-skill** | MCP integration for sub-agent orchestration. State persistence via IndexedDB/localStorage. Timestamp-based merge conflict resolution. | MCP patterns for future Phase F sub-agent coordination. |

**Key insight:** OpenClaw's `claude-max-api-proxy` provides a path for using the user's Claude subscription without an API key. However, `claude -p --model haiku "prompt"` (CLI Direct) achieves the same result with zero extra infrastructure. We use CLI Direct as our default, document claude-max-api-proxy as a deferred alternative.

### Claude Code `/insights` Research (2026-02-10)

Claude Code shipped a built-in `/insights` command (week of 2026-02-03). It uses Haiku to analyze sessions and generate an HTML report. This directly overlaps with Theme 4's planned classification system.

**`/insights` 6-stage pipeline:**
1. Filter sessions (skip <2 messages, <1min, sub-sessions)
2. Chunk long transcripts (>30k chars → 25k segments → summarize)
3. Facet extraction per session (Haiku, 4,096 max output tokens)
4. Aggregate analysis across sessions (Haiku, 8,192 max output tokens)
5. Executive summary
6. Render interactive HTML report at `~/.claude/usage-data/report.html`

**Facets extracted per session (cached at `~/.claude/usage-data/facets/<session-id>.json`):**
- 13 goal categories: `debug_investigate`, `implement_feature`, `fix_bug`, `write_script_tool`, `refactor_code`, `configure_system`, `create_pr_commit`, `analyze_data`, `understand_codebase`, `write_tests`, `write_docs`, `deploy_infra`, `warmup_minimal`
- 5 outcome levels: `not_achieved` → `fully_achieved`
- 6 satisfaction levels: `frustrated` → `happy`
- 12 friction types: `misunderstood_request`, `wrong_approach`, `buggy_code`, `user_rejected_action`, etc.
- 5 helpfulness levels: `unhelpful` → `essential`
- 5 session types: `single_task`, `multi_task`, `iterative_refinement`, `exploration`, `quick_question`
- 7 success categories: `fast_accurate_search`, `correct_code_edits`, `good_explanations`, etc.

**What `/insights` does NOT do (our opportunity):**
- No time-series trending — snapshot only, no history
- No cost-quality correlation — knows cost and quality separately, never connects them
- No cross-project comparison — flat list, no project grouping
- No inline integration — separate HTML file, not in any dashboard
- No proactive coaching — post-hoc report, not just-in-time nudges
- No gamification — no scores, streaks, or achievements

**Impact on Theme 4:** Our insights feature should NOT re-implement `/insights` facet extraction. Instead, we parasitize the cache (`~/.claude/usage-data/facets/*.json`), store in SQLite for trending, and build the time-series/coaching/ambient layer that `/insights` fundamentally cannot provide. See revised Theme 4 design.

### Other Reference Projects (2026-02-10)

| Project | Stars | What it does | Relevance |
|---------|-------|-------------|-----------|
| **Agent Sessions** | — | Local-first session aggregator for 7 CLI tools (Claude Code, Codex, Gemini, etc.). macOS-only. Read-only browsing, Apple Notes-style search. | UI reference for multi-tool session browsing. |
| **NanoClaw** | ~500 lines TS | Agent SDK + Apple containers for secure WhatsApp AI. | Container isolation pattern for Phase F. |
| **TinyClaw** | Small | Tiny wrapper for Claude Code on Discord/WhatsApp. File-based queue. | Not relevant for monitoring. |
| **NanoBot** (HKUDS) | ~4k lines | Ultra-lightweight personal AI assistant. Multi-platform messaging. | Not relevant for monitoring. |
