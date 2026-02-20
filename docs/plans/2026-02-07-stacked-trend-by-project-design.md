---
status: draft
date: 2026-02-07
---

# Stacked Contribution Trend by Project

## Context

When "All Projects" is selected on the Contributions page, the Contribution Trend chart shows a single combined line for all projects. Users want to see a breakdown by project — which projects are driving the most output over time.

This is deferred from the main UI/UX enhancement PR (Steps 1-7) and should be implemented as a separate follow-up.

## Approach

### Backend

**New query:** `get_contribution_trend_by_project()` in `crates/db/src/snapshots.rs`

Returns per-project daily trend data instead of global aggregates:
```rust
pub async fn get_contribution_trend_by_project(
    &self,
    range: TimeRange,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> DbResult<Vec<ProjectTrendPoint>>
```

**New type:**
```rust
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectTrendPoint {
    pub date: String,
    pub project_id: String,
    pub project_name: String,
    pub lines_added: i64,
    pub lines_removed: i64,
    pub commits: i64,
    pub sessions: i64,
}
```

**SQL:** Group existing trend query by `project_id`:
```sql
SELECT
    date(last_message_at, 'unixepoch') as date,
    project_id,
    COALESCE(project_display_name, project_id) as project_name,
    COALESCE(SUM(ai_lines_added), 0),
    COALESCE(SUM(ai_lines_removed), 0),
    COUNT(DISTINCT commit_hash) as commits,
    COUNT(*) as sessions
FROM sessions
WHERE date(last_message_at, 'unixepoch') >= ?1
  AND date(last_message_at, 'unixepoch') <= ?2
GROUP BY date, project_id
ORDER BY date ASC, project_id ASC
```

### API

Add optional `?groupBy=project` parameter to `GET /api/contributions`:
- Default (no param): returns flat `trend: Vec<DailyTrendPoint>` (current behavior)
- `groupBy=project`: additionally returns `trendByProject: Vec<ProjectTrendPoint>`

Or add a dedicated endpoint: `GET /api/contributions/trend?range=week&groupBy=project`

### Frontend

**Modify `TrendChart.tsx`:**

1. Add a toggle button in the chart header next to the metric toggle:
   ```
   [Lines] [Commits] [Sessions]     [Combined | By Project]
   ```

2. When "By Project" is active:
   - Switch from `LineChart` to `AreaChart`
   - Dynamically generate one `Area` per project:
     ```tsx
     {projects.map((project, i) => (
       <Area
         key={project}
         type="monotone"
         dataKey={project}
         stackId="project"
         stroke={COLORS[i % COLORS.length]}
         fill={COLORS[i % COLORS.length]}
         fillOpacity={0.6}
       />
     ))}
     ```
   - Color assignment: deterministic palette based on project index
   - Legend shows project names with matching colors
   - Tooltip shows per-project values + total

3. Data transformation:
   - Pivot `ProjectTrendPoint[]` into chart-ready format:
     ```ts
     // Input: [{ date, projectId, linesAdded }, ...]
     // Output: [{ date, "claude-view": 500, "claude-view": 300 }, ...]
     ```

### Color Palette for Projects

Use a 6-color accessible palette (distinct on colorblind vision):
```ts
const PROJECT_COLORS = [
  '#3b82f6', // blue-500
  '#22c55e', // green-500
  '#f59e0b', // amber-500
  '#8b5cf6', // violet-500
  '#ec4899', // pink-500
  '#06b6d4', // cyan-500
]
```

Assign deterministically: sort projects alphabetically, assign `COLORS[index % 6]`.

## Open Questions

- Should the stacked chart use `Area` (filled) or `Line` (just lines)? Stacked areas are more visual for part-to-whole, lines are cleaner for trend comparison.
- Maximum number of projects before chart becomes unreadable? Consider grouping small projects into "Other" if > 6 projects.
- Should this also work in the Efficiency chart (cost breakdown by project)?

## Files

| File | Changes |
|------|---------|
| `crates/db/src/snapshots.rs` | New `get_contribution_trend_by_project()` query + `ProjectTrendPoint` type |
| `crates/server/src/routes/contributions.rs` | Add `trendByProject` to response or new endpoint |
| `src/components/contributions/TrendChart.tsx` | Add "By Project" toggle, AreaChart rendering |
| `src/hooks/use-contributions.ts` | Update types if response shape changes |

## Dependencies

Depends on: Steps 1-7 from the main UI/UX enhancement plan being completed first (especially Step 1 — project filter fix, which ensures per-project queries work correctly).
