# vibe-recall — Progress Dashboard

> Single source of truth. Replaces scanning 12 plan files.
>
> **Last updated:** 2026-02-10

---

## Product Evolution

The product has evolved through 3 distinct stages. Each stage builds on the previous, not replacing it.

```
Stage 1: SESSION VIEWER (v0.1–v0.3)               ✅ SHIPPED
  "Browse and search Claude Code conversation history"
  └── Session browsing, JSONL parsing, search, export

Stage 2: AI FLUENCY COACH (v0.4–v1.0)             🔧 IN PROGRESS
  "Your AI fluency, measured"
  └── AI Fluency Score, metrics dashboard, analytics, insights
  └── Rename: claude-view → claude-score

Stage 3: AI AGENT OS (v1.x–v2.0)                  📐 DESIGNED
  "Mission Control for your AI workforce"
  └── Live session monitoring across all terminals
  └── Cost tracking, context usage, sub-agent visualization
  └── Resume sessions from web dashboard (Agent SDK)
  └── Kanban view of agent tasks
  └── Mobile access via Tailscale/Cloudflare tunnel
```

**Each stage expands the TAM:**

| Stage | Target User | Value Prop | TAM |
|-------|------------|------------|-----|
| Session Viewer | Claude Code user curious about history | "See your past sessions" | ~100K devs |
| AI Fluency Coach | Dev wanting to improve their AI workflow | "Get better at AI-assisted coding" | ~500K devs |
| AI Agent OS | Power user running 10-20+ concurrent agents | "Manage your AI workforce" | ~50K power users (but high willingness to pay) |

---

## Business Model: Open-Core

```
┌─────────────────────────────────────────────────────────┐
│  Personal Tier (Open Source)                            │
│  - Browse, search, export sessions                      │
│  - Full metrics: atomic units, derived stats, trends    │
│  - Git correlation (ultra-conservative, provable only)  │
│  - Mission Control: monitor all active sessions         │
│  - Cost tracking, context usage, sub-agent viz          │
│  - Resume sessions from web dashboard                   │
│  - `npx claude-view` — zero friction install            │
│                                                         │
│  ══════════════════════════════════════════════════════ │
│                                                         │
│  Enterprise Tier (Paid License)                         │
│  - Team aggregation (multi-user data)                   │
│  - Manager dashboards & admin controls                  │
│  - AI fluency scoring across employees                  │
│  - Multi-agent orchestration policies                   │
│  - Cost budgets & alerts per team/project               │
│  - Export for HR/compliance/audits                      │
└─────────────────────────────────────────────────────────┘
```

**Strategy:** Build all analytics features (enterprise-grade quality) in Personal tier first. Enterprise tier adds the **team aggregation layer** on top — same features, but across multiple users. Mission Control monitoring is Personal tier; orchestration policies and team cost budgets are Enterprise.

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
| **Phase 4: Distribution (npx)** | **DONE** | 7/7 tasks — checksum, OIDC publish, version guard, 3 releases shipped | Personal |
| **Phase 4B: Session Parser + UI Wiring** | **DONE** | 4/4 tasks — 7-type parser rewrite, TS types, compact/full toggle, Track 4 wiring | Personal |
| **Hardening: Security + Robustness** | **DONE** | 7/7 fixes — DOMPurify, XSS, ErrorBoundary, nesting cap, null safety, useEffect cleanup | Personal |
| **Thread Visualization & Dark Mode** | **DONE** | 5/5 tasks — buildThreadMap, ConversationView wiring, hover highlighting, dark mode, plan status | Personal |
| Phase 5: Enterprise Team Layer | Not started | — | **Enterprise** |
| **Deep Index Perf (Tasks 1-3)** | **DONE** | 3/3 tasks — tx batching, SIMD pre-filter, mtime re-index | Personal |
| **Deep Index Perf Instrumentation** | **DONE** | Timing breakdown (parse/write phase) in debug builds | Personal |
| **Deep Index Perf: rusqlite write phase** | **DONE** | 4/4 tasks — rusqlite dep, db_path, SQL constants, spawn_blocking write | Personal |
| **Session Loading Perf** | **DONE** | Paginated messages endpoint, tail-first loading | Personal |
| **Export Markdown** | **DONE** | Download + clipboard copy for context resumption | Personal |
| **Security Audit** | **DONE** | Critical/medium/low fixes — README accuracy, deps, unsafe code | Personal |
| **Session Discovery & Navigation** | **DONE** | 6/6 phases (A-F) — sidebar tree, project view, branch filters, expand/collapse, 438 tests | Personal |
| **Theme 2: Dashboard Analytics** | **DONE** | 5/5 features — time range filter, heatmap tooltips, sync redesign, AI generation breakdown, storage overview + responsive + observability + feature flags | Personal |
| **GTM Launch** | **In Progress** | README rewrite done, GTM strategy doc done, AI Fluency Score in progress (separate branch) | Personal |
| **Cold Start UX** | Pending | 0/7 tasks — bandwidth progress bar (TUI + frontend SSE overlay) | Personal |
| Phase 6: Search (Tantivy) | Deferred | — | Both |
| Phase 7: Multi-Tool Providers | Draft | Provider trait for Cursor, OpenCode, Aider, Windsurf, Cline | Both |
| App-Wide UI/UX Polish | Deferred | a11y, i18n, responsive, dark mode audit | Personal |
| **Theme 4: Chat Insights** | **DONE** | 8/8 phases, 39/39 tasks — classification, patterns, insights page shipped | Personal |
| **Ambient Coach (Insights v2)** | Pending | 0/13 tasks — facet cache ingest, fluency score, ambient coaching surfaces | Personal |
| **Rename to claude-score** | Approved | Plan written, 60+ files mapped — deferred until GTM launch features complete | Personal |
| **Mission Control** | Approved | 6 phases (A-F), 7,900+ lines of plans — live session monitoring, cost tracking, resume, sub-agent viz | Personal |
| **Mobile PWA** | Draft | 3 phases (M1-M3) — remote session monitoring via E2E encrypted relay + PWA. Depends on Mission Control A (M1) and F (M3) | Personal |

**Current focus:** GTM Launch (README repositioning, AI Fluency Score, demo GIF, Show HN prep)

**Recently completed:** Theme 2 Dashboard Analytics (5 features, 15 commits, 922 tests), GTM README rewrite, Session Discovery & Navigation (6 phases, 438 tests), v0.2.4 shipped

**Pre-release:** Privacy scrub complete — all personal identifiers removed from code, tests, docs, config. Archived plans deleted. Repo ready for public visibility.

**Code compiles:** Yes (cargo check passes, 548+ backend tests green, 552 frontend tests green, TypeScript compiles cleanly)

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

See `docs/plans/archived/2026-01-28-phase3-metrics-engine.md` for full plan.

---

## Phase 3.5: Full JSONL Parser — DONE

Full 7-type extraction (assistant, result, tool_use, tool_result, system, user, summary) with ParseDiagnostics and parse_version-triggered re-indexing. Extended `update_session_deep_fields` from 21 to 39 parameters.

See `docs/plans/archived/2026-01-29-full-jsonl-parser.md` for full plan.

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

See `docs/plans/archived/2026-01-31-session-parser-ui-wiring.md` for full plan.

---

## Hardening: Security + Robustness — DONE

7 TDD-first fixes across security, error handling, and robustness. 578 frontend tests green.

| # | Fix | Status | Notes |
|---|-----|--------|-------|
| 1 | DOMPurify + StructuredDataCard | **DONE** | `ALLOWED_TAGS: []`, `<pre>` rendering, 9 tests |
| 2 | UntrustedData XSS | **DONE** | Plaintext-only via DOMPurify |
| 3 | AgentProgressCard XSS | **DONE** | React auto-escape verified, JSDoc |
| 4 | ErrorBoundary integration | **DONE** | Wraps each MessageTyped in ConversationView Virtuoso list |
| 5 | Max nesting depth warning | **DONE** | Caps at 5 levels, `console.warn` on overflow |
| 6 | Null/undefined handling (10 components) | **DONE** | All card components handle missing data |
| 7 | useEffect cleanup | **DONE** | Listener cleanup on unmount verified |

See `docs/plans/archived/2026-02-02-hardening-final.md` for consolidated plan.

---

## Phase 4: Distribution (npx) — DONE

Ship `npx claude-view` with checksum verification, automated npm publish, and provenance attestation.

| # | Task | Status | Notes |
|---|------|--------|-------|
| — | Human setup: npm account + token + GitHub secret | **DONE** | OIDC trusted publisher configured |
| 1 | Add SHA256 checksum generation to CI | **DONE** | `checksums.txt` in GitHub Release |
| 2 | Add checksum verification to npx wrapper | **DONE** | SHA256 verify before execute |
| 3 | Add automated npm publish to CI | **DONE** | `--provenance --access public` |
| 4 | Add version sync check to CI | **DONE** | Tag vs package.json guard, `shell: bash` for Windows |
| 5 | Update release script message | **DONE** | Reflects auto npm publish |
| 6 | Dry run validation | **DONE** | `npm pack --dry-run` verified |
| 7 | First release | **DONE** | v0.2.0 → v0.2.3 shipped via OIDC trusted publisher |

See `docs/plans/archived/2026-01-29-phase4-npx-release.md` for full plan.

---

## GTM Launch — In Progress

Repositioning from "session browser" to "AI fluency tracker" for public launch.

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1 | README rewrite with new positioning | **DONE** | "Your AI fluency, measured" tagline, METR study hook, competitor matrix |
| 2 | GTM strategy document | **DONE** | `2026-02-07-gtm-launch-strategy.md` — positioning, content calendar, Show HN draft |
| 3 | AI Fluency Score (backend) | In progress | Separate branch — weighted score from 5 components |
| 4 | AI Fluency Score (frontend) | In progress | Separate branch — hero card on dashboard |
| 5 | Demo GIF | Pending | 30-second walkthrough for README + social |
| 6 | Launch blog post | Pending | "What I Learned from Analyzing 676 Claude Code Sessions" |
| 7 | Show HN post | Pending | Draft in strategy doc, needs timing |
| 8 | Twitter/X pre-launch content | Pending | Data insight threads, teaser charts |

See `docs/plans/2026-02-07-gtm-launch-strategy.md` for full plan.

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

## Deferred: Analytical Database (Pre-Aggregation / DuckDB)

**Decision: Keep current SQLite approach. Revisit when building Enterprise tier.**

**Context (2026-02-03):** During deep index performance work, we evaluated whether to pre-aggregate session metrics (reduce ~168k turn/invocation rows → ~800 session-level rows) or switch to DuckDB for analytical queries.

**Current performance (release mode, 1.2 GB / 676 sessions):**
- Parse phase: 170ms
- Write phase: 290ms
- Total: 460ms
- Incremental re-index: <100ms

**Projections (release mode, linear extrapolation):**

| Dataset | Sessions | Parse | Write | Total |
|---------|----------|-------|-------|-------|
| 1.2 GB (personal) | 676 | 170ms | 290ms | 460ms |
| 12 GB (work machine) | ~6,700 | ~1.7s | ~2.9s | ~4.6s |
| 50 GB (power user) | ~28,000 | ~7s | ~12s | ~19s |
| 100 GB (extreme) | ~56,000 | ~14s | ~24s | ~38s |

**Why defer:**
1. **Incremental indexing already solves the common case** — day-to-day re-index is <100ms regardless of total data size
2. **Full re-index is rare** — only on first run or `parse_version` bump (monthly at most)
3. **Cold start UX solves the perception problem** — 38s with a progress bar showing `2.7 GB/s` is acceptable; 38s with no feedback is not
4. **Pre-aggregation is a schema migration** — touches 6+ queries, changes data model. Better to design this into Enterprise tier from the start
5. **DuckDB adds a dependency** — separate engine, more binary size, more complexity for marginal gain at current scale
6. **SQLite handles up to 280 TB** — we're 1000x below the threshold where database choice matters

**When to revisit:**
- Enterprise tier design (Phase 5) — multi-user aggregation naturally requires pre-aggregated views
- If incremental re-index degrades (unlikely — it only touches changed files)
- If query latency on session list / dashboard becomes noticeable (currently <10ms)

---

## Deferred: App-Wide UI/UX Polish

**Decision: Batch all cross-cutting UI/UX concerns into a single phase after feature work completes.**

**Context (2026-02-05):** During Theme 3 design review, identified UI/UX polish items that apply across all features. Rather than address piecemeal per-feature, defer to a dedicated polish pass.

**Deferred items:**

| Category | Scope | Notes |
|----------|-------|-------|
| **Accessibility (a11y)** | App-wide | WCAG 2.1 AA audit, color contrast, screen reader, keyboard nav |
| **Internationalization (i18n)** | App-wide | Extract hardcoded strings, locale files, insight templates |
| **Responsive design** | App-wide | Mobile/tablet breakpoints, touch targets |
| **Dark mode audit** | App-wide | Verify all new components respect theme |
| **Loading states** | App-wide | Consistent skeleton/spinner patterns |
| **Error states** | App-wide | Consistent error message UX |

**When to execute:**
- After Themes 1-4 feature work ships
- Before v1.0 release
- Single dedicated phase with checklist

**Why batch:**
1. Avoids context-switching during feature dev
2. Ensures consistency across all features
3. More efficient to audit once vs per-feature
4. Can test holistically (e.g., full a11y audit)

---

## Mobile PWA — Draft

Remote session monitoring and control from mobile devices via Progressive Web App + E2E encrypted relay.

**Phases:**

| Phase | Name | Status | Description | Depends On |
|-------|------|--------|-------------|------------|
| M1 | Status Monitor | `draft` | Push notifications, session status cards, basic cost tracking on mobile | Mission Control Phase A |
| M2 | Read-Only Dashboard | `draft` | Full dashboard experience adapted for mobile, responsive charts/metrics | M1 |
| M3 | Interactive Control | `draft` | Resume/pause sessions, send prompts from mobile via Agent SDK relay | Mission Control Phase F |

Plan file: `2026-02-12-mobile-pwa-design.md` (to be created)

---

## Plan File Index

Clean 3-tier structure: active work only in main folder.

### Active Plans (in `/docs/plans/`)

| File | Status | Role |
|------|--------|------|
| `vibe-recall-v2-design.md` | approved | **Master roadmap** — 5-phase architecture |
| `2026-01-27-vibe-recall-analytics-design.md` | draft | **Analytics/Insights** — CLI stats, circle-back detection, insights generation (partially shipped via Phase 3, needs consolidation with skills PRD) |
| `2026-01-27-skills-usage-analytics-prd.md` | draft | **Skills analytics PRD** — to be consolidated into analytics design |
| `2026-01-27-export-pdf-design.md` | pending | **PDF export** — browser print-to-PDF, zero deps, ~30 lines |
| `2026-01-29-UI-TESTING-STRATEGY.md` | pending | **Testing reference** — Jest + RTL framework for 20+ components |
| `2026-02-03-cold-start-ux.md` | pending | **Cold start UX** — bandwidth progress bar (TUI + frontend SSE overlay), 7 tasks |
| `2026-02-03-readme-media-guide.md` | pending | **README media** — screenshot + demo GIF preparation guide |
| `2026-02-04-session-discovery-design.md` | pending | **Theme 1** — Session discovery & navigation enhancements |
| `2026-02-05-dashboard-analytics-design.md` | done | **Theme 2** — Dashboard & analytics enhancements (5 features, responsive, observability) |
| `2026-02-05-theme3-git-ai-contribution-design.md` | pending | **Theme 3** — Git integration & AI contribution tracking page |
| `2026-02-05-theme4-chat-insights-design.md` | pending | **Theme 4** — Chat insights & pattern discovery (see `theme4/PROGRESS.md` for detailed tracking) |
| `2026-02-07-gtm-launch-strategy.md` | in-progress | **GTM Launch** — positioning, competitive landscape, content strategy, Show HN plan |
| `2026-02-07-rename-to-claude-score.md` | approved | **Rename** — full rename plan: vibe-recall/claude-view → claude-score (60+ files, 4 tiers) |
| `2026-02-10-multi-tool-provider-design.md` | draft | **Phase 7** — Provider trait for Cursor, OpenCode, Aider, Windsurf, Cline support |
| `2026-02-04-brainstorm-checkpoint.md` | draft | **Brainstorm checkpoint** — resume point for future brainstorming |
| `2026-02-10-ambient-coach-insights.md` | pending | **Ambient Coach** — facet cache ingest from `/insights`, fluency score (0-100), session quality badges, dashboard coach card, pattern alerts, 6h auto-ingest cron |
| `mission-control/` | approved | **Mission Control** — live session monitoring, cost tracking, resume, sub-agent viz (6 phases, 8 files, see `mission-control/PROGRESS.md`) |
| `2026-02-12-mobile-pwa-design.md` | draft | **Mobile PWA** — remote session monitoring via E2E encrypted relay + PWA (3 phases: M1 Status Monitor, M2 Read-Only Dashboard, M3 Interactive Control). Depends on Mission Control Phase A (M1) and Phase F (M3) |

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
| `2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` | Hardening |
| `2026-02-02-hardening-final.md` | Hardening (consolidated) |
| `2026-01-29-phase4-npx-release.md` | Phase 4 Distribution |
| `2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md` | UI Redesign (superseded) |
| `2026-02-02-thread-visualization-polish.md` | Thread Visualization |
| `2026-01-31-session-parser-ui-wiring.md` | Phase 4B Parser |
| `2026-01-31-export-markdown.md` | Export |
| `2026-02-02-security-audit-critical.md` | Security Audit |
| `2026-02-02-security-audit-medium.md` | Security Audit |
| `2026-02-02-security-audit-low.md` | Security Audit |
| `2026-02-03-session-loading-perf.md` | Session Loading Perf |
| `2026-02-03-deep-index-perf.md` | Deep Index Perf |
| `2026-02-03-rusqlite-write-phase.md` | Perf: rusqlite Write |
| `2026-01-27-path-resolution-dfs-design.md` | Path Resolution |

---

## How to Use This File

- **Starting a session:** Read this file first. It tells you exactly where you are.
- **Checking Phase N design:** See `/archived/` for the completed implementation spec.
- **Adding new work:** Create plan in main folder, add entry to "At a Glance", then move to `/archived/` when done.
