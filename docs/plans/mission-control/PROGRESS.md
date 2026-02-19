---
status: approved
date: 2026-02-12
feature: mission-control
---

# Mission Control - Feature Progress

> Central dashboard for monitoring and managing all active Claude Code sessions across the machine.

## At a Glance

| Phase | Name | Status | Description |
|-------|------|--------|-------------|
| A | Read-Only Monitoring | `done` | JSONL file watching, session state machine, cost calculator, SSE, Grid view |
| B | Views & Layout | `done` | List/Kanban views, view switcher, keyboard shortcuts, mobile responsive |
| B2 | Intelligent Session States | `superseded` | Superseded by `2026-02-15-agent-state-hooks-design.md`. Original: 3-state model, pause classification, module scoping |
| C | Monitor Mode | `done` | Live chat grid, WebSocket + RichPane (HTML), verbose toggle, responsive pane grid |
| D | Sub-Agent Visualization | `done` | Swim lanes, sub-agent extraction, compact pills, timeline view |
| D.2 | Sub-Agent Deep Dive | `done` | Real-time progress, drill-down conversations, sub-agent WebSocket streaming |
| E | Custom Layout | `pending` | react-mosaic drag-and-drop, layout save/load, presets |
| F | Interactive Control | `pending` | Node.js sidecar, Agent SDK resume, dashboard chat, bidirectional WebSocket |
| G | Codex Multi-Provider Foundation | `pending` | Source-aware IDs/schema, provider adapters, startup/indexing root abstraction |
| H | Codex Historical Sessions | `pending` | Codex discovery + deep parse + `/api/sessions/*` parsing + historical UI source support |
| I | Codex Live Mission Control | `pending` | Codex watcher/process/parser integration into live manager + mixed-source Mission Control UI |
| J | Codex Hardening & Rollout | `pending` | Fixture corpus, migration/backfill hardening, source-scoped reindex, metrics, rollout flags |

## Dependencies

```
Phase A ──► Phase B ──► Phase B2 ──► Phase C ──► Phase D
                                                   │
                                          Phase E ◄─┘
                                          Phase F (independent of E, depends on A)

Phase A ──► Phase G ──► Phase H ──┐
                    └──► Phase I ─┼──► Phase J
                                  ┘
```

- **A** is the foundation - everything depends on it
- **B** depends on A (needs session data to display in different views)
- **B2** superseded by `2026-02-15-agent-state-hooks-design.md` (originally: replaces 5-state enum with 3-state, adds intelligent pause classification)
- **C** depends on B (Monitor is a view mode, needs the view switcher infrastructure)
- **D** depends on C (sub-agent viz appears inside Monitor panes and session cards)
- **E** depends on C (custom layout applies to Monitor mode panes)
- **F** depends on A only (Agent SDK sidecar just needs the session list to know what to resume)
- **G** depends on A (builds source-aware foundation on top of existing session/live infrastructure)
- **H** depends on G (Codex historical indexing/parsing requires source model + provider routing)
- **I** depends on G (Codex live monitoring requires source-tagged watcher/manager/process plumbing)
- **J** depends on H + I (hardening and rollout after historical + live paths are both implemented)

**External dependents:**
- **Mobile PWA** (M1: Status Monitor) depends on **Phase A** — needs JSONL file watching + session state machine for remote status
- **Mobile PWA** (M3: Interactive Control) depends on **Phase F** — needs Agent SDK sidecar for remote session resume/control

## Plan File Index

| File | Phase | Status |
|------|-------|--------|
| [`design.md`](design.md) | All | `approved` |
| [`phase-a-monitoring.md`](phase-a-monitoring.md) | A | `done` |
| [`phase-b-views-layout.md`](phase-b-views-layout.md) | B | `done` |
| [`phase-b2-intelligent-states.md`](phase-b2-intelligent-states.md) | B2 | `superseded` |
| [`phase-c-monitor-mode.md`](phase-c-monitor-mode.md) | C | `in-progress` |
| [`phase-d-subagent-viz.md`](phase-d-subagent-viz.md) | D | `done` |
| [`phase-d2-subagent-drilldown.md`](phase-d2-subagent-drilldown.md) | D.2 | `done` |
| [`phase-e-custom-layout.md`](phase-e-custom-layout.md) | E | `pending` |
| [`phase-f-interactive.md`](phase-f-interactive.md) | F | `pending` |
| [`phase-g-codex-foundation.md`](phase-g-codex-foundation.md) | G | `pending` |
| [`phase-h-codex-historical-sessions.md`](phase-h-codex-historical-sessions.md) | H | `pending` |
| [`phase-i-codex-live-mission-control.md`](phase-i-codex-live-mission-control.md) | I | `pending` |
| [`phase-j-codex-hardening-rollout.md`](phase-j-codex-hardening-rollout.md) | J | `pending` |

## Key Decisions Log

| Date | Decision | Context |
|------|----------|---------|
| 2026-02-16 | **Codex support uses explicit source-aware identity (`source`, `source_session_id`, canonical `id`) instead of path inference.** | Robust multi-provider requirement: avoid session ID collisions and eliminate Claude-only assumptions in indexing/live routes. |
| 2026-02-16 | **Monitor mode uses RichPane (HTML) exclusively -- no xterm.js.** xterm.js deferred to Phase F (Interactive Control) where we own the PTY via Agent SDK. Monitor mode reads JSONL (structured data) so HTML rendering is strictly better. Verbose toggle replaces raw/rich toggle. | Existing sessions run in VS Code/terminal -- we can't tap their PTY. Our only interface is JSONL log files. HTML renders markdown (tables, bold, code) better than terminal ANSI conversion. |
| 2026-02-15 | **NO unbounded AI classification on session discovery.** Tier 2 AI (claude CLI) disabled until rate-limited. Structural-only + fallback. | Phase B2 shipped with `spawn_ai_classification()` firing for every Paused session on startup. 40 JSONL files → 40 concurrent `claude -p` processes → timeouts, rate limits, infinite retry loop. Fix: removed `old_status.is_none()` trigger, replaced AI fallback with sync fallback. Re-add AI with `Semaphore(1)` when needed. |
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

## Phase D: Sub-Agent Visualization - Implementation Progress

**Status:** `in-progress` (4 of 7 tasks complete, 2026-02-16)

### Completed Work (Tasks 1-4)

#### ✅ Task 1: TimelineView Component (Complete)
- **Files:** `src/components/live/TimelineView.tsx` (234 lines), `TimelineView.test.tsx` (400+ lines)
- **Features:**
  - Gantt-like horizontal timeline visualization
  - Adaptive time intervals (5s, 10s, 30s, 1m, 5m, 10m based on duration)
  - Percentage-based CSS positioning (no chart libraries)
  - Radix UI tooltips with agent details
  - Color coding: green (complete), red (error), animated pulse (running)
  - Min 2px bar width for visibility
- **Tests:** 23 tests passing (coverage: empty state, intervals, positioning, tooltips, edge cases)
- **Quality:** Spec 100% ✓, Code approved, all tests passing

#### ✅ Task 2: useSubAgents Hook (Complete)
- **Files:** `src/components/live/use-sub-agents.ts` (31 lines), `use-sub-agents.test.ts` (320+ lines)
- **Features:**
  - Filters sub-agents by status (active, completed, errored)
  - Aggregates total cost (handles null/undefined with `?? 0`)
  - Convenience flags: activeCount, isAnyRunning
  - useMemo for stable references
- **Tests:** 20 tests passing (status filtering, cost aggregation, memoization, null handling, large arrays)
- **Quality:** Spec 100% ✓, Code approved, character-for-character match to plan

#### ✅ Task 3: CostTooltip Updates (Complete)
- **Files:** `src/components/live/CostTooltip.tsx`, `SessionCard.tsx`
- **Features:**
  - Added `subAgents?: SubAgentInfo[]` prop
  - Tree-structured breakdown with `├──` and `└──` characters
  - Main agent cost calculation: total - sum(sub-agents)
  - Monospace font, `.toFixed(4)` formatting
  - Sub-agent count indicator in SessionCard: "(N sub-agents)"
  - Conditional rendering (only when sub-agents have costs)
- **Quality:** Spec 100% ✓, Code approved with polish fixes applied

#### ✅ Task 4: UI Integration (Complete)
- **Files:** `MonitorPane.tsx`, `MonitorView.tsx`, `SwimLanes.tsx`, `SwimLanes.test.tsx` (NEW)
- **Features:**
  - **MonitorPane:** SubAgentPills in footer with expand callback
  - **MonitorView:** SwimLanes above terminal stream in expanded overlay
  - **SwimLanes:** Added `sessionActive` prop, fixed interface, component wiring
  - Conditional rendering based on `session.subAgents?.length > 0`
- **Tests:** 14 SwimLanes tests passing (rendering, sorting, metrics, sessionActive prop, scrolling)
- **Quality:** Spec 100% ✓ (after fixes), Code approved

### Test Coverage Summary

| Component | Tests | Status |
|-----------|-------|--------|
| TimelineView | 23 tests | ✅ All passing |
| useSubAgents | 20 tests | ✅ All passing |
| SwimLanes | 14 tests | ✅ All passing |
| **Total** | **57 tests** | **✅ 100% passing with vitest** |

### Backend Data Flow (Complete from Prior Tasks)

| Layer | File | Status |
|-------|------|--------|
| Types | `crates/core/src/subagent.rs` | ✅ SubAgentInfo, SubAgentStatus with ts-rs |
| Parsing | `crates/core/src/live_parser.rs` | ✅ SIMD finders, spawn/completion extraction |
| State | `crates/server/src/live/state.rs` | ✅ LiveSession.sub_agents field |
| Manager | `crates/server/src/live/manager.rs` | ✅ Accumulator tracking, cost calculation |
| API | SSE `session_updated` events | ✅ Automatic broadcast with full LiveSession |
| Frontend | `src/components/live/use-live-sessions.ts` | ✅ TypeScript types + subAgents field |

### Remaining Work (Tasks 5-7)

| # | Task | Status | Description |
|---|------|--------|-------------|
| 5 | Backend tests | 📋 Pending | JSONL parsing tests (spawn/completion detection, SIMD pre-filter, edge cases) |
| 6 | Frontend tests | 🔄 Mostly done | Component tests complete (57 tests), may need integration tests |
| 7 | Verification | 📋 Pending | Run full test suite, verify ts-rs generation, end-to-end testing |

### Files Created/Modified

**New Files (10):**
- `crates/core/src/subagent.rs`
- `src/components/live/TimelineView.tsx`
- `src/components/live/TimelineView.test.tsx`
- `src/components/live/use-sub-agents.ts`
- `src/components/live/use-sub-agents.test.ts`
- `src/components/live/SwimLanes.test.tsx`
- CSS animations in `src/index.css` (timeline-bar-growing, swimlane-progress)

**Modified Files (9):**
- `crates/core/src/lib.rs`
- `crates/core/src/live_parser.rs`
- `crates/server/src/live/state.rs`
- `crates/server/src/live/manager.rs`
- `src/components/live/use-live-sessions.ts`
- `src/components/live/CostTooltip.tsx`
- `src/components/live/SessionCard.tsx`
- `src/components/live/MonitorPane.tsx`
- `src/components/live/MonitorView.tsx`
- `src/components/live/SwimLanes.tsx`

### How to Test

```bash
# Start dev server
bun dev

# Run component tests
bun run vitest run src/components/live/TimelineView.test.tsx
bun run vitest run src/components/live/use-sub-agents.test.ts
bun run vitest run src/components/live/SwimLanes.test.tsx

# Type check
bun run typecheck

# Backend tests (when Task 5 complete)
cargo test -p vibe-recall-core -- live_parser
cargo test -p vibe-recall-core -- subagent
```

### Next Session Goals

1. **Task 5:** Add backend tests for JSONL sub-agent parsing
2. **Task 6:** Verify frontend test coverage, add integration tests if needed
3. **Task 7:** Run full verification suite, test end-to-end with real sessions
4. **Optional:** Add TimelineView to MonitorView for completed sessions (marked optional in spec)

## Phase D.2: Sub-Agent Deep Dive — Implementation Progress

**Status:** `done` (12 of 12 tasks complete, 2026-02-17)

| # | Task | Status | Commit |
|---|------|--------|--------|
| 1 | Progress Event SIMD Finder + LiveLine Extension | ✅ Done | `f2b0261` |
| 2 | SubAgentInfo Type Extension (current_activity) | ✅ Done | `9ad7c7d` |
| 3 | Manager Progress Event Processing | ✅ Done | `f36ad2e` |
| 4 | Sub-Agent Activity Display in SwimLanes | ✅ Done | `4726f04` |
| 5 | Sub-Agent File Resolution Utility | ✅ Done | `b7f81ec` |
| 6 | Sub-Agent Terminal WebSocket Endpoint | ✅ Done | `24428a8` |
| 7 | Sub-Agent Drill-Down Hook (Frontend) | ✅ Done | `66dbdb5` |
| 8 | Sub-Agent Drill-Down Panel Component | ✅ Done | `3284366` |
| 9 | SwimLanes Click-to-Expand Integration | ✅ Done | `6a0cdd5` |
| 10 | Backend Tests — Progress Events | ✅ Done | `0573d3e` |
| 11 | Frontend Tests — Drill-Down Integration | ✅ Done | `315aa64` |
| 12 | Verification & End-to-End | ✅ Done | — |

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
