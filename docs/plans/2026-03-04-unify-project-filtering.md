# Unify Project Filtering Across All Pages

**Status:** DONE (2026-03-04) — all 5 tasks implemented, shippable audit passed (SHIP IT)
**Estimated scope:** ~30 lines across 4 files, zero breaking changes
**Branch:** `worktree-monorepo-expo`

## Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `102f3b87` | feat(db): add `project` field to SessionFilterParams |
| 2 | `77e214cc` | feat(server): wire `project` query param to sessions endpoint |
| 3 | `0e1a6139` | feat(web): wire sidebarProject into sessions API call |
| 4 | `752018d6` | refactor(web): move activity data project filter client→server |
| 5 | `3a906264` | fix(db): handle NO_BRANCH sentinel in branch filter SQL |

Shippable audit: 4/4 passes green. 2130 tests (429 DB + 537 server + 1164 frontend), 0 failures. Full-stack wiring verified.

## Problem

The sidebar project filter (`?project=X`) is silently ignored on the Sessions History page. The `/api/sessions` endpoint has no `project` query parameter. Each page handles project filtering differently — some server-side, some client-side, some not at all.

## Root Cause

`SessionFilterParams` (the DB query struct) and `SessionsListQuery` (the API query struct) both lack a `project` field. The frontend's `useSessionsInfinite` hook accepts `sidebarProject` but never sends it to the API.

## Key Architectural Context

- The sidebar sends `ProjectSummary.name` as `?project=`, which is `COALESCE(NULLIF(git_root, ''), project_id)` — the "effective project identity"
- This value can match either `project_id` OR `git_root` in the sessions table
- A proven worktree-aware pattern already exists in `query_project_sessions()` at `crates/db/src/queries/dashboard.rs:109`:
  ```sql
  (s.project_id = ?1 OR (s.git_root IS NOT NULL AND s.git_root != '' AND s.git_root = ?1))
  ```
- Both `project_id` and `git_root` columns are already indexed
- Smart search (`/api/search`) and grep (`/api/grep`) are completely separate code paths — unaffected

## Tasks

### Task 1: Add `project` field to `SessionFilterParams` (backend DB layer)

**File:** `crates/db/src/queries/dashboard.rs`

1. Add `pub project: Option<String>` to the `SessionFilterParams` struct (~line 15), after `time_before`
2. In `append_filters()` (~line 299), add the worktree-aware project clause after the time_before block (~line 405):
   ```rust
   // Project filter (worktree-aware: match project_id OR git_root)
   if let Some(ref project) = params.project {
       qb.push(" AND (s.project_id = ");
       qb.push_bind(project.as_str());
       qb.push(" OR (s.git_root IS NOT NULL AND s.git_root != '' AND s.git_root = ");
       qb.push_bind(project.as_str());
       qb.push("))");
   }
   ```
3. Update `default_params()` test helper (~line 1043) to include `project: None`

### Task 2: Add `project` query param to `/api/sessions` endpoint (backend route)

**File:** `crates/server/src/routes/sessions.rs`

1. Add `pub project: Option<String>` to `SessionsListQuery` struct (~line 33), with doc comment `/// Optional project filter (matches project_id or git_root)`
2. In `list_sessions()` handler (~line 292), add `project: query.project,` to the `SessionFilterParams` construction

### Task 3: Wire `sidebarProject` into API call (frontend)

**File:** `apps/web/src/hooks/use-sessions-infinite.ts`

In `buildSearchParams()` (~line 20), add after the search param block (~line 26):
```typescript
// Project filter (server-side)
if (params.sidebarProject) sp.set('project', params.sidebarProject)
```

### Task 4: Remove client-side project filter from activity data (frontend)

**File:** `apps/web/src/hooks/use-activity-data.ts`

1. In the fetch function, add `project` to the API call: `if (sidebarProject) sp.set('project', sidebarProject)` (near the existing `branches` param)
2. Add `sidebarProject` to the `queryKey` array so project changes trigger refetch
3. Remove the client-side filter block (~lines 101-106):
   ```typescript
   // DELETE THIS:
   if (sidebarProject) {
     sessions = sessions.filter(
       (s) => ((s.gitRoot || null) ?? s.projectPath ?? s.project) === sidebarProject,
     )
   }
   ```

### Task 5: Fix `NO_BRANCH` sentinel bug (backend, bonus fix)

**File:** `crates/db/src/queries/dashboard.rs`

In `append_filters()`, the branch filter (~line 331) currently puts literal `~` in an SQL `IN` clause. Fix it to handle the `NO_BRANCH` sentinel (`~`) by emitting `git_branch IS NULL`:

```rust
if let Some(branches) = &params.branches {
    if !branches.is_empty() {
        let has_no_branch = branches.iter().any(|b| b == "~");
        let named: Vec<&str> = branches.iter().filter(|b| b.as_str() != "~").map(|b| b.as_str()).collect();

        if has_no_branch && named.is_empty() {
            qb.push(" AND s.git_branch IS NULL");
        } else if has_no_branch {
            qb.push(" AND (s.git_branch IS NULL OR s.git_branch IN (");
            let mut sep = qb.separated(", ");
            for b in &named {
                sep.push_bind(*b);
            }
            sep.push_unseparated("))");
        } else {
            qb.push(" AND s.git_branch IN (");
            let mut sep = qb.separated(", ");
            for b in branches {
                sep.push_bind(b.as_str());
            }
            sep.push_unseparated(")");
        }
    }
}
```

Note: Check if `~` is the correct sentinel value — verify in `apps/web/src/lib/constants.ts` (`NO_BRANCH`).

## Verification

1. `cargo test -p claude-view-db` — DB layer tests (Task 1, 5)
2. `cargo test -p claude-view-server` — Route handler tests (Task 2)
3. `cd apps/web && bunx vitest run` — Frontend tests (Task 3, 4)
4. Manual: Start dev server (`bun dev`), select a project in sidebar, verify Sessions History page only shows sessions from that project
5. Manual: Click "(no branch)" filter, verify it returns sessions with null branch (not zero results)

## Non-goals (don't do these)

- InsightsPage project filtering (separate feature gap, separate PR)
- Renaming `projectId` to `project` in contributions API (cosmetic, separate PR)
- Changing search/grep endpoints (completely separate architecture)
