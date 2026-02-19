---
status: done
date: 2026-02-19
---

# Page Reorganization: Mission Control as Home

Reposition the tool as a "Claude Code cockpit" by making Mission Control the landing page and consolidating analytics into a single tabbed page.

## Motivation

The current nav prioritizes analytics (Fluency score, Contributions, Insights) — features that are useful but not the primary reason someone opens the tool. The real daily driver is Mission Control: seeing what's running, what needs attention, and how much you're spending. Making it the home page aligns the UI with the actual usage pattern.

## Current State

**5 sidebar nav items:**

| # | Label | Route | Component |
|---|-------|-------|-----------|
| 1 | Fluency | `/` | `StatsDashboard` |
| 2 | Sessions | `/sessions` | `HistoryView` |
| 3 | Contributions | `/contributions` | `ContributionsPage` |
| 4 | Insights | `/insights` | `InsightsPage` |
| 5 | Mission Control | `/mission-control` | `MissionControlPage` |

## New State

**3 sidebar nav items:**

| # | Label | Icon | Route | Component |
|---|-------|------|-------|-----------|
| 1 | **Mission Control** | `Monitor` | `/` | `MissionControlPage` |
| 2 | **Sessions** | `Clock` | `/sessions` | `HistoryView` |
| 3 | **Analytics** | `BarChart3` | `/analytics` | `AnalyticsPage` (new) |

### Analytics Sub-tabs

The new `AnalyticsPage` is a thin wrapper with a tab bar. Each tab renders its existing component unchanged.

| Tab | Default? | URL | Component |
|-----|----------|-----|-----------|
| Overview | Yes | `/analytics` | `StatsDashboard` |
| Contributions | No | `/analytics?tab=contributions` | `ContributionsPage` |
| Insights | No | `/analytics?tab=insights` | `InsightsPage` |

## Redirects (preserve bookmarks)

| Old URL | New URL |
|---------|---------|
| `/mission-control` | `/` |
| `/contributions` | `/analytics?tab=contributions` |
| `/insights` | `/analytics?tab=insights` |

The old `/` (StatsDashboard) has no redirect needed — users landing on `/` will simply see Mission Control instead.

## What Changes

### Router (`src/router.tsx`)

- `index: true` changes from `StatsDashboard` to `MissionControlPage`
- New route: `{ path: 'analytics', element: <AnalyticsPage /> }`
- Add redirects for `/mission-control`, `/contributions`, `/insights`
- Remove standalone `/contributions` and `/insights` routes

### Sidebar (`src/components/Sidebar.tsx`)

- Reduce nav links from 5 to 3
- Reorder: Mission Control (home), Sessions, Analytics
- Update active-state matching for new routes

### Header (`src/components/Header.tsx`)

- Update breadcrumb logic for `/analytics` route (with tab context)
- Remove breadcrumb cases for `/contributions` and `/insights`

### New: `AnalyticsPage` (`src/pages/AnalyticsPage.tsx`)

Thin wrapper component:
- Reads `?tab=` search param (default: `overview`)
- Renders a horizontal tab bar with 3 tabs
- Renders the corresponding existing component below
- Tab bar uses same styling conventions as the rest of the app (explicit Tailwind classes, no shadcn tokens)

### App (`src/App.tsx`)

- No changes needed. `MissionControlPage` already receives `liveSessions` via outlet context.

## What Does NOT Change

- All existing page components (`StatsDashboard`, `ContributionsPage`, `InsightsPage`, `MissionControlPage`) — zero modifications to their internals
- Sidebar project/branch scope filter — works identically
- Command palette, keyboard shortcuts, settings
- Mission Control's internal view switcher (grid/kanban/list/monitor)
- Status bar, auth banner, cold start overlay

## Risks

| Risk | Mitigation |
|------|------------|
| Users with bookmarks to old URLs | Redirects handle all old routes |
| Mission Control shows empty state on first load (no active sessions) | Existing empty state already handles this well ("Start a session in your terminal") |
| Analytics tab state lost on navigation | `?tab=` param persists in URL; browser back/forward works |
