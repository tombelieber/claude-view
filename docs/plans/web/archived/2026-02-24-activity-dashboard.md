# Activity Dashboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an "Activity" page to claude-view that shows users where their Claude Code time was spent — summary stats, calendar heatmap, project breakdown, and daily session timeline.

**Architecture:** New `/activity` route with a single-page React component. One small backend change: expose `first_message_at` on `SessionInfo` (already queried from DB but not mapped). Frontend fetches all sessions for the selected time range via existing `/api/sessions` API with pagination and aggregates client-side. Project filtering is applied client-side after fetch (the `/api/sessions` endpoint does not support a `project` query param). Uses Recharts (already installed) for bar charts and pure CSS grid for the heatmap.

**Tech Stack:** React, TypeScript, Recharts, Tailwind CSS, TanStack Query, react-router-dom, Rust (one struct field + mapping)

**Task sequencing constraint:** Task 1 must be committed and TypeScript types regenerated before Task 3 can compile. `activity-utils.ts` references `session.firstMessageAt` which does not exist on `SessionInfo` until Task 1 runs the ts-rs export.

**Depends on:** CWD resolution fix (`2026-02-25-cwd-resolution-fix.md`) for accurate `projectPath` data (implements Issue 4 of `2026-02-24-reliability-release-design.md`). After that plan ships: `projectPath` is always sourced from `cwd` in JSONL (no garbled `@`/`.`/hyphen names), ghost sessions are eliminated, and `/api/sessions` returns only real conversation sessions. The activity dashboard works before the CWD fix but produces wrong project groupings and inflated session counts until then.

**Design doc:** `docs/plans/2026-02-24-activity-dashboard-design.md`

**Audit amendments applied:**
- Fixed: `limit=1000` silent cap → pagination loop fetching all pages
- Fixed: `modifiedAt - durationSeconds` approximation → expose real `first_message_at` from DB
- Fixed: `formatHumanDuration` duplicated then cleaned up → define once in `format-utils.ts` upfront
- Fixed: `useMemo` on raw `query.data` object → primitive memoization key per CLAUDE.md rules
- Fixed: sidebar project/branch filter ignored → branch passed to API, project filtered client-side (API has no `project` param)

---

### Task 1: Expose `first_message_at` on SessionInfo (backend)

**Files:**
- Modify: `crates/core/src/types.rs` (add field to SessionInfo struct)
- Modify: `crates/db/src/queries/row_types.rs` (map field in `into_session_info`)

**Step 1: Add `first_message_at` field to SessionInfo struct**

In `crates/core/src/types.rs`, after the `duration_seconds` field (line 269), add:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub first_message_at: Option<i64>,
```

**Step 2: Map the field in `into_session_info()`**

In `crates/db/src/queries/row_types.rs`, in the `into_session_info` method (line 578), after `duration_seconds: self.duration_seconds as u32,` (line 628), add:

```rust
            first_message_at: self.first_message_at,
```

**Step 3: Remove the `#[allow(dead_code)]` attribute and its comment on `first_message_at` in SessionRow**

In `crates/db/src/queries/row_types.rs` line 467, remove the entire `#[allow(dead_code)] // Used internally by git sync queries, not by into_session_info()` line. The field is now mapped in `into_session_info()` so the attribute and its comment are both stale.

**Step 4: Regenerate TypeScript types**

Run: `cargo test -p claude-view-core types::export_bindings_sessioninfo`

This runs the ts-rs-generated export test for `SessionInfo`, which writes the updated `src/types/generated/SessionInfo.ts` to disk. (ts-rs v11 generates one `export_bindings_<typename>` test per exported type; `types::tests` does NOT trigger export and must not be used here.)

**Step 5: Verify the field appears in generated TS**

Check that `src/types/generated/SessionInfo.ts` now contains `firstMessageAt?: number | null`.

**Step 5b: Fix exhaustive struct literals broken by the new field**

`SessionInfo` is used in exhaustive struct literals in **23** locations across the codebase. None use `..` struct update syntax — all will fail to compile with "missing field `first_message_at`" if not updated.

Fix all 23 locations at once with this indentation-preserving perl one-liner (tested on macOS BSD + GNU):

```bash
grep -rln "longest_task_preview: None," crates/ --include="*.rs" | \
  xargs perl -i -pe 'if (/^([ \t]+)longest_task_preview: None,$/) { $_ .= "${1}first_message_at: None,\n" }'
```

This appends `first_message_at: None,` after every `longest_task_preview: None,` line, matching its exact leading whitespace. After running it, visually verify one or two files look correct before committing.

The complete list (23 locations, verified by `grep -rn "longest_task_preview: None," crates/`):

| File | Line | Notes |
|------|------|-------|
| `crates/core/src/types.rs` | 1096 | test `test_session_info_modified_at_serializes_as_number` |
| `crates/core/src/types.rs` | 1222 | `make_test_session()` helper |
| `crates/core/src/patterns/mod.rs` | 235 | `make_session()` test helper |
| `crates/core/examples/debug_json.rs` | 65 | example literal |
| `crates/core/src/discovery.rs` | 396 | discovery literal |
| `crates/core/src/discovery.rs` | 1148 | discovery literal |
| `crates/db/src/queries/dashboard.rs` | 1028 | `make_session()` test helper |
| `crates/db/src/trends.rs` | 963 | test literal |
| `crates/db/tests/queries_shared.rs` | 75 | shared test helper |
| `crates/db/src/indexer.rs` | 316 | production literal (not a test — must not be missed) |
| `crates/server/src/routes/export.rs` | 304 | server route literal |
| `crates/server/src/routes/invocables.rs` | 231 | server route literal |
| `crates/server/src/routes/sessions.rs` | 677 | server route literal |
| `crates/server/src/routes/insights.rs` | 367 | server route literal |
| `crates/server/src/routes/projects.rs` | 177 | server route literal |
| `crates/server/src/routes/stats.rs` | 702 | test literal |
| `crates/server/src/routes/stats.rs` | 799 | test literal |
| `crates/server/src/routes/stats.rs` | 917 | test literal |
| `crates/server/src/routes/stats.rs` | 1040 | test literal |
| `crates/server/src/routes/stats.rs` | 1201 | test literal |
| `crates/server/src/routes/stats.rs` | 1318 | test literal |
| `crates/server/src/routes/stats.rs` | 1426 | test literal |
| `crates/server/src/routes/turns.rs` | 430 | server route literal |

**Step 6: Verify the workspace compiles, then run affected tests**

Run: `cargo check --workspace` (catches all 23 literal sites across all crates — stops here if any are missed)
Run: `cargo test -p claude-view-db filtered_query_tests`
Run: `cargo test -p claude-view-core types::tests`
Expected: All pass — the field was already queried, just not mapped. The struct literal fixes in Step 5b allow the build to compile.

**Step 7: Commit**

```bash
git add \
  crates/core/src/types.rs \
  crates/core/src/patterns/mod.rs \
  crates/core/examples/debug_json.rs \
  crates/core/src/discovery.rs \
  crates/db/src/queries/row_types.rs \
  crates/db/src/queries/dashboard.rs \
  crates/db/src/trends.rs \
  crates/db/tests/queries_shared.rs \
  crates/db/src/indexer.rs \
  crates/server/src/routes/export.rs \
  crates/server/src/routes/invocables.rs \
  crates/server/src/routes/sessions.rs \
  crates/server/src/routes/insights.rs \
  crates/server/src/routes/projects.rs \
  crates/server/src/routes/stats.rs \
  crates/server/src/routes/turns.rs \
  src/types/generated/SessionInfo.ts
git commit -m "feat: expose first_message_at on SessionInfo for accurate session start times"
```

---

### Task 2: Activity page skeleton + route + sidebar nav + shared formatter

**Files:**
- Create: `src/pages/ActivityPage.tsx`
- Modify: `src/router.tsx` (add route)
- Modify: `src/components/Sidebar.tsx` (add nav item)
- Modify: `src/lib/format-utils.ts` (add `formatHumanDuration`)

**Step 1: Add `formatHumanDuration` to format-utils.ts**

In `src/lib/format-utils.ts`, add at the end of the file (after `formatRelativeTime`):

```ts
/** Format seconds as human-readable duration: "2h 15m", "45m", "8s" */
export function formatHumanDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (hours === 0) return `${minutes}m`
  if (minutes === 0) return `${hours}h`
  return `${hours}h ${minutes}m`
}
```

**Step 2: Create the ActivityPage skeleton**

Create `src/pages/ActivityPage.tsx`:

```tsx
import { CalendarDays } from 'lucide-react'

export function ActivityPage() {
  return (
    <div className="h-full flex flex-col overflow-y-auto">
      <div className="px-6 pt-6 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <CalendarDays className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Activity</h1>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">Where your Claude Code time goes</p>
      </div>
      <div className="px-6 pb-6 text-sm text-gray-400">Coming soon — stats, heatmap, project breakdown, daily timeline</div>
    </div>
  )
}
```

**Step 3: Add the route to router.tsx**

In `src/router.tsx`, add import at top:
```tsx
import { ActivityPage } from './pages/ActivityPage'
```

Add route after the analytics line (after `{ path: 'analytics', element: <AnalyticsPage /> }`):
```tsx
{ path: 'activity', element: <ActivityPage /> },
```

**Step 4: Add sidebar nav item**

In `src/components/Sidebar.tsx`:

Import `CalendarDays` by adding it to the lucide-react import on line 3.

**Collapsed sidebar** — add between Analytics (lines 414-426) and Reports (lines 427-439). Copy the exact same class pattern used by the Analytics link:
```tsx
<Link
  to="/activity"
  className={cn(
    'p-2 rounded-md transition-colors',
    'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/activity'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
  title="Activity"
>
  <CalendarDays className="w-5 h-5" />
</Link>
```

**Expanded sidebar** — add between Analytics (lines 509-520) and Reports (lines 521-532). Copy the exact same class pattern used by the Analytics link:
```tsx
<Link
  to={`/activity${paramString ? `?${paramString}` : ""}`}
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
    location.pathname === '/activity'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
>
  <CalendarDays className="w-4 h-4" />
  <span className="font-medium">Activity</span>
</Link>
```

**Step 5: Verify it renders**

Run: `bun run dev` — navigate to `http://localhost:5173/activity`
Expected: See the skeleton page with "Activity" heading and placeholder text. Sidebar shows Activity nav item.

**Step 6: Commit**

```bash
git add src/lib/format-utils.ts src/pages/ActivityPage.tsx src/router.tsx src/components/Sidebar.tsx
git commit -m "feat: add Activity page skeleton with route, sidebar nav, and shared formatter"
```

---

### Task 3: Activity data hook — paginated fetch + aggregate sessions

**Files:**
- Create: `src/hooks/use-activity-data.ts`
- Create: `src/lib/activity-utils.ts`

**Step 1: Create the aggregation utility**

Create `src/lib/activity-utils.ts`:

```ts
import type { SessionInfo } from '../types/generated/SessionInfo'

/** A single day's aggregated activity */
export interface DayActivity {
  /** Date string YYYY-MM-DD */
  date: string
  /** Total seconds spent across all sessions */
  totalSeconds: number
  /** Number of sessions */
  sessionCount: number
  /** Sessions for this day, sorted by start time ascending */
  sessions: SessionInfo[]
}

/** A project's aggregated time */
export interface ProjectActivity {
  /** Project display name (last path segment) */
  name: string
  /** Full project path */
  projectPath: string
  /** Total seconds */
  totalSeconds: number
  /** Number of sessions */
  sessionCount: number
}

export interface ActivitySummary {
  totalSeconds: number
  sessionCount: number
  avgSessionSeconds: number
  longestSession: { seconds: number; project: string; title: string } | null
  busiestDay: { date: string; totalSeconds: number } | null
}

/** Get session start timestamp — prefer firstMessageAt, fall back to modifiedAt - duration */
export function sessionStartTime(session: SessionInfo): number {
  if (session.firstMessageAt && session.firstMessageAt > 0) {
    return session.firstMessageAt
  }
  // Fallback: modifiedAt (= last_message_at) minus duration. Guard against
  // negative results from corrupted data (CLAUDE.md: guard ts <= 0 at every layer).
  return Math.max(0, session.modifiedAt - session.durationSeconds)
}

/** Get the date string (YYYY-MM-DD) for a Unix timestamp in local timezone */
function dateKey(unixSeconds: number): string {
  if (unixSeconds <= 0) return ''
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** Get project display name from path */
export function projectDisplayName(projectPath: string): string {
  const parts = projectPath.split('/')
  return parts[parts.length - 1] || projectPath
}

/** Aggregate sessions into daily activity, sorted newest-first */
export function aggregateByDay(sessions: SessionInfo[]): DayActivity[] {
  const dayMap = new Map<string, DayActivity>()

  for (const session of sessions) {
    if (session.durationSeconds <= 0) continue
    const start = sessionStartTime(session)
    const key = dateKey(start)
    if (!key) continue

    let day = dayMap.get(key)
    if (!day) {
      day = { date: key, totalSeconds: 0, sessionCount: 0, sessions: [] }
      dayMap.set(key, day)
    }
    day.totalSeconds += session.durationSeconds
    day.sessionCount += 1
    day.sessions.push(session)
  }

  // Sort sessions within each day by start time ascending
  for (const day of dayMap.values()) {
    day.sessions.sort((a, b) => sessionStartTime(a) - sessionStartTime(b))
  }

  // Return days sorted newest-first
  return Array.from(dayMap.values()).sort((a, b) => b.date.localeCompare(a.date))
}

/** Aggregate sessions by project, sorted by total time descending */
export function aggregateByProject(sessions: SessionInfo[]): ProjectActivity[] {
  const projectMap = new Map<string, ProjectActivity>()

  for (const session of sessions) {
    if (session.durationSeconds <= 0) continue
    // After reliability release: projectPath is always set from cwd. The || session.project
    // fallback is retained for safety but is effectively dead code after that release.
    const path = session.projectPath || session.project
    let proj = projectMap.get(path)
    if (!proj) {
      proj = { name: projectDisplayName(path), projectPath: path, totalSeconds: 0, sessionCount: 0 }
      projectMap.set(path, proj)
    }
    proj.totalSeconds += session.durationSeconds
    proj.sessionCount += 1
  }

  return Array.from(projectMap.values()).sort((a, b) => b.totalSeconds - a.totalSeconds)
}

/** Compute summary statistics */
export function computeSummary(sessions: SessionInfo[], days: DayActivity[]): ActivitySummary {
  const validSessions = sessions.filter(s => s.durationSeconds > 0)
  const totalSeconds = validSessions.reduce((sum, s) => sum + s.durationSeconds, 0)
  const sessionCount = validSessions.length

  let longestSession: ActivitySummary['longestSession'] = null
  let maxDuration = 0
  for (const s of validSessions) {
    if (s.durationSeconds > maxDuration) {
      maxDuration = s.durationSeconds
      longestSession = {
        seconds: s.durationSeconds,
        project: projectDisplayName(s.projectPath || s.project),
        title: s.summary || s.preview || '(untitled)',
      }
    }
  }

  let busiestDay: ActivitySummary['busiestDay'] = null
  let maxDaySeconds = 0
  for (const day of days) {
    if (day.totalSeconds > maxDaySeconds) {
      maxDaySeconds = day.totalSeconds
      busiestDay = { date: day.date, totalSeconds: day.totalSeconds }
    }
  }

  return {
    totalSeconds,
    sessionCount,
    avgSessionSeconds: sessionCount > 0 ? Math.round(totalSeconds / sessionCount) : 0,
    longestSession,
    busiestDay,
  }
}
```

**Step 2: Create the data hook with pagination**

Create `src/hooks/use-activity-data.ts`:

```ts
import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import type { SessionInfo } from '../types/generated/SessionInfo'
import {
  aggregateByDay,
  aggregateByProject,
  computeSummary,
  type DayActivity,
  type ProjectActivity,
  type ActivitySummary,
} from '../lib/activity-utils'

const PAGE_SIZE = 200
const MAX_PAGES = 50 // Safety limit: 50 * 200 = 10,000 sessions max

export interface ActivityData {
  days: DayActivity[]
  projects: ProjectActivity[]
  summary: ActivitySummary
  sessions: SessionInfo[]
  /** Total sessions matching the query (from server) */
  totalCount: number
}

/**
 * Fetch ALL sessions for a time range (paginated) and compute activity aggregations.
 * Respects sidebar project/branch filters when provided.
 *
 * NOTE: The /api/sessions endpoint does not support a `project` query param,
 * so project filtering is applied client-side after fetch.
 */
export function useActivityData(
  timeAfter: number | null,
  timeBefore: number | null,
  sidebarProject?: string | null,
  sidebarBranch?: string | null,
) {
  // Fork sessions (parent_id IS NOT NULL) are intentionally included. Each fork is an
  // independent working session with its own durationSeconds. Counting them gives accurate
  // total working time. The API returns them as part of kind=Conversation sessions.
  const query = useQuery<{ sessions: SessionInfo[]; total: number }>({
    queryKey: ['activity-sessions', timeAfter, timeBefore, sidebarBranch ?? ''],
    queryFn: async () => {
      const allSessions: SessionInfo[] = []
      let offset = 0
      let total = 0

      // Paginate until we have all sessions (with safety limit)
      for (let page = 0; page < MAX_PAGES; page++) {
        const sp = new URLSearchParams()
        sp.set('limit', String(PAGE_SIZE))
        sp.set('offset', String(offset))
        sp.set('sort', 'recent')
        if (timeAfter !== null && timeAfter > 0) sp.set('time_after', String(timeAfter))
        if (timeBefore !== null && timeBefore > 0) sp.set('time_before', String(timeBefore))
        if (sidebarBranch) sp.set('branches', sidebarBranch)

        const resp = await fetch(`/api/sessions?${sp}`)
        if (!resp.ok) throw new Error('Failed to fetch activity sessions')
        const data = await resp.json()

        total = data.total ?? 0
        const sessions = data.sessions as SessionInfo[]
        allSessions.push(...sessions)

        // Check if we have all sessions
        if (allSessions.length >= total || sessions.length < PAGE_SIZE) {
          break
        }
        offset += PAGE_SIZE
      }

      return { sessions: allSessions, total }
    },
    staleTime: 60_000, // 1 minute
  })

  // Memoize on primitive key per CLAUDE.md rule: never raw parsed objects in useMemo deps.
  // Include first/last modifiedAt so content changes (not just count changes) invalidate the memo
  // after background refetches when session count is unchanged but data has updated.
  const sessionCount = query.data?.sessions.length ?? 0
  const totalCount = query.data?.total ?? 0
  const firstTs = query.data?.sessions[0]?.modifiedAt ?? 0
  const lastTs = query.data?.sessions[sessionCount - 1]?.modifiedAt ?? 0
  const memoKey = JSON.stringify([sessionCount, totalCount, firstTs, lastTs, timeAfter, timeBefore, sidebarProject, sidebarBranch])

  const activity = useMemo<ActivityData | null>(() => {
    if (!query.data) return null
    let { sessions } = query.data
    const { total } = query.data

    // Client-side project filter (API has no `project` param)
    if (sidebarProject) {
      sessions = sessions.filter(s => (s.projectPath || s.project) === sidebarProject)
    }

    const days = aggregateByDay(sessions)
    const projects = aggregateByProject(sessions)
    const summary = computeSummary(sessions, days)
    return { days, projects, summary, sessions, totalCount: total }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [memoKey])

  return {
    data: activity,
    isLoading: query.isLoading,
    error: query.error,
  }
}
```

**Step 3: Commit**

```bash
git add src/lib/activity-utils.ts src/hooks/use-activity-data.ts
git commit -m "feat: add activity data hook with paginated fetch and day/project/summary aggregation"
```

---

### Task 4: Summary stats section

**Files:**
- Create: `src/components/activity/SummaryStats.tsx`
- Modify: `src/pages/ActivityPage.tsx` (wire in)

**Step 1: Create SummaryStats component**

First, create the directory:
```bash
mkdir -p src/components/activity
```

Create `src/components/activity/SummaryStats.tsx`:

```tsx
import { Clock, Hash, TrendingUp, CalendarDays } from 'lucide-react'
import { formatHumanDuration } from '../../lib/format-utils'
import type { ActivitySummary } from '../../lib/activity-utils'

/** Format YYYY-MM-DD as readable day name */
function formatDayName(dateStr: string): string {
  const date = new Date(dateStr + 'T12:00:00') // Noon to avoid TZ issues
  return date.toLocaleDateString('en-US', { weekday: 'long' })
}

interface SummaryStatsProps {
  summary: ActivitySummary
  label: string // e.g. "This Week", "Today"
}

// V2 deferred: Week-over-week comparison card (design doc Section 1).
// useTimeRange already exposes comparisonLabel; needs a second parallel query
// for the previous period and delta computation. Not included in V1.
export function SummaryStats({ summary, label }: SummaryStatsProps) {
  if (summary.sessionCount === 0) {
    return (
      <div className="text-center py-8">
        <p className="text-sm text-gray-500 dark:text-gray-400">No activity for {label.toLowerCase()}</p>
        <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">Start a Claude Code session and it will show up here</p>
      </div>
    )
  }

  const cards = [
    {
      icon: Clock,
      label: 'Total Time',
      value: formatHumanDuration(summary.totalSeconds),
    },
    {
      icon: Hash,
      label: 'Sessions',
      value: String(summary.sessionCount),
    },
    {
      icon: TrendingUp,
      label: 'Avg Session',
      value: formatHumanDuration(summary.avgSessionSeconds),
    },
    {
      icon: CalendarDays,
      label: 'Busiest Day',
      value: summary.busiestDay ? formatDayName(summary.busiestDay.date) : '--',
    },
  ]

  return (
    <div>
      <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3">{label}</h2>
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        {cards.map((card) => (
          <div
            key={card.label}
            className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg px-4 py-3"
          >
            <div className="flex items-center gap-2 mb-1">
              <card.icon className="w-4 h-4 text-gray-400 dark:text-gray-500" />
              <span className="text-xs text-gray-500 dark:text-gray-400">{card.label}</span>
            </div>
            <div className="text-xl font-semibold text-gray-900 dark:text-gray-100">{card.value}</div>
          </div>
        ))}
      </div>
      {summary.longestSession && (
        <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          Longest: {formatHumanDuration(summary.longestSession.seconds)} on {summary.longestSession.project}
        </p>
      )}
    </div>
  )
}
```

**Step 2: Wire SummaryStats into ActivityPage**

Replace `src/pages/ActivityPage.tsx`:

```tsx
import { CalendarDays } from 'lucide-react'
import { useSearchParams } from 'react-router-dom'
import { useTimeRange } from '../hooks/use-time-range'
import { useActivityData } from '../hooks/use-activity-data'
import { SummaryStats } from '../components/activity/SummaryStats'
import { cn } from '../lib/utils'
import type { TimeRangePreset } from '../hooks/use-time-range'

const PRESETS: { id: TimeRangePreset; label: string }[] = [
  { id: 'today', label: 'Today' },
  { id: '7d', label: 'This Week' },
  { id: '30d', label: 'This Month' },
  { id: '90d', label: '3 Months' },
  { id: 'all', label: 'All Time' },
]

export function ActivityPage() {
  const [searchParams] = useSearchParams()
  const sidebarProject = searchParams.get('project')
  const sidebarBranch = searchParams.get('branch')

  const { state: timeRange, setPreset } = useTimeRange()
  const { data, isLoading, error } = useActivityData(
    timeRange.fromTimestamp,
    timeRange.toTimestamp,
    sidebarProject,
    sidebarBranch,
  )

  const activeLabel = PRESETS.find(p => p.id === timeRange.preset)?.label ?? 'Custom'

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-6 pb-2 flex items-center justify-between flex-wrap gap-2">
        <div className="flex items-center gap-2">
          <CalendarDays className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Activity</h1>
        </div>
        {/* Time range picker */}
        <div className="flex items-center gap-1">
          {PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => setPreset(preset.id)}
              className={cn(
                'px-3 py-1 text-xs font-medium rounded-md transition-colors duration-150 cursor-pointer',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
                timeRange.preset === preset.id
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
              )}
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      {/* Content */}
      <div className="px-6 pb-6 space-y-6">
        {isLoading && (
          <div className="flex items-center justify-center py-12 text-sm text-gray-400">Loading activity...</div>
        )}
        {error && (
          <div className="text-sm text-red-500">Failed to load activity: {error.message}</div>
        )}
        {data && (
          <>
            <SummaryStats summary={data.summary} label={activeLabel} />
            {/* Task 5: CalendarHeatmap will go here */}
            {/* Task 6: ProjectBreakdown will go here */}
            {/* Task 7: DailyTimeline will go here */}
          </>
        )}
      </div>
    </div>
  )
}
```

**Step 3: Verify in browser**

Run dev server, navigate to `/activity`. Expected: summary stat cards rendering with real data. Toggle time range presets. Select a project in sidebar — Activity page should filter to that project.

**Step 4: Commit**

```bash
git add src/components/activity/SummaryStats.tsx src/pages/ActivityPage.tsx
git commit -m "feat: add summary stats section to Activity page"
```

---

### Task 5: Calendar heatmap component

**Files:**
- Create: `src/components/activity/CalendarHeatmap.tsx`
- Modify: `src/pages/ActivityPage.tsx` (wire in)

**Step 1: Create the CalendarHeatmap**

Create `src/components/activity/CalendarHeatmap.tsx`:

```tsx
import { useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatHumanDuration } from '../../lib/format-utils'
import type { DayActivity } from '../../lib/activity-utils'

const DAY_LABELS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

/** Get intensity level 0-3 based on total seconds */
function intensityLevel(totalSeconds: number): number {
  if (totalSeconds === 0) return 0
  if (totalSeconds < 3600) return 1     // < 1h
  if (totalSeconds < 10800) return 2    // < 3h
  return 3                               // 3h+
}

const INTENSITY_CLASSES = [
  'bg-gray-100 dark:bg-gray-800',         // 0: no activity
  'bg-blue-200 dark:bg-blue-900',          // 1: < 1h
  'bg-blue-400 dark:bg-blue-700',          // 2: 1-3h
  'bg-blue-600 dark:bg-blue-500',          // 3: 3h+
] as const

interface CalendarHeatmapProps {
  days: DayActivity[]
  onDayClick?: (date: string) => void
  selectedDate?: string | null
}

export function CalendarHeatmap({ days, onDayClick, selectedDate }: CalendarHeatmapProps) {
  const [monthOffset, setMonthOffset] = useState(0)

  // Build lookup map: YYYY-MM-DD -> DayActivity
  // Memoize on content-aware key (dates + totals) to catch time-range changes
  // that return the same number of days but different data
  const daysKey = days.map(d => `${d.date}:${d.totalSeconds}`).join(',')
  const dayMap = useMemo(() => {
    const map = new Map<string, DayActivity>()
    for (const d of days) map.set(d.date, d)
    return map
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [daysKey])

  // Compute the grid for the current month
  const { monthLabel, cells } = useMemo(() => {
    const now = new Date()
    const target = new Date(now.getFullYear(), now.getMonth() + monthOffset, 1)
    const year = target.getFullYear()
    const month = target.getMonth()
    const label = target.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })

    // Days in this month
    const daysInMonth = new Date(year, month + 1, 0).getDate()
    const firstDayOfWeek = new Date(year, month, 1).getDay() // 0=Sun
    // Convert to Mon=0 format
    const startOffset = (firstDayOfWeek + 6) % 7

    const grid: { date: string; day: number; activity: DayActivity | undefined }[] = []

    // Pad start with empty cells
    for (let i = 0; i < startOffset; i++) {
      grid.push({ date: '', day: 0, activity: undefined })
    }

    for (let d = 1; d <= daysInMonth; d++) {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`
      grid.push({ date: dateStr, day: d, activity: dayMap.get(dateStr) })
    }

    return { monthLabel: label, cells: grid }
  }, [monthOffset, dayMap])

  // Arrange into rows (weeks)
  const weeks: typeof cells[] = []
  for (let i = 0; i < cells.length; i += 7) {
    weeks.push(cells.slice(i, i + 7))
  }
  // Pad the last week to 7 cells
  const lastWeek = weeks[weeks.length - 1]
  if (lastWeek) {
    while (lastWeek.length < 7) {
      lastWeek.push({ date: '', day: 0, activity: undefined })
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400">Activity Map</h2>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setMonthOffset(prev => prev - 1)}
            className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-800 transition-colors cursor-pointer"
            aria-label="Previous month"
          >
            <ChevronLeft className="w-4 h-4 text-gray-500" />
          </button>
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300 min-w-[140px] text-center">
            {monthLabel}
          </span>
          <button
            type="button"
            onClick={() => setMonthOffset(prev => Math.min(prev + 1, 0))}
            disabled={monthOffset >= 0}
            className={cn(
              'p-1 rounded transition-colors cursor-pointer',
              monthOffset >= 0
                ? 'text-gray-300 dark:text-gray-600 cursor-default'
                : 'hover:bg-gray-200 dark:hover:bg-gray-800 text-gray-500'
            )}
            aria-label="Next month"
          >
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Day labels + Calendar grid — scrollable on narrow screens */}
      <div className="overflow-x-auto">
      <div className="min-w-[280px]">
      <div className="grid grid-cols-7 gap-1 mb-1">
        {DAY_LABELS.map((label) => (
          <div key={label} className="text-[10px] text-center text-gray-400 dark:text-gray-500">
            {label}
          </div>
        ))}
      </div>

      {/* Calendar grid */}
      <div className="space-y-1">
        {weeks.map((week, wi) => (
          <div key={wi} className="grid grid-cols-7 gap-1">
            {week.map((cell, ci) => {
              if (!cell.date) {
                return <div key={ci} className="aspect-square rounded" />
              }
              const level = intensityLevel(cell.activity?.totalSeconds ?? 0)
              const isSelected = selectedDate === cell.date
              return (
                <button
                  key={cell.date}
                  type="button"
                  onClick={() => onDayClick?.(cell.date)}
                  className={cn(
                    'aspect-square rounded-sm transition-all duration-150 cursor-pointer relative group',
                    INTENSITY_CLASSES[level],
                    isSelected && 'ring-2 ring-blue-500 ring-offset-1 dark:ring-offset-gray-950',
                  )}
                  aria-label={`${cell.date}: ${cell.activity ? formatHumanDuration(cell.activity.totalSeconds) + ' across ' + cell.activity.sessionCount + ' sessions' : 'No activity'}`}
                >
                  {/* Tooltip */}
                  <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10 pointer-events-none">
                    <div className="bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 text-[10px] px-2 py-1 rounded whitespace-nowrap shadow-lg">
                      {cell.activity
                        ? `${cell.date}: ${formatHumanDuration(cell.activity.totalSeconds)} (${cell.activity.sessionCount} sessions)`
                        : `${cell.date}: No activity`
                      }
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        ))}
      </div>
      </div>{/* end min-w-[280px] */}
      </div>{/* end overflow-x-auto */}

      {/* Legend */}
      <div className="flex items-center gap-2 mt-3 text-[10px] text-gray-400 dark:text-gray-500">
        <span>Less</span>
        {INTENSITY_CLASSES.map((cls, i) => (
          <div key={i} className={cn('w-3 h-3 rounded-sm', cls)} />
        ))}
        <span>More</span>
      </div>
    </div>
  )
}
```

**Step 2: Wire into ActivityPage**

In `src/pages/ActivityPage.tsx`, add import:
```tsx
import { useState } from 'react'
import { CalendarHeatmap } from '../components/activity/CalendarHeatmap'
```

Add state for selected date:
```tsx
const [selectedDate, setSelectedDate] = useState<string | null>(null)
```

Replace the `{/* Task 5: CalendarHeatmap will go here */}` comment with:
```tsx
<CalendarHeatmap
  days={data.days}
  onDayClick={setSelectedDate}
  selectedDate={selectedDate}
/>
```

**Step 3: Verify — see the heatmap in browser, hover cells for tooltips, click to select**

**Step 4: Commit**

```bash
git add src/components/activity/CalendarHeatmap.tsx src/pages/ActivityPage.tsx
git commit -m "feat: add calendar heatmap to Activity page"
```

---

### Task 6: Project breakdown bar chart

**Files:**
- Create: `src/components/activity/ProjectBreakdown.tsx`
- Modify: `src/pages/ActivityPage.tsx` (wire in)

**Step 1: Create the ProjectBreakdown component**

Create `src/components/activity/ProjectBreakdown.tsx`:

```tsx
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import { formatHumanDuration } from '../../lib/format-utils'
import type { ProjectActivity } from '../../lib/activity-utils'

const BAR_COLORS = [
  '#3b82f6', // blue-500
  '#8b5cf6', // violet-500
  '#06b6d4', // cyan-500
  '#f59e0b', // amber-500
  '#10b981', // emerald-500
  '#ef4444', // red-500
  '#ec4899', // pink-500
  '#6b7280', // gray-500
]

interface ProjectBreakdownProps {
  projects: ProjectActivity[]
  onProjectClick?: (projectPath: string | null) => void
  selectedProject?: string | null
}

export function ProjectBreakdown({ projects, onProjectClick, selectedProject }: ProjectBreakdownProps) {
  if (projects.length === 0) {
    return null
  }

  const totalSeconds = projects.reduce((sum, p) => sum + p.totalSeconds, 0)

  // Show top 8 projects max
  const displayProjects = projects.slice(0, 8)
  const chartData = displayProjects.map((p) => ({
    name: p.name,
    seconds: p.totalSeconds,
    projectPath: p.projectPath,
    label: `${formatHumanDuration(p.totalSeconds)} (${totalSeconds > 0 ? Math.round((p.totalSeconds / totalSeconds) * 100) : 0}%)`,
  }))

  const chartHeight = Math.max(displayProjects.length * 36, 100)

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400">By Project</h2>
        {selectedProject && (
          <button
            type="button"
            onClick={() => onProjectClick?.(null)}
            className="text-xs text-blue-500 hover:text-blue-600 cursor-pointer"
          >
            Clear filter
          </button>
        )}
      </div>
      <div style={{ height: chartHeight }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={chartData}
            layout="vertical"
            margin={{ left: 10, right: 80, top: 0, bottom: 0 }}
          >
            <XAxis type="number" hide />
            <YAxis
              type="category"
              dataKey="name"
              width={120}
              tick={{ fontSize: 12, fill: 'currentColor' }}
              tickLine={false}
              axisLine={false}
              className="text-gray-600 dark:text-gray-300"
            />
            <Tooltip
              content={({ payload }) => {
                if (!payload?.[0]) return null
                const d = payload[0].payload
                return (
                  <div className="bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 text-xs px-3 py-2 rounded-lg shadow-lg">
                    <p className="font-medium">{d.name}</p>
                    <p>{d.label}</p>
                  </div>
                )
              }}
            />
            <Bar
              dataKey="seconds"
              radius={[0, 4, 4, 0]}
              cursor="pointer"
              onClick={(_, index) => {
                const entry = chartData[index]
                if (entry?.projectPath) {
                  onProjectClick?.(
                    selectedProject === entry.projectPath ? null : entry.projectPath
                  )
                }
              }}
            >
              {chartData.map((entry, i) => (
                <Cell
                  key={entry.projectPath}
                  fill={BAR_COLORS[i % BAR_COLORS.length]}
                  opacity={selectedProject && selectedProject !== entry.projectPath ? 0.3 : 1}
                />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}
```

**Step 2: Wire into ActivityPage**

Add import:
```tsx
import { ProjectBreakdown } from '../components/activity/ProjectBreakdown'
```

Add state:
```tsx
const [selectedProject, setSelectedProject] = useState<string | null>(null)
```

Replace `{/* Task 6: ProjectBreakdown will go here */}` with:
```tsx
<ProjectBreakdown
  projects={data.projects}
  onProjectClick={setSelectedProject}
  selectedProject={selectedProject}
/>
```

**Step 3: Verify — bar chart renders, click a project to filter, opacity dims unselected**

**Step 4: Commit**

```bash
git add src/components/activity/ProjectBreakdown.tsx src/pages/ActivityPage.tsx
git commit -m "feat: add project breakdown bar chart to Activity page"
```

---

### Task 7: Daily session timeline + final wiring

**Files:**
- Create: `src/components/activity/DailyTimeline.tsx`
- Modify: `src/pages/ActivityPage.tsx` (wire in, connect all filters)

**Step 1: Create DailyTimeline component**

Create `src/components/activity/DailyTimeline.tsx`:

```tsx
import { useMemo } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { Clock, ArrowRight } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatHumanDuration } from '../../lib/format-utils'
import type { DayActivity } from '../../lib/activity-utils'
import { sessionStartTime, projectDisplayName } from '../../lib/activity-utils'
import { buildSessionUrl } from '../../lib/url-utils'
import type { SessionInfo } from '../../types/generated/SessionInfo'

function formatTime(unixSeconds: number): string {
  if (unixSeconds <= 0) return '--'
  return new Date(unixSeconds * 1000).toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

function formatDayHeader(dateStr: string): string {
  const date = new Date(dateStr + 'T12:00:00')
  const today = new Date()
  const yesterday = new Date()
  yesterday.setDate(yesterday.getDate() - 1)

  const isToday = date.toDateString() === today.toDateString()
  const isYesterday = date.toDateString() === yesterday.toDateString()

  const dayName = date.toLocaleDateString('en-US', { weekday: 'long', month: 'short', day: 'numeric' })
  if (isToday) return `Today — ${dayName}`
  if (isYesterday) return `Yesterday — ${dayName}`
  return dayName
}

interface DailyTimelineProps {
  days: DayActivity[]
  selectedDate?: string | null
  selectedProject?: string | null
  maxDays?: number
}

// V2 deferred: "Load more days" button / lazy-load (design doc Section 4).
// Current: hard cap at maxDays=14. Add a "Show all" toggle in V2.
export function DailyTimeline({ days, selectedDate, selectedProject, maxDays = 14 }: DailyTimelineProps) {
  const [searchParams] = useSearchParams()

  const filteredDays = useMemo(() => {
    let result = days

    // Filter by selected date
    if (selectedDate) {
      result = result.filter(d => d.date === selectedDate)
    }

    // Filter sessions within days by project
    if (selectedProject) {
      result = result.map(day => {
        const filtered = day.sessions.filter(s => (s.projectPath || s.project) === selectedProject)
        return {
          ...day,
          sessions: filtered,
          totalSeconds: filtered.reduce((sum, s) => sum + s.durationSeconds, 0),
          sessionCount: filtered.length,
        }
      }).filter(day => day.sessions.length > 0)
    }

    return result.slice(0, maxDays)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [days.map(d => `${d.date}:${d.sessionCount}`).join(','), selectedDate, selectedProject, maxDays])

  if (filteredDays.length === 0) {
    return (
      <div className="text-center py-8 text-sm text-gray-400 dark:text-gray-500">
        No sessions for this period
      </div>
    )
  }

  return (
    <div>
      <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3">Session Timeline</h2>
      <div className="space-y-4">
        {filteredDays.map((day) => (
          <div key={day.date}>
            {/* Day header */}
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {formatDayHeader(day.date)}
              </h3>
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {day.sessionCount} {day.sessionCount === 1 ? 'session' : 'sessions'} — {formatHumanDuration(day.totalSeconds)}
              </span>
            </div>

            {/* Session rows */}
            <div className="space-y-1 ml-2 border-l-2 border-gray-200 dark:border-gray-800 pl-3">
              {day.sessions.map((session: SessionInfo) => {
                const start = sessionStartTime(session)
                const end = session.modifiedAt
                const title = session.summary || session.preview || '(untitled)'
                const truncatedTitle = title.length > 60 ? title.slice(0, 57) + '...' : title

                return (
                  <Link
                    key={session.id}
                    to={buildSessionUrl(session.id, searchParams)}
                    className={cn(
                      'flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors duration-150',
                      'hover:bg-gray-100 dark:hover:bg-gray-900 cursor-pointer group'
                    )}
                  >
                    {/* Time range */}
                    <span className="text-xs text-gray-400 dark:text-gray-500 font-mono whitespace-nowrap min-w-[110px]">
                      {formatTime(start)} — {formatTime(end)}
                    </span>

                    {/* Project badge */}
                    <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 px-2 py-0.5 rounded whitespace-nowrap">
                      {projectDisplayName(session.projectPath || session.project)}
                    </span>

                    {/* Title */}
                    <span className="flex-1 text-gray-700 dark:text-gray-300 truncate text-xs">
                      {truncatedTitle}
                    </span>

                    {/* Duration */}
                    <span className="text-xs font-medium text-gray-500 dark:text-gray-400 whitespace-nowrap flex items-center gap-1">
                      <Clock className="w-3 h-3" />
                      {formatHumanDuration(session.durationSeconds)}
                    </span>

                    {/* Arrow */}
                    <ArrowRight className="w-3 h-3 text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100 transition-opacity" />
                  </Link>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
```

**Step 2: Final ActivityPage.tsx with all sections wired**

Replace `src/pages/ActivityPage.tsx` with the final version below. This supersedes the incremental edits from Tasks 5 and 6 — if you skipped those intermediate commits, this single replacement is sufficient:

```tsx
import { useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { CalendarDays } from 'lucide-react'
import { useTimeRange } from '../hooks/use-time-range'
import { useActivityData } from '../hooks/use-activity-data'
import { SummaryStats } from '../components/activity/SummaryStats'
import { CalendarHeatmap } from '../components/activity/CalendarHeatmap'
import { ProjectBreakdown } from '../components/activity/ProjectBreakdown'
import { DailyTimeline } from '../components/activity/DailyTimeline'
import { cn } from '../lib/utils'
import type { TimeRangePreset } from '../hooks/use-time-range'

const PRESETS: { id: TimeRangePreset; label: string }[] = [
  { id: 'today', label: 'Today' },
  { id: '7d', label: 'This Week' },
  { id: '30d', label: 'This Month' },
  { id: '90d', label: '3 Months' },
  { id: 'all', label: 'All Time' },
]

export function ActivityPage() {
  const [searchParams] = useSearchParams()
  const sidebarProject = searchParams.get('project')
  const sidebarBranch = searchParams.get('branch')

  const { state: timeRange, setPreset } = useTimeRange()
  const { data, isLoading, error } = useActivityData(
    timeRange.fromTimestamp,
    timeRange.toTimestamp,
    sidebarProject,
    sidebarBranch,
  )

  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [selectedProject, setSelectedProject] = useState<string | null>(null)

  const activeLabel = PRESETS.find(p => p.id === timeRange.preset)?.label ?? 'Custom'

  // Clear filters when time range changes
  const handlePresetChange = (preset: TimeRangePreset) => {
    setPreset(preset)
    setSelectedDate(null)
    setSelectedProject(null)
  }

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-6 pb-2 flex items-center justify-between flex-wrap gap-2">
        <div className="flex items-center gap-2">
          <CalendarDays className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Activity</h1>
        </div>
        {/* Time range picker */}
        <div className="flex items-center gap-1">
          {PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => handlePresetChange(preset.id)}
              className={cn(
                'px-3 py-1 text-xs font-medium rounded-md transition-colors duration-150 cursor-pointer',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
                timeRange.preset === preset.id
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
              )}
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      {/* Active filters indicator */}
      {(selectedDate || selectedProject) && (
        <div className="px-6 pb-2 flex items-center gap-2 text-xs">
          <span className="text-gray-400">Filtered by:</span>
          {selectedDate && (
            <button
              type="button"
              onClick={() => setSelectedDate(null)}
              className="bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded cursor-pointer hover:bg-blue-200 dark:hover:bg-blue-900/60"
            >
              {selectedDate} x
            </button>
          )}
          {selectedProject && (
            <button
              type="button"
              onClick={() => setSelectedProject(null)}
              className="bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded cursor-pointer hover:bg-blue-200 dark:hover:bg-blue-900/60"
            >
              {selectedProject.split('/').pop()} x
            </button>
          )}
        </div>
      )}

      {/* Content */}
      <div className="px-6 pb-6 space-y-6">
        {isLoading && (
          <div className="flex items-center justify-center py-12 text-sm text-gray-400">Loading activity...</div>
        )}
        {error && (
          <div className="text-sm text-red-500">Failed to load activity: {error.message}</div>
        )}
        {data && (
          <>
            <SummaryStats summary={data.summary} label={activeLabel} />
            {data.summary.sessionCount > 0 && (
              <>
                <CalendarHeatmap
                  days={data.days}
                  onDayClick={setSelectedDate}
                  selectedDate={selectedDate}
                />
                <ProjectBreakdown
                  projects={data.projects}
                  onProjectClick={setSelectedProject}
                  selectedProject={selectedProject}
                />
                <DailyTimeline
                  days={data.days}
                  selectedDate={selectedDate}
                  selectedProject={selectedProject}
                />
              </>
            )}
          </>
        )}
      </div>
    </div>
  )
}
```

**Step 3: Full integration test in browser**

1. Navigate to `/activity`
2. Verify summary stats show real numbers
3. Toggle time range presets (Today, This Week, etc.)
4. Hover heatmap cells for tooltips
5. Click a heatmap cell — timeline filters to that day, filter chip appears
6. Click a project bar — timeline filters to that project, opacity changes on bars
7. Click a session row — navigates to session detail
8. Clear filters via the x chips
9. Select a project in sidebar — Activity page filters to that project
10. Check dark mode (toggle system or app setting)

**Step 4: Commit**

```bash
# Include all activity components (no-ops for files already committed in Tasks 5/6)
git add src/components/activity/DailyTimeline.tsx src/components/activity/CalendarHeatmap.tsx src/components/activity/ProjectBreakdown.tsx src/pages/ActivityPage.tsx
git commit -m "feat: add daily timeline and wire all sections together on Activity page"
```

---

## Deferred to V2

These features are in the design doc but intentionally excluded from V1:

- **Week-over-week comparison** (design doc Section 1): Requires second parallel query for previous period + delta computation. `useTimeRange` already exposes `comparisonLabel`.
- **"Load more days" button** (design doc Section 4): Simplified to `maxDays = 14` hard cap. Add lazy-load / "Show all" toggle in V2.
- **Virtualized list** (design doc Section 4): Plain `.map()` render is sufficient for ~14 days with ~5-10 sessions each. Add virtualization if performance degrades.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `sp.set('project', sidebarProject)` — API has no `project` param, silently ignored | Blocker | Removed server param; project filtering now applied client-side in `useActivityData` after fetch |
| 2 | `while (true)` pagination loop has no max iteration guard | Blocker | Changed to `for (page = 0; page < MAX_PAGES; page++)` with `MAX_PAGES = 50` (10K sessions cap) |
| 3 | `dayMap` useMemo keyed on `days.length` — stale heatmap when day count unchanged but content differs | Blocker | Changed key to `days.map(d => '${d.date}:${d.totalSeconds}').join(',')` — content-aware |
| 4 | `cargo test -p claude-view-db queries_sessions_test` matches zero tests (module is `filtered_query_tests`) | Warning | Fixed test target to `filtered_query_tests` |
| 5 | `cargo test -p claude-view-core export_bindings` matches zero tests | Warning | Fixed to `cargo test -p claude-view-core types::tests` (later superseded by #26 — final command is `types::export_bindings_sessioninfo`) |
| 6 | `memoKey` template literal: `null` coerces to `"null"` string — collision with literal "null" values | Warning | Changed to `JSON.stringify([...])` for unambiguous serialization |
| 7 | `if (timeAfter)` is falsy when `timeAfter === 0` (violates CLAUDE.md timestamp guard rule) | Warning | Changed to `if (timeAfter !== null && timeAfter > 0)` for both `timeAfter` and `timeBefore` |
| 8 | Collapsed sidebar: `rounded-lg` + `duration-150` vs existing `rounded-md`; expanded: `gap-3 px-3 py-2` vs `gap-2 px-2 py-1.5`; active state colors differ | Warning | Copied exact CSS classes from existing Analytics nav item for both collapsed and expanded |
| 9 | `#[allow(dead_code)]` comment text stale after mapping added | Minor | Clarified step: remove entire attribute line including comment |
| 10 | Design doc features (WoW comparison, load-more, virtualization) absent with no acknowledgment | Warning | Added "Deferred to V2" section + inline `// V2 deferred:` comments in code |
| 11 | Zero sessions shows `0s` / `0` in stat cards — no empty-state message | Minor | Added early return in `SummaryStats` with "No activity" message; `ActivityPage` gates heatmap/chart/timeline behind `sessionCount > 0` |
| 12 | Calendar heatmap `grid-cols-7` has no responsive handling for narrow screens | Minor | Added `overflow-x-auto` + `min-w-[280px]` wrapper around day labels and grid |
| 13 | Tasks 5/6 incremental edits are superseded by Task 7 full replacement — no note | Minor | Added note to Task 7 Step 2: "This supersedes the incremental edits from Tasks 5 and 6" |
| 14 | Task 1 Step 4: `-- --ignored` flag skips the test (it has no `#[ignore]`), silently aborting TS type regeneration | Blocker | Removed `-- --ignored` from the cargo test command |
| 15 | Task 1: adding `first_message_at` to `SessionInfo` breaks two exhaustive struct literals in tests — plan did not cover this | Blocker | Added Step 5b: update `make_session()` in `dashboard.rs` and test struct literal in `types.rs` with `first_message_at: None,` |
| 16 | Task 6 `Bar onClick` handler: `(entry) => ...` is wrong arity — Recharts v3 passes `(data, index, event)`, not a single entry | Blocker | Changed to `(_, index) => { const entry = chartData[index]; ... }` — matching the existing `CategoryBarChart.tsx` pattern |
| 17 | Task 3 has no note that `session.firstMessageAt` requires Task 1 to be complete before TypeScript accepts it | Minor | Added "Task sequencing constraint" note at top of plan |
| 18 | Step 5b covered only 2 of 6 exhaustive `SessionInfo` struct literals — 4 compile errors left unaddressed including production `indexer.rs` | Blocker | Expanded Step 5b to enumerate all 6 locations with file paths |
| 19 | Task 1 Step 7 `git add` missed `patterns/mod.rs`, `queries_shared.rs`, `indexer.rs`, `dashboard.rs` — incomplete commit | Blocker | Added all 6 affected files to the `git add` command |
| 20 | `formatHumanDuration`: `Math.round((seconds % 3600) / 60)` produces `"1h 60m"` for sessions near hour boundaries | Blocker | Changed to `Math.floor` |
| 21 | `useActivityData` memoKey used only `sessionCount` — stale after background refetch returning same count with different content | Blocker | Added `firstTs`/`lastTs` (first and last session `modifiedAt`) to memoKey |
| 22 | `DailyTimeline` `useMemo` used raw `days` array as dep — violates CLAUDE.md "never raw objects" rule | Warning | Changed dep to `days.map(d => '${d.date}:${d.sessionCount}').join(',')` primitive key |
| 23 | Task 7 `git add` missed `CalendarHeatmap.tsx` and `ProjectBreakdown.tsx` — incomplete commit when skipping Tasks 5/6 intermediate commits | Blocker | Added all activity component files to the `git add` |
| 24 | `src/components/activity/` directory never explicitly created — human implementer would get `No such file or directory` | Warning | Added `mkdir -p src/components/activity` to Task 4 Step 1 |
| 25 | Step 5b listed only 6 exhaustive `SessionInfo` struct literal locations but grep found 22 — 16 compile errors left unaddressed in `discovery.rs`, `server/routes/stats.rs` (×7), `sessions.rs`, `export.rs`, `invocables.rs`, `insights.rs`, `projects.rs`, `turns.rs`, `examples/debug_json.rs` | Blocker | Expanded Step 5b to enumerate all 22 locations with file:line table; added `cargo check --workspace` to Step 6; updated Step 7 git add to include all 10 additional files |
| 26 | Step 4 said `cargo test -p claude-view-core types::tests` regenerates `SessionInfo.ts` — false. ts-rs v11 generates a separate `types::export_bindings_sessioninfo` test; the `tests` module doesn't call any export function | Blocker | Changed Step 4 command to `cargo test -p claude-view-core types::export_bindings_sessioninfo` |
| 27 | Step 5b perl one-liner used `${1 =~ s\|...\|r}` — not valid Perl interpolation syntax, would fail or produce wrong output | Blocker | Replaced with correct `perl -i -pe 'if (/^([ \t]+)longest_task_preview: None,$/) { $_ .= "${1}first_message_at: None,\n" }'` |
| 28 | Step 5b table listed 22 locations but grep found 23 — `crates/db/src/trends.rs:963` was missing from the table and from Step 7 `git add` | Blocker | Added `trends.rs:963` to the table (count now 23); added `crates/db/src/trends.rs` to Step 7 git add |
| 29 | `sessionStartTime` fallback `modifiedAt - durationSeconds` could return a negative timestamp for corrupted sessions — violates CLAUDE.md "guard `ts <= 0` at every layer" rule | Low | Wrapped fallback in `Math.max(0, ...)` |
