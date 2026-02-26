# claude-view — Progress Dashboard

> Session-start file. What's active, what's done, what's next.
>
> **See also:** [`docs/VISION.md`](../VISION.md) (product vision) | [`docs/ROADMAP.md`](../ROADMAP.md) (module roadmap)
>
> **Last updated:** 2026-02-26

---

## Current Focus

- Mission Control Phase E+ (custom layout, interactive features)
- Session Backup integration into claude-view (standalone tool done at `claude-backup`)
- Star / Label Sessions

## Recently Completed

- Monorepo Restructure (Turborepo + Bun workspaces: `apps/web`, `apps/mobile`, `apps/landing`, `packages/shared`, `packages/design-tokens`)
- Reliability Release (centralized path config, JSONL-based session classification, cwd-based path resolve, sandbox docs)
- Pricing Engine Overhaul (unified ModelPricing, litellm auto-fetch, 200k tiering, 3-tier fallback, 26 tests)
- Full-Text Search Phase 6 (Tantivy backend, `GET /api/search`, Cmd+K, SearchBar, SearchResultCard — in-session Ctrl+F deferred)
- Action Log Tab (filterable action timeline, react-virtuoso, 13 category filters, wired into SessionDetailPanel)
- Notification Sound (use-notification-sound hook, NotificationSoundPopover)
- Infinite Scroll for session lists (use-sessions-infinite hook)
- OAuth Usage Pill (OAuthUsagePill component with tests)
- Sparkline Stats Grid (ActivitySparkline component)
- Custom Skill Registry (user-level custom skill discovery and registry auto-reindex)
- JSONL Ground Truth Recovery (startup state derivation from JSONL, staleness hack removed)
- Mission Control Phases A-D (monitoring, views, monitor mode, sub-agent viz, drilldown)
- AI Fluency Score (merged)
- Process-Gated Discovery, Page Reorganization
- Session Sort Redesign, Command Palette Redesign
- Hook Events + Terminal View Modes
- Rename: vibe-recall → claude-view

---

## At a Glance

| Phase | Status | Tier |
|-------|--------|------|
| Phase 1: Foundation | **DONE** | Personal |
| Phase 2A-1: Parallel Indexing | **DONE** | Personal |
| Phase 2A-2: Invocable Registry | **DONE** | Personal |
| Phase 2B: Token & Model Tracking | **DONE** | Personal |
| Phase 2C: API Split + UX Polish | **DONE** | Personal |
| Phase 3: Metrics Engine | **DONE** | Personal |
| Phase 3.5: Full JSONL Parser | **DONE** | Personal |
| Phase 4: Distribution (npx) | **DONE** | Personal |
| Phase 4B: Session Parser + UI Wiring | **DONE** | Personal |
| Hardening: Security + Robustness | **DONE** | Personal |
| Thread Visualization & Dark Mode | **DONE** | Personal |
| Deep Index Perf (all tracks) | **DONE** | Personal |
| Session Loading Perf | **DONE** | Personal |
| Export Markdown | **DONE** | Personal |
| Security Audit | **DONE** | Personal |
| Session Discovery & Navigation | **DONE** | Personal |
| Theme 2: Dashboard Analytics | **DONE** | Personal |
| Theme 3: Git AI Contribution | **DONE** | Personal |
| Theme 4: Chat Insights | **DONE** | Personal |
| Ambient Coach (Insights v2) | **DONE** | Personal |
| Cold Start UX | **DONE** | Personal |
| GTM Launch | **In Progress** | Personal |
| Phase 6: Search (Tantivy) | **DONE** | Both |
| Action Log Tab | **DONE** | Personal |
| Notification Sound | **DONE** | Personal |
| Infinite Scroll | **DONE** | Personal |
| OAuth Usage Pill | **DONE** | Personal |
| Sparkline Stats Grid | **DONE** | Personal |
| Reliability Release | **DONE** (PR #14) | Both |
| Pricing Engine Overhaul | **DONE** | Both |
| **Mission Control** | **In Progress** (A-D done, E-J pending) | Personal |
| **Star / Label Sessions** | **Pending** | Both |
| **Session Backup** | **Done (standalone)**; integration pending | Both |
| **Monorepo Restructure** | **DONE** | Both |
| **Mobile Remote M1** | **Deferred (next cycle)** | Both |
| Phase 5: Enterprise Team Layer | Not started | **Enterprise** |

---

## Active Plan Index

Plans in `docs/plans/` (active work only):

| File | Status | Description |
|------|--------|-------------|
| `2026-02-18-full-text-search-design.md` | done | Phase 6: Tantivy search, Cmd+K, scoped search (in-session Ctrl+F deferred) |
| `2026-02-19-action-log-tab-design.md` | done | Filterable action timeline in SessionDetailPanel |
| `2026-02-19-action-log-tab-impl.md` | done | 9 tasks for action log implementation |
| `2026-02-19-notification-sound-design.md` | done | Audio notifications for session events |
| `2026-02-19-notification-sound-impl.md` | done | Implementation plan for notification sounds |
| `2026-02-19-pricing-engine-overhaul.md` | done | Unified ModelPricing, litellm auto-fetch, 200k tiering, 3-tier fallback |
| `2026-02-19-sessions-infinite-scroll.md` | done | Infinite scroll for session lists |
| `2026-02-19-restore-sparkline-stats-grid.md` | done | Restore sparkline stats grid |
| `2026-02-19-oauth-usage-pill-design.md` | done | OAuth usage pill feature |
| `2026-02-21-custom-skill-registry-*.md` | done | User-level custom skill discovery and registry auto-reindex |
| `2026-02-21-jsonl-ground-truth-recovery*.md` | done | Startup state from JSONL ground truth, removed staleness hack |
| `2026-02-20-*` (8 files) | various | Recent active designs (pricing, liveness, history, renderers, etc.) |
| `2026-02-24-reliability-release-issues.md` | done (PR #14) | 4 foundation bugs: path config, hooks, session count, path resolve |
| `2026-02-24-star-label-sessions-design.md` | approved (concept) | Named bookmarks on sessions — CLI + every UI surface |
| `2026-02-24-session-backup-design.md` | done (standalone) | Standalone tool at `claude-backup` repo; integration into claude-view pending |
| `mission-control/` | in-progress | Phases A-D done, E-J pending (see `mission-control/PROGRESS.md`) |
| `2026-02-25-monorepo-restructure-design.md` | done | Turborepo monorepo: `apps/web`, `apps/mobile`, `packages/shared` |
| `2026-02-25-monorepo-restructure-impl.md` | done | 12 tasks: git mv web SPA, workspaces, Expo scaffold, landing page |
| `mobile-remote/2026-02-25-clawmini-mobile-m1-design.md` | deferred (next cycle) | Expo native app: live dashboard, keypair auth, dumb relay |
| `mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md` | deferred (next cycle) | 12 tasks: relay fixes, pair screen, dashboard, deploy pipeline |

**Execution order:** Monorepo restructure first → then M1 starting at Phase 2 (Phase 1 overlaps). See `mobile-remote/PROGRESS.md` for details.

**Other locations:**
- `docs/plans/backlog/` — 25 deferred/draft plans (epics, marketplace, mobile app (Expo), etc.)
- `docs/plans/archived/` — All completed phase plans and theme work

---

## Code Health

- **Compiles:** Yes (cargo check passes)
- **Backend tests:** 548+
- **Frontend tests:** 794
- **TypeScript:** Clean

---

## How to Use This File

- **Starting a session:** Read this file first. Check "Current Focus" and "At a Glance".
- **Product context:** Read `docs/VISION.md` for product evolution and business model.
- **What's next:** Read `docs/ROADMAP.md` for module roadmap and priorities.
- **Specific phase design:** Check `archived/` for the completed implementation spec.
- **Deferred ideas:** Check `backlog/` for draft/deferred plans.
- **Adding new work:** Create plan in `docs/plans/`, add to "Active Plan Index", move to `archived/` when done.
