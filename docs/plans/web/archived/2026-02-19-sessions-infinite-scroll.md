---
status: approved
date: 2026-02-19
---

# Sessions Infinite Scroll Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the client-side "load all 5000 sessions" approach with server-side filtered pagination and frontend infinite scroll.

**Architecture:** Push all filtering/sorting/pagination into SQL via a new `query_sessions_filtered()` DB method. The route handler becomes a thin pass-through. Frontend uses `useInfiniteQuery` with an IntersectionObserver sentinel to load 30 sessions per page on scroll. The ActivitySparkline is removed from HistoryView (deferred to a future lightweight counts endpoint).

**Tech Stack:** Rust/sqlx `QueryBuilder` for dynamic SQL, React Query `useInfiniteQuery`, IntersectionObserver API

---

### Task 1: DB — Add `SessionFilterParams` struct and `query_sessions_filtered()` method

**Files:**
- Modify: `crates/db/src/queries/dashboard.rs`

**Step 1: Define the filter params struct**

Add at the top of `dashboard.rs`, after the existing imports:

```rust
/// Parameters for filtered, paginated session queries.
/// All fields are optional — omitted fields apply no filter.
pub struct SessionFilterParams {
    pub q: Option<String>,
    pub branches: Option<Vec<String>>,
    pub models: Option<Vec<String>>,
    pub has_commits: Option<bool>,
    pub has_skills: Option<bool>,
    pub min_duration: Option<i64>,
    pub min_files: Option<i64>,
    pub min_tokens: Option<i64>,
    pub high_reedit: Option<bool>,
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
    pub sort: String,       // "recent", "tokens", "prompts", "files_edited", "duration"
    pub limit: i64,         // default 30
    pub offset: i64,        // default 0
}
```

**Step 2: Implement `query_sessions_filtered()`**

Add inside the `impl Database` block, after `list_all_sessions()`:

```rust
/// Query sessions with server-side filtering, sorting, and pagination.
///
/// Returns (sessions, total_matching_count).
/// Uses sqlx::QueryBuilder for safe dynamic WHERE clauses.
pub async fn query_sessions_filtered(
    &self,
    params: &SessionFilterParams,
) -> DbResult<(Vec<SessionInfo>, usize)> {
    // --- Shared WHERE clause builder ---
    // We build the WHERE fragment once and use it for both COUNT and SELECT.

    let select_cols = r#"
        s.id, s.project_id, s.preview, s.turn_count,
        s.last_message_at, s.file_path,
        s.project_path, s.project_display_name,
        s.size_bytes, s.last_message, s.files_touched, s.skills_used,
        s.tool_counts_edit, s.tool_counts_read, s.tool_counts_bash, s.tool_counts_write,
        s.message_count,
        COALESCE(s.summary_text, s.summary) AS summary,
        s.git_branch, s.is_sidechain, s.deep_indexed_at,
        s.total_input_tokens,
        s.total_output_tokens,
        s.cache_read_tokens AS total_cache_read_tokens,
        s.cache_creation_tokens AS total_cache_creation_tokens,
        s.api_call_count AS turn_count_api,
        s.primary_model,
        s.user_prompt_count, s.api_call_count, s.tool_call_count,
        s.files_read, s.files_edited,
        s.files_read_count, s.files_edited_count, s.reedited_files_count,
        s.duration_seconds, s.first_message_at, s.commit_count,
        s.thinking_block_count, s.turn_duration_avg_ms, s.turn_duration_max_ms,
        s.api_error_count, s.compaction_count, s.agent_spawn_count,
        s.bash_progress_count, s.hook_progress_count, s.mcp_progress_count,
        s.lines_added, s.lines_removed, s.loc_source,
        s.summary_text, s.parse_version,
        s.category_l1, s.category_l2, s.category_l3,
        s.category_confidence, s.category_source, s.classified_at,
        s.prompt_word_count, s.correction_count, s.same_file_edit_count
    "#;

    // Helper closure: appends all WHERE clauses to a QueryBuilder.
    // Called twice — once for COUNT(*), once for SELECT.
    fn append_filters<'args>(
        qb: &mut sqlx::QueryBuilder<'args, sqlx::Sqlite>,
        params: &'args SessionFilterParams,
    ) {
        qb.push(" WHERE s.is_sidechain = 0");

        // Text search
        if let Some(q) = &params.q {
            let pattern = format!("%{}%", q);
            qb.push(" AND (s.preview LIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR s.last_message LIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR s.project_display_name LIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        // Branch filter (IN list)
        if let Some(branches) = &params.branches {
            if !branches.is_empty() {
                qb.push(" AND s.git_branch IN (");
                let mut sep = qb.separated(", ");
                for b in branches {
                    sep.push_bind(b.as_str());
                }
                sep.push_unseparated(")");
            }
        }

        // Model filter (IN list)
        if let Some(models) = &params.models {
            if !models.is_empty() {
                qb.push(" AND s.primary_model IN (");
                let mut sep = qb.separated(", ");
                for m in models {
                    sep.push_bind(m.as_str());
                }
                sep.push_unseparated(")");
            }
        }

        // has_commits
        if let Some(has) = params.has_commits {
            if has {
                qb.push(" AND s.commit_count > 0");
            } else {
                qb.push(" AND s.commit_count = 0");
            }
        }

        // has_skills — skills_used is a JSON array string, '[]' means empty
        if let Some(has) = params.has_skills {
            if has {
                qb.push(" AND s.skills_used != '[]' AND s.skills_used != ''");
            } else {
                qb.push(" AND (s.skills_used = '[]' OR s.skills_used = '')");
            }
        }

        // min_duration
        if let Some(min) = params.min_duration {
            qb.push(" AND s.duration_seconds >= ");
            qb.push_bind(min);
        }

        // min_files
        if let Some(min) = params.min_files {
            qb.push(" AND s.files_edited_count >= ");
            qb.push_bind(min);
        }

        // min_tokens (input + output)
        if let Some(min) = params.min_tokens {
            qb.push(" AND (COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0)) >= ");
            qb.push_bind(min);
        }

        // high_reedit (reedit rate > 0.2)
        if let Some(true) = params.high_reedit {
            qb.push(" AND s.files_edited_count > 0 AND CAST(s.reedited_files_count AS REAL) / s.files_edited_count > 0.2");
        }

        // time_after
        if let Some(after) = params.time_after {
            qb.push(" AND s.last_message_at >= ");
            qb.push_bind(after);
        }

        // time_before
        if let Some(before) = params.time_before {
            qb.push(" AND s.last_message_at <= ");
            qb.push_bind(before);
        }
    }

    // --- COUNT query ---
    let mut count_qb = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM sessions s");
    append_filters(&mut count_qb, params);

    let total: (i64,) = count_qb
        .build_query_as()
        .fetch_one(self.pool())
        .await?;
    let total = total.0 as usize;

    // --- DATA query ---
    let mut data_qb = sqlx::QueryBuilder::new(format!("SELECT {} FROM sessions s", select_cols));
    append_filters(&mut data_qb, params);

    // ORDER BY
    match params.sort.as_str() {
        "tokens" => data_qb.push(" ORDER BY (COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0)) DESC"),
        "prompts" => data_qb.push(" ORDER BY s.user_prompt_count DESC"),
        "files_edited" => data_qb.push(" ORDER BY s.files_edited_count DESC"),
        "duration" => data_qb.push(" ORDER BY s.duration_seconds DESC"),
        _ => data_qb.push(" ORDER BY s.last_message_at DESC"), // "recent"
    };

    // LIMIT + OFFSET
    data_qb.push(" LIMIT ");
    data_qb.push_bind(params.limit);
    data_qb.push(" OFFSET ");
    data_qb.push_bind(params.offset);

    let rows: Vec<SessionRow> = data_qb
        .build_query_as()
        .fetch_all(self.pool())
        .await?;

    let sessions = rows
        .into_iter()
        .map(|r| {
            let pid = r.project_id.clone();
            r.into_session_info(&pid)
        })
        .collect();

    Ok((sessions, total))
}
```

**Step 3: Run existing tests to verify no regression**

Run: `cargo test -p claude-view-db`
Expected: All existing tests pass (new code is additive, no existing code modified).

**Step 4: Commit**

```bash
git add crates/db/src/queries/dashboard.rs
git commit -m "feat(db): add query_sessions_filtered with dynamic SQL filtering"
```

---

### Task 2: DB — Write tests for `query_sessions_filtered`

**Files:**
- Modify: `crates/db/src/queries/dashboard.rs` (add tests at bottom, or create new test module)

**Step 1: Write tests for the new DB method**

Add to the test module in `crates/db/` (or create `crates/db/src/queries/dashboard_tests.rs` if preferred). These tests verify the SQL filtering works at the DB layer:

```rust
#[cfg(test)]
mod filtered_query_tests {
    use super::*;
    use crate::Database;
    use claude_view_core::{SessionInfo, ToolCounts};

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            file_path: format!("/path/{}.jsonl", id),
            modified_at,
            size_bytes: 2048,
            preview: format!("Preview for {}", id),
            last_message: format!("Last message for {}", id),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(10),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
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
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
        }
    }

    fn default_params() -> SessionFilterParams {
        SessionFilterParams {
            q: None,
            branches: None,
            models: None,
            has_commits: None,
            has_skills: None,
            min_duration: None,
            min_files: None,
            min_tokens: None,
            high_reedit: None,
            time_after: None,
            time_before: None,
            sort: "recent".to_string(),
            limit: 30,
            offset: 0,
        }
    }

    #[tokio::test]
    async fn test_no_filters_returns_all() {
        let db = test_db().await;
        for i in 0..5 {
            let s = make_session(&format!("s-{i}"), "proj", 1700000000 + i);
            db.insert_session(&s, "proj", "Project").await.unwrap();
        }
        let (sessions, total) = db.query_sessions_filtered(&default_params()).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(sessions.len(), 5);
    }

    #[tokio::test]
    async fn test_pagination_limit_offset() {
        let db = test_db().await;
        for i in 0..10 {
            let s = make_session(&format!("s-{i}"), "proj", 1700000000 + i);
            db.insert_session(&s, "proj", "Project").await.unwrap();
        }
        let params = SessionFilterParams { limit: 3, offset: 2, ..default_params() };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 10);
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_text_search() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.preview = "Fix authentication bug".to_string();
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.preview = "Add new feature".to_string();
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams { q: Some("auth".to_string()), ..default_params() };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_branch_filter() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.git_branch = Some("main".to_string());
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.git_branch = Some("feature/auth".to_string());
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            branches: Some(vec!["main".to_string()]),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_has_commits_filter() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.commit_count = 3;
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let s2 = make_session("s-2", "proj", 1700000001);
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams { has_commits: Some(true), ..default_params() };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-1");
    }

    #[tokio::test]
    async fn test_time_range_filter() {
        let db = test_db().await;
        let s1 = make_session("s-1", "proj", 1700000000);
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let s2 = make_session("s-2", "proj", 1720000000);
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams {
            time_after: Some(1710000000),
            ..default_params()
        };
        let (sessions, total) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(sessions[0].id, "s-2");
    }

    #[tokio::test]
    async fn test_sort_by_duration() {
        let db = test_db().await;
        let mut s1 = make_session("s-1", "proj", 1700000000);
        s1.duration_seconds = 100;
        db.insert_session(&s1, "proj", "Project").await.unwrap();

        let mut s2 = make_session("s-2", "proj", 1700000001);
        s2.duration_seconds = 5000;
        db.insert_session(&s2, "proj", "Project").await.unwrap();

        let params = SessionFilterParams { sort: "duration".to_string(), ..default_params() };
        let (sessions, _) = db.query_sessions_filtered(&params).await.unwrap();
        assert_eq!(sessions[0].id, "s-2"); // longest first
    }
}
```

**Step 2: Run the new tests**

Run: `cargo test -p claude-view-db -- filtered_query_tests`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add crates/db/src/queries/dashboard.rs
git commit -m "test(db): add tests for query_sessions_filtered"
```

---

### Task 3: Route — Add `q` param, `has_more` to response, refactor `list_sessions` handler

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

**Step 1: Add `q` to `SessionsListQuery`**

Add after the existing fields in `SessionsListQuery`:

```rust
/// Text search across preview, last_message, project name
pub q: Option<String>,
```

**Step 2: Add `has_more` to `SessionsListResponse`**

```rust
pub struct SessionsListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub has_more: bool,    // NEW
    pub filter: String,
    pub sort: String,
}
```

**Step 3: Refactor `list_sessions` handler**

Replace the body of `list_sessions` with a thin pass-through to the DB method:

```rust
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionsListQuery>,
) -> ApiResult<Json<SessionsListResponse>> {
    let filter = query.filter.unwrap_or_else(|| "all".to_string());
    let sort = query.sort.unwrap_or_else(|| "recent".to_string());
    let limit = query.limit.unwrap_or(30);
    let offset = query.offset.unwrap_or(0);

    // Validate filter (kept for backward compat — legacy single-value filter)
    if !VALID_FILTERS.contains(&filter.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid filter '{}'. Valid options: {}",
            filter,
            VALID_FILTERS.join(", ")
        )));
    }

    // Validate sort
    if !VALID_SORTS.contains(&sort.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid sort '{}'. Valid options: {}",
            sort,
            VALID_SORTS.join(", ")
        )));
    }

    // Map legacy filter param to the new structured params
    let has_commits = match (query.has_commits, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "has_commits") => Some(true),
        _ => None,
    };
    let high_reedit = match (query.high_reedit, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "high_reedit") => Some(true),
        _ => None,
    };
    let min_duration = match (query.min_duration, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "long_session") => Some(1800),
        _ => None,
    };

    let params = claude_view_db::SessionFilterParams {
        q: query.q,
        branches: query.branches.map(|s| s.split(',').map(|b| b.trim().to_string()).collect()),
        models: query.models.map(|s| s.split(',').map(|m| m.trim().to_string()).collect()),
        has_commits,
        has_skills: query.has_skills,
        min_duration,
        min_files: query.min_files,
        min_tokens: query.min_tokens,
        high_reedit,
        time_after: query.time_after,
        time_before: query.time_before,
        sort: sort.clone(),
        limit,
        offset,
    };

    let (sessions, total) = state.db.query_sessions_filtered(&params).await?;
    let has_more = (offset + limit) < total as i64;

    Ok(Json(SessionsListResponse {
        sessions,
        total,
        has_more,
        filter,
        sort,
    }))
}
```

**Step 4: Re-export `SessionFilterParams` from the db crate's public API**

In `crates/db/src/lib.rs`, add:

```rust
pub use queries::dashboard::SessionFilterParams;
```

(Or wherever the `pub mod queries` re-export chain is.)

**Step 5: Run tests**

Run: `cargo test -p claude-view-server -- routes::sessions`
Expected: Existing tests pass. The `test_list_sessions_pagination` test should still work since `limit` and `offset` are preserved.

**Step 6: Regenerate TypeScript types**

Run: `cargo test -p claude-view-server` (ts-rs generates types on test)
Then verify `src/types/generated/SessionsListResponse.ts` includes `hasMore: boolean`.

**Step 7: Commit**

```bash
git add crates/server/src/routes/sessions.rs crates/db/src/lib.rs src/types/generated/SessionsListResponse.ts
git commit -m "feat(server): refactor list_sessions to use server-side SQL filtering"
```

---

### Task 4: Frontend — Create `useSessionsInfinite` hook

**Files:**
- Create: `src/hooks/use-sessions-infinite.ts`

**Step 1: Create the hook**

```typescript
// src/hooks/use-sessions-infinite.ts
import { useInfiniteQuery } from '@tanstack/react-query'
import type { SessionsListResponse } from '../types/generated'
import type { SessionFilters } from './use-session-filters'

const PAGE_SIZE = 30

interface SessionsQueryParams {
  filters: SessionFilters
  search: string
  timeAfter?: number
  timeBefore?: number
  sidebarProject?: string | null
  sidebarBranch?: string | null
}

function buildSearchParams(params: SessionsQueryParams, offset: number): URLSearchParams {
  const sp = new URLSearchParams()
  sp.set('limit', String(PAGE_SIZE))
  sp.set('offset', String(offset))
  sp.set('sort', params.filters.sort)

  if (params.search) sp.set('q', params.search)

  // Merge sidebar branch with filter branches
  const branches = [...params.filters.branches]
  if (params.sidebarBranch && !branches.includes(params.sidebarBranch)) {
    branches.push(params.sidebarBranch)
  }
  if (branches.length > 0) sp.set('branches', branches.join(','))

  if (params.filters.models.length > 0) sp.set('models', params.filters.models.join(','))

  if (params.filters.hasCommits === 'yes') sp.set('has_commits', 'true')
  if (params.filters.hasCommits === 'no') sp.set('has_commits', 'false')

  if (params.filters.hasSkills === 'yes') sp.set('has_skills', 'true')
  if (params.filters.hasSkills === 'no') sp.set('has_skills', 'false')

  if (params.filters.minDuration !== null) sp.set('min_duration', String(params.filters.minDuration))
  if (params.filters.minFiles !== null) sp.set('min_files', String(params.filters.minFiles))
  if (params.filters.minTokens !== null) sp.set('min_tokens', String(params.filters.minTokens))
  if (params.filters.highReedit === true) sp.set('high_reedit', 'true')

  if (params.timeAfter) sp.set('time_after', String(params.timeAfter))
  if (params.timeBefore) sp.set('time_before', String(params.timeBefore))

  return sp
}

async function fetchSessionsPage(
  params: SessionsQueryParams,
  offset: number,
): Promise<SessionsListResponse> {
  const sp = buildSearchParams(params, offset)
  const response = await fetch(`/api/sessions?${sp}`)
  if (!response.ok) throw new Error('Failed to fetch sessions')
  return response.json()
}

export function useSessionsInfinite(params: SessionsQueryParams) {
  return useInfiniteQuery({
    queryKey: ['sessions-infinite', params],
    queryFn: ({ pageParam }) => fetchSessionsPage(params, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, _allPages, lastPageParam) => {
      if (!lastPage.hasMore) return undefined
      return lastPageParam + PAGE_SIZE
    },
    // Flatten all pages into a single sessions array for convenience
    select: (data) => ({
      sessions: data.pages.flatMap(p => p.sessions),
      total: data.pages[0]?.total ?? 0,
    }),
  })
}
```

**Step 2: Commit**

```bash
git add src/hooks/use-sessions-infinite.ts
git commit -m "feat(hooks): add useSessionsInfinite with server-side pagination"
```

---

### Task 5: Frontend — Add `useDebounce` hook

**Files:**
- Create: `src/hooks/use-debounce.ts`

**Step 1: Create the hook**

```typescript
// src/hooks/use-debounce.ts
import { useState, useEffect } from 'react'

export function useDebounce<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)

  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs)
    return () => clearTimeout(timer)
  }, [value, delayMs])

  return debounced
}
```

**Step 2: Commit**

```bash
git add src/hooks/use-debounce.ts
git commit -m "feat(hooks): add useDebounce utility hook"
```

---

### Task 6: Frontend — Refactor HistoryView to use infinite scroll

**Files:**
- Modify: `src/components/HistoryView.tsx`

This is the largest task. The key changes:

1. Replace `useAllSessions` with `useSessionsInfinite`
2. Remove ~80 lines of client-side filtering logic
3. Add IntersectionObserver sentinel at bottom
4. Remove `ActivitySparkline` (it needs all sessions, which we no longer load)
5. Keep grouping logic (applied to accumulated pages)
6. Debounce search text before sending to server

**Step 1: Update imports**

Replace:
```typescript
import { useProjectSummaries, useAllSessions } from '../hooks/use-projects'
```

With:
```typescript
import { useProjectSummaries } from '../hooks/use-projects'
import { useSessionsInfinite } from '../hooks/use-sessions-infinite'
import { useDebounce } from '../hooks/use-debounce'
```

Remove the `ActivitySparkline` import.

**Step 2: Replace data fetching and filtering**

Replace the data-fetching block (lines ~82-258 of current HistoryView) with:

```typescript
export function HistoryView() {
  const navigate = useNavigate()
  const { data: summaries } = useProjectSummaries()

  // URL-persisted filter/sort state
  const [searchParams, setSearchParams] = useSearchParams()
  const [filters, setFilters] = useSessionFilters(searchParams, setSearchParams)

  const { state: timeRange, setPreset, setCustomRange } = useTimeRange()
  const isMobile = useIsMobile()

  const { data: classifyStatus } = useQuery({
    queryKey: ['classify-status'],
    queryFn: async () => {
      const res = await fetch('/api/classify/status')
      if (!res.ok) return null
      return res.json() as Promise<ClassifyStatusResponse>
    },
    staleTime: 30_000,
  })

  const [searchText, setSearchText] = useState('')
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const sentinelRef = useRef<HTMLDivElement>(null)

  // Debounce search text (300ms) so we don't fire a request per keystroke
  const debouncedSearch = useDebounce(searchText, 300)

  // Sidebar global filters from URL
  const sidebarProject = searchParams.get('project') || null
  const sidebarBranch = searchParams.get('branch') || null

  // Server-side filtered + paginated query
  const {
    data,
    isLoading,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = useSessionsInfinite({
    filters,
    search: debouncedSearch,
    timeAfter: timeRange.fromTimestamp ?? undefined,
    timeBefore: undefined, // Could add if custom range has end
    sidebarProject,
    sidebarBranch,
  })

  const sessions = data?.sessions ?? []
  const total = data?.total ?? 0

  // Detect deep-link context
  const hasDeepLinkSort = filters.sort !== 'recent'
  const hasDeepLinkFilter = filters.hasCommits !== 'any' || filters.hasSkills !== 'any' || filters.highReedit !== null || filters.minDuration !== null
  const hasDeepLink = hasDeepLinkSort || hasDeepLinkFilter

  // Focus search on mount (only if not deep-linked)
  useEffect(() => {
    if (!hasDeepLink) {
      searchRef.current?.focus()
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // IntersectionObserver for infinite scroll
  useEffect(() => {
    const sentinel = sentinelRef.current
    if (!sentinel) return

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && hasNextPage && !isFetchingNextPage) {
          fetchNextPage()
        }
      },
      { rootMargin: '200px' } // trigger 200px before reaching bottom
    )

    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  // Map project display names
  const projectDisplayNames = useMemo(() => {
    if (!summaries) return new Map<string, string>()
    const map = new Map<string, string>()
    for (const s of summaries) {
      map.set(s.name, s.displayName)
    }
    return map
  }, [summaries])

  // Extract branches/models from loaded sessions for filter popover
  // NOTE: These are only from loaded pages, not all sessions.
  // For a complete list, a dedicated /api/branches endpoint exists.
  const availableBranches = useMemo(() => {
    const set = new Set<string>()
    for (const s of sessions) {
      if (s.gitBranch) set.add(s.gitBranch)
    }
    return [...set].sort()
  }, [sessions])

  const availableModels = useMemo(() => {
    const set = new Set<string>()
    for (const s of sessions) {
      if (s.primaryModel) set.add(s.primaryModel)
    }
    return [...set].sort()
  }, [sessions])

  const isFiltered = debouncedSearch || sidebarProject || sidebarBranch || timeRange.preset !== '30d' || selectedDate || filters.sort !== 'recent' || filters.hasCommits !== 'any' || filters.hasSkills !== 'any' || filters.highReedit !== null || filters.minDuration !== null || filters.minFiles !== null || filters.minTokens !== null || filters.branches.length > 0 || filters.models.length > 0
```

**Step 3: Update grouping logic**

The grouping still works — it operates on the accumulated `sessions` array. Keep the existing grouping code but reference `sessions` instead of `filteredSessions`:

```typescript
  const tooManyToGroup = shouldDisableGrouping(sessions.length)

  // ... existing groupByAutoReset logic (reference sessions.length) ...

  const groups = useMemo(() => {
    if (filters.groupBy !== 'none' && !tooManyToGroup) {
      return groupSessions(sessions, filters.groupBy)
    }
    return filters.sort === 'recent' ? groupSessionsByDate(sessions) : [{ label: SORT_LABELS[filters.sort], sessions }]
  }, [sessions, filters.groupBy, filters.sort, tooManyToGroup])
```

**Step 4: Remove ActivitySparkline from render**

Remove the `<ActivitySparkline ... />` block from the JSX.

**Step 5: Add sentinel div and loading indicator at bottom of session list**

After the session list (after the `groups.map(...)` block), add:

```tsx
{/* Infinite scroll sentinel */}
<div ref={sentinelRef} className="h-1" />
{isFetchingNextPage && (
  <div className="flex justify-center py-4">
    <div className="w-5 h-5 border-2 border-gray-300 dark:border-gray-600 border-t-gray-600 dark:border-t-gray-300 rounded-full animate-spin" />
  </div>
)}
{!hasNextPage && sessions.length > 0 && (
  <div className="text-center py-4 text-xs text-gray-400">
    All {total} sessions loaded
  </div>
)}
```

**Step 6: Update the filter summary count**

Replace:
```tsx
<span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
  {filteredSessions.length} of {allSessions.length}
</span>
```

With:
```tsx
<span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
  {total} sessions
</span>
```

**Step 7: Update `clearAll` function**

```typescript
function clearAll() {
  setSearchText('')
  setPreset('all')
  setSelectedDate(null)
  setFilters(DEFAULT_FILTERS)
}
```

(Same as before — no change needed.)

**Step 8: Verify the table view also works**

The `CompactSessionTable` component receives `sessions` as a prop — it just renders what it's given. With infinite scroll, it will render the accumulated pages. The table `onSort` callback changes `filters.sort`, which triggers a new server query (the `useSessionsInfinite` queryKey includes filters).

**Step 9: Run dev server and verify**

Run: `bun run dev`
Open browser to the sessions/history page.
Expected:
- Initial load shows ~30 sessions
- Scrolling to bottom loads 30 more
- Search box debounces and filters server-side
- Filter popover works (branch, model, duration, etc.)
- Sort dropdown works
- "All X sessions loaded" appears at bottom when fully scrolled

**Step 10: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "feat(ui): refactor HistoryView to infinite scroll with server-side pagination"
```

---

### Task 7: Frontend — Populate filter popovers from /api/branches endpoint

**Files:**
- Modify: `src/components/HistoryView.tsx`
- Modify: `src/hooks/use-projects.ts` (or create a small hook)

The filter popover needs a complete list of branches and models, not just from loaded pages. The `/api/branches` endpoint already exists.

**Step 1: Use the existing branches endpoint**

In HistoryView, replace the `availableBranches` useMemo with a query:

```typescript
const { data: availableBranches = [] } = useQuery({
  queryKey: ['all-branches'],
  queryFn: async () => {
    const res = await fetch('/api/branches')
    if (!res.ok) return []
    return res.json() as Promise<string[]>
  },
  staleTime: 60_000,
})
```

For models, add a similar endpoint if needed, or keep the client-side extraction from loaded sessions (acceptable for MVP since model diversity is low).

**Step 2: Commit**

```bash
git add src/components/HistoryView.tsx
git commit -m "feat(ui): use /api/branches for filter popover instead of client-side extraction"
```

---

### Task 8: Wiring verification — end-to-end test

**Files:** None (manual verification)

**Step 1: Start the dev server**

Run: `bun run dev`

**Step 2: Verify the full pipeline**

Checklist:
- [ ] Sessions page loads first 30 sessions on mount
- [ ] Scrolling down loads next batch (observe network tab for `?offset=30`)
- [ ] Search box debounces (type, wait 300ms, see network request with `q=...`)
- [ ] Branch filter works (select branch, sessions update from server)
- [ ] Sort dropdown works (change sort, sessions re-fetch from page 1)
- [ ] "X sessions" count matches `total` from API response
- [ ] Changing filters resets to page 1 (not appending to existing)
- [ ] Table view works with infinite scroll
- [ ] Empty state shows when no results match
- [ ] No console errors

**Step 3: Run all existing tests**

Run: `cargo test -p claude-view-server -- routes::sessions`
Run: `cargo test -p claude-view-db`

Expected: All tests pass.

**Step 4: Final commit**

If any fixups were needed during verification, commit them now.

---

### Task 9: Cleanup — Remove unused code

**Files:**
- Modify: `src/hooks/use-projects.ts` — remove `fetchAllSessions` and `useAllSessions` if no other consumers exist
- Modify: `src/components/HistoryView.tsx` — remove any dead imports

**Step 1: Check for other consumers of `useAllSessions`**

Run: `grep -rn "useAllSessions" src/`

If only `HistoryView` used it (which is the case as of this writing), remove:
- `fetchAllSessions()` function
- `useAllSessions()` hook

**Step 2: Remove unused imports from HistoryView**

Remove `ActivitySparkline` import and any other dead imports.

**Step 3: Commit**

```bash
git add src/hooks/use-projects.ts src/components/HistoryView.tsx
git commit -m "refactor: remove unused useAllSessions hook and ActivitySparkline import"
```
