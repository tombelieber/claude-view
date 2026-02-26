# claude-view — Progress Dashboard

> Session-start file. What's active, what's done, what's next.
>
> **See also:** [`docs/VISION.md`](../VISION.md) (product vision) | [`docs/ROADMAP.md`](../ROADMAP.md) (module roadmap)
>
> **Last updated:** 2026-02-27

---

## Launch Alignment

> **Strategy:** Build B (agent control), Launch C (mobile). See GTM repo `plans/active/2026-02-26-launch-roadmap.md`.
>
> **Critical path:** Phase F → M2 → LAUNCH 1 → Plan Runner → M3 → LAUNCH 2

## Current Focus

**L1 Launch Checkpoints** (updated 2026-02-26):

| # | Checkpoint | What It Means | Status | Depends On |
|---|-----------|---------------|--------|------------|
| 1 | **Phase F: Agent Control** | Node.js sidecar + Agent SDK. Type messages, approve tools, resume sessions from web dashboard. | **Impl plan ready** (17 tasks) | Phase A (done) |
| 2 | **M1: Mobile Monitor** | Expo phone app — see running agents, get push notifications, QR pairing. Read-only. | **Impl plan ready** (10 tasks, audited) | Phase A-D APIs (done) |
| 3 | **M2: Mobile Control** | Phone app gains control — send messages, approve/reject tools, spawn agents from phone. | Not started | Needs #1 + #2 |
| 4 | **Launch assets** | 60s demo video, Product Hunt listing, Show HN post, landing page update. | Not started | Needs #3 working |
| 5 | **LAUNCH 1** | Ship it. "I shipped a feature from my phone." | — | Needs #4 |

**Critical path:** `Phase F (#1) → M2 (#3) → Launch`
**Parallel work:** #1 and #2 are independent — build simultaneously.

**Plans:**
- Phase F design: [`mission-control/phase-f-interactive.md`](mission-control/phase-f-interactive.md)
- Phase F impl: [`mission-control/phase-f-impl.md`](mission-control/phase-f-impl.md) (17 tasks, verified)
- M1 design: [`mobile-remote/2026-02-25-clawmini-mobile-m1-design.md`](mobile-remote/2026-02-25-clawmini-mobile-m1-design.md)
- M1 impl: [`mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md`](mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md) (10 tasks, audited for monorepo)
- M2 plan: TBD (extends M1 + Phase F)

**Optional / lower priority:**

- Phase E (Mission Control custom layout — drag-and-drop panes) — polish, do when convenient. Plan: [`mission-control/phase-e-custom-layout.md`](mission-control/phase-e-custom-layout.md)

## Recently Completed

- **Branch `worktree-monorepo-expo` — SHIPPABLE** (verified 2026-02-27): 0 TS errors, 1117 web tests pass, 575 Rust tests pass, clean build. One production bug fix (`cleanPreviewText` backslash), infra upgrades (Biome, Lefthook, cargo-deny, CI), monorepo restructure. No breaking changes, no regressions.
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
| **Mission Control** | **In Progress** (A-D done, E polish, **F = critical path**) | Personal |
| **Phase F: Agent Control** | **Not started — L1 CRITICAL PATH** | Both |
| **Mobile Remote M1** | **Impl plan ready** (10 tasks, audited) — L1 parallel build | Both |
| **Mobile Remote M2** | **Not started — L1 launch trigger** | Both |
| **Monorepo Restructure** | **DONE** | Both |
| **Plan Runner (Phase K)** | Not started — L2 | Both |
| Star / Label Sessions | Deferred (L0 nice-to-have) | Both |
| Session Backup | Done (standalone); integration deferred | Both |
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
| `mission-control/` | in-progress | A-D done, **F = L1 critical path** (see `mission-control/PROGRESS.md`) |
| `mission-control/phase-f-interactive.md` | **has design** | Agent SDK sidecar, spawn/resume/control from dashboard |
| `mission-control/phase-f-impl.md` | **impl plan ready** (17 tasks) | Task breakdown for Phase F implementation |
| `mobile-remote/2026-02-25-clawmini-mobile-m1-design.md` | **has design — L1 parallel** | Expo native app: live dashboard, keypair auth, dumb relay |
| `mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md` | **impl plan ready** (10 tasks, audited) | Shared pkg, relay fixes, pair screen, dashboard, push, TestFlight |
| `mobile-remote/m2-mobile-control-design.md` | **TO WRITE — L1 blocker** | Mobile control: approve/reject, send messages, spawn from phone |
| `mission-control/phase-e-custom-layout.md` | not started (parallel, lower pri) | react-mosaic custom layout — polish, not blocking L1 |
| `2026-02-25-monorepo-restructure-design.md` | done | Turborepo monorepo: `apps/web`, `apps/mobile`, `packages/shared` |
| `2026-02-25-monorepo-restructure-impl.md` | done | 12 tasks: git mv web SPA, workspaces, Expo scaffold, landing page |
| `2026-02-24-star-label-sessions-design.md` | deferred (L0 nice-to-have) | Named bookmarks on sessions |
| `2026-02-24-session-backup-design.md` | done (standalone); integration deferred | Standalone tool at `claude-backup` repo |

**L1 execution order:** Phase F + M1 (parallel) → M2 → LAUNCH 1

See GTM repo `plans/active/2026-02-26-launch-roadmap.md` for full strategy.
See `mobile-remote/PROGRESS.md` for M1 phase details.

**Other locations:**
- `docs/plans/backlog/` — 25 deferred/draft plans (epics, marketplace, mobile app (Expo), etc.)
- `docs/plans/archived/` — All completed phase plans and theme work

---

## Deferred / Pre-Launch Checklist

Items removed during monorepo cleanup that need attention before specific milestones:

| Item | When Needed | Notes |
|------|-------------|-------|
| `apple-app-site-association` | Before mobile app Universal Links | Needs real Apple Team ID; removed placeholder with `TEAMID` |
| Mobile app icons (`apps/mobile/assets/images/`) | Before mobile app store submission | Currently gitignored (1x1 placeholders); replace with real icons |
| `package-lock.json` | Only if npm workspace support needed | Removed — npm can't resolve `workspace:*` protocol; Bun is the dev tool |

---

## Code Health

- **Compiles:** Yes (cargo check + `bun run build` pass)
- **Backend tests:** 575 (cargo test --workspace, 0 failures)
- **Frontend tests:** 1117 (vitest, 74 files, 0 failures)
- **TypeScript:** 0 errors (`tsc --noEmit`)
- **Clippy:** 1 cosmetic warning (SidecarManager Default derive)
- **Last verified:** 2026-02-27 on branch `worktree-monorepo-expo`

---

## How to Use This File

- **Starting a session:** Read this file first. Check "Current Focus" and "At a Glance".
- **Product context:** Read `docs/VISION.md` for product evolution and business model.
- **What's next:** Read `docs/ROADMAP.md` for module roadmap and priorities.
- **Specific phase design:** Check `archived/` for the completed implementation spec.
- **Deferred ideas:** Check `backlog/` for draft/deferred plans.
- **Adding new work:** Create plan in `docs/plans/`, add to "Active Plan Index", move to `archived/` when done.
