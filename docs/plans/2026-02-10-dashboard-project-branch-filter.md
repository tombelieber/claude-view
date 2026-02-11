---
status: pending
date: 2026-02-10
---

# Dashboard Project/Branch Filter Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the existing sidebar project→branch filter through to the dashboard (fluency) page so all stats, trends, heatmap, and AI generation data respect the selected project/branch scope.

**Architecture:** The frontend already reads `?project` and `?branch` URL params and passes them to `useDashboardStats()`. The gap is entirely backend: the Rust API handler ignores these params, and none of the ~35 DB queries accept project/branch filters. We add `project`/`branch` to the `DashboardQuery` struct, thread them through the handler, and add optional WHERE clauses to every DB query. We also fix a frontend param name mismatch (`branches` → `branch`).

**Tech Stack:** Rust (Axum, sqlx), TypeScript (React, TanStack Query)

---

## Architecture Decision: Dynamic SQL via `?N IS NULL OR column = ?N`

The contributions page (`snapshots.rs`) uses separate query branches (`if let Some(pid) = project_id { ... } else { ... }`). That approach doubles the SQL surface area per query.

Instead, we use **SQLite's `?N IS NULL OR column = ?N` pattern** — a single query handles both filtered and unfiltered cases with no branching. SQLite's query planner optimizes this well (it short-circuits when the param is NULL). This keeps the code DRY and the diff minimal.

```sql
-- Single query handles all 4 combinations:
-- (no filter, project only, branch only, project+branch)
WHERE is_sidechain = 0
  AND (?3 IS NULL OR project_id = ?3)
  AND (?4 IS NULL OR git_branch = ?4)
```

The `sessions` table already has `CREATE INDEX idx_sessions_project_branch ON sessions(project_id, git_branch)` (migration 10), so filtered queries use the index.

---

## Design Decisions & Edge Cases

**Modify existing functions in-place (no dead code).** Instead of creating new `_filtered` variants alongside the old functions, we modify the existing signatures directly. Every existing call site gets updated to pass `None, None` for backward-compatible behavior. This avoids dead code.

**Heatmap filtering policy.** The heatmap always shows a fixed 90-day window (not affected by the time-range picker). However, it DOES respect project/branch filters — when viewing a single project, the heatmap should reflect that project's activity. This matches the UX expectation: "show me this project's last 90 days."

**`git_branch` NULL handling.** Sessions without a git repo have `git_branch = NULL`. The pattern handles this correctly:
- Branch filter = `None` → `NULL IS NULL` = true → NULL branches included ✓
- Branch filter = `Some("main")` → `'main' IS NULL` = false → `git_branch = 'main'` → only matching ✓
- Sessions with NULL branch never match a branch filter, which is correct behavior.

**`top_invocables_by_kind` sidechain bugfix.** The existing `top_invocables_by_kind()` (no range) does NOT join the sessions table and does NOT filter `is_sidechain = 0`. The range variant already does. Adding the sessions join for project/branch filtering also brings the sidechain filter for consistency. This is an intentional behavioral fix — dashboard invocable stats should exclude sidechains, matching every other dashboard query. The impact is negligible (sidechains rarely have invocations).

**`get_week_trends` and `/api/trends` are NOT affected.** `get_week_trends()` is called from `routes/trends.rs:23` and always shows global (unfiltered) week-over-week metrics. It continues to pass `None, None` for project/branch internally. No changes needed to the `/api/trends` endpoint.

---

## Commit Strategy

Every commit compiles the full workspace (`cargo build`) and passes all tests.

| Commit | Tasks | What |
|--------|-------|------|
| C1 | Task 1 | Frontend fix (`branches` → `branch`) |
| C2 | Tasks 2+3+4+5 | All DB + server changes (atomic) |
| C3 | Task 6 | Integration + DB-level filter tests |
| — | Task 7 | Manual smoke test (no commit) |

C2 is atomic because DB signature changes require server call-site updates for the workspace to compile. Inside C2, we work bottom-up (DB → server) and test per-crate.

---

## Task 1: Fix Frontend Hook Param Name Mismatch

**Files:** `src/hooks/use-dashboard.ts`

The hook sends `branches` (plural) to the API, but the URL param is `branch` (singular) and the backend will expect `branch`. This is a silent bug — the backend ignores unknown params, so it never broke, but project/branch filtering was **never actually sent to the API**.

**Step 1.1: Fix the param name in `fetchDashboardStats`**

In `src/hooks/use-dashboard.ts`, change:
- Line 17: rename parameter `branches` → `branch`
- Line 20: rename `branches` → `branch`, and `'branches'` → `'branch'`
- Line 53: rename parameter `branches` → `branch`
- Line 55: rename `branches` → `branch` in queryKey and queryFn

```typescript
// Line 17 — function signature + body
async function fetchDashboardStats(project?: string, branch?: string, timeRange?: TimeRangeParams): Promise<ExtendedDashboardStats> {
  const params = new URLSearchParams()
  if (project) params.set('project', project)
  if (branch) params.set('branch', branch)  // was: params.set('branches', branches)

// Line 53 — hook signature + body
export function useDashboardStats(project?: string, branch?: string, timeRange?: TimeRangeParams | null) {
  return useQuery({
    queryKey: ['dashboard-stats', project, branch, timeRange?.from, timeRange?.to],
    queryFn: () => fetchDashboardStats(project, branch, timeRange ?? undefined),
```

**Step 1.2: Verify call site is unchanged**

`StatsDashboard.tsx:46-50` already passes `branchFilter` (singular string). No change needed.

**Step 1.3: Run frontend tests**

```bash
cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test -- use-dashboard StatsDashboard --no-coverage 2>&1
```

**Step 1.4: Commit**

```bash
git add src/hooks/use-dashboard.ts
git commit -m "fix(dashboard): rename hook param 'branches' to 'branch' to match URL convention"
```

---

## Task 2: Add project/branch to DB Dashboard Queries (`queries.rs`)

**Files:** `crates/db/src/queries.rs`

This is the bulk of the work. Modify all existing dashboard query functions in-place to accept `project: Option<&str>, branch: Option<&str>`.

**Step 2.1: Update `top_invocables_by_kind` — add sessions join + project/branch**

Change `queries.rs:1172`. Old signature: `async fn top_invocables_by_kind(&self, kind: &str)`. New:

```rust
async fn top_invocables_by_kind(
    &self,
    kind: &str,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<Vec<SkillStat>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT inv.name, COUNT(*) as cnt
        FROM invocations i
        JOIN invocables inv ON i.invocable_id = inv.id
        INNER JOIN sessions s ON i.session_id = s.id
        WHERE inv.kind = ?1 AND s.is_sidechain = 0
          AND (?2 IS NULL OR s.project_id = ?2)
          AND (?3 IS NULL OR s.git_branch = ?3)
        GROUP BY inv.name
        ORDER BY cnt DESC
        LIMIT 10
        "#,
    )
    .bind(kind)
    .bind(project)
    .bind(branch)
    .fetch_all(self.pool())
    .await?;
```

Note: Preserves existing alias convention (`i` = invocations, `inv` = invocables). Adds sessions join + `is_sidechain = 0` (bugfix: aligns with the `_with_range` variant).

**Step 2.2: Update `top_invocables_by_kind_with_range` — add project/branch**

Change `queries.rs:1523`. Old signature: `async fn top_invocables_by_kind_with_range(&self, kind: &str, from: i64, to: i64)`. New:

```rust
async fn top_invocables_by_kind_with_range(
    &self,
    kind: &str,
    from: i64,
    to: i64,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<Vec<SkillStat>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT i.name, COUNT(*) as cnt
        FROM invocations inv
        INNER JOIN invocables i ON inv.invocable_id = i.id
        INNER JOIN sessions s ON inv.session_id = s.id
        WHERE i.kind = ?1 AND s.is_sidechain = 0
          AND s.last_message_at >= ?2 AND s.last_message_at <= ?3
          AND (?4 IS NULL OR s.project_id = ?4)
          AND (?5 IS NULL OR s.git_branch = ?5)
        GROUP BY i.name
        ORDER BY cnt DESC
        LIMIT 10
        "#,
    )
    .bind(kind)
    .bind(from)
    .bind(to)
    .bind(project)
    .bind(branch)
    .fetch_all(self.pool())
    .await?;
```

Note: Preserves existing alias convention (`inv` = invocations, `i` = invocables — yes, swapped from the other function; this is an existing inconsistency we preserve).

**Step 2.3: Update `get_dashboard_stats` — add project/branch to signature + all 7 queries**

Change `queries.rs:1200`. Old signature: `pub async fn get_dashboard_stats(&self)`. New:

```rust
pub async fn get_dashboard_stats(
    &self,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<DashboardStats> {
```

7 queries to update (bind `project`, `branch` after any existing bindings):

1. **Total sessions** (line ~1203):
```sql
SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
```
Bind: `project` as `?1`, `branch` as `?2`.

2. **Total projects** (line ~1208):
```sql
SELECT COUNT(DISTINCT project_id) FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
```

3. **Heatmap 90-day** (line ~1215):
```sql
SELECT date(last_message_at, 'unixepoch') as day, COUNT(*) as cnt FROM sessions
WHERE last_message_at >= ?1 AND is_sidechain = 0
  AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR git_branch = ?3)
GROUP BY day ORDER BY day ASC
```
Bind: `ninety_days_ago` as `?1`, `project` as `?2`, `branch` as `?3`.

4. **Top invocables** (line ~1237-1240):
```rust
let top_skills = self.top_invocables_by_kind("skill", project, branch).await?;
let top_commands = self.top_invocables_by_kind("command", project, branch).await?;
let top_mcp_tools = self.top_invocables_by_kind("mcp_tool", project, branch).await?;
let top_agents = self.top_invocables_by_kind("agent", project, branch).await?;
```

5. **Top 5 projects** (line ~1243):
```sql
SELECT project_id, COALESCE(project_display_name, project_id), COUNT(*) as cnt
FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
GROUP BY project_id ORDER BY cnt DESC LIMIT 5
```

6. **Top 5 longest sessions** (line ~1266):
```sql
SELECT id, preview, project_id, COALESCE(project_display_name, project_id), duration_seconds
FROM sessions WHERE is_sidechain = 0 AND duration_seconds > 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
ORDER BY duration_seconds DESC LIMIT 5
```

7. **Tool totals** (line ~1292):
```sql
SELECT COALESCE(SUM(tool_counts_edit), 0), COALESCE(SUM(tool_counts_read), 0),
       COALESCE(SUM(tool_counts_bash), 0), COALESCE(SUM(tool_counts_write), 0)
FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
```

**Step 2.4: Update `get_dashboard_stats_with_range` — add project/branch to signature + all 7 queries**

Change `queries.rs:1329`. Old signature: `pub async fn get_dashboard_stats_with_range(&self, from: Option<i64>, to: Option<i64>)`. New:

```rust
pub async fn get_dashboard_stats_with_range(
    &self,
    from: Option<i64>,
    to: Option<i64>,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<DashboardStats> {
```

7 queries to update. For queries that currently bind `from` as `?1`, `to` as `?2`:
```sql
AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
```
Bind `project` as `?3`, `branch` as `?4`.

For the heatmap (binds `ninety_days_ago` as `?1` only):
```sql
AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR git_branch = ?3)
```

For top invocables:
```rust
let top_skills = self.top_invocables_by_kind_with_range("skill", from, to, project, branch).await?;
let top_commands = self.top_invocables_by_kind_with_range("command", from, to, project, branch).await?;
let top_mcp_tools = self.top_invocables_by_kind_with_range("mcp_tool", from, to, project, branch).await?;
let top_agents = self.top_invocables_by_kind_with_range("agent", from, to, project, branch).await?;
```

**Step 2.5: Update `get_all_time_metrics` — add project/branch**

Change `queries.rs:1476`. Old signature: `pub async fn get_all_time_metrics(&self)`. New:

```rust
pub async fn get_all_time_metrics(
    &self,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<(u64, u64, u64, u64)> {
```

4 queries to update:

1. **Session count** (line ~1479):
```sql
SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
```

2. **Total tokens** — turns join (line ~1484):
```sql
SELECT COALESCE(SUM(COALESCE(t.input_tokens, 0) + COALESCE(t.output_tokens, 0)), 0)
FROM turns t INNER JOIN sessions s ON t.session_id = s.id
WHERE s.is_sidechain = 0
  AND (?1 IS NULL OR s.project_id = ?1) AND (?2 IS NULL OR s.git_branch = ?2)
```

3. **Total files edited** (line ~1496):
```sql
SELECT COALESCE(SUM(files_edited_count), 0) FROM sessions WHERE is_sidechain = 0
  AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
```

4. **Total commits linked** — session_commits join (line ~1503):
```sql
SELECT COUNT(*) FROM session_commits sc
INNER JOIN sessions s ON sc.session_id = s.id WHERE s.is_sidechain = 0
  AND (?1 IS NULL OR s.project_id = ?1) AND (?2 IS NULL OR s.git_branch = ?2)
```

**Step 2.6: Update `get_oldest_session_date` — add project/branch**

Change `queries.rs:1634`. Old signature: `pub async fn get_oldest_session_date(&self)`. New:

```rust
pub async fn get_oldest_session_date(
    &self,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<Option<i64>> {
    let result: (Option<i64>,) = sqlx::query_as(
        r#"
        SELECT MIN(last_message_at) FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0
          AND (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR git_branch = ?2)
        "#,
    )
    .bind(project)
    .bind(branch)
    .fetch_one(self.pool())
    .await?;
    Ok(result.0)
}
```

**Step 2.7: Update `get_ai_generation_stats` — add project/branch**

Change `queries.rs:1695`. Old signature: `pub async fn get_ai_generation_stats(&self, from: Option<i64>, to: Option<i64>)`. New:

```rust
pub async fn get_ai_generation_stats(
    &self,
    from: Option<i64>,
    to: Option<i64>,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<AIGenerationStats> {
```

3 queries to update. Each currently binds `from` as `?1`, `to` as `?2`:
```sql
AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
```

1. **Aggregate file stats + tokens** (line ~1712): add `?3`/`?4` filter, bind project/branch.
2. **Token usage by model** (line ~1731): add `?3`/`?4` filter, bind project/branch.
3. **Token usage by project** (line ~1764): add `?3`/`?4` filter, bind project/branch.

**Step 2.8: Update ALL existing DB test call sites**

These existing tests call functions whose signatures changed. Update each to pass `None, None`:

| Test function (queries.rs) | Line | Old call | New call |
|---|---|---|---|
| `test_get_dashboard_stats` | 2897 | `db.get_dashboard_stats().await` | `db.get_dashboard_stats(None, None).await` |
| `test_get_dashboard_stats_with_range` | 3160 | `db.get_dashboard_stats_with_range(Some(1500), Some(2500))` | `db.get_dashboard_stats_with_range(Some(1500), Some(2500), None, None)` |
| `test_get_dashboard_stats_with_range` | 3174 | `db.get_dashboard_stats_with_range(None, None)` | `db.get_dashboard_stats_with_range(None, None, None, None)` |
| `test_get_all_time_metrics` | 3272 | `db.get_all_time_metrics().await` | `db.get_all_time_metrics(None, None).await` |
| `test_get_ai_generation_stats` | 3374 | `db.get_ai_generation_stats(None, None).await` | `db.get_ai_generation_stats(None, None, None, None).await` |
| `test_get_ai_generation_stats` | 3412 | `db.get_ai_generation_stats(Some(900), Some(1100))` | `db.get_ai_generation_stats(Some(900), Some(1100), None, None)` |

**Step 2.9: Test DB crate**

```bash
cargo test -p vibe-recall-db -- --no-capture
```

Expected: All existing tests pass (behavior unchanged — `None, None` = no filter).

**Do NOT commit yet** — the server crate won't compile because it still calls old signatures.

---

## Task 3: Add project/branch to Trends Queries (`trends.rs`)

**Files:** `crates/db/src/trends.rs`

**Step 3.1: Update `get_trends_for_periods` — add project/branch to all 12 queries**

Change `trends.rs:199`. Old signature: `async fn get_trends_for_periods(&self, curr_start, curr_end, prev_start, prev_end)`. New:

```rust
async fn get_trends_for_periods(
    &self,
    curr_start: i64,
    curr_end: i64,
    prev_start: i64,
    prev_end: i64,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<WeekTrends> {
```

All 12 queries (6 metrics × 2 periods) currently bind period start as `?1` and period end as `?2`. Add to each:
```sql
AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
```
Bind `project` as `?3`, `branch` as `?4`.

For queries on the sessions table directly (session count, prompts, files_edited, reedited):
```sql
WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2
  AND (?3 IS NULL OR project_id = ?3) AND (?4 IS NULL OR git_branch = ?4)
```

For joins (turns join, session_commits join):
```sql
WHERE s.is_sidechain = 0 AND s.last_message_at >= ?1 AND s.last_message_at <= ?2
  AND (?3 IS NULL OR s.project_id = ?3) AND (?4 IS NULL OR s.git_branch = ?4)
```

**Step 3.2: Update `get_trends_with_range` — thread params through**

Change `trends.rs:174`. Old signature: `pub async fn get_trends_with_range(&self, from: i64, to: i64)`. New:

```rust
pub async fn get_trends_with_range(
    &self,
    from: i64,
    to: i64,
    project: Option<&str>,
    branch: Option<&str>,
) -> DbResult<WeekTrends> {
    let duration = to - from;
    let comp_end = from - 1;
    let comp_start = comp_end - duration;
    self.get_trends_for_periods(from, to, comp_start, comp_end, project, branch).await
}
```

**Step 3.3: Update `get_week_trends` — pass None, None**

Change `trends.rs:191`. No signature change (remains `pub async fn get_week_trends(&self)`). Only update the internal call:

```rust
pub async fn get_week_trends(&self) -> DbResult<WeekTrends> {
    let (curr_start, curr_end) = current_week_bounds();
    let (prev_start, prev_end) = previous_week_bounds();
    self.get_trends_for_periods(curr_start, curr_end, prev_start, prev_end, None, None).await
}
```

**Step 3.4: Test DB crate (trends)**

```bash
cargo test -p vibe-recall-db -- trends --no-capture
```

Expected: All existing trend tests pass (behavior unchanged).

**Do NOT commit yet** — the server crate still won't compile.

---

## Task 4: Update Server Handlers + DashboardQuery (`stats.rs`)

**Files:** `crates/server/src/routes/stats.rs`

**Step 4.1: Add project/branch to DashboardQuery**

Change `stats.rs:23-31`:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DashboardQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// Optional project filter (matches sessions.project_id).
    pub project: Option<String>,
    /// Optional branch filter (matches sessions.git_branch).
    pub branch: Option<String>,
}
```

**Step 4.2: Update `dashboard_stats` handler — thread project/branch to all DB calls**

In `stats.rs`, update the `dashboard_stats` handler (line 172):

1. **data_start_date** (line 194): pass project/branch
```rust
let data_start_date = state.db
    .get_oldest_session_date(query.project.as_deref(), query.branch.as_deref())
    .await.ok().flatten();
```

2. **base stats** (lines 200-204): pass project/branch
```rust
let base = match if has_time_range {
    state.db.get_dashboard_stats_with_range(
        query.from, query.to,
        query.project.as_deref(), query.branch.as_deref(),
    ).await
} else {
    state.db.get_dashboard_stats(
        query.project.as_deref(), query.branch.as_deref(),
    ).await
} {
    // ... error handling unchanged
};
```

3. **trends** (line 231): pass project/branch
```rust
match state.db.get_trends_with_range(
    from, to,
    query.project.as_deref(), query.branch.as_deref(),
).await {
```

4. **all-time metrics** (line 254): pass project/branch
```rust
match state.db.get_all_time_metrics(
    query.project.as_deref(), query.branch.as_deref(),
).await {
```

**Step 4.3: Update `ai_generation_stats` handler — pass project/branch**

In `stats.rs`, update the `ai_generation_stats` handler (line 471):

```rust
match state.db.get_ai_generation_stats(
    query.from, query.to,
    query.project.as_deref(), query.branch.as_deref(),
).await {
```

**Step 4.4: Update `storage_stats` handler — pass None, None to `get_oldest_session_date`**

In `stats.rs`, the `storage_stats` handler (line 339) also calls `get_oldest_session_date`:

```rust
let oldest_session_date = match state.db.get_oldest_session_date(None, None).await {
    Ok(date) => date,
    // ... unchanged
};
```

Storage stats are always global (not filtered by project/branch).

**Step 4.5: Run server tests**

```bash
cargo test -p vibe-recall-server -- --no-capture
```

Expected: All existing tests pass (they send no project/branch query params → DashboardQuery defaults to None → no filter → same behavior).

**Step 4.6: Run full workspace build**

```bash
cargo build 2>&1
```

Expected: Clean compile with zero errors. Every crate compiles.

**Step 4.7: Commit (Tasks 2+3+4 together — atomic)**

```bash
git add crates/db/src/queries.rs crates/db/src/trends.rs crates/server/src/routes/stats.rs
git commit -m "feat(dashboard): add project/branch filter params to all dashboard DB queries and handlers

Thread optional project/branch filter through every dashboard-related DB query
and API handler. Uses SQLite '?N IS NULL OR col = ?N' pattern so a single query
handles both filtered and unfiltered cases.

Modified functions (all in-place, no new functions):
- get_dashboard_stats: 7 sub-queries + 2 helpers
- get_dashboard_stats_with_range: 7 sub-queries + 2 helpers
- get_all_time_metrics: 4 queries
- get_ai_generation_stats: 3 queries
- get_oldest_session_date: 1 query
- get_trends_for_periods: 12 queries (6 metrics × 2 periods)
- get_trends_with_range: pass-through
- top_invocables_by_kind: added sessions join + is_sidechain filter (bugfix)
- top_invocables_by_kind_with_range: added project/branch params
- DashboardQuery: added project/branch fields
- dashboard_stats handler: threads params through
- ai_generation_stats handler: threads params through
- storage_stats handler: passes None/None (global)

Leverages existing idx_sessions_project_branch index."
```

---

## Task 5: (TDD) Write Failing Tests, Then Verify They Pass

**Files:**
- `crates/db/src/queries.rs` (test module)
- `crates/server/src/routes/stats.rs` (test module)

Now that the implementation is in place, we write tests that exercise the filter behavior. These tests would have failed before Task 2 (params were ignored) but should pass now.

**Step 5.1: Write DB-level test for `get_dashboard_stats` with project/branch filter**

Add to the `tests` module in `queries.rs`:

```rust
#[tokio::test]
async fn test_get_dashboard_stats_with_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        duration_seconds: 600,
        ..make_session("sess-filter-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let s2 = SessionInfo {
        git_branch: Some("develop".to_string()),
        duration_seconds: 300,
        ..make_session("sess-filter-b", "proj-y", now - 200)
    };
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter — should see both
    let stats = db.get_dashboard_stats(None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.total_projects, 2);

    // Project filter — should see only proj-x
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_projects, 1);

    // Project + branch filter — matching
    let stats = db.get_dashboard_stats(Some("proj-x"), Some("main")).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Project + wrong branch = 0
    let stats = db.get_dashboard_stats(Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 0);

    // Branch-only filter (no project)
    let stats = db.get_dashboard_stats(None, Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Tool totals should reflect filtered sessions
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.tool_totals.edit, 5);   // make_session sets edit=5

    // Longest sessions should be filtered (duration_seconds > 0, so they appear)
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.longest_sessions.len(), 1, "only proj-x's session");
    assert_eq!(stats.longest_sessions[0].id, "sess-filter-a");

    let stats = db.get_dashboard_stats(Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.longest_sessions.len(), 0, "wrong branch = no sessions");
}
```

**Step 5.2: Write DB-level test for `get_all_time_metrics` with project filter**

```rust
#[tokio::test]
async fn test_get_all_time_metrics_with_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        ..make_session("sess-atm-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let mut s2 = make_session("sess-atm-b", "proj-y", now - 200);
    s2.git_branch = Some("develop".to_string());
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter
    let (sessions, _, _, _) = db.get_all_time_metrics(None, None).await.unwrap();
    assert_eq!(sessions, 2);

    // Project filter
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), None).await.unwrap();
    assert_eq!(sessions, 1);

    // Project + branch filter
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), Some("main")).await.unwrap();
    assert_eq!(sessions, 1);

    // Project + wrong branch
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(sessions, 0);
}
```

**Step 5.3: Write DB-level test for `get_oldest_session_date` with filter**

```rust
#[tokio::test]
async fn test_get_oldest_session_date_with_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    // Insert session_a at now-200, session_b at now-100
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        ..make_session("sess-old-a", "proj-x", now - 200)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let mut s2 = make_session("sess-old-b", "proj-y", now - 100);
    s2.git_branch = Some("develop".to_string());
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter — oldest across all
    let oldest = db.get_oldest_session_date(None, None).await.unwrap();
    assert!(oldest.is_some());

    // Filter proj-y — should get session_b's timestamp
    let oldest = db.get_oldest_session_date(Some("proj-y"), None).await.unwrap();
    assert!(oldest.is_some());

    // Filter non-existent project — should be None
    let oldest = db.get_oldest_session_date(Some("proj-z"), None).await.unwrap();
    assert!(oldest.is_none());
}
```

**Step 5.4: Write DB-level test for `get_dashboard_stats_with_range` with project filter**

```rust
#[tokio::test]
async fn test_get_dashboard_stats_with_range_and_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    // 3 sessions: proj-x at t=1000, proj-x at t=2000, proj-y at t=2000
    let s1 = SessionInfo {
        modified_at: 1000,
        git_branch: Some("main".to_string()),
        ..make_session("sess-rp-1", "proj-x", 1000)
    };
    let s2 = SessionInfo {
        modified_at: 2000,
        git_branch: Some("main".to_string()),
        ..make_session("sess-rp-2", "proj-x", 2000)
    };
    let mut s3 = SessionInfo {
        modified_at: 2000,
        ..make_session("sess-rp-3", "proj-y", 2000)
    };
    s3.git_branch = Some("develop".to_string());

    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();
    db.insert_session(&s2, "proj-x", "Project X").await.unwrap();
    db.insert_session(&s3, "proj-y", "Project Y").await.unwrap();

    // Time range 1500-2500 + no project filter: sess-rp-2 and sess-rp-3
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 2);

    // Time range 1500-2500 + project filter proj-x: only sess-rp-2
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Time range 1500-2500 + project proj-x + branch develop: 0
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 0);
}
```

**Step 5.5: Write API-level integration test for project/branch filtering**

Add to `stats.rs` test module:

```rust
#[tokio::test]
async fn test_dashboard_stats_with_project_filter() {
    let db = test_db().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Insert sessions for two different projects
    let session_a = SessionInfo {
        id: "sess-proj-a".to_string(),
        project: "project-alpha".to_string(),
        project_path: "/home/user/project-alpha".to_string(),
        file_path: "/path/sess-proj-a.jsonl".to_string(),
        modified_at: now - 86400,
        size_bytes: 2048,
        preview: "Alpha session".to_string(),
        last_message: "Test msg A".to_string(),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: ToolCounts { edit: 5, read: 10, bash: 3, write: 2 },
        message_count: 20,
        turn_count: 8,
        summary: None,
        git_branch: Some("main".to_string()),
        is_sidechain: false,
        deep_indexed: false,
        total_input_tokens: Some(10000),
        total_output_tokens: Some(5000),
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: None,
        primary_model: None,
        user_prompt_count: 10,
        api_call_count: 20,
        tool_call_count: 50,
        files_read: vec![],
        files_edited: vec![],
        files_read_count: 15,
        files_edited_count: 5,
        reedited_files_count: 2,
        duration_seconds: 600,
        commit_count: 3,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        api_error_count: 0,
        compaction_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        summary_text: None,
        parse_version: 0,
    };
    db.insert_session(&session_a, "project-alpha", "Project Alpha").await.unwrap();

    let mut session_b = session_a.clone();
    session_b.id = "sess-proj-b".to_string();
    session_b.project = "project-beta".to_string();
    session_b.project_path = "/home/user/project-beta".to_string();
    session_b.file_path = "/path/sess-proj-b.jsonl".to_string();
    session_b.preview = "Beta session".to_string();
    session_b.git_branch = Some("develop".to_string());
    db.insert_session(&session_b, "project-beta", "Project Beta").await.unwrap();

    let app = build_app(db);

    // Filter by project
    let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["totalSessions"], 1, "should only count project-alpha sessions");
    assert_eq!(json["totalProjects"], 1);

    // Filter by project + branch
    let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha&branch=main").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["totalSessions"], 1);

    // Filter by project + wrong branch = 0 sessions
    let (status, body) = do_get(app.clone(), "/api/stats/dashboard?project=project-alpha&branch=develop").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["totalSessions"], 0);

    // No filter — both sessions
    let (status, body) = do_get(app, "/api/stats/dashboard").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["totalSessions"], 2);
}

#[tokio::test]
async fn test_ai_generation_stats_with_project_filter() {
    let db = test_db().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let session_a = SessionInfo {
        id: "sess-aigen-a".to_string(),
        project: "project-alpha".to_string(),
        project_path: "/home/user/project-alpha".to_string(),
        file_path: "/path/sess-aigen-a.jsonl".to_string(),
        modified_at: now - 86400,
        size_bytes: 2048,
        preview: "Alpha AI".to_string(),
        last_message: "msg".to_string(),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: ToolCounts::default(),
        message_count: 10,
        turn_count: 5,
        summary: None,
        git_branch: Some("main".to_string()),
        is_sidechain: false,
        deep_indexed: false,
        total_input_tokens: None,
        total_output_tokens: None,
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: None,
        primary_model: None,
        user_prompt_count: 5,
        api_call_count: 10,
        tool_call_count: 20,
        files_read: vec![],
        files_edited: vec![],
        files_read_count: 5,
        files_edited_count: 3,
        reedited_files_count: 0,
        duration_seconds: 300,
        commit_count: 0,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        api_error_count: 0,
        compaction_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        summary_text: None,
        parse_version: 0,
    };
    db.insert_session(&session_a, "project-alpha", "Project Alpha").await.unwrap();

    let app = build_app(db);

    // Filter by project
    let (status, body) = do_get(app.clone(), "/api/stats/ai-generation?project=project-alpha").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["filesCreated"], 3);

    // Filter by non-existent project = 0
    let (status, body) = do_get(app, "/api/stats/ai-generation?project=project-nope").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["filesCreated"], 0);
}
```

**Step 5.6: Write DB-level test for `get_trends_with_range` with project/branch filter**

```rust
#[tokio::test]
async fn test_get_trends_with_range_and_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let from = now - 7 * 86400;
    let to = now;

    // proj-x session within range
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        user_prompt_count: 5,
        files_edited_count: 3,
        reedited_files_count: 1,
        ..make_session("sess-trend-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    // proj-y session within range
    let s2 = SessionInfo {
        git_branch: Some("develop".to_string()),
        user_prompt_count: 10,
        files_edited_count: 6,
        reedited_files_count: 2,
        ..make_session("sess-trend-b", "proj-y", now - 200)
    };
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter — trends include both sessions
    let trends = db.get_trends_with_range(from, to, None, None).await.unwrap();
    assert_eq!(trends.session_count.current, 2);
    assert_eq!(trends.total_files_edited.current, 9); // 3 + 6

    // Project filter — only proj-x
    let trends = db.get_trends_with_range(from, to, Some("proj-x"), None).await.unwrap();
    assert_eq!(trends.session_count.current, 1);
    assert_eq!(trends.total_files_edited.current, 3);

    // Project + branch filter
    let trends = db.get_trends_with_range(from, to, Some("proj-x"), Some("main")).await.unwrap();
    assert_eq!(trends.session_count.current, 1);

    // Project + wrong branch = 0
    let trends = db.get_trends_with_range(from, to, Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(trends.session_count.current, 0);
    assert_eq!(trends.total_files_edited.current, 0);
}
```

**Step 5.7: Run all tests**

```bash
cargo test -p vibe-recall-db -- --no-capture
cargo test -p vibe-recall-server -- --no-capture
```

Expected: All tests pass, including the new filter-behavior tests.

**Step 5.8: Commit**

```bash
git add crates/db/src/queries.rs crates/db/src/trends.rs crates/server/src/routes/stats.rs
git commit -m "test: add project/branch filter tests for dashboard queries and API endpoints

DB-level tests:
- test_get_dashboard_stats_with_project_filter: 7 filter combinations inc. longest_sessions
- test_get_all_time_metrics_with_project_filter: 4 filter combinations
- test_get_oldest_session_date_with_filter: global, filtered, non-existent
- test_get_dashboard_stats_with_range_and_project_filter: time+project combos
- test_get_trends_with_range_and_project_filter: 4 filter combinations across 12 queries

API-level tests:
- test_dashboard_stats_with_project_filter: project, project+branch, wrong branch, no filter
- test_ai_generation_stats_with_project_filter: project, non-existent project"
```

---

## Task 6: Full Build + Frontend Smoke Test

**Files:** None to modify — verification only.

**Step 6.1: Full workspace build**

```bash
cargo build 2>&1
```

Expected: Zero errors, zero warnings related to our changes.

**Step 6.2: Run ALL tests across both crates**

```bash
cargo test -p vibe-recall-db --no-capture
cargo test -p vibe-recall-server --no-capture
```

Expected: All pass.

**Step 6.3: Start dev server and verify in browser**

```bash
cargo build -p vibe-recall-server && cargo run -p vibe-recall-server
```

In another terminal:
```bash
cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun run dev
```

**Manual verification checklist:**

1. Open `http://localhost:5173`
2. Click a project in the sidebar → dashboard updates to show only that project's stats
3. Click a branch under the project → dashboard narrows further
4. Check "Most Active Projects" card reflects the filter
5. Click "Clear" on the project filter → dashboard returns to global view
6. Navigate to Sessions tab → filter persists (existing behavior)
7. Navigate back to Fluency tab → filter still active

**Step 6.4: Verify network requests**

Open DevTools → Network. When a project is selected, the dashboard API call should show:
```
/api/stats/dashboard?project=project-name&branch=main&from=...&to=...
```

Confirm the response JSON has filtered data (not global totals).

---

## Summary

| Task | What | Commit |
|------|------|--------|
| 1 | Fix frontend hook param `branches` → `branch` | C1 |
| 2 | Add project/branch to all DB queries in `queries.rs` (~24 queries + 2 helpers) | — |
| 3 | Add project/branch to all trend queries in `trends.rs` (12 queries) | — |
| 4 | Add project/branch to `DashboardQuery` + thread through all handlers | C2 (atomic with T2+T3) |
| 5 | Write filter-behavior tests (DB-level + API-level) | C3 |
| 6 | Full build + smoke test | — |

**Totals:** ~38 queries modified, 0 new functions, 0 dead code, 0 new dependencies, 0 migrations, 1 frontend file changed. Every commit compiles the full workspace and passes all tests.
