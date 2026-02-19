---
status: done
date: 2026-02-12
---

# Session Endpoint Unification — Kill the Path-Construction Pattern

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate the leaky abstraction where the frontend must know the filesystem directory layout to fetch session content. The server should resolve file paths internally from the session ID alone.

**Architecture:** Replace the two-param filesystem endpoints (`/api/session/:project_dir/:session_id`) with new sub-routes under the existing DB endpoint (`/api/sessions/:id/parsed`, `/api/sessions/:id/messages`). These look up `file_path` from the DB — no path construction, no `project_dir` param, no worktree fallback needed. Then update the three frontend hooks to drop the `projectDir` dependency, which unblocks ConversationView from its 3-call chain.

**Tech Stack:** Rust (Axum, sqlx), React (TanStack Query), TypeScript

---

## Why This Is Needed

The current design has a **leaky abstraction**: the client passes `project_dir` to the server, which joins it into a filesystem path. This breaks for worktree sessions because `project_id` (used for UI grouping) doesn't match the actual directory where the JSONL lives.

The band-aid fix (`resolve_session_path` with DB fallback) works but adds complexity at the wrong layer. The server already knows the real `file_path` in the DB — it should just use it.

### Before (3-call chain, project_dir dependency)

```
ConversationView
  → useSessionDetail(sessionId)                          GET /api/sessions/:id         (DB)
  → useSession(sessionDetail.project, sessionId)         GET /api/session/:dir/:id     (filesystem, path-constructed)
  → useSessionMessages(sessionDetail.project, sessionId) GET /api/session/:dir/:id/msg (filesystem, path-constructed)
```

### After (3 independent calls, no project_dir)

```
ConversationView
  → useSessionDetail(sessionId)      GET /api/sessions/:id          (DB, unchanged)
  → useSession(sessionId)            GET /api/sessions/:id/parsed   (DB file_path → filesystem, lazy for export)
  → useSessionMessages(sessionId)    GET /api/sessions/:id/messages (DB file_path → filesystem)
```

---

## Task 1: Add `get_session_file_path` DB Method

**Files:**
- Modify: `crates/db/src/queries/sessions.rs`
- Modify: `crates/db/tests/queries_sessions_test.rs`

**Step 1: Write the failing test**

Add to `crates/db/tests/queries_sessions_test.rs`:

```rust
#[tokio::test]
async fn test_get_session_file_path() {
    let db = Database::new_in_memory().await.unwrap();

    // Not in DB → None
    let result = db.get_session_file_path("nonexistent").await.unwrap();
    assert!(result.is_none());

    // Insert session with known file_path
    let session = make_session("fp-test", "proj", 1700000000);
    // make_session sets file_path to "/home/user/.claude/projects/proj/fp-test.jsonl"
    db.insert_session(&session, "proj", "Project").await.unwrap();

    let result = db.get_session_file_path("fp-test").await.unwrap();
    assert_eq!(
        result.as_deref(),
        Some("/home/user/.claude/projects/proj/fp-test.jsonl")
    );
}
```

> **Note:** `make_session` requires 3 args: `(id, project, modified_at)`. The shared helper is in `crates/db/tests/queries_shared.rs`. It sets `file_path` to the format `/home/user/.claude/projects/{project}/{id}.jsonl`.

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-db -- test_get_session_file_path`
Expected: FAIL — `get_session_file_path` method doesn't exist

**Step 3: Write minimal implementation**

In `crates/db/src/queries/sessions.rs`, add to the `impl Database` block:

```rust
/// Look up a session's JSONL file path by session ID.
///
/// Returns `None` if the session doesn't exist in the DB.
/// The returned path is always absolute (set during indexing).
pub async fn get_session_file_path(&self, session_id: &str) -> DbResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT file_path FROM sessions WHERE id = ?1",
    )
    .bind(session_id)
    .fetch_optional(self.pool())
    .await?;
    Ok(row.map(|(p,)| p))
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-db -- test_get_session_file_path`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries/sessions.rs crates/db/tests/queries_sessions_test.rs
git commit -m "feat(db): add get_session_file_path query method"
```

---

## Task 2: Add `GET /api/sessions/:id/parsed` Endpoint

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

**Step 1: Write the tests**

Add to the `#[cfg(test)] mod tests` block in `sessions.rs`.

**Important:** Axum returns a bare 404 for unmatched routes (no JSON body). Our API returns structured JSON `{"error": "Session not found", "details": "..."}`. Tests MUST assert on the JSON body to distinguish "route not matched" from "our handler returned 404".

```rust
// ========================================================================
// GET /api/sessions/:id/parsed tests
// ========================================================================

#[tokio::test]
async fn test_get_session_parsed_not_in_db() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/parsed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_parsed_file_gone() {
    let db = test_db().await;
    let mut session = make_session("parsed-test", "proj", 1700000000);
    session.file_path = "/nonexistent/path.jsonl".to_string();
    db.insert_session(&session, "proj", "Project").await.unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/parsed-test/parsed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_parsed_success() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("success-test.jsonl");
    // Minimal valid JSONL: a single user message (format matches parser.rs line 1021)
    std::fs::write(
        &session_file,
        r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
    ).unwrap();

    let mut session = make_session("parsed-ok", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project").await.unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/parsed-ok/parsed").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["messages"].is_array(), "Response should contain messages array");
}
```

> **Note:** The success test uses `tempfile::tempdir()` which is already a dev-dependency (used by the existing `test_resolve_path_*` tests). The JSONL format must match what `vibe_recall_core::parse_session` expects — check the actual parser if the format above doesn't work and adjust accordingly.

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-server -- test_get_session_parsed`
Expected: FAIL — route doesn't exist yet

**Step 3: Write minimal implementation**

Add the handler function in `sessions.rs`:

```rust
/// GET /api/sessions/:id/parsed — Get full parsed session by ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
pub async fn get_session_parsed(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<ParsedSession>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let session = vibe_recall_core::parse_session(&path).await?;
    Ok(Json(session))
}
```

Register in the `router()` function — add this line **before** the legacy routes:

```rust
.route("/sessions/{id}/parsed", get(get_session_parsed))
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p vibe-recall-server -- test_get_session_parsed`
Expected: All 3 tests PASS (not_in_db, file_gone, success)

> If the success test fails because the JSONL format doesn't match the parser, adjust the fixture data to match. Check `crates/core/src/parser.rs` for the expected line format. Do NOT skip this test — it's the only one that validates the happy path works end-to-end.

**Step 5: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "feat(api): add GET /api/sessions/:id/parsed endpoint"
```

---

## Task 3: Add `GET /api/sessions/:id/messages` Endpoint

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

**Step 1: Write the tests**

Add to the `#[cfg(test)] mod tests` block:

```rust
// ========================================================================
// GET /api/sessions/:id/messages tests
// ========================================================================

#[tokio::test]
async fn test_get_session_messages_by_id_not_in_db() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/nonexistent/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_messages_by_id_file_gone() {
    let db = test_db().await;
    let mut session = make_session("msg-test", "proj", 1700000000);
    session.file_path = "/nonexistent/path.jsonl".to_string();
    db.insert_session(&session, "proj", "Project").await.unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/msg-test/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Session not found");
}

#[tokio::test]
async fn test_get_session_messages_by_id_success() {
    let db = test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let session_file = tmp.path().join("msg-success.jsonl");
    // Minimal valid JSONL with at least one message (format matches parser.rs line 1021)
    std::fs::write(
        &session_file,
        r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
    ).unwrap();

    let mut session = make_session("msg-ok", "proj", 1700000000);
    session.file_path = session_file.to_str().unwrap().to_string();
    db.insert_session(&session, "proj", "Project").await.unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions/msg-ok/messages?limit=10&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["messages"].is_array(), "Response should contain messages array");
    assert!(json["total"].is_number(), "Response should contain total count");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-server -- test_get_session_messages_by_id`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
/// GET /api/sessions/:id/messages — Get paginated messages by session ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
pub async fn get_session_messages_by_id(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<vibe_recall_core::PaginatedMessages>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let result = vibe_recall_core::parse_session_paginated(&path, limit, offset).await?;
    Ok(Json(result))
}
```

Register in the `router()` function — add **before** the legacy routes:

```rust
.route("/sessions/{id}/messages", get(get_session_messages_by_id))
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p vibe-recall-server -- test_get_session_messages_by_id`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "feat(api): add GET /api/sessions/:id/messages endpoint"
```

---

## Task 4: Update Frontend Hooks — Drop `projectDir`

**Files:**
- Modify: `src/hooks/use-session.ts`
- Modify: `src/hooks/use-session-messages.ts`
- Modify: `src/hooks/use-session-detail.ts` (cleanup duplicate HttpError)
- Modify: `src/hooks/use-contributions.ts` (cleanup duplicate HttpError)

**Step 1: Update `use-session.ts`**

Change `useSession` to take only `sessionId`. The fetch URL changes from `/api/session/:dir/:id` to `/api/sessions/:id/parsed`. The `HttpError` class, `isNotFoundError`, and type re-exports stay unchanged:

```typescript
import { useQuery } from '@tanstack/react-query'
import type { ParsedSession } from '../types/generated'

// Re-export for backward compatibility with existing imports
export type { ToolCall, Message } from '../types/generated'

// Alias ParsedSession to SessionData for backward compatibility
export type SessionData = ParsedSession

/** Error subclass that carries the HTTP status code. */
export class HttpError extends Error {
  constructor(message: string, public readonly status: number) {
    super(message)
    this.name = 'HttpError'
  }
}

/** Type-safe check for a 404 HttpError. */
export function isNotFoundError(err: unknown): boolean {
  return err instanceof HttpError && err.status === 404
}

async function fetchSession(sessionId: string): Promise<SessionData> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/parsed`)
  if (!response.ok) {
    throw new HttpError('Failed to fetch session', response.status)
  }
  return response.json()
}

export function useSession(sessionId: string | null) {
  return useQuery({
    queryKey: ['session', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchSession(sessionId)
    },
    enabled: !!sessionId,
    retry: (_, error) => !isNotFoundError(error),
  })
}
```

**Changes from current code:**
- `fetchSession` param: `(projectDir, sessionId)` → `(sessionId)`
- Fetch URL: `/api/session/${dir}/${id}` → `/api/sessions/${id}/parsed`
- `useSession` param: `(projectDir, sessionId)` → `(sessionId)`
- `queryKey`: `['session', projectDir, sessionId]` → `['session', sessionId]`
- `enabled`: `!!projectDir && !!sessionId` → `!!sessionId`
- Error guard in `queryFn`: `if (!projectDir || !sessionId)` → `if (!sessionId)`

**Step 2: Update `use-session-messages.ts`**

Change `useSessionMessages` to take only `sessionId`. Fetch URL changes from `/api/session/:dir/:id/messages` to `/api/sessions/:id/messages`. This file already imports `HttpError` and `isNotFoundError` from `use-session.ts` — no change needed there.

```typescript
import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

const PAGE_SIZE = 100

async function fetchMessages(
  sessionId: string,
  offset: number,
  limit: number,
): Promise<PaginatedMessages> {
  const response = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages?limit=${limit}&offset=${offset}`
  )
  if (!response.ok) throw new HttpError('Failed to fetch messages', response.status)
  return response.json()
}

export function useSessionMessages(sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', sessionId],
    queryFn: async ({ pageParam }) => {
      if (!sessionId) throw new Error('sessionId is required')

      if (pageParam === -1) {
        // Initial load: probe for total, then fetch the last PAGE_SIZE messages.
        const probe = await fetchMessages(sessionId, 0, 1)
        const tailOffset = Math.max(0, probe.total - PAGE_SIZE)
        return fetchMessages(sessionId, tailOffset, PAGE_SIZE)
      }

      return fetchMessages(sessionId, pageParam, PAGE_SIZE)
    },
    initialPageParam: -1 as number,
    getNextPageParam: () => undefined, // No downward pagination needed — already at the end
    getPreviousPageParam: (firstPage) => {
      // Load older messages (lower offsets) when scrolling up
      if (firstPage.offset === 0) return undefined // Already at the beginning
      const prevOffset = Math.max(0, firstPage.offset - PAGE_SIZE)
      return prevOffset
    },
    enabled: !!sessionId,
    retry: (_, error) => !isNotFoundError(error),
  })
}
```

**Changes from current code:**
- `fetchMessages` param: `(projectDir, sessionId, offset, limit)` → `(sessionId, offset, limit)`
- Fetch URL: `/api/session/${dir}/${id}/messages` → `/api/sessions/${id}/messages`
- `useSessionMessages` param: `(projectDir, sessionId)` → `(sessionId)`
- `queryKey`: `['session-messages', projectDir, sessionId]` → `['session-messages', sessionId]`
- `enabled`: `!!projectDir && !!sessionId` → `!!sessionId`
- All `fetchMessages` call sites inside `queryFn`: drop `projectDir` first arg

**Step 3: Cleanup `use-session-detail.ts`**

Remove the duplicate `HttpError` class (lines 4-10). Import from `use-session.ts` instead. **Preserve the existing `failureCount` retry pattern** — don't silently change retry behavior:

```typescript
import { useQuery } from '@tanstack/react-query'
import type { SessionDetail } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

async function fetchSessionDetail(sessionId: string): Promise<SessionDetail> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}`)
  if (!response.ok) {
    throw new HttpError('Failed to fetch session detail', response.status)
  }
  return response.json()
}

export function useSessionDetail(sessionId: string | null) {
  return useQuery({
    queryKey: ['session-detail', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchSessionDetail(sessionId)
    },
    enabled: !!sessionId,
    retry: (failureCount, error) => {
      if (isNotFoundError(error)) return false
      return failureCount < 3
    },
  })
}
```

**Changes from current code:**
- Remove local `HttpError` class (lines 4-10)
- Add import: `import { HttpError, isNotFoundError } from './use-session'`
- Retry logic: `error instanceof HttpError && error.status === 404` → `isNotFoundError(error)` (same behavior, uses shared helper)
- **`failureCount` pattern preserved** — this hook retries up to 3 times for non-404 errors

**Step 4: Cleanup `use-contributions.ts`**

Remove the duplicate `HttpError` class (lines 74-80). Import from `use-session.ts`. **Preserve the existing `failureCount` retry pattern**:

Delete the local `HttpError` class definition and add this import at the top of the file, alongside the existing imports:

```typescript
import { HttpError, isNotFoundError } from './use-session'
```

Update `fetchSessionContribution` to use the imported `HttpError` (no change needed — it already throws `new HttpError(...)` which will now reference the imported class).

Update the retry in `useSessionContribution`:

```typescript
retry: (failureCount, error) => {
  if (isNotFoundError(error)) return false
  return failureCount < 3
},
```

**Changes from current code:**
- Remove local `HttpError` class (lines 74-80)
- Add import from `./use-session`
- Retry: `error instanceof HttpError && error.status === 404` → `isNotFoundError(error)`
- **`failureCount < 3` pattern preserved** — same retry ceiling

**Step 5: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`
Expected: No type errors

**Step 6: Commit**

```bash
git add src/hooks/use-session.ts src/hooks/use-session-messages.ts src/hooks/use-session-detail.ts src/hooks/use-contributions.ts
git commit -m "refactor(hooks): drop projectDir param, use /api/sessions/:id/* endpoints"
```

---

## Task 5: Update ConversationView — Remove projectDir Chain

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Simplify the hook calls**

The key change: `useSession` and `useSessionMessages` no longer need `projectDir`. They fire immediately with just `sessionId`, independently of `useSessionDetail`.

Change lines ~71-80 from:

```tsx
// useSession and useSessionMessages require projectDir from sessionDetail
const { data: session, isLoading: isSessionLoading, error: sessionError } = useSession(projectDir || null, sessionId || null)
const {
  data: pagesData,
  isLoading: isMessagesLoading,
  error: messagesError,
  fetchPreviousPage,
  hasPreviousPage,
  isFetchingPreviousPage,
} = useSessionMessages(projectDir || null, sessionId || null)
```

To:

```tsx
// These now fire immediately — no waiting for sessionDetail
const { data: session, error: sessionError } = useSession(sessionId || null)
const {
  data: pagesData,
  isLoading: isMessagesLoading,
  error: messagesError,
  fetchPreviousPage,
  hasPreviousPage,
  isFetchingPreviousPage,
} = useSessionMessages(sessionId || null)
```

**CRITICAL:** The existing loading/error logic (lines 83-89) must remain **unchanged**:

```tsx
// Detect when DB has the session but the JSONL file is gone from disk
const isFileGone = !!sessionDetail
  && (isNotFoundError(messagesError) || isNotFoundError(sessionError))

// Only gate initial render on paginated messages — the full session fetch
// loads in the background for export use. This ensures faster time-to-first-content.
const isLoading = isFileGone ? false : (isMessagesLoading || (!sessionDetail && !detailError))
const error = isFileGone ? null : (detailError || messagesError)
```

> **Why `(!sessionDetail && !detailError)` must stay:** Without the `&& !detailError` guard, a failed detail fetch keeps `isLoading = true` forever (infinite skeleton). The current code is correct — do NOT simplify to `!sessionDetail`.

The rest of the component (`exportsReady`, `handleExportHtml`, etc.) references the `session` variable which still comes from `useSession` — no variable renames needed. The export handlers continue to work because `const { data: session } = useSession(...)` destructures identically.

Also: `useProjectSessions(projectDir || undefined, ...)` on line 92 still needs `projectDir` from `sessionDetail` for the sidebar session list. This is fine — `sessionDetail` still fetches, it just no longer blocks the messages.

**Step 2: Remove `isSessionLoading` destructuring**

The old code destructured `isLoading: isSessionLoading` from `useSession` but the current loading logic (line 88) already doesn't use it — it gates on `isMessagesLoading` only. If `isSessionLoading` is destructured anywhere, remove the destructuring since `useSession` data is only used for exports (background, non-blocking).

**Step 3: Verify the app works**

Run: `bun run dev` and navigate to a session (normal + worktree)
Expected: Session loads, messages render, no console errors, no server errors

**Step 4: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "refactor(ui): ConversationView fires hooks in parallel, no projectDir chain"
```

---

## Task 6: Migrate Legacy Endpoints + Remove Band-Aid Code

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

> **Why not just delete the old endpoints?** External scripts or browser bookmarks may reference `/api/session/:project_dir/:session_id`. We keep them working during the transition but switch their internals to use DB-based resolution, which lets us delete the `resolve_session_path` band-aid entirely.

**Step 1: Rewrite legacy handlers to use `get_session_file_path`**

Replace the bodies of `get_session` and `get_session_messages` to use the new DB-based path resolution. The `project_dir` param is accepted but ignored — paths come from DB:

```rust
/// DEPRECATED: Use `GET /api/sessions/:id/parsed` instead.
/// Kept for backward compatibility. Will be removed in v0.6.
///
/// The `project_dir` parameter is now ignored — path resolution is DB-based.
#[deprecated(note = "Use get_session_parsed instead")]
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path((_project_dir, session_id)): Path<(String, String)>,
) -> ApiResult<Json<ParsedSession>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let session = vibe_recall_core::parse_session(&path).await?;
    Ok(Json(session))
}

/// DEPRECATED: Use `GET /api/sessions/:id/messages` instead.
/// Kept for backward compatibility. Will be removed in v0.6.
///
/// The `project_dir` parameter is now ignored — path resolution is DB-based.
#[deprecated(note = "Use get_session_messages_by_id instead")]
pub async fn get_session_messages(
    State(state): State<Arc<AppState>>,
    Path((_project_dir, session_id)): Path<(String, String)>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<vibe_recall_core::PaginatedMessages>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let result = vibe_recall_core::parse_session_paginated(&path, limit, offset).await?;
    Ok(Json(result))
}
```

**Step 2: Delete `resolve_session_path` and `resolve_session_path_with_base`**

Delete both functions (lines 19-76 in the current file). They are no longer called by any code path since the legacy handlers now use `get_session_file_path`.

Also remove the `sqlx` import if it was only used by `resolve_session_path_with_base` (check — the inline `sqlx::query_as` on line 56 is the only direct sqlx usage in this file; the new handlers go through `state.db.get_session_file_path()`).

**Step 3: Update the router with `#[allow(deprecated)]`**

`#[allow(deprecated)]` cannot be placed inline on `.route()` calls in a builder chain — it's only valid on items or blocks. Apply it at the function level:

```rust
/// Create the sessions routes router.
#[allow(deprecated)] // Legacy /session/ routes kept for backward compat until v0.6
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session_detail))
        .route("/sessions/{id}/parsed", get(get_session_parsed))
        .route("/sessions/{id}/messages", get(get_session_messages_by_id))
        // Legacy endpoints — backward compat, remove in v0.6
        .route("/session/{project_dir}/{session_id}", get(get_session))
        .route("/session/{project_dir}/{session_id}/messages", get(get_session_messages))
        .route("/branches", get(list_branches))
}
```

**Step 4: Delete the `resolve_session_path` tests and helper**

Delete these items from the test module:
- `insert_session_with_file_path` helper function (line ~1253)
- `test_resolve_path_naive_path_exists` (line ~1263)
- `test_resolve_path_worktree_fallback` (line ~1284)
- `test_resolve_path_worktree_deleted_jsonl_persists` (line ~1314)
- `test_resolve_path_file_deleted_everywhere` (line ~1346)
- `test_resolve_path_not_in_db` (line ~1375)
- `test_resolve_path_naive_takes_priority` (line ~1394)

These tests validated `resolve_session_path_with_base` which is now deleted. The new endpoints have their own tests from Tasks 2-3, and the legacy handlers now use the same DB-based code path.

**Step 5: Run all tests**

Run: `cargo test -p vibe-recall-server -- routes::sessions`
Expected: All tests pass (existing tests + new tests from Tasks 2-3, minus the 6 deleted resolve_path tests and the deleted helper)

**Step 6: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "refactor(api): remove resolve_session_path, migrate legacy endpoints to DB resolution"
```

---

## Task 7: Final Verification

**Step 1: Run full test suite**

```bash
cargo test -p vibe-recall-server
cargo test -p vibe-recall-db
```

Both must pass with zero failures.

**Step 2: TypeScript verification**

```bash
bunx tsc --noEmit
```

Must pass with zero errors.

**Step 3: Manual smoke test**

```bash
bun run dev
```

Verify:
- [ ] Normal session loads (messages + sidebar metrics)
- [ ] Worktree session loads (was previously broken without the band-aid)
- [ ] Deleted session shows "file gone" UI (not infinite skeleton)
- [ ] Session list shows all sessions (including worktree ones)
- [ ] Export (HTML/Markdown/PDF) works on a session
- [ ] Copy markdown to clipboard works
- [ ] No errors in server logs (only WARN for truly-missing sessions)
- [ ] No errors in browser console
- [ ] Legacy endpoint still works: `curl http://localhost:47892/api/session/<project_dir>/<session_id>` returns session JSON

**Step 4: Squash or keep commits, push**

```bash
git log --oneline  # verify commit history
git push origin main
```

---

## Architecture After This Change

```
Frontend                          Server                          Storage
────────                          ──────                          ───────
useSessionDetail(id)  ──────────► GET /api/sessions/:id  ───────► SQLite (metadata)
                                    │
useSession(id)        ──────────► GET /api/sessions/:id/parsed ─► SQLite (file_path) → JSONL
                                    │
useSessionMessages(id) ─────────► GET /api/sessions/:id/messages► SQLite (file_path) → JSONL
                                    │
                                    └── Server owns path resolution.
                                        Client never constructs filesystem paths.
```

**Key invariant:** The client passes session UUIDs. The server resolves everything else. The `file_path` column in SQLite is the single source of truth for where the JSONL lives.

**Legacy endpoints** (`/api/session/:project_dir/:session_id`) remain registered but now internally ignore `project_dir` and use the same DB-based resolution. They will be removed in v0.6.

---

## What This Does NOT Change

- **DB-based endpoints** (`/api/sessions`, `/api/sessions/:id`) — unchanged
- **Session indexing** — `file_path` population logic unchanged
- **Worktree merging** — `resolve_worktree_parent` still merges worktree sessions into parent project for UI grouping
- **`useProjectSessions` hook** — still uses `/api/sessions` with project filter (DB-only, unaffected)
- **Export handlers** — still reference `session` variable from `useSession`, destructuring is identical

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `make_session` called with 2 args instead of 3 | Blocker | Added `1700000000` third arg in Task 1 test |
| 2 | No happy-path integration tests | Blocker | Added `test_get_session_parsed_success` (Task 2) and `test_get_session_messages_by_id_success` (Task 3) |
| 3 | Task 6 contradiction: delete `resolve_session_path` but keep legacy endpoints that call it | Blocker | Rewrote legacy handlers to use DB-based resolution first, THEN delete `resolve_session_path` |
| 4 | `#[allow(deprecated)]` on `.route()` chain (invalid Rust) | Blocker | Moved to function-level attribute on `router()` |
| 5 | Loading logic `!sessionDetail` missing `&& !detailError` | Blocker | Explicitly preserved existing correct logic, added warning not to simplify |
| 6 | `use-session-detail.ts` retry pattern drops `failureCount` | Warning | Preserved `failureCount < 3` pattern, only swapped `instanceof` for `isNotFoundError()` |
| 7 | `use-contributions.ts` retry pattern drops `failureCount` | Warning | Same fix — preserved `failureCount < 3` |
| 8 | Test vacuous 404 from Axum vs our handler | Warning | All tests assert on JSON body `json["error"]`, not just status code |
| 9 | Task 2 test `test_get_session_parsed_not_in_db` lacked body assertion | Warning | Added `json["error"]` assertion to all 404 tests |
| 10 | Missing note about `sqlx` import cleanup | Minor | Added note in Task 6 Step 2 |
| 11 | JSONL fixture uses `"type":"human"` but parser expects `"type":"user"` | Blocker | Fixed both Task 2 and Task 3 fixtures to use `{"type":"user","message":{"content":"Hello"}}` matching parser.rs line 1021 |
