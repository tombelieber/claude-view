---
status: approved
date: 2026-01-28
supersedes:
  - 2026-01-27-api-schema-bonus-fields-design.md
  - 2026-01-27-ux-polish-a11y-sidenav-urls.md
---

# Phase 2C: API Optimization + UX Polish

> Split `/api/projects` into lightweight summaries + paginated per-project sessions. Add dashboard stats endpoint. Fix accessibility, redesign sidebar, add human-readable URLs.

## Context

The current `/api/projects` returns ALL sessions for ALL projects in a single response (~676 KB for 542 sessions). The sidebar only needs project names and counts (~2 KB). The dashboard aggregates stats client-side from all sessions. This plan fixes the over-fetching and adds frontend polish.

**What's already done (not in this plan):**
- Migration 4 added `summary`, `git_branch`, `is_sidechain`, `deep_indexed_at` columns
- `SessionInfo` Rust struct + TypeScript types already include all these fields + Phase 2B token fields
- Copy-to-clipboard already exists in `Message.tsx`
- `ConversationView.tsx` already uses `react-virtuoso`

**Clean cutover:** `/api/projects` changes from `ProjectInfo[]` (with sessions) to `ProjectSummary[]` (counts only). Frontend updates in the same plan. No backward-compat shim — localhost dev tool.

---

## Part A: Backend — API Split + Dashboard (10 steps)

### New Types

```rust
/// Lightweight project for sidebar — NO sessions array.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub session_count: usize,
    pub active_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<i64>,
}

/// Paginated sessions response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionsPage {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Pre-computed dashboard stats.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_sessions: usize,
    pub total_projects: usize,
    pub heatmap: Vec<DayActivity>,
    pub top_skills: Vec<(String, usize)>,
    pub top_projects: Vec<ProjectStat>,
    pub tool_totals: ToolCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DayActivity {
    pub date: String,  // "2026-01-28"
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStat {
    pub name: String,
    pub display_name: String,
    pub session_count: usize,
}
```

### API Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /api/projects` (modified) | Sidebar + StatusBar | `ProjectSummary[]` (~2 KB) |
| `GET /api/projects/:id/sessions` (new) | ProjectView | `SessionsPage` (~50 KB/page) |
| `GET /api/stats/dashboard` (new) | StatsDashboard | `DashboardStats` (~5 KB) |

Query params for `/api/projects/:id/sessions`:

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | int | `50` | Page size |
| `offset` | int | `0` | Pagination offset |
| `sort` | string | `recent` | `recent`, `oldest`, `messages` |
| `branch` | string | — | Filter by git branch |
| `include_sidechains` | bool | `false` | Include sub-agent sessions |

### Steps

| # | Step | Files | Test |
|---|------|-------|------|
| 1 | Add `ProjectSummary`, `SessionsPage`, `DashboardStats`, `DayActivity`, `ProjectStat` types | `crates/core/src/types.rs` | Compiles, ts-rs exports |
| 2 | Migration 7: Add indexes `(project_id, git_branch)` and `(is_sidechain)` | `crates/db/src/migrations.rs` | Migration runs |
| 3 | Add `list_project_summaries()` query — GROUP BY project_id, COUNT, MAX(last_message_at), exclude sidechains by default | `crates/db/src/queries.rs` | Unit test: correct counts, no session arrays |
| 4 | Add `list_sessions_for_project(project_id, limit, offset, sort, branch, include_sidechains)` — paginated with capped arrays (5 files, 3 skills per session) | `crates/db/src/queries.rs` | Unit test: pagination, filters, sort, array caps |
| 5 | Add `get_dashboard_stats()` — heatmap (90 days), top 10 skills, top 5 projects, tool totals | `crates/db/src/queries.rs` | Unit test: aggregations correct |
| 6 | Update `GET /api/projects` → return `ProjectSummary[]` instead of `ProjectInfo[]` | `crates/server/src/routes/projects.rs` | Update existing tests, verify no `sessions` key, response <5 KB |
| 7 | Add `GET /api/projects/:id/sessions` with query param parsing | `crates/server/src/routes/projects.rs` | Test: pagination, branch filter, sidechain exclusion |
| 8 | Add `GET /api/stats/dashboard` endpoint | `crates/server/src/routes/stats.rs` (new) | Test: heatmap shape, top skills populated |
| 9 | Re-export new types from `db` and `core` crates | `crates/db/src/lib.rs`, `crates/core/src/lib.rs` | Compiles |
| 10 | Full workspace test — verify no regressions | — | `cargo test --workspace` green |

---

## Part B: Frontend — API Consumption + UX Polish (14 steps)

### Steps

| # | Step | Files | Verify |
|---|------|-------|--------|
| 11 | **Reduced motion + skip link** — Add `prefers-reduced-motion` media query to `index.css`. Add skip-to-content `<a>` in `App.tsx`, `id="main"` on `<main>` | `src/index.css`, `src/App.tsx` | `bun run typecheck` |
| 12 | **Header a11y** — `aria-label` on HelpCircle and Settings icon buttons. `focus-visible:ring-2 focus-visible:ring-blue-400` on all interactive elements | `src/components/Header.tsx` | typecheck |
| 13 | **SessionCard fix nested interactive** — Change root element from `<button>` to `<article>`. The parent `<Link>` in `DateGroupedList` provides the interactive wrapper | `src/components/SessionCard.tsx` | typecheck |
| 14 | **DateGroupedList + SearchResults a11y** — Add `focus-visible:ring-2` on `<Link>` wrappers. Remove noop `onClick={() => {}}` from SessionCard usage | `src/components/DateGroupedList.tsx`, `src/components/SearchResults.tsx` | typecheck |
| 15 | **Sidebar a11y** — `focus-visible:ring-2` on project links. `aria-current="page"` on active project | `src/components/Sidebar.tsx` | typecheck |
| 16 | **StatsDashboard a11y** — `aria-label` on heatmap day cells (e.g. "Jan 28: 5 sessions"). `focus-visible` on all buttons and links | `src/components/StatsDashboard.tsx` | typecheck |
| 17 | **ConversationView a11y** — `aria-label` on export and back buttons. `focus-visible` rings | `src/components/ConversationView.tsx` | typecheck |
| 18 | **Wire new API hooks** — `useProjectSummaries()` replaces `useProjects()`. Add `useProjectSessions(id, opts)` with pagination. Add `useDashboardStats()` | `src/hooks/use-projects.ts` (modify), `src/hooks/use-dashboard.ts` (new) | typecheck |
| 19 | **Update Sidebar + StatusBar + App** — Consume `ProjectSummary[]` (no sessions array). StatusBar uses `sessionCount` field, not `.sessions.length`. App passes summaries to Sidebar, CommandPalette | `src/App.tsx`, `src/components/Sidebar.tsx`, `src/components/StatusBar.tsx` | typecheck, sidebar renders with counts |
| 20 | **Update ProjectView** — Fetch sessions via `useProjectSessions(projectId)`. Pagination with "Load more" button. Sort/sidechain/branch filters as URL query params (`?sort=recent&branch=main&sidechains=true`) | `src/components/ProjectView.tsx` | typecheck, sessions load on project click, filters survive refresh |
| 21 | **Update HistoryView** — Consume new session hooks instead of `projects.flatMap(p => p.sessions)` | `src/components/HistoryView.tsx` | typecheck |
| 22 | **Update StatsDashboard** — Consume `useDashboardStats()` instead of client-side aggregation from all sessions | `src/components/StatsDashboard.tsx` | typecheck, dashboard renders from API |
| 23 | **VSCode-style Sidebar redesign** — Compact rows with chevron toggles, folder icons. Arrow-key navigation. ARIA `role="tree"` / `role="treeitem"` | `src/components/Sidebar.tsx` | typecheck, keyboard nav works |
| 24 | **Human-readable session URLs** — Slug utility generates `/project/:projectId/session/:slug` from session preview. Add route, update breadcrumbs, redirect legacy `/session/:projectId/:sessionId` | `src/lib/url-slugs.ts` (new), `src/router.tsx`, `src/components/Header.tsx`, `src/components/DateGroupedList.tsx`, `src/components/SearchResults.tsx` | typecheck, URLs readable, legacy URLs redirect |

---

## Acceptance Criteria

### Backend (Part A)

**AC-1:** `GET /api/projects` response < 5 KB for 500 sessions across 10 projects. No `sessions` key in response objects. Each object has `sessionCount`, `activeCount`, `lastActivityAt`.

**AC-2:** `GET /api/projects/:id/sessions?limit=50&offset=0` returns exactly 50 sessions with `total` field showing full count. `?offset=50` returns the remainder.

**AC-3:** `?include_sidechains=false` (default) excludes `is_sidechain=true` sessions from both project counts and session lists.

**AC-4:** `?branch=main` returns only sessions with `git_branch='main'`.

**AC-5:** `?sort=recent|oldest|messages` orders correctly.

**AC-6:** Session `filesTouched` capped at 5 items, `skillsUsed` at 3 items in list response.

**AC-7:** `GET /api/stats/dashboard` returns heatmap with 90 days of `{date, count}` entries, top 10 skills with counts, top 5 projects with session counts, and aggregate tool totals.

### Frontend (Part B)

**AC-8:** Sidebar loads from `ProjectSummary[]`. No sessions array fetched on app load.

**AC-9:** Clicking a project fetches sessions via `/api/projects/:id/sessions`. "Load more" button for pagination.

**AC-10:** Sort, branch, and sidechain filter state persisted in URL query params. Refresh preserves state.

**AC-11:** StatsDashboard renders from `/api/stats/dashboard`. No client-side aggregation of all sessions.

**AC-12:** All icon-only buttons have `aria-label`. All interactive elements have `focus-visible:ring-2`. No nested interactive elements.

**AC-13:** Skip-to-content link appears on Tab focus. `prefers-reduced-motion` disables animations.

**AC-14:** Session URLs are human-readable: `/project/:projectId/session/fix-the-login-bug-974d98a2`. Legacy `/session/:projectId/:sessionId` still resolves.

**AC-15:** `bun run typecheck` passes. `cargo test --workspace` green.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Breaking frontend during cutover | Steps 18-19 wire new hooks before removing old data flow. TypeScript catches mismatches. |
| Capped arrays lose data | Card already truncates visually. Full arrays available in conversation view. |
| Dashboard stats stale during indexing | Dashboard shows "Indexing..." overlay when indexing state is active (from SSE). |
| VSCode sidebar complexity | Step 23 is self-contained. If it takes too long, the basic sidebar from Step 19 already works. |
| Human-readable URL collisions | Slug includes 8-char session ID suffix: `fix-login-bug-974d98a2`. Collision-free. |

---

## Out of Scope

- Full-text search across summaries (needs Tantivy — Phase 4)
- Real-time updates via WebSocket
- Font change to Geist (evaluate separately)
- Summary word cloud / topic clustering
- Virtualizing DateGroupedList (grouped virtualization is complex — separate plan)
