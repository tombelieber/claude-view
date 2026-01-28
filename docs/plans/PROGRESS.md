# vibe-recall — Progress Dashboard

> Single source of truth. Replaces scanning 12 plan files.
>
> **Last updated:** 2026-01-28

---

## At a Glance

| Phase | Status | Progress |
|-------|--------|----------|
| **Phase 1: Foundation** | **DONE** | 8/8 tasks |
| **Phase 2A-1: Parallel Indexing** | **DONE** | 11/11 steps — pipeline works in production |
| **Phase 2A-2: Invocable Registry** | **DONE** | 12/12 steps — skill/tool tracking + perf fixes |
| **Phase 2B: Token & Model Tracking** | **DONE** | 12/12 steps — turns, models, token APIs |
| **Phase 2C: API Split + UX Polish** | **Approved** | 0/24 steps |
| Phase 2D: Session Health | Deferred | — |
| Phase 3: Metrics & Analytics | Not started | — |
| Phase 4: Search (Tantivy) | Not started | — |
| Phase 5: Distribution (npx) | Not started | — |

**Current focus:** Phase 2C approved — API split + UX polish

**Code compiles:** Yes (cargo check passes, 308 tests green)

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

## Phase 2C: API Split + UX Polish — Approved

Split over-fetching API into focused endpoints + frontend accessibility and polish.

**Part A — Backend (10 steps):** Split `/api/projects` into `ProjectSummary[]`, add paginated `/api/projects/:id/sessions`, add `/api/stats/dashboard`.

**Part B — Frontend (14 steps):** Accessibility fixes (a11y), wire new API hooks, VSCode sidebar redesign, human-readable URLs.

See `docs/plans/2026-01-28-phase2c-api-split-ux-polish.md` for full plan.

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
| `phase2c-api-split-ux-polish.md` | approved | Phase 2C — API split + UX polish, 24 steps |
| `skills-usage-analytics-prd.md` | draft | PRD spanning Phase 2A-2 (registry) + 2B (tokens) + Phase 3 |
| `phase2b-token-model-tracking.md` | done | Phase 2B — token/model tracking, 12/12 steps |
| `vibe-recall-analytics-design.md` | draft | Design doc for Phase 3 (analytics) |
| `path-resolution-dfs-design.md` | done | Archived — shipped |
| `phase2-backend-integration.md` | done | Archived — shipped |
| `rust-backend-parity-fix.md` | done | Archived — shipped |
| `startup-ux-parallel-indexing.md` | superseded | Archived — merged into Phase 2 |

---

## How to Use This File

- **Starting a session:** Read this file first. It tells you exactly where you are.
- **After finishing work:** Update the step tracker above.
- **Adding new work:** Add to "Queued Work" section, not a new plan file.
