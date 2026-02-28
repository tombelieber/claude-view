# Work Reports Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Reports page that generates ultra-lean AI work summaries from session data, streamed live via SSE.

**Architecture:** New `reports` DB table + aggregate preview query. New `crates/core/src/report.rs` for context digest builder + prompt template. New `crates/server/src/routes/reports.rs` with 5 endpoints (streaming generate via Claude CLI stdout pipe, list, get, delete, preview). React page with time-aware dual cards, streaming markdown, and two-layer output (AI bullets + raw stats).

**Tech Stack:** Rust (Axum, sqlx, tokio), Claude CLI (--print flag, streaming stdout), SSE, React, TypeScript, ts-rs

**Design doc:** `docs/plans/2026-02-21-work-reports-design.md`

---

### Task 1: DB Migration — reports table

**Files:**
- Modify: `crates/db/src/migrations.rs` (append Migration 25)

**Step 1: Write the migration test**

Add to `crates/db/src/migrations.rs` in the `tests` module:

```rust
#[tokio::test]
async fn test_migration25_reports_table_exists() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM pragma_table_info('reports')"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(column_names.contains(&"report_type"), "Missing report_type column");
    assert!(column_names.contains(&"date_start"), "Missing date_start column");
    assert!(column_names.contains(&"date_end"), "Missing date_end column");
    assert!(column_names.contains(&"content_md"), "Missing content_md column");
    assert!(column_names.contains(&"context_digest"), "Missing context_digest column");
    assert!(column_names.contains(&"session_count"), "Missing session_count column");
    assert!(column_names.contains(&"project_count"), "Missing project_count column");
    assert!(column_names.contains(&"total_duration_secs"), "Missing total_duration_secs column");
    assert!(column_names.contains(&"total_cost_cents"), "Missing total_cost_cents column");
    assert!(column_names.contains(&"generation_ms"), "Missing generation_ms column");
    assert!(column_names.contains(&"created_at"), "Missing created_at column");
}

#[tokio::test]
async fn test_migration25_reports_check_constraints() {
    let pool = setup_db().await;

    // Valid report_type should work
    let result = sqlx::query(
        "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('daily', '2026-02-21', '2026-02-21', '- Shipped search', 8, 3, 15120, 680)"
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "Valid report_type 'daily' should be accepted");

    // Invalid report_type should fail
    let result = sqlx::query(
        "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('invalid', '2026-02-21', '2026-02-21', 'test', 0, 0, 0, 0)"
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "Invalid report_type should be rejected");
}

#[tokio::test]
async fn test_migration25_reports_indexes() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_reports%'"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();
    assert!(index_names.contains(&"idx_reports_date"), "Missing idx_reports_date index");
    assert!(index_names.contains(&"idx_reports_type"), "Missing idx_reports_type index");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_migration25 -- --nocapture`
Expected: FAIL — `reports` table does not exist

**Step 3: Write the migration**

Append to `MIGRATIONS` array in `crates/db/src/migrations.rs`:

```rust
// Migration 25: Work Reports table
r#"
CREATE TABLE IF NOT EXISTS reports (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    report_type         TEXT NOT NULL,
    date_start          TEXT NOT NULL,
    date_end            TEXT NOT NULL,
    content_md          TEXT NOT NULL,
    context_digest      TEXT,
    session_count       INTEGER NOT NULL,
    project_count       INTEGER NOT NULL,
    total_duration_secs INTEGER NOT NULL,
    total_cost_cents    INTEGER NOT NULL,
    generation_ms       INTEGER,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    CONSTRAINT valid_report_type CHECK (report_type IN ('daily', 'weekly', 'custom'))
);
"#,
r#"CREATE INDEX IF NOT EXISTS idx_reports_date ON reports(date_start, date_end);"#,
r#"CREATE INDEX IF NOT EXISTS idx_reports_type ON reports(report_type);"#,
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-db test_migration25 -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs
git commit -m "feat(db): add reports table (migration 25)"
```

---

### Task 2: DB Queries — reports CRUD + preview

**Files:**
- Create: `crates/db/src/queries/reports.rs`
- Modify: `crates/db/src/queries/mod.rs` (add `pub mod reports;`)
- Modify: `crates/db/src/lib.rs` (re-export types)

**Step 1: Write the failing tests**

Create `crates/db/src/queries/reports.rs`:

```rust
//! Report CRUD queries and preview aggregation.

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A saved report row.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    #[ts(type = "number")]
    pub id: i64,
    pub report_type: String,
    pub date_start: String,
    pub date_end: String,
    pub content_md: String,
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number")]
    pub project_count: i64,
    #[ts(type = "number")]
    pub total_duration_secs: i64,
    #[ts(type = "number")]
    pub total_cost_cents: i64,
    #[ts(type = "number | null")]
    pub generation_ms: Option<i64>,
    pub created_at: String,
}

/// Preview stats for a date range (no AI, pure DB aggregation).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportPreview {
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number")]
    pub project_count: i64,
    #[ts(type = "number")]
    pub total_duration_secs: i64,
    #[ts(type = "number")]
    pub total_cost_cents: i64,
    pub projects: Vec<ProjectPreview>,
}

/// Per-project summary in the preview.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreview {
    pub name: String,
    #[ts(type = "number")]
    pub session_count: i64,
}

impl Database {
    /// Insert a new report and return its id.
    pub async fn insert_report(
        &self,
        report_type: &str,
        date_start: &str,
        date_end: &str,
        content_md: &str,
        context_digest: Option<&str>,
        session_count: i64,
        project_count: i64,
        total_duration_secs: i64,
        total_cost_cents: i64,
        generation_ms: Option<i64>,
    ) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO reports (report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#,
        )
        .bind(report_type)
        .bind(date_start)
        .bind(date_end)
        .bind(content_md)
        .bind(context_digest)
        .bind(session_count)
        .bind(project_count)
        .bind(total_duration_secs)
        .bind(total_cost_cents)
        .bind(generation_ms)
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }

    /// List all reports, newest first.
    pub async fn list_reports(&self) -> DbResult<Vec<ReportRow>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, i64, i64, i64, i64, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at FROM reports ORDER BY created_at DESC"
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at)| ReportRow {
                id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at,
            })
            .collect())
    }

    /// Get a single report by id.
    pub async fn get_report(&self, id: i64) -> DbResult<Option<ReportRow>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, String, i64, i64, i64, i64, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at FROM reports WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at)| ReportRow {
            id, report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at,
        }))
    }

    /// Delete a report by id. Returns true if a row was deleted.
    pub async fn delete_report(&self, id: i64) -> DbResult<bool> {
        let result = sqlx::query("DELETE FROM reports WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Aggregate preview stats for sessions in a date range.
    ///
    /// Uses `first_message_at` (unix timestamp) for filtering.
    /// `start_ts` and `end_ts` are unix timestamps for the range bounds.
    pub async fn get_report_preview(&self, start_ts: i64, end_ts: i64) -> DbResult<ReportPreview> {
        // Aggregate stats
        let stats: (i64, i64, i64, i64) = sqlx::query_as(
            r#"SELECT
                COUNT(*) as session_count,
                COUNT(DISTINCT project_display_name) as project_count,
                COALESCE(SUM(duration_seconds), 0) as total_duration,
                0 as total_cost
            FROM sessions
            WHERE first_message_at >= ? AND first_message_at <= ?"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_one(self.pool())
        .await?;

        // Per-project breakdown
        let project_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT project_display_name, COUNT(*) as cnt
               FROM sessions
               WHERE first_message_at >= ? AND first_message_at <= ?
               GROUP BY project_display_name
               ORDER BY cnt DESC"#,
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_all(self.pool())
        .await?;

        let projects = project_rows
            .into_iter()
            .map(|(name, session_count)| ProjectPreview { name, session_count })
            .collect();

        Ok(ReportPreview {
            session_count: stats.0,
            project_count: stats.1,
            total_duration_secs: stats.2,
            total_cost_cents: stats.3,
            projects,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn test_insert_and_get_report() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db
            .insert_report("daily", "2026-02-21", "2026-02-21", "- Shipped search", None, 8, 3, 15120, 680, Some(14200))
            .await
            .unwrap();
        assert!(id > 0);

        let report = db.get_report(id).await.unwrap().unwrap();
        assert_eq!(report.report_type, "daily");
        assert_eq!(report.content_md, "- Shipped search");
        assert_eq!(report.session_count, 8);
    }

    #[tokio::test]
    async fn test_list_reports_newest_first() {
        let db = Database::new_in_memory().await.unwrap();
        db.insert_report("daily", "2026-02-20", "2026-02-20", "day 1", None, 5, 2, 3600, 100, None).await.unwrap();
        db.insert_report("daily", "2026-02-21", "2026-02-21", "day 2", None, 8, 3, 7200, 200, None).await.unwrap();

        let reports = db.list_reports().await.unwrap();
        assert_eq!(reports.len(), 2);
        // Newest first
        assert_eq!(reports[0].date_start, "2026-02-21");
    }

    #[tokio::test]
    async fn test_delete_report() {
        let db = Database::new_in_memory().await.unwrap();
        let id = db.insert_report("weekly", "2026-02-17", "2026-02-21", "week summary", None, 32, 5, 64800, 2450, None).await.unwrap();

        assert!(db.delete_report(id).await.unwrap());
        assert!(db.get_report(id).await.unwrap().is_none());
        assert!(!db.delete_report(id).await.unwrap()); // already deleted
    }

    #[tokio::test]
    async fn test_get_report_preview_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let preview = db.get_report_preview(0, i64::MAX).await.unwrap();
        assert_eq!(preview.session_count, 0);
        assert_eq!(preview.project_count, 0);
        assert!(preview.projects.is_empty());
    }

    #[tokio::test]
    async fn test_get_report_preview_with_sessions() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert test sessions
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, duration_seconds) VALUES ('s1', 'p1', '/tmp/s1.jsonl', 'Test', 'claude-view', 1000, 3600)"
        ).execute(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, duration_seconds) VALUES ('s2', 'p1', '/tmp/s2.jsonl', 'Test', 'claude-view', 1100, 1800)"
        ).execute(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, duration_seconds) VALUES ('s3', 'p2', '/tmp/s3.jsonl', 'Test', 'vicky-wiki', 1200, 900)"
        ).execute(db.pool()).await.unwrap();

        let preview = db.get_report_preview(0, 2000).await.unwrap();
        assert_eq!(preview.session_count, 3);
        assert_eq!(preview.project_count, 2);
        assert_eq!(preview.total_duration_secs, 6300);
        assert_eq!(preview.projects.len(), 2);
        // claude-view should be first (most sessions)
        assert_eq!(preview.projects[0].name, "claude-view");
        assert_eq!(preview.projects[0].session_count, 2);
    }
}
```

**Step 2: Register the module**

Add `pub mod reports;` to `crates/db/src/queries/mod.rs`.

Re-export in `crates/db/src/lib.rs`:
```rust
pub use queries::reports::{ReportRow, ReportPreview, ProjectPreview};
```

**Step 3: Run tests**

Run: `cargo test -p claude-view-db reports -- --nocapture`
Expected: PASS (all 5 tests)

**Step 4: Commit**

```bash
git add crates/db/src/queries/reports.rs crates/db/src/queries/mod.rs crates/db/src/lib.rs
git commit -m "feat(db): add reports CRUD queries and preview aggregation"
```

---

### Task 3: Core — Context Digest Builder

**Files:**
- Create: `crates/core/src/report.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod report;`)

**Step 1: Write the failing test first**

Create `crates/core/src/report.rs` with types, `to_prompt_text()`, and `build_report_prompt()`. Include comprehensive tests for prompt generation.

Key types:
- `ReportType` enum (Daily, Weekly, Custom)
- `ContextDigest` struct with `projects`, `top_tools`, `top_skills`, `summary_line`
- `ProjectDigest`, `BranchDigest`, `SessionDigest` structs
- `to_prompt_text(&self) -> String` formats the structured text block
- `build_report_prompt(digest: &ContextDigest) -> String` wraps with system prompt

Test that:
1. `to_prompt_text()` contains all project names, branch names, session prompts
2. `build_report_prompt()` contains the "5-8 bullet points" instruction
3. Empty digest produces a valid but minimal prompt
4. Long prompts get truncated (session first_prompt capped at 100 chars)

**Step 2: Register the module**

Add `pub mod report;` to `crates/core/src/lib.rs`.

**Step 3: Run tests**

Run: `cargo test -p claude-view-core report -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/core/src/report.rs crates/core/src/lib.rs
git commit -m "feat(core): add context digest builder and report prompt template"
```

---

### Task 4: Core — Streaming CLI Method

**Files:**
- Modify: `crates/core/src/llm/types.rs` (add streaming types if needed)
- Modify: `crates/core/src/llm/claude_cli.rs` (add `stream_completion()`)
- Modify: `crates/core/src/llm/provider.rs` (add trait method)

**Step 1: Add `stream_completion()` to ClaudeCliProvider**

This is the key new capability. Unlike `complete()` which waits for the full response, `stream_completion()` spawns the CLI and returns a `tokio::sync::mpsc::Receiver<String>` that yields text chunks as they arrive from stdout.

```rust
/// Spawn Claude CLI and stream stdout chunks via a channel.
///
/// Returns (receiver, join_handle). The receiver yields text chunks.
/// When the CLI exits, the channel closes.
pub fn stream_completion(
    &self,
    prompt: String,
) -> Result<(tokio::sync::mpsc::Receiver<String>, tokio::task::JoinHandle<Result<(), LlmError>>), LlmError> {
    // ... strip env vars, spawn with -p --model, pipe stdout via BufReader
}
```

Use `tokio::io::BufReader` on the child's stdout, `lines()` stream, send each line through the mpsc channel.

**Step 2: Write a unit test** that verifies the method compiles and the channel types are correct. (Integration test with real CLI is manual.)

**Step 3: Run tests**

Run: `cargo test -p claude-view-core llm -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/core/src/llm/
git commit -m "feat(core): add stream_completion() to ClaudeCliProvider for SSE piping"
```

---

### Task 5: Server — Reports Route

**Files:**
- Create: `crates/server/src/routes/reports.rs`
- Modify: `crates/server/src/routes/mod.rs` (add `pub mod reports;` and register)

**Step 1: Build the route module**

5 endpoints:

1. `POST /api/reports/generate` — Accepts JSON body `{reportType, dateStart, dateEnd}`. Queries DB for sessions in range, builds ContextDigest, spawns Claude CLI via `stream_completion()`, returns SSE stream with `chunk` and `done`/`error` events. On completion, persists to DB. Uses `AtomicBool` guard to prevent concurrent generation.

2. `GET /api/reports` — Returns `Vec<ReportRow>` JSON.

3. `GET /api/reports/:id` — Returns single `ReportRow` or 404.

4. `DELETE /api/reports/:id` — Deletes report, returns 204 or 404.

5. `GET /api/reports/preview` — Query params `?start_ts=...&end_ts=...`. Returns `ReportPreview` JSON.

Context digest assembly (in the generate handler):
- Query sessions in range with `first_message_at` filter
- Group by `project_display_name` then `git_branch`
- Include `preview` (first prompt), `category_l2` (classification), `duration_seconds`
- Query `invocations` for top tools/skills in range
- Query `session_commits` for commit counts per project
- Build `ContextDigest` and call `build_report_prompt()`

**Step 2: Register in mod.rs**

Add to `crates/server/src/routes/mod.rs`:
```rust
pub mod reports;
```

And in `api_routes()`:
```rust
.nest("/api", reports::router())
```

**Step 3: Write route tests**

Test: list empty, insert+list, get by id, delete, preview with/without sessions. SSE generate endpoint is an integration test (needs CLI).

**Step 4: Run tests**

Run: `cargo test -p claude-view-server reports -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/server/src/routes/reports.rs crates/server/src/routes/mod.rs
git commit -m "feat(server): add /api/reports endpoints with SSE streaming generation"
```

---

### Task 6: Frontend — Types and Hooks

**Files:**
- Create: `src/types/generated/ReportRow.ts` (auto-generated via `cargo test` with ts-rs)
- Create: `src/types/generated/ReportPreview.ts`
- Create: `src/types/generated/ProjectPreview.ts`
- Create: `src/hooks/use-reports.ts`
- Create: `src/hooks/use-report-preview.ts`
- Create: `src/hooks/use-report-generate.ts`
- Create: `src/hooks/use-smart-defaults.ts`

**Step 1: Generate TS types**

Run `cargo test` in `claude-view-db` to trigger ts-rs export of `ReportRow`, `ReportPreview`, `ProjectPreview`.

**Step 2: Create hooks**

`use-reports.ts` — SWR fetch of `GET /api/reports`. Returns `{ data, isLoading, error, mutate }`.

`use-report-preview.ts` — SWR fetch of `GET /api/reports/preview?start_ts=...&end_ts=...`. Accepts `startTs` and `endTs` as params. Returns `{ data: ReportPreview, isLoading, error }`.

`use-report-generate.ts` — The key hook. Returns `{ generate, isGenerating, streamedText, report, error }`. The `generate()` function POSTs to `/api/reports/generate`, reads the SSE response stream using `fetch()` + `ReadableStream`, accumulates `streamedText` from `chunk` events, and sets `report` on `done`. Calls `mutate()` from `use-reports` to refresh the list.

`use-smart-defaults.ts` — Pure client logic. Returns `{ primary, secondary }` card configs based on current time and preview data. Logic:
- Morning + today < 2 sessions → primary = yesterday, secondary = today
- Monday morning → primary = last week, secondary = today
- Else → primary = today, secondary = this week

**Step 3: Run type check**

Run: `bunx tsc --noEmit`
Expected: PASS (no type errors)

**Step 4: Commit**

```bash
git add src/types/generated/ src/hooks/use-reports.ts src/hooks/use-report-preview.ts src/hooks/use-report-generate.ts src/hooks/use-smart-defaults.ts
git commit -m "feat(frontend): add report hooks (preview, generate with SSE streaming, smart defaults)"
```

---

### Task 7: Frontend — Components

**Files:**
- Create: `src/components/reports/ReportCard.tsx`
- Create: `src/components/reports/ReportContent.tsx`
- Create: `src/components/reports/ReportDetails.tsx`
- Create: `src/components/reports/ReportHistory.tsx`

**Step 1: Build ReportCard**

4 states: PREVIEW, STREAMING, COMPLETE, EMPTY.

- PREVIEW: Shows date label, preview stats from `useReportPreview`, "Generate Report" button
- STREAMING: Shows date label, streaming markdown (use existing markdown renderer), disabled "Generating..." button with spinner
- COMPLETE: Shows rendered markdown content, Copy/Export/Redo buttons, expandable Details section
- EMPTY: Shows "No sessions" message with nudge to nearest useful range

Props: `{ label: string, dateStart: string, dateEnd: string, type: 'daily'|'weekly', existingReport?: ReportRow }`

**Step 2: Build ReportContent**

Renders markdown content (reuse existing markdown renderer from ConversationView). Copy button copies raw markdown to clipboard. Export button downloads `.md` file. Redo button triggers re-generation.

**Step 3: Build ReportDetails**

Expandable section (collapsed by default). Shows raw stats: cost, tokens, top tools, per-project session counts. Data comes from `ReportPreview` (no AI, pure DB stats).

**Step 4: Build ReportHistory**

Simple list below the two active cards. Renders `ReportRow[]` from `useReports()`. Click loads report into main display. Shows date, type badge, relative time.

**Step 5: Run type check**

Run: `bunx tsc --noEmit`
Expected: PASS

**Step 6: Commit**

```bash
git add src/components/reports/
git commit -m "feat(frontend): add ReportCard, ReportContent, ReportDetails, ReportHistory components"
```

---

### Task 8: Frontend — Page, Routing, Sidebar

**Files:**
- Create: `src/pages/ReportsPage.tsx`
- Modify: `src/router.tsx` (add route)
- Modify: `src/components/Sidebar.tsx` (add nav item)

**Step 1: Build ReportsPage**

Composes the two ReportCards (primary/secondary from `useSmartDefaults`) + ReportHistory. Handles state where clicking a history item shows that report in the main area.

**Step 2: Add route**

In `src/router.tsx`, add to children array:
```tsx
{ path: 'reports', element: <ReportsPage /> },
```

Import: `import { ReportsPage } from './pages/ReportsPage'`

**Step 3: Add sidebar nav item**

In `src/components/Sidebar.tsx`, add a nav item with `FileText` icon from lucide-react, linking to `/reports`. Place it between Analytics and Settings.

**Step 4: Run type check + dev server**

Run: `bunx tsc --noEmit`
Run: `bun run dev` — navigate to `/reports`, verify page renders

**Step 5: Commit**

```bash
git add src/pages/ReportsPage.tsx src/router.tsx src/components/Sidebar.tsx
git commit -m "feat(frontend): add Reports page with routing and sidebar navigation"
```

---

### Task 9: Wiring Verification

**MANDATORY per CLAUDE.md wiring checklist.**

**Step 1: Verify backend wiring**

- [ ] `reports::router()` is registered in `api_routes()` in `crates/server/src/routes/mod.rs`
- [ ] Migration 25 is appended to `MIGRATIONS` array (not a separate file)
- [ ] `pub mod reports;` in `crates/db/src/queries/mod.rs`
- [ ] Re-exports in `crates/db/src/lib.rs`

**Step 2: Verify frontend wiring**

- [ ] Route `{ path: 'reports', ... }` in `src/router.tsx`
- [ ] Sidebar nav item links to `/reports`
- [ ] Hooks import API base URL correctly (same pattern as other hooks)

**Step 3: Full compilation check**

Run: `cargo check --workspace`
Run: `bunx tsc --noEmit`

**Step 4: End-to-end manual test**

1. `bun run dev:server` (start Rust backend)
2. `bun run dev` (start Vite frontend)
3. Navigate to `http://localhost:5173/reports`
4. Verify preview cards show session counts
5. Click "Generate Report" on a card with sessions
6. Verify SSE stream renders markdown live
7. Verify report appears in history after completion
8. Verify Copy button copies markdown to clipboard
9. Verify Delete removes from history

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: wire up Reports page end-to-end"
```

---

## Task Dependency Graph

```
Task 1 (DB Migration)
  └── Task 2 (DB Queries)
        └── Task 5 (Server Routes)
              └── Task 6 (Frontend Hooks)
                    └── Task 7 (Frontend Components)
                          └── Task 8 (Page + Routing)
                                └── Task 9 (Wiring Verification)

Task 3 (Core Digest) ──┐
Task 4 (Core Stream) ──┘── Task 5 (Server Routes)
```

Tasks 1-4 can be parallelized (1+2 sequential, 3+4 sequential, but both pairs independent).
Tasks 5-9 are sequential.
