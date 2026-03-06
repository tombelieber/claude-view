# Session Archive & Resizable Sidebar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users archive (soft-delete) sessions from history with undo, and make all three sidebar sections (Tabs, Scope, Recent) resizable and collapsible VS Code-style.

**Architecture:** Two independent features. Feature 1 adds an `archived_at` column + DB flag + file-move API + context menu / bulk-select UI with undo toast. Feature 2 replaces the sidebar's static flex layout with `react-resizable-panels` for draggable/collapsible sections persisted to localStorage.

**Tech Stack:** Rust (Axum, sqlx), React, `react-resizable-panels`, `@radix-ui/react-context-menu`, `sonner` (existing), Zustand (existing).

**Design doc:** `docs/plans/2026-03-06-session-archive-resizable-sidebar-design.md`

**Rollback strategy:** Revert migration 50 by re-creating the `valid_sessions` view without the `archived_at IS NULL` filter, then `ALTER TABLE sessions DROP COLUMN archived_at`. Frontend changes are purely additive — revert the commits.

---

## Feature 1: Session Archive

### Task 1: Add `archived_at` column and update `valid_sessions` view

**Files:**
- Modify: `crates/db/src/migrations.rs` (append to `MIGRATIONS` array)

**Step 1: Write Migration 50**

The `MIGRATIONS` array is a `&[&str]` — each migration is a raw string literal. There is **no `LATEST_MIGRATION` constant** — the migration version is derived from the array index + 1. Append a new entry at the end of the array (currently ends at ~line 714 with `];`).

Because this migration has multiple statements (ALTER TABLE + DROP VIEW + CREATE VIEW), it **must** be wrapped in `BEGIN;...COMMIT;` — otherwise `sqlx::query()` only executes the first statement and silently drops the rest. The migration test runner detects `BEGIN;` to switch to `raw_sql()`.

Insert before the closing `];` of the `MIGRATIONS` array:

```rust
// Migration 50: Add archived_at column for session archiving
r#"BEGIN;
ALTER TABLE sessions ADD COLUMN archived_at TEXT;
DROP VIEW IF EXISTS valid_sessions;
CREATE VIEW valid_sessions AS SELECT * FROM sessions WHERE is_sidechain = 0 AND archived_at IS NULL;
COMMIT;"#,
```

**Important:** Do NOT look for a `LATEST_MIGRATION` constant — it does not exist. Just append to the array.

**Step 2: Fix the migration 49 test**

The existing test `test_migration_49_removes_cost_estimate_cents` at ~line 1182 uses `MIGRATIONS.last()` to get migration 49. After appending migration 50, `MIGRATIONS.last()` returns migration 50 instead. Fix the test:

```rust
// BEFORE (line 1182-1183):
// Migration 49 is the last entry in MIGRATIONS.
let migration_49 = super::MIGRATIONS.last().expect("missing migration 49");

// AFTER:
// Migration 49 is at index 48 (0-indexed).
let migration_49 = &super::MIGRATIONS[48];
```

**Step 3: Run migration test**

Run: `cargo test -p claude-view-db migrations`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/db/src/migrations.rs
git commit -m "feat(db): add archived_at column and update valid_sessions view (migration 50)"
```

---

### Task 1b: Audit and fix direct `FROM sessions` queries that bypass `valid_sessions`

**Context:** The `valid_sessions` view now filters out archived sessions, but **many queries use `FROM sessions` directly** and will still show archived sessions. Every non-test, non-admin query that shows sessions to the user must either use `valid_sessions` or add `AND archived_at IS NULL`.

**Files to audit:**
- `crates/db/src/queries/dashboard.rs` — `list_sessions_for_project` (~line 133/173), any `FROM sessions` usage
- `crates/db/src/queries/contributions.rs` — ~5 queries use `FROM sessions`
- `crates/db/src/queries/stats.rs` — `FROM sessions` (~line 219)
- `crates/db/src/queries/insights.rs` — `FROM sessions` (~line 451)
- `crates/db/src/queries/trends.rs` — `FROM sessions` (~line 43)
- `crates/db/src/queries/git_correlation.rs` — any `FROM sessions`
- `crates/db/src/queries/classification.rs` — any `FROM sessions`
- `crates/db/src/queries/sessions.rs` — 10+ queries use `FROM sessions`

**Step 1: Find all direct `FROM sessions` usage**

```bash
grep -rn "FROM sessions" crates/db/src/queries/ --include="*.rs" | grep -v "valid_sessions" | grep -v "FROM sessions_" | grep -v "#\[cfg(test)\]" | grep -v "// test"
```

**Step 2: For each query, decide:**
- If it's a user-facing list/aggregate → switch to `FROM valid_sessions` OR add `AND s.archived_at IS NULL`
- If it's an internal/admin query (migration, indexer, archive handler itself) → leave as `FROM sessions`
- If it's a test → leave as `FROM sessions`

**Step 3: Apply fixes**

For most queries, the simplest fix is replacing `FROM sessions s` with `FROM valid_sessions s` — the view already has `is_sidechain = 0 AND archived_at IS NULL`. This is safe because `valid_sessions` is a simple `SELECT *` view.

**Step 4: Test**

Run: `cargo test -p claude-view-db`
Expected: PASS (tests use `FROM sessions` directly and should still work since test sessions won't have `archived_at` set)

**Step 5: Commit**

```bash
git add crates/db/src/queries/
git commit -m "fix(db): switch user-facing queries from sessions to valid_sessions to exclude archived"
```

---

### Task 2: Add archive/unarchive DB operations

**Files:**
- Modify: `crates/db/src/queries/sessions.rs`
- Add test: `crates/db/tests/` (integration test file, matching existing test patterns)

**Step 1: Write failing tests**

Tests in this codebase use `#[tokio::test]` with `Database::new_in_memory().await`. They do **NOT** use `#[sqlx::test]` or bare `SqlitePool` parameters.

Create a test file at `crates/db/tests/archive_sessions.rs`:

```rust
use claude_view_db::Database;

#[tokio::test]
async fn test_archive_session() {
    let db = Database::new_in_memory().await.unwrap();
    // Insert a test session via raw SQL (matching sessions table schema)
    sqlx::query(
        "INSERT INTO sessions (id, file_path, is_sidechain) VALUES ('test-1', '/tmp/test.jsonl', 0)"
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive it
    let result = db.archive_session("test-1").await.unwrap();
    assert_eq!(result, Some("/tmp/test.jsonl".to_string()));

    // Verify archived_at is set
    let archived: Option<(String,)> = sqlx::query_as(
        "SELECT archived_at FROM sessions WHERE id = 'test-1'"
    )
    .fetch_optional(db.pool())
    .await
    .unwrap();
    assert!(archived.is_some());

    // Verify session no longer appears in valid_sessions
    let in_view: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM valid_sessions WHERE id = 'test-1'"
    )
    .fetch_optional(db.pool())
    .await
    .unwrap();
    assert!(in_view.is_none());

    // Archive again should return None (already archived)
    let result2 = db.archive_session("test-1").await.unwrap();
    assert_eq!(result2, None);
}

#[tokio::test]
async fn test_unarchive_session() {
    let db = Database::new_in_memory().await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, file_path, is_sidechain) VALUES ('test-2', '/tmp/test2.jsonl', 0)"
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Archive then unarchive
    db.archive_session("test-2").await.unwrap();
    let result = db.unarchive_session("test-2", "/tmp/restored.jsonl").await.unwrap();
    assert!(result);

    // Verify archived_at is NULL and file_path updated
    let row: (Option<String>, String) = sqlx::query_as(
        "SELECT archived_at, file_path FROM sessions WHERE id = 'test-2'"
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(row.0.is_none());
    assert_eq!(row.1, "/tmp/restored.jsonl");

    // Verify session reappears in valid_sessions
    let in_view: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM valid_sessions WHERE id = 'test-2'"
    )
    .fetch_optional(db.pool())
    .await
    .unwrap();
    assert!(in_view.is_some());
}

#[tokio::test]
async fn test_bulk_archive() {
    let db = Database::new_in_memory().await.unwrap();
    for i in 1..=5 {
        sqlx::query(
            "INSERT INTO sessions (id, file_path, is_sidechain) VALUES (?1, ?2, 0)"
        )
        .bind(format!("bulk-{i}"))
        .bind(format!("/tmp/bulk-{i}.jsonl"))
        .execute(db.pool())
        .await
        .unwrap();
    }

    let ids: Vec<String> = (1..=3).map(|i| format!("bulk-{i}")).collect();
    let results = db.archive_sessions_bulk(&ids).await.unwrap();
    assert_eq!(results.len(), 3);

    // Verify 3 archived, 2 still visible
    let visible: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valid_sessions WHERE id LIKE 'bulk-%'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(visible.0, 2);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-db archive`
Expected: FAIL — functions don't exist yet

**Step 3: Implement archive/unarchive functions**

Add to `crates/db/src/queries/sessions.rs`:

```rust
/// Archive a session: set archived_at timestamp.
/// Returns the file_path so the caller can move the file.
pub async fn archive_session(&self, session_id: &str) -> DbResult<Option<String>> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query_scalar::<_, String>(
        "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path"
    )
    .bind(&now)
    .bind(session_id)
    .fetch_optional(self.pool())
    .await?;
    Ok(result)
}

/// Unarchive a session: clear archived_at, update file_path to new location.
pub async fn unarchive_session(&self, session_id: &str, new_file_path: &str) -> DbResult<bool> {
    let rows = sqlx::query(
        "UPDATE sessions SET archived_at = NULL, file_path = ?1 WHERE id = ?2 AND archived_at IS NOT NULL"
    )
    .bind(new_file_path)
    .bind(session_id)
    .execute(self.pool())
    .await?
    .rows_affected();
    Ok(rows > 0)
}

/// Archive multiple sessions in a single transaction.
/// Returns vec of (session_id, file_path) for file moves.
pub async fn archive_sessions_bulk(&self, session_ids: &[String]) -> DbResult<Vec<(String, String)>> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = self.pool().begin().await?;
    let mut results = Vec::new();
    for id in session_ids {
        let result = sqlx::query_scalar::<_, String>(
            "UPDATE sessions SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL RETURNING file_path"
        )
        .bind(&now)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(path) = result {
            results.push((id.clone(), path));
        }
    }
    tx.commit().await?;
    Ok(results)
}

/// Update a session's file_path in the DB (e.g. after moving to/from archive).
pub async fn update_session_file_path(&self, session_id: &str, new_path: &str) -> DbResult<bool> {
    let rows = sqlx::query("UPDATE sessions SET file_path = ?1 WHERE id = ?2")
        .bind(new_path)
        .bind(session_id)
        .execute(self.pool())
        .await?
        .rows_affected();
    Ok(rows > 0)
}

/// Bulk unarchive: clear archived_at for multiple sessions.
pub async fn unarchive_sessions_bulk(&self, session_ids: &[String], file_paths: &[(String, String)]) -> DbResult<usize> {
    let mut tx = self.pool().begin().await?;
    let mut count = 0usize;
    for (id, new_path) in file_paths {
        let rows = sqlx::query(
            "UPDATE sessions SET archived_at = NULL, file_path = ?1 WHERE id = ?2 AND archived_at IS NOT NULL"
        )
        .bind(new_path)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        count += rows as usize;
    }
    tx.commit().await?;
    Ok(count)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-db archive`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries/sessions.rs crates/db/tests/archive_sessions.rs
git commit -m "feat(db): add archive/unarchive session query functions with bulk transaction support"
```

---

### Task 3: Add archive/unarchive API routes

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

**Important:** This codebase uses `ApiResult<Json<T>>` (custom error type implementing `IntoResponse`) — NOT `Result<Json<T>, StatusCode>`. The `ApiError` enum has variants: `NotFound`, `Internal`, `BadRequest`, etc. All handlers must follow this pattern.

**Important:** `update_session_file_path` already exists at `sessions.rs:921` — do NOT re-create it.

**Step 1: Add imports**

At the top of `sessions.rs`, add `post` to the routing import:

```rust
use axum::{extract::{Path, Query, State}, routing::{get, post}, Json, Router};
```

**Step 2: Add request/response types**

Near the top of `sessions.rs`, add:

```rust
#[derive(Deserialize)]
struct BulkArchiveRequest {
    ids: Vec<String>,
}

#[derive(Serialize)]
struct ArchiveResponse {
    archived: bool,
}

#[derive(Serialize)]
struct BulkArchiveResponse {
    archived_count: usize,
}
```

**Step 3: Implement archive handler**

```rust
async fn archive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let file_path = state.db.archive_session(&id).await
        .map_err(|e| {
            tracing::error!("Failed to archive session {id}: {e}");
            ApiError::Internal(format!("archive failed: {e}"))
        })?
        .ok_or(ApiError::NotFound(format!("Session {id} not found or already archived")))?;

    // Move file to ~/.claude-view/archives/
    let src = std::path::PathBuf::from(&file_path);
    let archive_dir = archive_base_dir();
    let project_dir = src.parent()
        .and_then(|p| p.file_name())
        .unwrap_or_default();
    let dest_dir = archive_dir.join(project_dir);

    // Attempt file move — failure is non-fatal (DB flag is the source of truth)
    if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
        tracing::warn!("Failed to create archive dir: {e}");
    } else if let Some(file_name) = src.file_name() {
        let dest = dest_dir.join(file_name);
        match tokio::fs::rename(&src, &dest).await {
            Ok(()) => {
                let _ = state.db.update_session_file_path(&id, dest.to_str().unwrap_or_default()).await;
            }
            Err(e) => {
                tracing::warn!("Failed to move session file to archive: {e}");
                // DB already marked as archived — indexer guard will skip it
            }
        }
    }

    Ok(Json(ArchiveResponse { archived: true }))
}

/// Returns ~/.claude-view/archives/
fn archive_base_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude-view")
        .join("archives")
}
```

**Step 4: Implement unarchive handler**

Path traversal safety: validate that the relative path contains only `Normal` components (no `..` or `/` root).

```rust
async fn unarchive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let current_path = state.db.get_session_file_path(&id).await
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or(ApiError::NotFound(format!("Session {id} not found")))?;

    let archive_base = archive_base_dir();
    let current = std::path::PathBuf::from(&current_path);

    let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
        // Security: validate no path traversal in relative components
        use std::path::Component;
        if !relative.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(ApiError::BadRequest("Invalid archive path".to_string()));
        }

        let original = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("projects")
            .join(relative);

        // Move file back — failure is non-fatal
        if current.exists() {
            if let Some(parent) = original.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Err(e) = tokio::fs::rename(&current, &original).await {
                tracing::warn!("Failed to move file back from archive: {e}");
            }
        }

        original.to_str().unwrap_or_default().to_string()
    } else {
        // File not in archive dir (file move failed during archive) — just clear the flag
        current_path
    };

    state.db.unarchive_session(&id, &new_path).await
        .map_err(|e| ApiError::Internal(format!("unarchive failed: {e}")))?;

    Ok(Json(ArchiveResponse { archived: false }))
}
```

**Step 5: Implement bulk archive handler**

```rust
async fn bulk_archive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let results = state.db.archive_sessions_bulk(&body.ids).await
        .map_err(|e| ApiError::Internal(format!("bulk archive failed: {e}")))?;

    let archive_dir = archive_base_dir();
    for (id, file_path) in &results {
        let src = std::path::PathBuf::from(file_path);
        let project_dir = src.parent().and_then(|p| p.file_name()).unwrap_or_default();
        let dest_dir = archive_dir.join(project_dir);
        let _ = tokio::fs::create_dir_all(&dest_dir).await;
        if let Some(file_name) = src.file_name() {
            let dest = dest_dir.join(file_name);
            if let Ok(()) = tokio::fs::rename(&src, &dest).await {
                let _ = state.db.update_session_file_path(id, dest.to_str().unwrap_or_default()).await;
            }
        }
    }

    Ok(Json(BulkArchiveResponse { archived_count: results.len() }))
}
```

**Step 6: Implement bulk unarchive handler**

```rust
async fn bulk_unarchive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let archive_base = archive_base_dir();
    let mut file_paths: Vec<(String, String)> = Vec::new();

    for id in &body.ids {
        let current_path = match state.db.get_session_file_path(id).await {
            Ok(Some(p)) => p,
            _ => continue,
        };
        let current = std::path::PathBuf::from(&current_path);
        let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
            use std::path::Component;
            if !relative.components().all(|c| matches!(c, Component::Normal(_))) {
                continue;
            }
            let original = dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join("projects")
                .join(relative);
            if current.exists() {
                if let Some(parent) = original.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::rename(&current, &original).await;
            }
            original.to_str().unwrap_or_default().to_string()
        } else {
            current_path
        };
        file_paths.push((id.clone(), new_path));
    }

    let count = state.db.unarchive_sessions_bulk(&body.ids, &file_paths).await
        .map_err(|e| ApiError::Internal(format!("bulk unarchive failed: {e}")))?;

    Ok(Json(BulkArchiveResponse { archived_count: count }))
}
```

**Step 7: Register routes**

In the `router()` function (~line 591), add these routes. In Axum 0.8, static path segments (`/sessions/archive`) take priority over parameterized segments (`/sessions/{id}/archive`) at the same depth, so ordering is not critical — but register bulk routes first for clarity:

```rust
.route("/sessions/archive", post(bulk_archive_handler))
.route("/sessions/unarchive", post(bulk_unarchive_handler))
.route("/sessions/{id}/archive", post(archive_session_handler))
.route("/sessions/{id}/unarchive", post(unarchive_session_handler))
```

**Step 8: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "feat(server): add archive/unarchive session API routes"
```

---

### Task 4: Add `show_archived` filter to session list query

**Files:**
- Modify: `crates/db/src/queries/dashboard.rs`
- Modify: `crates/server/src/routes/sessions.rs`

**Step 1: Add filter param**

In `SessionFilterParams` (`dashboard.rs` ~line 15), add:

```rust
pub show_archived: Option<bool>,
```

**Step 2: Update query builder**

In `query_sessions_filtered` (~line 263), the function constructs a `QueryBuilder` with a base `FROM` clause. Currently hardcoded to `FROM valid_sessions s`. When `show_archived` is `Some(true)`, switch to `FROM sessions s` with `is_sidechain = 0` (to include archived sessions).

There are **TWO** `QueryBuilder` initializations that both hardcode `valid_sessions` — both must be updated:

- **Line 440** (COUNT query): `sqlx::QueryBuilder::new("SELECT COUNT(*) FROM valid_sessions s")`
- **Line 448** (DATA query): `sqlx::QueryBuilder::new(format!("SELECT {} FROM valid_sessions s", select_cols))`

Add the base table selection BEFORE both:

```rust
let base_from = if params.show_archived == Some(true) {
    "sessions s"
} else {
    "valid_sessions s"
};
```

Then update BOTH QueryBuilder initializations:

```rust
// Line 440 — COUNT query:
let mut count_qb = sqlx::QueryBuilder::new(format!("SELECT COUNT(*) FROM {base_from}"));

// Line 448 — DATA query:
let mut data_qb = sqlx::QueryBuilder::new(format!("SELECT {select_cols} FROM {base_from}"));
```

When using `FROM sessions s` (show_archived mode), add `AND s.is_sidechain = 0` to the WHERE filters to maintain the sidechain filter that `valid_sessions` normally provides. Add this in the `append_filters` closure:

```rust
if params.show_archived == Some(true) {
    qb.push(" AND s.is_sidechain = 0");
}
```

**Step 3: Wire up query param in route**

In `SessionsListQuery` (`sessions.rs` ~line 33), add:

```rust
pub show_archived: Option<bool>,
```

In the `list_sessions` handler, pass it through to `SessionFilterParams`:

```rust
show_archived: query.show_archived,
```

**Step 4: Test**

Run: `cargo test -p claude-view-db dashboard`
Run: `cargo test -p claude-view-server`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries/dashboard.rs crates/server/src/routes/sessions.rs
git commit -m "feat(db): add show_archived filter to session list query"
```

---

### Task 5: Install frontend dependencies

**Files:**
- Modify: `apps/web/package.json`

**Step 1: Install packages**

```bash
cd apps/web && bun add @radix-ui/react-context-menu
```

**Step 2: Commit**

```bash
git add apps/web/package.json bun.lock
git commit -m "chore(web): add @radix-ui/react-context-menu dependency"
```

---

### Task 6: Add archive API hooks

**Files:**
- Create: `apps/web/src/hooks/use-archive-session.ts`
- Modify: `apps/web/src/hooks/use-sessions-infinite.ts` (add `showArchived` param)

**Step 1: Create the mutation hooks**

**Important:** This codebase uses bare `/api/...` paths for all fetch calls — there is no `VITE_API_URL` env var. Use `'/api/...'` directly, matching all other hooks. Import `toast` from `'sonner'` — NOT from `'../lib/toast'` (that's a different, DOM-based toast system).

```tsx
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

async function archiveSession(id: string): Promise<void> {
  const res = await fetch(`/api/sessions/${id}/archive`, { method: 'POST' })
  if (!res.ok) throw new Error(`Archive failed: ${res.status}`)
}

async function unarchiveSession(id: string): Promise<void> {
  const res = await fetch(`/api/sessions/${id}/unarchive`, { method: 'POST' })
  if (!res.ok) throw new Error(`Unarchive failed: ${res.status}`)
}

async function archiveSessionsBulk(ids: string[]): Promise<void> {
  const res = await fetch('/api/sessions/archive', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ids }),
  })
  if (!res.ok) throw new Error(`Bulk archive failed: ${res.status}`)
}

export function useArchiveSession() {
  const qc = useQueryClient()

  const archive = useMutation({
    mutationFn: archiveSession,
    onSuccess: (_data, sessionId) => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast('Session archived', {
        action: {
          label: 'Undo',
          onClick: () => unarchiveMutation.mutate(sessionId),
        },
        duration: 5000,
      })
    },
    onError: () => toast.error('Failed to archive session'),
  })

  const unarchiveMutation = useMutation({
    mutationFn: unarchiveSession,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast.success('Session restored')
    },
    onError: () => toast.error('Failed to restore session'),
  })

  const bulkArchive = useMutation({
    mutationFn: archiveSessionsBulk,
    onSuccess: (_data, ids) => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast(`${ids.length} sessions archived`, {
        action: {
          label: 'Undo',
          onClick: () => {
            Promise.all(ids.map(unarchiveSession)).then(() => {
              qc.invalidateQueries({ queryKey: ['sessions'] })
              toast.success(`${ids.length} sessions restored`)
            })
          },
        },
        duration: 5000,
      })
    },
    onError: () => toast.error('Failed to archive sessions'),
  })

  return { archive, unarchive: unarchiveMutation, bulkArchive }
}
```

**Step 2: Update `useSessionsInfinite` to accept `showArchived`**

In `apps/web/src/hooks/use-sessions-infinite.ts`, add `showArchived?: boolean` to the hook's params interface. Forward it to the API call as `show_archived` query parameter. Add it to the query key so React Query refetches when it changes:

```tsx
// In the params interface:
showArchived?: boolean

// In the query key:
queryKey: ['sessions', { ...otherParams, showArchived }]

// In the fetch URL construction (using existing URLSearchParams pattern):
if (showArchived) params.set('show_archived', 'true')
```

**Step 3: Commit**

```bash
git add apps/web/src/hooks/use-archive-session.ts apps/web/src/hooks/use-sessions-infinite.ts
git commit -m "feat(web): add archive/unarchive session mutation hooks"
```

---

### Task 7: Add context menu to SessionCard

**Files:**
- Modify: `apps/web/src/components/SessionCard.tsx`

**Step 1: Add imports**

Add to the existing lucide-react import:

```tsx
import { Archive, /* ...existing icons... */ } from 'lucide-react'
import * as ContextMenu from '@radix-ui/react-context-menu'
```

**Step 2: Extend props interface**

Read the current `SessionCardProps` interface and add these new optional props:

```tsx
onArchive?: (sessionId: string) => void
selectable?: boolean
selected?: boolean
onSelectToggle?: (sessionId: string) => void
```

**Step 3: Add `group relative` classes to `<article>`**

The `<article>` element currently does NOT have `group` or `relative` classes. Add both to the `cn(...)` call so the hover archive button and checkbox positioning work:

```tsx
<article className={cn("group relative", /* ...existing classes... */)} >
```

**Step 4: Wrap in Radix ContextMenu**

Wrap the `<article>` return in `ContextMenu.Root` with `ContextMenu.Trigger asChild`:

```tsx
<ContextMenu.Root>
  <ContextMenu.Trigger asChild>
    <article className={cn("group relative", ...)} >
      {selectable && (
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onSelectToggle?.(session.id)}
          className="absolute top-2 left-2 z-10"
          onClick={(e) => e.stopPropagation()}
        />
      )}
      {/* existing card content */}
      {onArchive && !selectable && (
        <button
          onClick={(e) => { e.preventDefault(); e.stopPropagation(); onArchive(session.id) }}
          className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700"
          title="Archive session"
        >
          <Archive className="w-4 h-4 text-gray-500" />
        </button>
      )}
    </article>
  </ContextMenu.Trigger>
  <ContextMenu.Portal>
    <ContextMenu.Content className="min-w-[160px] bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 p-1 z-50">
      <ContextMenu.Item
        className="flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300"
        onSelect={() => onArchive?.(session.id)}
      >
        <Archive className="w-4 h-4" />
        Archive session
      </ContextMenu.Item>
    </ContextMenu.Content>
  </ContextMenu.Portal>
</ContextMenu.Root>
```

**Note on Tailwind classes:** Use `gray-*` classes to match the existing HistoryView design system, not `zinc-*`.

**Step 5: Commit**

```bash
git add apps/web/src/components/SessionCard.tsx
git commit -m "feat(web): add context menu and hover archive button to SessionCard"
```

---

### Task 8: Add bulk select mode + archive toolbar to HistoryView

**Files:**
- Modify: `apps/web/src/components/HistoryView.tsx`

**Step 1: Add bulk selection state**

```tsx
import { useArchiveSession } from '../hooks/use-archive-session'

const [bulkMode, setBulkMode] = useState(false)
const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
const [showArchived, setShowArchived] = useState(false)
const { archive, bulkArchive } = useArchiveSession()

const toggleSelect = (id: string) => {
  setSelectedIds(prev => {
    const next = new Set(prev)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  })
}

const handleBulkArchive = () => {
  if (selectedIds.size === 0) return
  bulkArchive.mutate([...selectedIds], {
    onSuccess: () => {
      setSelectedIds(new Set())
      setBulkMode(false)
    },
  })
}
```

**Step 2: Pass `showArchived` to `useSessionsInfinite`**

Find where `useSessionsInfinite` is called (~line 138) and add `showArchived` to its params.

**Step 3: Add toolbar above session list**

Insert between the filters area and the session list (`<div className="mt-5">`):

```tsx
{/* Bulk action toolbar */}
<div className="flex items-center gap-2 px-4 py-2 border-b border-gray-200 dark:border-gray-700">
  <button
    onClick={() => { setBulkMode(!bulkMode); setSelectedIds(new Set()) }}
    className={`text-sm px-2 py-1 rounded ${bulkMode ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300' : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'}`}
  >
    {bulkMode ? 'Cancel selection' : 'Select'}
  </button>
  {bulkMode && selectedIds.size > 0 && (
    <>
      <span className="text-sm text-gray-500">{selectedIds.size} selected</span>
      <button
        onClick={handleBulkArchive}
        className="text-sm px-3 py-1 rounded bg-red-50 dark:bg-red-900/30 text-red-600 dark:text-red-400 hover:bg-red-100 dark:hover:bg-red-900/50"
      >
        Archive
      </button>
    </>
  )}
  <button
    onClick={() => setShowArchived(!showArchived)}
    className={`text-sm px-2 py-1 rounded ml-auto ${showArchived ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700' : 'text-gray-500'}`}
  >
    {showArchived ? 'Showing archived' : 'Show archived'}
  </button>
</div>
```

**Step 4: Pass props through to SessionCard**

When rendering `<SessionCard>` items (~line 565), pass the new props:

```tsx
<SessionCard
  session={session}
  onArchive={(id) => archive.mutate(id)}
  selectable={bulkMode}
  selected={selectedIds.has(session.id)}
  onSelectToggle={toggleSelect}
/>
```

**Step 5: Commit**

```bash
git add apps/web/src/components/HistoryView.tsx
git commit -m "feat(web): add bulk select mode and archive toolbar to HistoryView"
```

---

### Task 9: Add context menu to CompactSessionTable rows

**Files:**
- Modify: `apps/web/src/components/CompactSessionTable.tsx`

**Step 1: Add context menu to table rows**

CompactSessionTable uses TanStack Table's `table.getRowModel().rows.map(...)` to render `<tr>` elements. You **cannot** wrap a `<tr>` in a non-`<tr>` element inside `<tbody>` — it breaks HTML table structure.

Use `<ContextMenu.Root>` + `<ContextMenu.Trigger asChild>` with the `<tr>` as the child:

```tsx
import * as ContextMenu from '@radix-ui/react-context-menu'
import { Archive } from 'lucide-react'

// For each row:
<ContextMenu.Root>
  <ContextMenu.Trigger asChild>
    <tr key={row.id} className={cn("group", /* existing classes */)}>
      {selectable && (
        <td className="w-8 px-2">
          <input
            type="checkbox"
            checked={selected?.has(row.original.id)}
            onChange={() => onSelectToggle?.(row.original.id)}
            onClick={(e) => e.stopPropagation()}
          />
        </td>
      )}
      {/* existing td cells */}
    </tr>
  </ContextMenu.Trigger>
  <ContextMenu.Portal>
    <ContextMenu.Content className="min-w-[160px] bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 p-1 z-50">
      <ContextMenu.Item
        className="flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700"
        onSelect={() => onArchive?.(row.original.id)}
      >
        <Archive className="w-4 h-4" />
        Archive session
      </ContextMenu.Item>
    </ContextMenu.Content>
  </ContextMenu.Portal>
</ContextMenu.Root>
```

Add `onArchive`, `selectable`, `selected` (Set), `onSelectToggle` props to `CompactSessionTableProps`.

When `selectable` is true, add a checkbox `<th>` header column.

**Step 2: Commit**

```bash
git add apps/web/src/components/CompactSessionTable.tsx
git commit -m "feat(web): add context menu and bulk select to CompactSessionTable"
```

---

### Task 10: Indexer guard — skip archived sessions on re-index

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs`

**Step 1: Update the pre-loaded session map to include archived status**

The indexer pre-loads all session metadata in bulk at ~line 3222 using `get_sessions_needing_deep_index()` or a similar bulk query that populates an `existing_map: HashMap`. Do **NOT** add a per-session DB query inside the indexing loop — that would cause N+1 queries and defeat the existing optimization.

Instead, modify the pre-load query to also fetch `archived_at`:

1. Find the query that populates `existing_map` (look for `HashMap` construction around line 3222)
2. Add `archived_at` to the SELECT clause
3. In the struct that holds the map values, add `archived_at: Option<String>`
4. In the indexing loop where the map is checked (~line 3282-3293), add an early skip:

```rust
// If session is archived, skip re-indexing
if let Some(existing) = existing_map.get(&session_id) {
    if existing.archived_at.is_some() {
        continue; // Skip archived sessions
    }
}
```

**Step 2: Test**

Run: `cargo test -p claude-view-db indexer`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat(db): skip archived sessions during re-indexing via pre-loaded map"
```

---

## Feature 2: VS Code-style Resizable/Collapsible Sidebar

### Task 11: Install react-resizable-panels

**Files:**
- Modify: `apps/web/package.json`

**Step 1: Install**

```bash
cd apps/web && bun add react-resizable-panels
```

**Step 2: Commit**

```bash
git add apps/web/package.json bun.lock
git commit -m "chore(web): add react-resizable-panels dependency"
```

---

### Task 12: Create SectionHeader component

**Files:**
- Create: `apps/web/src/components/sidebar/SectionHeader.tsx`

**Step 1: Create component**

```tsx
import { ChevronDown, ChevronRight } from 'lucide-react'

interface SectionHeaderProps {
  title: string
  collapsed: boolean
  onToggle: () => void
  actions?: React.ReactNode
}

export function SectionHeader({ title, collapsed, onToggle, actions }: SectionHeaderProps) {
  return (
    <button
      onClick={onToggle}
      className="flex items-center gap-1 w-full px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 select-none"
    >
      {collapsed ? (
        <ChevronRight className="w-3.5 h-3.5 shrink-0" />
      ) : (
        <ChevronDown className="w-3.5 h-3.5 shrink-0" />
      )}
      <span className="truncate">{title}</span>
      {actions && (
        <span className="ml-auto flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {actions}
        </span>
      )}
    </button>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/sidebar/SectionHeader.tsx
git commit -m "feat(web): create SectionHeader component for collapsible sidebar sections"
```

---

### Task 13: Add sidebar panel state to useAppStore

**Files:**
- Modify: `apps/web/src/store/app-store.ts`

**Step 1: Add panel collapse states**

Add to the `AppState` interface:

```ts
sidebarTabsCollapsed: boolean
sidebarScopeCollapsed: boolean
sidebarRecentCollapsed: boolean
toggleSidebarSection: (section: 'tabs' | 'scope' | 'recent') => void
```

In the `create()` implementation, add initial values and the action. Use a `switch` statement instead of computed key — TypeScript strict mode rejects `set((state) => ({ [computedKey]: !state[computedKey] }))` because the computed key is a `string`, not a narrowed `keyof AppState`:

```ts
sidebarTabsCollapsed: false,
sidebarScopeCollapsed: false,
sidebarRecentCollapsed: false,

toggleSidebarSection: (section) => {
  switch (section) {
    case 'tabs':
      set((state) => ({ sidebarTabsCollapsed: !state.sidebarTabsCollapsed }))
      break
    case 'scope':
      set((state) => ({ sidebarScopeCollapsed: !state.sidebarScopeCollapsed }))
      break
    case 'recent':
      set((state) => ({ sidebarRecentCollapsed: !state.sidebarRecentCollapsed }))
      break
  }
},
```

Add the new fields to `partialize` so they persist to localStorage:

```ts
partialize: (state) => ({
  // ...existing fields
  sidebarTabsCollapsed: state.sidebarTabsCollapsed,
  sidebarScopeCollapsed: state.sidebarScopeCollapsed,
  sidebarRecentCollapsed: state.sidebarRecentCollapsed,
})
```

**Step 2: Commit**

```bash
git add apps/web/src/store/app-store.ts
git commit -m "feat(web): add sidebar section collapse state to app store"
```

---

### Task 14: Refactor Sidebar.tsx to use resizable panels

**Files:**
- Modify: `apps/web/src/components/Sidebar.tsx`

This is the largest task. The existing three zones (Tabs, Scope, Recent) get wrapped in `PanelGroup` / `Panel` / `PanelResizeHandle`.

**Step 1: Import and setup**

```tsx
import { PanelGroup, Panel, PanelResizeHandle } from 'react-resizable-panels'
import { SectionHeader } from './sidebar/SectionHeader'
```

Read `sidebarTabsCollapsed`, `sidebarScopeCollapsed`, `sidebarRecentCollapsed`, and `toggleSidebarSection` from `useAppStore`.

**Step 2: Replace the static flex layout**

Currently the expanded sidebar body (~lines 527-754) is a `div` with `flex flex-col`. The `<nav>` (Zone 1) is at ~line 543, Zone 2 (Scope) at ~line 635, Zone 3 (Recent/QuickJump) at ~line 749.

Replace the inner layout with:

```tsx
<PanelGroup
  direction="vertical"
  autoSaveId="sidebar-panels"
  className="flex-1 min-h-0"
>
  {/* Zone 1: Navigation Tabs */}
  <Panel
    id="tabs"
    collapsible
    minSize={5}
    defaultSize={20}
    collapsedSize={0}
    onCollapse={() => !sidebarTabsCollapsed && toggleSidebarSection('tabs')}
    onExpand={() => sidebarTabsCollapsed && toggleSidebarSection('tabs')}
  >
    <SectionHeader
      title="Navigation"
      collapsed={sidebarTabsCollapsed}
      onToggle={() => toggleSidebarSection('tabs')}
    />
    {!sidebarTabsCollapsed && (
      <nav aria-label="Main navigation" className="flex flex-col gap-0.5 px-2 py-1 overflow-y-auto">
        {/* ...existing tab links... */}
      </nav>
    )}
  </Panel>

  <PanelResizeHandle className="group h-1 shrink-0 flex items-center justify-center">
    <div className="h-px w-full bg-gray-200 dark:bg-gray-700 group-hover:h-0.5 group-hover:bg-blue-400 dark:group-hover:bg-blue-500 group-active:bg-blue-500 transition-all" />
  </PanelResizeHandle>

  {/* Zone 2: Scope */}
  <Panel
    id="scope"
    collapsible
    minSize={10}
    defaultSize={55}
    collapsedSize={0}
    onCollapse={() => !sidebarScopeCollapsed && toggleSidebarSection('scope')}
    onExpand={() => sidebarScopeCollapsed && toggleSidebarSection('scope')}
  >
    <SectionHeader
      title="Scope"
      collapsed={sidebarScopeCollapsed}
      onToggle={() => toggleSidebarSection('scope')}
      actions={selectedProjectId ? <ClearButton /> : undefined}
    />
    {!sidebarScopeCollapsed && (
      <div className="flex flex-col min-h-0 flex-1 overflow-y-auto">
        {/* ...existing view mode toggle, expand/collapse buttons, project tree... */}
      </div>
    )}
  </Panel>

  <PanelResizeHandle className="group h-1 shrink-0 flex items-center justify-center">
    <div className="h-px w-full bg-gray-200 dark:bg-gray-700 group-hover:h-0.5 group-hover:bg-blue-400 dark:group-hover:bg-blue-500 group-active:bg-blue-500 transition-all" />
  </PanelResizeHandle>

  {/* Zone 3: Recent */}
  <Panel
    id="recent"
    collapsible
    minSize={5}
    defaultSize={25}
    collapsedSize={0}
    onCollapse={() => !sidebarRecentCollapsed && toggleSidebarSection('recent')}
    onExpand={() => sidebarRecentCollapsed && toggleSidebarSection('recent')}
  >
    <SectionHeader
      title="Recent"
      collapsed={sidebarRecentCollapsed}
      onToggle={() => toggleSidebarSection('recent')}
    />
    {!sidebarRecentCollapsed && (
      selectedProjectId ? (
        <QuickJumpZone project={selectedProjectId} branch={selectedBranch} />
      ) : (
        <div className="px-3 py-4 text-xs text-gray-400 text-center">
          Select a project to see recent sessions
        </div>
      )
    )}
  </Panel>
</PanelGroup>
```

**Key changes:**
- Remove the old `flex-1` and `border-t` separators between zones
- Each zone's content is conditionally rendered based on collapse state
- The `autoSaveId="sidebar-panels"` handles persisting panel sizes to localStorage automatically
- Panel `onCollapse`/`onExpand` callbacks sync with Zustand store
- Recent panel shows empty state when no project selected (matching current UX where QuickJumpZone only renders with a project)
- Resize handles use polished `group` hover styling (consolidated from Task 15)

**Step 3: Handle all-collapsed edge case**

After the `PanelGroup`, add:

```tsx
{sidebarTabsCollapsed && sidebarScopeCollapsed && sidebarRecentCollapsed && (
  <div className="flex-1 flex items-center justify-center text-xs text-gray-400">
    Click a header to expand
  </div>
)}
```

**Step 4: Test manually**

- Verify all three sections render
- Drag dividers — sections resize
- Click section headers — sections collapse/expand
- Refresh page — sizes and collapse states persist
- Collapse sidebar entirely (existing functionality) — still works

**Step 5: Commit**

```bash
git add apps/web/src/components/Sidebar.tsx
git commit -m "feat(web): refactor sidebar to use resizable/collapsible panels (VS Code style)"
```

---

### Task 15: End-to-end verification

**Step 1: Build frontend**

```bash
cd apps/web && bun run build
```

**Step 2: Run Rust server**

```bash
bun run dev:server
```

**Step 3: Verify Feature 1 (Archive)**

1. Open `/sessions` in browser
2. Right-click a session card → "Archive session" appears in context menu
3. Click archive → session disappears + toast with "Undo" shows
4. Click "Undo" within 5s → session reappears
5. Enable bulk mode → checkboxes appear → select multiple → click Archive
6. Toggle "Show archived" → archived sessions appear with muted style
7. Check `~/.claude-view/archives/` → JSONL files are there
8. Restart server → archived sessions stay hidden
9. Verify stats/insights/contributions pages do NOT show archived sessions

**Step 4: Verify Feature 2 (Resizable sidebar)**

1. Sidebar shows three sections with headers: Navigation, Scope, Recent
2. Drag dividers between sections → sections resize
3. Click section header → section collapses (only header visible)
4. Click again → section expands
5. Refresh page → sizes and collapse states persist
6. Collapse entire sidebar (existing button) → still works
7. Expand sidebar → panel sizes are remembered

**Step 5: Commit any final fixes**

```bash
git add -A
git commit -m "fix: end-to-end verification fixes for archive and resizable sidebar"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Migration syntax `50 => { ... }` is wrong — MIGRATIONS is a `&[&str]` array, not a match block | Blocker | Rewrote Task 1 migration as raw string literal appended to array |
| 2 | No `LATEST_MIGRATION` constant exists — plan referenced phantom constant | Blocker | Removed "update LATEST_MIGRATION" instruction entirely |
| 3 | Multi-statement migration without `BEGIN;...COMMIT;` silently drops CREATE VIEW | Blocker | Wrapped migration SQL in `BEGIN;...COMMIT;` block |
| 4 | Tests use `#[sqlx::test]` + `Database::new(pool)` — neither exists in this codebase | Blocker | Rewrote tests to use `#[tokio::test]` + `Database::new_in_memory().await` |
| 5 | `bulk_unarchive_handler` registered in routes but never implemented | Blocker | Added full `bulk_unarchive_handler` implementation + `unarchive_sessions_bulk` DB function |
| 6 | Handler return type `Result<Json<T>, StatusCode>` doesn't match codebase pattern `ApiResult<Json<T>>` | Blocker | Rewrote all handlers to use `ApiResult<Json<T>>` with `ApiError` variants |
| 7 | Task 3 Step 6 adds `update_session_file_path` but it already exists at sessions.rs:921 | Blocker | Removed Step 6 — function already exists |
| 8 | `useSessionsInfinite` hook never updated to pass `show_archived` to backend | Blocker | Added Step 2 to Task 6: update `useSessionsInfinite` params |
| 9 | `VITE_API_URL` doesn't exist in this codebase — all hooks use bare `/api/...` paths | Blocker | Removed `API_BASE` — use bare paths matching codebase convention |
| 10 | `valid_sessions` view doesn't cover 53 direct `FROM sessions` queries — archived sessions leak into stats, contributions, insights, trends | Critical | Added new Task 1b to audit and fix all direct `FROM sessions` queries |
| 11 | Indexer guard uses per-file DB queries instead of pre-loaded HashMap — N+1 anti-pattern | High | Rewrote Task 10 to extend existing pre-loaded `existing_map` with `archived_at` field |
| 12 | Bulk archive uses N+1 individual calls without transaction | High | Rewrote `archive_sessions_bulk` to use explicit transaction with `pool().begin()` |
| 13 | `<tr>` wrapped in ContextMenu.Root breaks HTML table structure | High | Task 9 now uses `<ContextMenu.Trigger asChild>` on the `<tr>` element directly |
| 14 | Path traversal in unarchive handler — `../` components not validated | High | Added `Component::Normal` validation on relative path in unarchive handler |
| 15 | TOCTOU: `exists()` before `rename()` in archive handler | High | Removed `exists()` guard — attempt rename directly, handle error |
| 16 | Missing `routing::post` import in sessions.rs | High | Added explicit import step in Task 3 Step 1 |
| 17 | TypeScript computed key in `toggleSidebarSection` fails strict mode | Medium | Replaced with `switch` statement in Task 13 |
| 18 | Two toast systems exist — must use `sonner`, not `lib/toast.ts` | Medium | Added explicit warning in Task 6 about using `'sonner'` import |
| 19 | QueryBuilder base table switching needs to work with actual QueryBuilder pattern | Medium | Task 4 Step 2 now describes integrating with QueryBuilder init, not string formatting |
| 20 | Recent panel has no empty state when no project selected | Medium | Added empty state JSX in Task 14 Panel "recent" |
| 21 | `zinc-*` vs `gray-*` class inconsistency | Medium | Changed all classes to `gray-*` matching HistoryView design system |
| 22 | Axum route ordering explanation was wrong — static segments win over `{id}` | Minor | Corrected explanation in Task 3 Step 7 |
| 23 | Task 3 compile ordering: handler code references `update_session_file_path` before it's added | Minor | Moot — function already exists (fix #7) |
| 24 | Article needs explicit `group relative` classes (currently absent) | Minor | Task 7 Step 3 now explicitly adds both classes to `<article>` cn() call |
| 25 | `Archive` icon import not explicit enough | Minor | Task 7 Step 1 now lists the import explicitly |
| 26 | Merged Task 15 (resize handle polish) into Task 14 — eliminated redundant task | Minor | Consolidated resize handle styling into PanelResizeHandle in Task 14 |
| 27 | Added rollback strategy to plan header | Minor | Added Rollback strategy section after Architecture |
| 28 | `update_session_file_path` does NOT exist — `get_session_file_path` exists at line 921 but the UPDATE variant was never written | Blocker | Re-added `update_session_file_path` function to Task 2 Step 3 |
| 29 | Migration test `MIGRATIONS.last()` at line 1183 will break after appending migration 50 | Blocker | Added Step 2 to Task 1: fix test to use `MIGRATIONS[48]` instead of `.last()` |
| 30 | Task 4 only parameterizes one QueryBuilder; COUNT query at line 440 still hardcoded to `valid_sessions` | High | Updated Task 4 to explicitly list BOTH lines 440 and 448 with code for both |
