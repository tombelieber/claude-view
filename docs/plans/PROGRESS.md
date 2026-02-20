# claude-view — Progress Dashboard

> Session-start file. What's active, what's done, what's next.
>
> **See also:** [`docs/VISION.md`](../VISION.md) (product vision) | [`docs/ROADMAP.md`](../ROADMAP.md) (module roadmap)
>
> **Last updated:** 2026-02-21

---

## Current Focus

- Mission Control Phase E+ (custom layout, interactive features)
- Full-Text Search (Phase 6 — Tantivy)
- Action Log Tab

## Recently Completed

- Mission Control Phases A-D (monitoring, views, monitor mode, sub-agent viz, drilldown)
- AI Fluency Score (merged)
- Process-Gated Discovery, Page Reorganization
- Session Sort Redesign, Command Palette Redesign, Notification Sound Design
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
| **Phase 6: Search (Tantivy)** | **Approved** | Both |
| **Mission Control** | **In Progress** | Personal |
| **Action Log Tab** | **Pending** | Personal |
| Phase 5: Enterprise Team Layer | Not started | **Enterprise** |

---

## Active Plan Index

Plans in `docs/plans/` (active work only):

| File | Status | Description |
|------|--------|-------------|
| `2026-02-18-full-text-search-design.md` | approved | Phase 6: Tantivy search, Cmd+K, scoped search |
| `2026-02-19-action-log-tab-design.md` | approved | Filterable action timeline in SessionDetailPanel |
| `2026-02-19-action-log-tab-impl.md` | pending | 9 tasks for action log implementation |
| `2026-02-19-notification-sound-design.md` | approved | Audio notifications for session events |
| `2026-02-19-notification-sound-impl.md` | pending | Implementation plan for notification sounds |
| `2026-02-19-pricing-engine-overhaul.md` | pending | Pricing engine redesign |
| `2026-02-19-sessions-infinite-scroll.md` | pending | Infinite scroll for session lists |
| `2026-02-19-restore-sparkline-stats-grid.md` | pending | Restore sparkline stats grid |
| `2026-02-19-oauth-usage-pill-design.md` | pending | OAuth usage pill feature |
| `2026-02-20-*` (8 files) | various | Recent active designs (pricing, liveness, history, renderers, etc.) |
| `mission-control/` | in-progress | Phases A-D done, E-J pending (see `mission-control/PROGRESS.md`) |

**Other locations:**
- `docs/plans/backlog/` — 25 deferred/draft plans (epics, marketplace, mobile PWA, etc.)
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
