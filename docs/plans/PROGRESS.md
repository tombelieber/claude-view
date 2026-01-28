# vibe-recall — Progress Dashboard

> Single source of truth. Replaces scanning 12 plan files.
>
> **Last updated:** 2026-01-29

---

## Business Model: Open-Core

```
┌─────────────────────────────────────────────────────────┐
│  Personal Tier (Open Source)                            │
│  - Browse, search, export sessions                      │
│  - Full metrics: atomic units, derived stats, trends    │
│  - Git correlation (ultra-conservative, provable only)  │
│  - `npx claude-view` — zero friction install            │
│                                                         │
│  ══════════════════════════════════════════════════════ │
│                                                         │
│  Enterprise Tier (Paid License)                         │
│  - Team aggregation (multi-user data)                   │
│  - Manager dashboards & admin controls                  │
│  - AI fluency scoring across employees                  │
│  - Export for HR/compliance/audits                      │
└─────────────────────────────────────────────────────────┘
```

**Strategy:** Build all analytics features (enterprise-grade quality) in Personal tier first. Enterprise tier adds the **team aggregation layer** on top — same features, but across multiple users.

---

## At a Glance

| Phase | Status | Progress | Tier |
|-------|--------|----------|------|
| **Phase 1: Foundation** | **DONE** | 8/8 tasks | Personal |
| **Phase 2A-1: Parallel Indexing** | **DONE** | 11/11 steps | Personal |
| **Phase 2A-2: Invocable Registry** | **DONE** | 12/12 steps | Personal |
| **Phase 2B: Token & Model Tracking** | **DONE** | 12/12 steps | Personal |
| **Phase 2C: API Split + UX Polish** | **DONE** | 24/24 steps | Personal |
| Phase 2D: Session Health | Merged into Phase 3 | — | — |
| **Phase 3: Metrics Engine** | **DONE** | 48/48 steps — atomic units, derived metrics, git correlation, trends, export | Personal |
| Phase 4: Distribution (npx) | Not started | — | Personal |
| Phase 5: Enterprise Team Layer | Not started | — | **Enterprise** |
| Phase 6: Search (Tantivy) | Deferred | — | Both |

**Current focus:** Phase 4 Distribution (npx) — next up

**Code compiles:** Yes (cargo check passes, 224+ backend tests green, TypeScript compiles cleanly)

---

## Phase 2A-1: Parallel Indexing — DONE

All steps complete and working in production (491 sessions, 0.1s Pass 1, 1.8s Pass 2).

| # | Step | Status | Notes |
|---|------|--------|-------|
| 1 | Fix discovery.rs compilation | **DONE** | Project compiles |
| 6 | `session_index.rs` — parse sessions-index.json | **DONE** | Both formats supported |
| 7 | `indexer_parallel.rs` — pass_1_read_indexes | **DONE** | Working in production |
| 8 | `indexer_parallel.rs` — read_file_fast + parse_bytes | **DONE** | mmap + SIMD |
| 9 | `indexer_parallel.rs` — pass_2_deep_index (pipeline) | **DONE** | Parallel JSONL parsing works |
| 10 | `indexer_parallel.rs` — run_background_index | **DONE** | Orchestrator working |
| 11 | IndexingState + AppState | **DONE** | Lock-free atomics |
| 12 | `main.rs` — server-first startup | **DONE** | Server ready before indexing |
| 13 | SSE `/api/indexing/progress` | **DONE** | Streaming events |
| 15 | TUI progress display | **DONE** | indicatif spinners |
| 16 | Acceptance tests | **DONE** | Integration tests pass |
| 17 | Performance benchmarks | **DONE** | Benchmark tool exists |

---

## Phase 2A-2: Invocable Registry — DONE

Tracks which skills, commands, agents, and MCP tools get used. Includes perf fixes for existing indexer.

All 12 steps complete. 284 tests pass across workspace.

| # | Step | Status | Notes |
|---|------|--------|-------|
| P1 | Fix `read_file_fast()` — zero-copy mmap | **DONE** | Removed `.to_vec()`, parse directly from mmap |
| P2 | Fix `parse_bytes()` — hoist Finders | **DONE** | All Finders hoisted, passed by &ref |
| 2 | `registry.rs` — parse plugins, scan dirs | **DONE** | Registry struct + lookup maps + 20 built-in tools |
| 4 | Migration 5 — invocables/invocations tables | **DONE** | Schema + 3 indexes |
| 3 | `invocation.rs` — classify_tool_use | **DONE** | Skill/Task/MCP/builtin classification + 28 tests |
| 5 | `queries.rs` — invocable + invocation CRUD + batch | **DONE** | 5 new methods + batch writes + stats overview |
| 9 | Extend `parse_bytes()` → ParseResult | **DONE** | RawInvocation extraction with SIMD pre-filter |
| 9b | Integrate invocations into pass_2_deep_index | **DONE** | Classify + batch insert in pipeline |
| 10b | Update `run_background_index` — tokio::join! | **DONE** | Pass 1 ∥ Registry build |
| 11b | Update AppState — `RwLock<Option<Registry>>` | **DONE** | RegistryHolder type alias |
| 12b | Update main.rs — registry holder | **DONE** | Registry passed to background + API |
| 14 | Routes: `/api/invocables`, `/api/stats/overview` | **DONE** | Two new GET endpoints |

---

## Phase 2B: Token & Model Tracking — DONE

Tracks per-API-call token usage and model identity. 2 new endpoints, extended session responses.

| # | Step | Status | Notes |
|---|------|--------|-------|
| 1 | `RawTurn` type + extend `ParseResult` | **DONE** | `types.rs` |
| 2 | Migration 6: `models` + `turns` tables | **DONE** | Schema + indexes |
| 3 | `batch_upsert_models` + `batch_insert_turns` | **DONE** | Transaction-batched writes |
| 4 | `parse_model_id()` helper | **DONE** | Provider/family extraction |
| 5 | SIMD turn extraction in `parse_bytes()` | **DONE** | `usage_finder` pre-filter |
| 6 | Integrate into `pass_2_deep_index()` | **DONE** | Models + turns batch insert |
| 7 | `get_all_models()` + `get_token_stats()` | **DONE** | Aggregate queries |
| 8 | `GET /api/models` route | **DONE** | Models with usage counts |
| 9 | `GET /api/stats/tokens` route | **DONE** | Token economics |
| 10 | Session queries with token LEFT JOIN | **DONE** | 6 new fields on SessionInfo |
| 11 | Golden parse test fixtures | **DONE** | Turn data in fixtures |
| 12 | Acceptance test: full pipeline | **DONE** | AC-13 token verification |

---

## Phase 2C: API Split + UX Polish — DONE

All 24 steps complete. Shipped in commit `4c12be4`.

**Part A — Backend (10 steps):** `ProjectSummary[]` API, paginated `/api/projects/:id/sessions`, `/api/stats/dashboard`, Migration 7 indexes, 30+ tests.

**Part B — Frontend (14 steps):** Full a11y pass (focus-visible, aria-labels, skip link, reduced motion), new API hooks, VSCode-style sidebar with tree roles + arrow-key nav, human-readable session URLs via slug utility.

---

## Phase 3: Metrics Engine — DONE (Personal Tier, Enterprise-Grade)

Pure facts, no judgment. Collect atomic units, compute derived metrics, let users interpret.

**Part A — Backend (28 steps):** Migration 8, atomic unit extraction (user prompts, API calls, tool calls, files read/edited, re-edits, duration), skill invocation detection, pipeline integration, derived metrics (tokens/prompt, re-edit rate, tool density, edit velocity, read-to-edit ratio), git correlation (Tier 1-2), trends (week-over-week), index metadata, 7 new API routes (filter/sort, export, status, git sync), golden tests, edge case tests.

**Part B — Frontend (20 steps):** TypeScript type exports, 5 new hooks (useTrends, useExport, useStatus, useGitSync, extended useDashboardStats), MetricCard + DashboardMetricsGrid (6 cards with trends), RecentCommits, SessionMetricsBar, FilesTouchedPanel, CommitsPanel, SessionCard metrics row + time range, FilterSortBar with URL persistence, Settings page, StatusBar data freshness footer, loading states (Skeleton/ErrorState/EmptyState), accessibility audit (WCAG, Lucide icons, aria-labels, focus-visible), 6 E2E test files.

**15 commits, 224 backend tests passing, TypeScript compiles cleanly.**

**Key design decisions:**
- **No health labels** — Show metrics, not judgment (Smooth/Turbulent removed)
- **Atomic units** — Measure smallest provable units (prompts, files, tokens)
- **Derived on read** — Store atomic units, compute metrics in API layer
- **Ultra-conservative git** — Only Tier 1-2 (provable evidence), no fuzzy matching
- **UI/UX Pro Max** — Data-dense dashboard style, Fira fonts, Lucide icons

See `docs/plans/2026-01-28-phase3-metrics-engine.md` for full plan.

---

## Phase 5: Enterprise Team Layer — Not Started

Multi-user aggregation and admin features. Transforms single-user analytics into team-wide insights.

**Scope:**
- Team/organization model (users belong to teams)
- Aggregated dashboards (team-wide health, commits, skills)
- Admin controls (who can see what)
- Manager views (employee AI fluency scoring)
- Compliance exports (CSV/PDF for HR audits)
- License validation (paid tier enforcement)

**Enterprise use cases:**
- "How is my team using Claude Code?"
- "Who are the most effective AI-assisted developers?"
- "What's the ROI on our Claude Pro seats?"
- "Export usage data for quarterly reviews"

Plan file: TBD (will be created when Phase 5 begins)

---

## Plan File Index

Quick reference so you never have to scan the folder again.

| File | Status | Role |
|------|--------|------|
| `vibe-recall-v2-design.md` | approved | **Master roadmap** — 5-phase architecture |
| `phase2-parallel-indexing-and-registry.md` | approved | **Active work** — Phase 2A-2 registry + perf fixes |
| `sqlite-indexer-startup-ux.md` | done | All 6 tasks delivered by Phase 2 work |
| `vibe-recall-phase1-implementation.md` | done | All 7 tasks complete (workspace, types, parser, discovery, server) |
| `ux-polish-a11y-sidenav-urls.md` | superseded | Merged into Phase 2C |
| `api-schema-bonus-fields-design.md` | superseded | Merged into Phase 2C |
| `phase2c-api-split-ux-polish.md` | done | Phase 2C — API split + UX polish, 24/24 steps |
| `skills-usage-analytics-prd.md` | superseded | PRD merged into Phase 3 plan |
| `phase2b-token-model-tracking.md` | done | Phase 2B — token/model tracking, 12/12 steps |
| `phase3-metrics-engine.md` | done | **Phase 3** — atomic units, derived metrics, git correlation, 48 steps |
| `2026-01-29-phase3b-git-sync-orchestrator.md` | done | **Phase 3B** — wire git sync orchestrator, fix sync route, auto-sync on startup, fix frontend refresh |
| `vibe-recall-analytics-design.md` | superseded | Merged into Phase 3 plan |
| `path-resolution-dfs-design.md` | done | Archived — shipped |
| `phase2-backend-integration.md` | done | Archived — shipped |
| `rust-backend-parity-fix.md` | done | Archived — shipped |
| `startup-ux-parallel-indexing.md` | superseded | Archived — merged into Phase 2 |

---

## How to Use This File

- **Starting a session:** Read this file first. It tells you exactly where you are.
- **After finishing work:** Update the step tracker above.
- **Adding new work:** Add to "Queued Work" section, not a new plan file.
