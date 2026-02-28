# claude-view — Progress Dashboard

> Session-start file. What's active, what's done, what's next.
>
> **See also:** [`docs/VISION.md`](../VISION.md) (product vision) | [`docs/ROADMAP.md`](../ROADMAP.md) (module roadmap)
>
> **Last updated:** 2026-03-01 (Plugin shipped, Phase F done)

---

## Launch Alignment

> **Strategy:** Build B (agent control), Launch C (mobile). See GTM repo `plans/active/2026-02-26-launch-roadmap.md`.
>
> **Critical path:** Phase F → M2 → LAUNCH 1 → Plan Runner → M3 → LAUNCH 2

## Current Focus

**L1 Launch Checkpoints** (updated 2026-02-26):

| # | Checkpoint | What It Means | Status | Depends On |
|---|-----------|---------------|--------|------------|
| 1 | **Phase F: Agent Control** | Node.js sidecar + Agent SDK. Type messages, approve tools, resume sessions from web dashboard. | **DONE** (sidecar + DashboardChat + PermissionDialog + ChatStatusBar) | Phase A (done) |
| 2 | **M1: Mobile Monitor** | Expo phone app — see running agents, get push notifications, QR pairing. Read-only. | **DONE** (10/10 tasks: QR pairing, relay, push, dashboard, detail sheet) | Phase A-D APIs (done) |
| 3 | **M2: Mobile Control** | Phone app gains control — send messages, approve/reject tools, spawn agents from phone. | Not started | Needs #1 + #2 |
| 4 | **Landing page follow-up** | App Store badges, mobile signup CTA, Twitter handle, self-host fonts. | **Partial — custom 404 deferred** | See [`landing/2026-03-01-landing-page-followup.md`](landing/2026-03-01-landing-page-followup.md) |
| 5 | **Launch assets** | 60s demo video, Product Hunt listing, Show HN post, landing page update. | Not started | Needs #3 working |
| 6 | **LAUNCH 1** | Ship it. "I shipped a feature from my phone." | — | Needs #5 |

**Critical path:** `M2 design → M2 build → Launch assets → LAUNCH 1`
**Phase F + M1 both done.** Next: M2 (mobile control) extends both.

**Plans:**
- Phase F design: [`mission-control/phase-f-interactive.md`](mission-control/phase-f-interactive.md)
- Phase F impl: [`mission-control/phase-f-impl.md`](mission-control/phase-f-impl.md) (17 tasks, verified)
- M1 design: [`mobile/2026-02-25-clawmini-mobile-m1-design.md`](mobile/2026-02-25-clawmini-mobile-m1-design.md)
- M1 impl: [`mobile/2026-02-25-clawmini-mobile-m1-impl.md`](mobile/2026-02-25-clawmini-mobile-m1-impl.md) (10 tasks, audited for monorepo)
- M2 plan: TBD (extends M1 + Phase F)

**Optional / lower priority:**

- Phase E (Mission Control custom layout — drag-and-drop panes) — polish, do when convenient. Plan: [`mission-control/phase-e-custom-layout.md`](mission-control/phase-e-custom-layout.md)

## Recently Completed

- **Claude View Plugin** (2026-03-01): `@claude-view/plugin` Claude Code plugin — auto-starts Rust server via SessionStart hook, bundles 8 MCP tools (session/cost/fluency), adds 3 skills (`/session-recap`, `/daily-cost`, `/standup`). `packages/mcp/` demoted to private workspace. 7 commits, 14 files, shippable audit passed. Plan: [`cross-cutting/2026-03-01-claude-view-plugin-impl.md`](cross-cutting/2026-03-01-claude-view-plugin-impl.md)
- **M1: Mobile Monitor** (2026-03-01): Expo app with QR pairing, NaCl-encrypted relay connection, OneSignal push, session dashboard with grouping, session detail sheet. 10/10 tasks verified. Plan: [`mobile/2026-02-25-clawmini-mobile-m1-impl.md`](mobile/2026-02-25-clawmini-mobile-m1-impl.md)
- **Phase F: Agent Control** (2026-03-01): Node.js sidecar + Agent SDK IPC. DashboardChat with streaming, ResumePreFlight cost estimation, PermissionDialog with countdown auto-deny, ChatStatusBar. 20+ commits, fully wired into AppState. Plan: [`mission-control/phase-f-impl.md`](mission-control/phase-f-impl.md)
- **OneSignal Push Migration** (2026-03-01): Replace Expo Push with OneSignal for push notifications. Relay calls OneSignal REST API (no more token storage), mobile uses OneSignal SDK with `external_user_id` targeting. 12 tasks, 9 files, shippable audit passed (SHIP IT). Plan: [`mobile/2026-03-01-onesignal-push-impl.md`](mobile/2026-03-01-onesignal-push-impl.md)
- **Landing Page & Docs Site** (2026-03-01): Astro 5 + Starlight site replacing placeholder. 19 pages: marketing homepage, pricing, 13 Starlight docs, blog, changelog. Agent SEO (llms.txt, Schema.org, HowTo, BreadcrumbList), zero-JS animations, Tailwind 4. 12 tasks, 10 commits, ~45 files, shippable audit passed (SHIP IT). Plan: [`landing/archived/2026-02-28-landing-page-impl.md`](landing/archived/2026-02-28-landing-page-impl.md)
- **Clean User Message + IDE File Chip** (2026-03-01): Strip XML noise tags (`<system-reminder>`, `<ide_opened_file>`, etc.) from `lastUserMessage` before 200-char truncation. Extract IDE file context as `lastUserFile` field. File chip on web + mobile SessionCards. 7 tasks, 6 commits, 13 files, shippable audit passed. Plan: [`server/archived/2026-02-28-clean-user-message-impl.md`](server/archived/2026-02-28-clean-user-message-impl.md)
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
| **Mission Control** | **DONE** (A-F done, E theming ongoing) | Personal |
| **Phase F: Agent Control** | **DONE** | Both |
| **Mobile Remote M1** | **DONE** (10/10 tasks verified) | Both |
| **Mobile Remote M2** | **Not started — L1 launch trigger** | Both |
| **Monorepo Restructure** | **DONE** | Both |
| **Plan Runner (Phase K)** | Not started — L2 | Both |
| Star / Label Sessions | Deferred (L0 nice-to-have) | Both |
| Session Backup | Done (standalone); integration deferred | Both |
| Phase 5: Enterprise Team Layer | Not started | **Enterprise** |

---

## Active Plan Index

Plans are organized by area. Each area has its own `PROGRESS.md` with active/completed/backlog files.

| Area | Dashboard | Active | Description |
|------|-----------|--------|-------------|
| Web | [`web/PROGRESS.md`](web/PROGRESS.md) | 7 active | React SPA — chat input, conversation sharing, context bar, renderers |
| Mobile | [`mobile/PROGRESS.md`](mobile/PROGRESS.md) | M1 done, M2 TBD | Expo native app — monitor, push, control |
| Landing | [`landing/PROGRESS.md`](landing/PROGRESS.md) | 1 active (L1 blocker) | Astro site — follow-up: badges, CTA, fonts |
| Server | [`server/PROGRESS.md`](server/PROGRESS.md) | 3 active | Rust backend — plugin/MCP, session backup |
| Relay | [`relay/PROGRESS.md`](relay/PROGRESS.md) | 0 active | Cloud relay (changes tracked in mobile/) |
| Mission Control | [`mission-control/PROGRESS.md`](mission-control/PROGRESS.md) | Phase F done, E theming | Agent control shipped; E polish ongoing |
| Cross-cutting | [`cross-cutting/PROGRESS.md`](cross-cutting/PROGRESS.md) | Plugin done | Monorepo, infra, types, plugin |
| Backlog | [`backlog/`](backlog/) | 25 deferred | Epics, marketplace, future work |
| Archived | [`archived/`](archived/) | — | Pre-monorepo era completed plans |

**L1 execution order:** ~~Phase F~~ (done) + ~~M1~~ (done) → M2 design → M2 build → LAUNCH 1

See GTM repo `plans/active/2026-02-26-launch-roadmap.md` for full strategy.
See `mobile/PROGRESS.md` for M1 phase details.

---

## L1 Prerequisites (Blockers — Human Setup Required)

All code is shipped. These are cloud console + CLI steps requiring your accounts.

| # | Prerequisite | Blocker For | Checklist |
|---|-------------|-------------|-----------|
| 1 | **Supabase project** — create project, enable auth, configure OAuth | Auth, sharing, relay JWT | [Deployment checklist D1-D5](cross-cutting/2026-02-28-deployment-checklist.md) |
| 2 | **Cloudflare share worker** — R2 bucket, D1 database, deploy worker | Conversation sharing | [Deployment checklist D6-D14](cross-cutting/2026-02-28-deployment-checklist.md) |
| 3 | **Fly.io relay secrets** — Supabase URL, Sentry, PostHog | Relay JWT auth | [Deployment checklist D15-D16](cross-cutting/2026-02-28-deployment-checklist.md) |
| 4 | **OneSignal account** — create app, upload APNs .p8 key | Push notifications | [E2E checklist Phase 0](mobile/2026-02-28-m1-e2e-checklist.md) |
| 5 | **OneSignal env vars** — `flyctl secrets set ONESIGNAL_APP_ID + REST_API_KEY` | Push notifications | [E2E checklist Phase 0](mobile/2026-02-28-m1-e2e-checklist.md) |
| 6 | **EAS project init** — `cd apps/mobile && eas init` | Mobile builds | [E2E checklist Phase 0](mobile/2026-02-28-m1-e2e-checklist.md) |
| 7 | **Apple Developer account** — needed for TestFlight + push entitlement | App Store submission | [E2E checklist Phase 0](mobile/2026-02-28-m1-e2e-checklist.md) |
| 8 | **Mobile app icons** — replace 1x1 placeholders in `apps/mobile/assets/` | App Store submission | Currently gitignored |
| 9 | **Privacy policy URL** | App Store submission | Required by Apple |

**Do in order:** #1 → #2-3 (parallel) → #4-7 (parallel) → #8-9 (before submission)

**Checklists:**
- Infra setup: [`cross-cutting/2026-02-28-deployment-checklist.md`](cross-cutting/2026-02-28-deployment-checklist.md)
- Mobile E2E: [`mobile/2026-02-28-m1-e2e-checklist.md`](mobile/2026-02-28-m1-e2e-checklist.md)

---

## Deferred / Non-Blocking

| Item | When Needed | Notes |
|------|-------------|-------|
| `apple-app-site-association` | Before mobile app Universal Links | Needs real Apple Team ID; removed placeholder with `TEAMID` |
| `package-lock.json` | Only if npm workspace support needed | Removed — npm can't resolve `workspace:*` protocol; Bun is the dev tool |

---

## Code Health

- **Compiles:** Yes (cargo check + `bun run build` pass)
- **Backend tests:** 1177 (core 652 + server 525, 2 pre-existing failures unrelated)
- **Frontend tests:** 1117 (vitest, 74 files, 0 failures)
- **MCP tests:** 24 pass (6 files, 69 assertions)
- **Plugin validation:** 13/13 checks pass (files, JSON, executable)
- **TypeScript:** 0 errors (`tsc --noEmit`)
- **Clippy:** 1 cosmetic warning (SidecarManager Default derive)
- **Last verified:** 2026-03-01 on branch `worktree-monorepo-expo`

---

## How to Use This File

- **Starting a session:** Read this file first. Check "Current Focus" and "At a Glance".
- **Product context:** Read `docs/VISION.md` for product evolution and business model.
- **What's next:** Read `docs/ROADMAP.md` for module roadmap and priorities.
- **Area-specific plans:** Check the area's `PROGRESS.md` (e.g., `web/PROGRESS.md`, `server/PROGRESS.md`).
- **Completed designs:** Check `{area}/archived/` for recently completed plans, or `archived/` for pre-monorepo era.
- **Deferred ideas:** Check `backlog/` for draft/deferred plans.
- **Adding new work:** Create plan in the appropriate area directory (e.g., `web/`, `server/`), add to that area's `PROGRESS.md`.
