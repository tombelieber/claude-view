---
status: approved
date: 2026-02-19
---

# Restore Activity Sparkline & Stats Grid Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore the ActivitySparkline chart and 6-card stats grid with a new lightweight backend endpoint instead of loading all sessions client-side.

**Architecture:** New `GET /api/sessions/activity` returns auto-bucketed `{date, count}[]` (~30-60 points). Sparkline becomes self-contained with its own fetch. StorageOverview reverts to 6-card grid while keeping new additive features (paths, index size).

**Tech Stack:** Rust/Axum (backend), SQLite/sqlx (query), React/Recharts (frontend), TanStack Query (data fetching)

---

### Task 1: DB query — `session_activity_histogram`

**Files:**
- Modify: `crates/db/src/queries/dashboard.rs` (add method at ~line 243, before `query_sessions_filtered`)
- Test: `crates/db/tests/acceptance_tests.rs`

**Step 1: Write the failing test**

Add to `crates/db/tests/acceptance_tests.rs`:

```rust
#[tokio::test]
async fn test_session_activity_histogram() {
    let db = test_db().await;
    seed_sessions(&db, 5).await; // existing helper

    let (activity, bucket) = db.session_activity_histogram().await.unwrap();

    assert_eq!(bucket, "day"); // <60 days span → daily
    assert!(!activity.is_empty());
    let total: i64 = activity.iter().map(|a| a.count).sum();
    assert_eq!(total, 5); // all 5 sessions accounted for
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db -- test_session_activity_histogram -v`
Expected: FAIL — `session_activity_histogram` not defined

**Step 3: Write implementation**

Add to `crates/db/src/queries/dashboard.rs` on the `impl Database` block:

```rust
/// Activity histogram for sparkline chart.
/// Auto-buckets by day/week/month based on data span.
/// Returns (Vec<ActivityPoint>, bucket_name).
pub async fn session_activity_histogram(&self) -> DbResult<(Vec<ActivityPoint>, String)> {
    // 1. Determine span
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(MIN(last_message_at), 0), COALESCE(MAX(last_message_at), 0) \
         FROM sessions WHERE last_message_at > 0 AND is_sidechain = 0"
    )
    .fetch_one(self.pool())
    .await?;

    let span_days = (row.1 - row.0) / 86400;
    let (group_expr, bucket) = if span_days > 365 {
        ("strftime('%Y-%m', last_message_at, 'unixepoch')", "month")
    } else if span_days > 60 {
        ("strftime('%Y-W%W', last_message_at, 'unixepoch')", "week")
    } else {
        ("DATE(last_message_at, 'unixepoch')", "day")
    };

    // 2. Run grouped count
    let sql = format!(
        "SELECT {group_expr} AS date, COUNT(*) AS count \
         FROM sessions \
         WHERE last_message_at > 0 AND is_sidechain = 0 \
         GROUP BY date ORDER BY date"
    );

    let rows: Vec<ActivityPoint> = sqlx::query_as(&sql)
        .fetch_all(self.pool())
        .await?;

    Ok((rows, bucket.to_string()))
}
```

Add the struct (above the `impl Database` block, near `SessionFilterParams`):

```rust
/// A single point in the activity histogram.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ActivityPoint {
    pub date: String,
    pub count: i64,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-db -- test_session_activity_histogram -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries/dashboard.rs crates/db/tests/acceptance_tests.rs
git commit -m "feat(db): add session_activity_histogram with auto-bucketing"
```

---

### Task 2: Re-export `ActivityPoint` from db crate

**Files:**
- Modify: `crates/db/src/lib.rs:17` (add re-export)

**Step 1: Add re-export**

Add after the existing `pub use queries::SessionFilterParams;` line:

```rust
pub use queries::ActivityPoint;
```

Also add to `crates/db/src/queries/mod.rs` if `ActivityPoint` isn't accessible via `dashboard::`:

```rust
pub use dashboard::ActivityPoint;
```

**Step 2: Verify it compiles**

Run: `cargo check -p claude-view-db`
Expected: no errors

**Step 3: Commit**

```bash
git add crates/db/src/lib.rs crates/db/src/queries/mod.rs
git commit -m "chore(db): re-export ActivityPoint"
```

---

### Task 3: API route — `GET /api/sessions/activity`

**Files:**
- Modify: `crates/server/src/routes/sessions.rs` (add handler + response struct + route)

**Step 1: Write the failing test**

Add to the `mod tests` block in `crates/server/src/routes/sessions.rs`:

```rust
#[tokio::test]
async fn test_session_activity() {
    let db = test_db().await;
    seed_minimal_sessions(&db).await;
    let app = test_app(db);

    let (status, body) = do_get(app, "/api/sessions/activity").await;
    assert_eq!(status, StatusCode::OK);

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(resp["activity"].is_array());
    assert!(resp["bucket"].is_string());
    let activity = resp["activity"].as_array().unwrap();
    assert!(!activity.is_empty());
    // Each point has date + count
    assert!(activity[0]["date"].is_string());
    assert!(activity[0]["count"].is_number());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server -- test_session_activity -v`
Expected: FAIL — handler doesn't exist

**Step 3: Write implementation**

Add response struct near `SessionsListResponse`:

```rust
/// Response for GET /api/sessions/activity
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionActivityResponse {
    pub activity: Vec<claude_view_db::ActivityPoint>,
    pub bucket: String,
}
```

Add handler:

```rust
/// GET /api/sessions/activity — Activity histogram for sparkline chart.
pub async fn session_activity(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SessionActivityResponse>> {
    let (activity, bucket) = state.db.session_activity_histogram().await?;
    Ok(Json(SessionActivityResponse { activity, bucket }))
}
```

Add route to `router()` fn (after the `/branches` line):

```rust
.route("/sessions/activity", get(session_activity))
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server -- test_session_activity -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "feat(api): add GET /api/sessions/activity endpoint"
```

---

### Task 4: Refactor `ActivitySparkline` to use new API

**Files:**
- Modify: `src/components/ActivitySparkline.tsx`

**Step 1: Rewrite the component**

Replace the props interface and data logic. Keep the Recharts rendering mostly intact:

- Remove props: `sessions`, `selectedDate`, `onDateSelect`
- Remove imports: `countSessionsByDay`, `toDateKey`, `SessionInfo`
- Add `useQuery` fetch from `/api/sessions/activity`
- Accept data as `{date: string, count: number}[]` + `bucket: string`
- Remove click handler on dots (display only)
- Remove the `selectedDate` highlight logic
- Keep: theme detection, tooltip, area chart rendering, responsive container

New props: none (self-contained). The component fetches its own data.

```tsx
import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  AreaChart, Area, XAxis, YAxis, CartesianGrid,
  Tooltip, ResponsiveContainer, type TooltipProps,
} from 'recharts'
import { useTheme } from '../hooks/use-theme'

interface ActivityPoint {
  date: string
  count: number
}

interface ChartDatum {
  date: number   // timestamp ms for XAxis
  count: number
  label: string  // formatted for tooltip
}

export function ActivitySparkline() {
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'

  const { data } = useQuery({
    queryKey: ['session-activity'],
    queryFn: async () => {
      const res = await fetch('/api/sessions/activity')
      if (!res.ok) throw new Error('Failed to fetch activity')
      return res.json() as Promise<{ activity: ActivityPoint[]; bucket: string }>
    },
    staleTime: 60_000,
  })

  const chartData = useMemo((): ChartDatum[] => {
    if (!data?.activity) return []
    return data.activity.map(pt => {
      // Parse the date string into a timestamp for the XAxis
      const ts = pt.date.includes('W')
        ? parseWeekDate(pt.date)
        : new Date(pt.date).getTime()
      return {
        date: ts,
        count: pt.count,
        label: formatBucketLabel(pt.date, data.bucket),
      }
    })
  }, [data])

  const totalSessions = useMemo(
    () => chartData.reduce((sum, d) => sum + d.count, 0),
    [chartData]
  )

  if (chartData.length === 0) return null

  // ... keep existing Recharts rendering (AreaChart, gradient, tooltip, etc.)
  // Remove: onClick handler, selectedDate dot styling, active dot callback
}

function parseWeekDate(weekStr: string): number {
  // "2026-W08" → approximate timestamp for that week
  const [year, w] = weekStr.split('-W').map(Number)
  const jan1 = new Date(year, 0, 1)
  return jan1.getTime() + (w - 1) * 7 * 86400000
}

function formatBucketLabel(date: string, bucket: string): string {
  if (bucket === 'month') {
    const d = new Date(date + '-01')
    return d.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })
  }
  if (bucket === 'week') return date.replace('-W', ' W')
  const d = new Date(date)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}
```

**Step 2: Verify it compiles**

Run: `bunx tsc --noEmit`
Expected: no type errors

**Step 3: Commit**

```bash
git add src/components/ActivitySparkline.tsx
git commit -m "refactor(ui): ActivitySparkline self-contained with /api/sessions/activity"
```

---

### Task 5: Restore `ActivitySparkline` in `HistoryView`

**Files:**
- Modify: `src/components/HistoryView.tsx`

**Step 1: Add import**

Add back (near the other component imports):

```tsx
import { ActivitySparkline } from './ActivitySparkline'
```

**Step 2: Add JSX**

Insert the sparkline between the filter header `</div>` and `{/* Search + Filters bar */}`:

```tsx
{/* Activity sparkline chart */}
<ActivitySparkline />
```

No props needed — it's self-contained.

**Step 3: Verify it renders**

Run: `bun run dev`, open browser, confirm sparkline appears above the search bar.

**Step 4: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "feat(ui): restore ActivitySparkline in HistoryView"
```

---

### Task 6: Restore StorageOverview 6-card stats grid

**Files:**
- Modify: `src/components/StorageOverview.tsx`

**Step 1: Restore the grid**

Replace the current 3-card grid + inline timestamps section with the original 6-card layout. Find the section that starts with `{/* Primary Metrics — count cards */}` and replace it (plus the inline timestamps `<div>` below it) with:

```tsx
{/* Counts Grid - Responsive: 2 cols mobile, 3 cols tablet, 6 cols desktop */}
<div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
  <StatCard label="Sessions" value={formatNumber(stats?.sessionCount ?? 0)} icon={MessageSquare} />
  <StatCard label="Projects" value={formatNumber(stats?.projectCount ?? 0)} icon={FolderOpen} />
  <StatCard label="Commits" value={formatNumber(stats?.commitCount ?? 0)} icon={GitCommit} />
  <StatCard
    label="Oldest Session"
    value={formatTimestamp(stats?.oldestSessionDate ?? null)}
    icon={Calendar}
  />
  <StatCard label="Index Built" value={formatTimestamp(stats?.lastIndexAt ?? null)} icon={Database} />
  <StatCard label="Last Git Sync" value={formatTimestamp(stats?.lastGitSyncAt ?? null)} icon={GitBranch} />
</div>
```

Keep all new additive features intact (paths in donut chart, app data path callout).

**Step 2: Verify it renders**

Run: `bun run dev`, navigate to Storage page, confirm 6 stat cards in responsive grid.

**Step 3: Commit**

```bash
git add src/components/StorageOverview.tsx
git commit -m "feat(ui): restore 6-card stats grid in StorageOverview"
```

---

### Task 7: Wiring verification (end-to-end)

**Files:** none (verification only)

**Step 1: Build backend**

Run: `cargo build -p claude-view-server`
Expected: compiles clean

**Step 2: Run all backend tests**

Run: `cargo test -p claude-view-db -p claude-view-server`
Expected: all pass

**Step 3: Run frontend type check**

Run: `bunx tsc --noEmit`
Expected: no errors

**Step 4: Hit the endpoint manually**

Run: `curl -s http://localhost:47892/api/sessions/activity | jq .`
Expected: `{ "activity": [...], "bucket": "day"|"week"|"month" }`

**Step 5: Visual check**

Open browser → Sessions page. Confirm:
- Sparkline chart renders above the search bar
- Chart shows date histogram with correct bucket labels
- No click interaction (display only)

Open browser → Storage page. Confirm:
- 6 stat cards in responsive grid
- All values populated (Sessions, Projects, Commits, Oldest Session, Index Built, Last Git Sync)
- Path info still visible in donut chart breakdown
