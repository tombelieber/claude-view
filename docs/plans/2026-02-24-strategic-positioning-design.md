---
status: approved
date: 2026-02-24
topic: strategic-positioning
---

# Strategic Positioning: claude-view as AI Coding Command Center

## One-Line Positioning

**"Mission Control for AI coding agents — monitor, orchestrate, and command your fleet from desktop or phone."**

## Market Context

The AI coding tool market has stratified into four layers:

| Layer | Examples | What they do |
|-------|----------|-------------|
| 4: App Builders | Lovable ($1.8B), Base44 (acq. by Wix) | "Prompt to app" for non-devs |
| 3: IDE Agents | Kiro (AWS), Cursor, Windsurf | Single-session, spec-driven coding |
| 2: Terminal Agents | Claude Code, OpenCode (100k+ stars) | Raw power, expert-only |
| 1: Mobile/Remote | Happy Coder (8.2k stars) | Mobile access to Layer 2 |

**Nobody occupies the orchestration layer between 2 and 3.** Kiro does specs but can't orchestrate parallel agents. Claude Code does parallel agents (Task tool) but has no dashboard. Happy gives you mobile access but zero intelligence.

**claude-view occupies Layer 2.5: Orchestration + Intelligence.**

## Competitive Analysis

### vs Happy Coder

| | Happy Coder | claude-view |
|---|---|---|
| Architecture | Wraps `claude` CLI (fragile) | Reads session files (stable) |
| Mobile | Native iOS + Android (Expo) | PWA (no app store) |
| Session interaction | Full remote control + voice | M1: monitoring, M2+: full control |
| Analytics | None — live view only | Deep — history, cost, patterns, search |
| Multi-session | Start sessions manually | **Orchestrate N parallel agents** |
| Spec/plan-driven | No | **Plan runner (Phase K)** |
| Stars / community | 8.2k stars, 26 contributors | Growing |

**Edge:** Analytics moat, orchestration capability, zero CLI modification. Happy is a walkie-talkie to one agent. claude-view is NASA Mission Control.

**Risk:** Happy has mobile market mindshare and native app store presence. Mitigated by differentiated value prop (you don't compete on "chat from phone" — you compete on "command your fleet from phone").

### vs Kiro (AWS)

| | Kiro | claude-view |
|---|---|---|
| Spec workflow | Requirements → Design → Tasks (polished) | Plan files with frontmatter (simpler) |
| Execution | Single agent, single IDE, sequential | **N agents, N worktrees, parallel** |
| Agent hooks | Event-driven triggers (on-save, etc.) | Session monitoring + orchestration |
| Mobile | None | **Yes** |
| Monitoring | None | **Deep** (cost, context, sub-agents) |
| Lock-in | Must use Kiro IDE | Works with any terminal/IDE |

**Edge:** Parallel execution at scale, mobile access, works alongside existing tools. Kiro forces you into their IDE; claude-view works with whatever you already use.

### vs OpenCode

| | OpenCode | claude-view |
|---|---|---|
| What it is | Terminal AI agent (Go, Bubble Tea TUI) | Orchestration dashboard |
| Multi-provider | Yes (OpenAI, Anthropic, Gemini, etc.) | Claude-focused (Codex planned) |
| Monitoring | None | **Deep** |
| Mobile | None | **Yes** |
| Community | 100k+ stars, 2.5M monthly users | Growing |

**Edge:** Completely different category. OpenCode is an agent; claude-view manages agents. Potential integration target, not competitor.

### Competitive Matrix

| Capability | claude-view | Happy | Kiro | OpenCode | Cursor |
|---|---|---|---|---|---|
| Multi-session monitoring | **Deep** | Live only | No | No | No |
| Parallel orchestration | **Planned** | No | No | No | No |
| Spec/plan-driven | **Planned** | No | **Yes** | Plan mode | No |
| Mobile | PWA | **Native** | No | No | No |
| Session interaction | Planned (F) | **Yes** | **Yes** | **Yes** | **Yes** |
| Analytics | **Deep** | None | None | None | None |
| Full-text search | **Tantivy** | None | In-IDE | None | In-IDE |
| Architecture risk | Low | **High** | Low | Low | Low |
| Works with existing sessions | **Yes** | No | No | No | No |

## Product Architecture: Three Engines

```
┌──────────────────────────────────────────────────────────────────┐
│                        claude-view                                │
│                                                                   │
│  Engine 1: MONITOR     Engine 2: CONTROL     Engine 3: ORCHESTRATE│
│  (Phases A-D, done)    (Phase F, planned)    (Phase K, new)      │
│                                                                   │
│  • Session discovery   • Resume session      • Plan runner        │
│  • Live SSE            • Send messages        • Parallel agents   │
│  • Cost/context        • Tool approval        • Auto-worktree     │
│  • Sub-agents          • Spawn session        • Auto-review/test  │
│  • Analytics           • Agent SDK sidecar    • Artifact inspect  │
│  • Full-text search    • Bidirectional WS     • Merge/conflict    │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │                     MOBILE LAYER                              ││
│  │  M1: Monitor    M2: Control    M3: Orchestrate   M4: Inspect ││
│  └──────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────┘
```

### Engine 3: Plan Runner (Phase K)

The most differentiated piece. User provides plan files (markdown + frontmatter), claude-view:

1. Parses plan files, builds dependency DAG
2. Creates git worktrees per plan
3. Spawns Agent SDK sessions in parallel (Semaphore-bounded, default 3)
4. Streams progress to dashboard + mobile
5. Runs tests on completion
6. Presents results for approval (diff viewer, test output, cost)
7. Merges approved worktrees into base branch

#### Plan File Format

```markdown
---
id: add-auth-middleware
depends_on: []
parallel: true
test: "cargo test -p server auth::"
model: sonnet
max_cost: 2.00
---

# Add Auth Middleware

## Context
...

## Requirements
1. ...

## Acceptance Criteria
- [ ] ...
```

#### Plan Runner Architecture

| Component | Location | Responsibility |
|-----------|----------|---------------|
| Plan Parser | `crates/core/` | Parse frontmatter, extract deps, validate |
| Dependency Resolver | `crates/core/` | Build DAG, determine parallelism |
| Worktree Manager | `crates/server/` | `git worktree add/remove`, lifecycle |
| Agent Pool | `crates/server/` + sidecar | Spawn Agent SDK sessions, Semaphore-bounded |
| Result Collector | `crates/server/` | Track status, diff, test output, cost per plan |
| Review Gate | `crates/server/` | Run tests, lint, present to user |
| Merge Engine | `crates/server/` | Fast-forward merge, conflict detection |

#### Plan Runner vs Kiro Specs

| | Kiro Specs | claude-view Plan Runner |
|---|---|---|
| Input | Single prompt → auto-generates reqs/design/tasks | User provides plan files |
| Execution | Single agent, sequential tasks | **N agents, N worktrees, parallel** |
| Review | Human reviews in IDE | Dashboard: diff, tests, approve/reject |
| Mobile | No | **Yes** (launch, monitor, approve from phone) |
| Scope | One feature at a time | **10 features simultaneously** |
| Cost control | Not visible | Per-plan budgets + real-time tracking |

## Mobile Strategy

| Milestone | Engine | From your phone |
|-----------|--------|----------------|
| **M1** (current) | Monitor | See sessions, status, cost, last message over 5G |
| **M2** | Control | Send messages, approve tools, spawn sessions |
| **M3** | Orchestrate | Launch plan sets, monitor parallel progress, approve results |
| **M4** | Orchestrate+ | View artifacts (diffs, screenshots), resolve merge conflicts |

**Mobile design principle:** On your phone you don't write code — you command, observe, and decide. The phone is the general's tablet, not the soldier's rifle.

## Three-Month Roadmap

| Month | Focus | Deliverable |
|---|---|---|
| 1 | Mobile M1 + Phase F start | Scan QR → see sessions on phone. Sidecar boots. |
| 2 | Phase F complete + M2 | Spawn/resume from dashboard. Control from phone. |
| 3 | Plan Runner MVP (Phase K) + M3 | Run 3 plans in parallel, approve from phone. |

Month 3 scope is deliberately limited: 3 parallel max, manual plan files, test + diff review only, fast-forward merge only.

## Risks & Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Agent SDK instability | Plan runner breaks | Pin version, adapter interface, integration tests |
| Parallel worktrees thrash I/O | Machine slows | Semaphore (default 3), user-configurable |
| Plan files too freeform | Agent misinterprets | Opinionated schema, validate before exec, dry-run |
| Happy ships orchestration | Lose edge | Unlikely — CLI-wrapping makes multi-worktree hard |
| Kiro adds multi-agent | Lose parallel edge | Kiro is IDE-bound, we're browser + mobile |
| Cost overruns | Users burn $50 | Per-plan `max_cost`, dashboard totals, abort threshold |

## Monetization Angle

| Tier | What | Why they'd pay |
|---|---|---|
| Free | Monitor + analytics | Hooks users, builds community |
| Pro | Control (Phase F) + 3 parallel plans | Power users running multiple sessions daily |
| Team | 10+ parallel, shared dashboards, team analytics | Engineering teams doing AI sprints |

## North Star Vision

This document covers the near-term (3-month) positioning as "AI Coding Command Center."

The long-term vision — making AI coding as easy as breathing, the "iPhone of AI coding" — is documented separately in:

**`docs/plans/vision-iphone-of-ai-coding.md`**

That vision informs every design decision but is not the immediate build target. Build the power-user command center first, evolve toward consumer-grade simplicity.

## Related Documents

| Doc | What |
|-----|------|
| `docs/plans/mission-control/design.md` | Mission Control full design spec |
| `docs/plans/mission-control/phase-f-interactive.md` | Phase F: Interactive Control (Engine 2) |
| `docs/plans/mobile-remote/design.md` | Mobile remote architecture |
| `docs/plans/vision-iphone-of-ai-coding.md` | Long-term vision (Approach C) |

## Sources

- [Happy Coder](https://github.com/slopus/happy) — 8.2k stars, mobile Claude Code client
- [Happy Coder features](https://happy.engineering/docs/features/)
- [Kiro](https://kiro.dev/) — AWS agentic IDE, spec-driven development
- [Kiro Steering & Autopilot](https://kiro.dev/docs/steering/)
- [OpenCode](https://github.com/opencode-ai/opencode) — 100k+ stars, terminal AI agent
- [Lovable](https://lovable.dev/) — $1.8B valuation AI app builder
- [Parallelizing AI Coding Agents](https://ainativedev.io/news/how-to-parallelize-ai-coding-agents)
- [Git Worktrees for Parallel AI Agents](https://medium.com/@mabd.dev/git-worktrees-the-secret-weapon-for-running-multiple-ai-coding-agents-in-parallel-e9046451eb96)
