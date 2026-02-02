# vibe-recall — Progress Dashboard

> Single source of truth. Replaces scanning 12 plan files.
>
> **Last updated:** 2026-01-31

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
| **Phase 3.5: Full JSONL Parser** | **DONE** | 10/10 tasks — full 7-type extraction, ParseDiagnostics, parse_version re-index | Personal |
| **Phase 4: Distribution (npx)** | **IN PROGRESS** | 5/7 tasks done — awaiting human setup (npm token) + first release | Personal |
| **Phase 4B: Session Parser + UI Wiring** | **DONE** | 4/4 tasks — 7-type parser rewrite, TS types, compact/full toggle, Track 4 wiring | Personal |
| Phase 5: Enterprise Team Layer | Not started | — | **Enterprise** |
| Phase 6: Search (Tantivy) | Deferred | — | Both |

**Current focus:** Phase 4 Distribution (npx) — code done, awaiting human setup (npm account + token + GitHub secret) then first release

**Recently completed:** Phase 4B Session Parser + UI Wiring — parser rewrite emits all 7 JSONL types, compact/full toggle in conversation UI

**Pre-release:** Privacy scrub complete — all personal identifiers removed from code, tests, docs, config. Archived plans deleted. Repo ready for public visibility.

**Code compiles:** Yes (cargo check passes, 577+ backend tests green, TypeScript compiles cleanly)

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

## Phase 3.5: Full JSONL Parser — DONE

Full 7-type extraction (assistant, result, tool_use, tool_result, system, user, summary) with ParseDiagnostics and parse_version-triggered re-indexing. Extended `update_session_deep_fields` from 21 to 39 parameters.

See `docs/plans/2026-01-29-full-jsonl-parser.md` for full plan.

---

## Phase 4B: Session Parser + UI Wiring — DONE

Upgraded session parser to emit all 7 JSONL line types and wired to frontend conversation UI with compact/full toggle.

5 commits, 239 backend tests passing, 0 TypeScript errors in changed files.

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1 | Extend Role enum + Message struct | **DONE** | 7 Role variants, uuid/parent_uuid/metadata fields |
| 2 | Rewrite parse_session() | **DONE** | serde_json::Value dispatch, all 7 types, 14 new tests |
| 3 | Regenerate TypeScript types | **DONE** | ts-rs auto-generated Role.ts + Message.ts |
| 4 | Wire frontend + compact/full toggle | **DONE** | Segmented control, filterMessages, Track 4 cards, dark mode |

See `docs/plans/2026-01-31-session-parser-ui-wiring.md` for full plan.

---

## Phase 4: Distribution (npx) — IN PROGRESS

Ship `npx claude-view` with checksum verification, automated npm publish, and provenance attestation.

| # | Task | Status | Notes |
|---|------|--------|-------|
| — | Human setup: npm account + token + GitHub secret | **PENDING** | Must be done before Task 7 |
| 1 | Add SHA256 checksum generation to CI | **DONE** | `checksums.txt` in GitHub Release |
| 2 | Add checksum verification to npx wrapper | **DONE** | SHA256 verify before execute |
| 3 | Add automated npm publish to CI | **DONE** | `--provenance --access public` |
| 4 | Add version sync check to CI | **DONE** | Tag vs package.json guard, `shell: bash` for Windows |
| 5 | Update release script message | **DONE** | Reflects auto npm publish |
| 6 | Dry run validation | **PENDING** | `npm pack --dry-run` + local smoke test |
| 7 | First release | **PENDING** | Tag + push + verify npm + e2e test |

See `docs/plans/2026-01-29-phase4-npx-release.md` for full plan.

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

Clean 3-tier structure: active work only in main folder.

### Active Plans (in `/docs/plans/`)

| File | Status | Role |
|------|--------|------|
| `vibe-recall-v2-design.md` | approved | **Master roadmap** — 5-phase architecture |
| `2026-01-29-phase4-npx-release.md` | pending | **Current work** — checksum verification, npm publish CI, release pipeline |
| `2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` | pending | **Pre-release security** — 7 TDD-first fixes (DOMPurify, XSS, error boundaries, nesting, null safety) |
| `2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md` | pending | **Phase 1 UI rebuild** — 4-phase implementation covering message types, XML cards, hierarchy |
| `2026-01-31-session-parser-ui-wiring.md` | done | **Session parser rewrite** — 7-type parser, compact/full toggle, Track 4 wiring |
| `2026-01-29-UI-TESTING-STRATEGY.md` | pending | **Testing reference** — Jest + RTL framework for 20+ components |

### Reference Plans (in `/docs/plans/archived/`)

All phases completed. Keep for reference only — do not modify.

| File | Phase |
|------|-------|
| `2026-01-27-vibe-recall-phase1-implementation.md` | Phase 1 |
| `2026-01-27-phase2-parallel-indexing-and-registry.md` | Phase 2A |
| `2026-01-28-phase2b-token-model-tracking.md` | Phase 2B |
| `2026-01-28-phase2c-api-split-ux-polish.md` | Phase 2C |
| `2026-01-28-phase3-metrics-engine.md` | Phase 3 |
| `2026-01-29-phase3b-git-sync-orchestrator.md` | Phase 3B |
| `2026-01-29-full-jsonl-parser.md` | Phase 3.5 |
| `2026-01-29-jsonl-parser-spec.md` | Phase 3.5 |
| `2026-01-28-session-view-ux-polish.md` | UX |
| `2026-01-28-xml-card-full-coverage-design.md` | UX |
| `2026-01-28-rust-ts-type-sync-design.md` | Type Sync |
| `2026-01-27-history-view-date-grouping-design.md` | UX |
| `2026-01-29-pre-release-privacy-scrub.md` | Release Prep |

---

## How to Use This File

- **Starting a session:** Read this file first. It tells you exactly where you are.
- **Checking Phase N design:** See `/archived/` for the completed implementation spec.
- **Adding new work:** Create plan in main folder, add entry to "At a Glance", then move to `/archived/` when done.
