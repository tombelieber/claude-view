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
- **OpenClaw** calls Anthropic API directly via Pi runtime, does NOT use Claude Code CLI
- **NanoClaw** uses Agent SDK to spawn sessions in containers via stdin/stdout
- **PTY attachment** not feasible on macOS (TIOCSTI blocked, reptyr unsupported)
- **Prompt caching** makes resume cost-effective: 90% discount on cached tokens, 5-min TTL
- **Anthropic API is stateless** - every request sends full conversation history
- **Auto-compaction** triggers at ~75-95% context window usage
- **Existing tools**: claude-code-ui (XState), claude-code-monitor (hooks), clog (web viewer) - none combine monitoring + control + cost tracking
