---
status: approved
date: 2026-02-05
purpose: Theme 4 Master Design — Chat Insights & Pattern Discovery
---

# Theme 4: Chat Insights & Pattern Discovery

> **Goal:** Help users understand their AI coding patterns, discover improvement opportunities, and manage system health.

## User Stories

| Persona | Question | Feature |
|---------|----------|---------|
| Individual Dev | "How can I improve my prompting?" | Pattern discovery |
| Individual Dev | "What do I use AI for?" | Category breakdown |
| Individual Dev | "Am I getting better?" | Benchmarks & trends |
| Power User | "What's my data status?" | System page |

## Design Principles

1. **Human-readable insights** — Every chart has plain-English "so what?" line
2. **Progressive disclosure** — Hero → Quick stats → Tabs → Drill-down
3. **Async by default** — Classification never blocks; progress always visible
4. **Zero config default** — Claude CLI works without setup
5. **BYOK ready** — Provider settings for API keys, future monetization

---

## Information Architecture

**Two new pages:**

```
/insights          ← Pattern discovery & self-improvement
/system            ← Nerd stats & system management
```

**Nav structure:**

```
Sidebar
├── Dashboard
├── History
├── Contributions    ← Theme 3
├── Insights         ← Theme 4 (NEW)
├── Projects
└── System           ← Theme 4 (NEW)
```

---

## Classification System (LLM-Powered)

### Two-Level Hierarchy (30 Categories)

```
Code Work (12 leaves)
├── Feature
│   ├── new-component
│   ├── add-functionality
│   └── integration
├── Bug Fix
│   ├── error-fix
│   ├── logic-fix
│   └── performance-fix
├── Refactor
│   ├── cleanup
│   ├── pattern-migration
│   └── dependency-update
└── Testing
    ├── unit-tests
    ├── integration-tests
    └── test-fixes

Support Work (9 leaves)
├── Docs
│   ├── code-comments
│   ├── readme-guides
│   └── api-docs
├── Config
│   ├── env-setup
│   ├── build-tooling
│   └── dependencies
└── Ops
    ├── ci-cd
    ├── deployment
    └── monitoring

Thinking Work (9 leaves)
├── Planning
│   ├── brainstorming
│   ├── design-doc
│   └── task-breakdown
├── Explanation
│   ├── code-understanding
│   ├── concept-learning
│   └── debug-investigation
└── Architecture
    ├── system-design
    ├── data-modeling
    └── api-design
```

### LLM Provider Configuration

**Default: Claude CLI** (uses existing subscription)

```bash
claude -p --output-format json --dangerously-skip-permissions --model haiku "prompt"
```

**BYOK Options:**
- Anthropic API (bring your key)
- OpenAI-compatible endpoint

### Background Classification Flow

```
Deep Index (fast, 3s)
    ↓
Sessions stored with category = NULL
    ↓
User triggers "Classify" from /system page
    ↓
Background job spawns (async)
    ↓
Progress via SSE: "1,247 / 5,865 (21.3%)"
    ↓
UI updates as batches complete
```

---

## Pattern Detection (60+ Patterns)

### Pattern Categories

| Category | Examples |
|----------|----------|
| **Prompt** | Length, question vs command, specificity, context given |
| **Session** | Duration, turn count, warmup effect, fatigue signal |
| **Temporal** | Time of day, day of week, break impact, consecutive sessions |
| **Workflow** | Skill sequences, category transitions, planning ratio |
| **Model** | Model-task fit, switching patterns, cost-quality tradeoff |
| **Codebase** | Language efficiency, file type patterns, project complexity |
| **Outcome** | Commit rate by category, abandoned sessions, revert correlation |
| **Behavioral** | Retry patterns, escalation, abandonment triggers |

### Impact Scoring

Each pattern scored 0-1 based on:
- Effect size (how much improvement)
- Sample size (statistical confidence)
- Actionability (can user act on it)

---

## Implementation Phases

| Phase | Scope | Dependencies |
|-------|-------|--------------|
| **Phase 1** | Foundation (data model, CLI integration, job runner) | None |
| **Phase 2** | Classification system (API, UI, providers) | Phase 1 |
| **Phase 3** | System page (/system) | Phase 1 |
| **Phase 4** | Pattern detection engine | Phase 1 |
| **Phase 5** | Insights page - Core | Phase 4 |
| **Phase 6** | Insights page - Categories tab | Phase 2, 5 |
| **Phase 7** | Insights page - Trends tab | Phase 5 |
| **Phase 8** | Insights page - Benchmarks tab | Phase 5 |

### Parallel Execution Order

```
         Phase 1 (Foundation)
              ↓
    ┌─────────┼─────────┐
    ↓         ↓         ↓
Phase 2   Phase 3   Phase 4
    │         │         │
    └─────────┼─────────┘
              ↓
         Phase 5 (Core Insights)
              ↓
    ┌─────────┼─────────┐
    ↓         ↓         ↓
Phase 6   Phase 7   Phase 8
```

**Can run in parallel:**
- Phase 2, 3, 4 (after Phase 1)
- Phase 6, 7, 8 (after Phase 5)

---

## Detailed Phase Plans

Each phase has its own detailed plan file:

- `theme4-phase1-foundation.md`
- `theme4-phase2-classification.md`
- `theme4-phase3-system-page.md`
- `theme4-phase4-pattern-engine.md`
- `theme4-phase5-insights-core.md`
- `theme4-phase6-categories-tab.md`
- `theme4-phase7-trends-tab.md`
- `theme4-phase8-benchmarks-tab.md`

---

## Acceptance Criteria Summary

### Must Have (MVP)

- [ ] Claude CLI spawning works
- [ ] Background classification with progress (SSE)
- [ ] `/system` page: storage, performance, health
- [ ] `/insights` page: hero insight, patterns tab
- [ ] 20+ patterns calculated and ranked
- [ ] Time range filter

### Should Have

- [ ] All 60+ patterns
- [ ] Categories tab with treemap
- [ ] Trends tab with charts
- [ ] Benchmarks tab with Then vs Now
- [ ] BYOK: Anthropic API, OpenAI-compatible

### Nice to Have

- [ ] Category drill-down to L3
- [ ] Skill learning curve chart
- [ ] Monthly report PDF export

---

## Related Documents

- Theme 1: `2026-02-04-session-discovery-design.md`
- Theme 2: `2026-02-05-dashboard-analytics-design.md`
- Theme 3: `2026-02-05-theme3-git-ai-contribution-design.md`
- Progress: `theme4/PROGRESS.md`
