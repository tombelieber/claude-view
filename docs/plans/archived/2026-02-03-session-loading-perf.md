---
status: done
date: 2026-02-03
---

# Session Loading Performance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce time-to-first-content for large sessions (100-1000+ messages) from "parse everything, send everything, render everything" to "show the latest 100 messages instantly, load more on scroll."

**Architecture:** Three independent fixes applied in order of effort: (1) gzip compression on all API responses, (2) cursor-based pagination on the session messages endpoint, (3) tail-first loading so the most recent messages appear first. The frontend Virtuoso setup stays — it already handles DOM virtualization correctly.

**Tech Stack:** Rust (Axum, tower-http compression), React (react-virtuoso `endReached`), TypeScript, TanStack Query

---

## Task 1: Add Gzip Compression to Axum

This is the lowest-effort, highest-immediate-impact change. JSON compresses ~80%, so a 5MB session response becomes ~1MB over the wire.

**Files:**
- Modify: `Cargo.toml:20` (workspace dep — add `"compression-gzip"` feature)
- Modify: `crates/server/src/lib.rs:22-24` (add CompressionLayer import)
- Modify: `crates/server/src/lib.rs:94-97` (add compression layer to app)
- Test: `crates/server/src/lib.rs` (add compression test)

**Step 1: Write the failing test**

Add to the test module in `crates/server/src/lib.rs`:

```rust
#[tokio::test]
async fn test_api_response_is_gzip_compressed() {
    let app = create_app(test_db().await);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .header("Accept-Encoding", "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-encoding").map(|v| v.to_str().unwrap()),
        Some("gzip"),
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server -- tests::test_api_response_is_gzip_compressed`
Expected: FAIL — no `content-encoding: gzip` header in response.

**Step 3: Enable compression feature in workspace Cargo.toml**

In `Cargo.toml` line 20, change:
```toml
tower-http = { version = "0.6", features = ["cors", "fs", "trace", "compression-gzip"] }
```

**Step 4: Add CompressionLayer to the app**

In `crates/server/src/lib.rs`:

Add to imports:
```rust
use tower_http::compression::CompressionLayer;
```

In `create_app_full()` (line 94) and `create_app_with_indexing_and_static()` (line 124), add the compression layer to the middleware stack — place it **before** CORS so responses get compressed:

```rust
let mut app = Router::new()
    .merge(api_routes(state))
    .layer(CompressionLayer::new())
    .layer(cors_layer())
    .layer(TraceLayer::new_for_http());
```

Apply the same change to both `create_app_full` and `create_app_with_indexing_and_static`.

**Step 5: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server -- tests::test_api_response_is_gzip_compressed`
Expected: PASS

**Step 6: Run full server test suite**

Run: `cargo test -p vibe-recall-server`
Expected: All tests pass. Some tests may need the `Accept-Encoding: gzip` header added if they assert on body content (since responses may now be compressed). If any body-asserting tests fail, they likely need to NOT send `Accept-Encoding: gzip` or decompress before asserting.

**Step 7: Commit**

```bash
git add Cargo.toml crates/server/src/lib.rs
git commit -m "perf: add gzip compression to all API responses"
```

---

## Task 2: Add Paginated Session Messages Endpoint (Backend)

New endpoint `GET /api/session/:project/:id/messages?limit=100&offset=0` that returns a page of messages plus total count. The existing `GET /api/session/:project/:id` stays unchanged for backward compatibility.

**Files:**
- Modify: `crates/core/src/types.rs` (add `PaginatedMessages` response type)
- Modify: `crates/core/src/parser.rs` (add `parse_session_paginated` function)
- Modify: `crates/server/src/routes/sessions.rs` (add paginated endpoint + handler)
- Test: `crates/server/src/routes/sessions.rs` (add pagination tests)
- Test: `crates/core/src/parser.rs` (add parser pagination tests)

**Step 1: Add `PaginatedMessages` type to core**

In `crates/core/src/types.rs`, after `ParsedSession` (after line 166):

```rust
/// A paginated slice of session messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PaginatedMessages {
    pub messages: Vec<Message>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}
```

**Step 2: Write failing test for `parse_session_paginated`**

In `crates/core/src/parser.rs` tests:

```rust
#[tokio::test]
async fn test_parse_session_paginated_first_page() {
    let path = fixtures_path().join("large_session.jsonl");
    let result = parse_session_paginated(&path, 10, 0).await.unwrap();
    assert_eq!(result.messages.len(), 10);
    assert_eq!(result.total, 200);
    assert_eq!(result.offset, 0);
    assert_eq!(result.limit, 10);
    assert!(result.has_more);
    assert!(result.messages[0].content.contains("Question number 1"));
}

#[tokio::test]
async fn test_parse_session_paginated_last_page() {
    let path = fixtures_path().join("large_session.jsonl");
    let result = parse_session_paginated(&path, 10, 195).await.unwrap();
    assert_eq!(result.messages.len(), 5); // only 5 remaining
    assert_eq!(result.total, 200);
    assert!(!result.has_more);
}

#[tokio::test]
async fn test_parse_session_paginated_beyond_end() {
    let path = fixtures_path().join("large_session.jsonl");
    let result = parse_session_paginated(&path, 10, 999).await.unwrap();
    assert_eq!(result.messages.len(), 0);
    assert_eq!(result.total, 200);
    assert!(!result.has_more);
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-core -- tests::test_parse_session_paginated`
Expected: FAIL — function doesn't exist.

**Step 4: Implement `parse_session_paginated`**

In `crates/core/src/parser.rs`, add after `parse_session`:

```rust
/// Parse a session JSONL file and return a paginated slice of messages.
///
/// Parses the full file (necessary for correct message ordering and thinking
/// attachment), then returns the requested slice. The total message count is
/// included so the frontend can compute pagination state.
///
/// # Arguments
/// * `file_path` - Path to the JSONL session file
/// * `limit` - Maximum number of messages to return
/// * `offset` - Number of messages to skip from the start
pub async fn parse_session_paginated(
    file_path: &Path,
    limit: usize,
    offset: usize,
) -> Result<PaginatedMessages, ParseError> {
    let session = parse_session(file_path).await?;
    let total = session.messages.len();
    let messages: Vec<Message> = session
        .messages
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    let has_more = offset + messages.len() < total;

    Ok(PaginatedMessages {
        messages,
        total,
        offset,
        limit,
        has_more,
    })
}
```

Make sure to add `PaginatedMessages` to the public exports in `crates/core/src/lib.rs`.

**Step 5: Run core tests**

Run: `cargo test -p vibe-recall-core -- tests::test_parse_session_paginated`
Expected: PASS

**Step 6: Write failing test for the API endpoint**

In `crates/server/src/routes/sessions.rs` tests:

```rust
#[test]
fn test_paginated_messages_serialization() {
    use vibe_recall_core::PaginatedMessages;
    let result = PaginatedMessages {
        messages: vec![
            Message::user("Hello"),
            Message::assistant("Hi"),
        ],
        total: 100,
        offset: 0,
        limit: 2,
        has_more: true,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"total\":100"));
    assert!(json.contains("\"hasMore\":true"));
}
```

**Step 7: Add the paginated endpoint handler**

In `crates/server/src/routes/sessions.rs`:

Add query params struct:
```rust
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SessionMessagesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
```

Add handler:
```rust
/// GET /api/session/:project_dir/:session_id/messages?limit=100&offset=0
///
/// Returns a paginated slice of parsed messages for a session.
/// Default limit is 100, default offset is 0.
pub async fn get_session_messages(
    State(_state): State<Arc<AppState>>,
    Path((project_dir, session_id)): Path<(String, String)>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<vibe_recall_core::PaginatedMessages>> {
    let project_dir_decoded = urlencoding::decode(&project_dir)
        .map_err(|_| ApiError::ProjectNotFound(project_dir.clone()))?
        .into_owned();

    let projects_dir = vibe_recall_core::claude_projects_dir()?;
    let session_path = projects_dir
        .join(&project_dir_decoded)
        .join(&session_id)
        .with_extension("jsonl");

    if !session_path.exists() {
        return Err(ApiError::SessionNotFound(format!(
            "{}/{}",
            project_dir_decoded, session_id
        )));
    }

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let result = vibe_recall_core::parse_session_paginated(&session_path, limit, offset).await?;
    Ok(Json(result))
}
```

Add route in `router()`:
```rust
.route("/session/{project_dir}/{session_id}/messages", get(get_session_messages))
```

**Step 8: Run server tests**

Run: `cargo test -p vibe-recall-server`
Expected: All tests pass.

**Step 9: Commit**

```bash
git add crates/core/src/types.rs crates/core/src/parser.rs crates/core/src/lib.rs crates/server/src/routes/sessions.rs
git commit -m "feat: add paginated session messages endpoint"
```

---

## Task 3: Frontend Paginated Loading with Infinite Scroll

Replace the single-fetch `useSession` with paginated fetching using the new `/messages` endpoint. Wire Virtuoso's `endReached` to load more pages.

**Files:**
- Create: `src/hooks/use-session-messages.ts` (new paginated hook)
- Modify: `src/components/ConversationView.tsx` (swap to paginated hook, wire Virtuoso endReached)
- Modify: `src/hooks/use-session.ts` (keep for backward compat — exports only, but used by paginated hook internally for metadata)

**Step 1: Create `use-session-messages.ts` hook**

Create `src/hooks/use-session-messages.ts`:

```typescript
import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'

const PAGE_SIZE = 100

async function fetchMessages(
  projectDir: string,
  sessionId: string,
  offset: number,
  limit: number,
): Promise<PaginatedMessages> {
  const response = await fetch(
    `/api/session/${encodeURIComponent(projectDir)}/${encodeURIComponent(sessionId)}/messages?limit=${limit}&offset=${offset}`
  )
  if (!response.ok) throw new Error('Failed to fetch messages')
  return response.json()
}

export function useSessionMessages(projectDir: string | null, sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', projectDir, sessionId],
    queryFn: ({ pageParam = 0 }) => {
      if (!projectDir || !sessionId) throw new Error('projectDir and sessionId are required')
      return fetchMessages(projectDir, sessionId, pageParam, PAGE_SIZE)
    },
    initialPageParam: 0,
    getNextPageParam: (lastPage) => {
      if (!lastPage.hasMore) return undefined
      return lastPage.offset + lastPage.limit
    },
    enabled: !!projectDir && !!sessionId,
  })
}
```

**Step 2: Update ConversationView to use paginated hook**

In `src/components/ConversationView.tsx`:

Replace:
```typescript
import { useSession } from '../hooks/use-session'
```
With:
```typescript
import { useSession } from '../hooks/use-session'
import { useSessionMessages } from '../hooks/use-session-messages'
```

In the component body, add the paginated query alongside the existing session query (which is still needed for metadata):

```typescript
const { data: session, isLoading: isSessionLoading, error: sessionError } = useSession(projectDir, sessionId)
const {
  data: pagesData,
  isLoading: isMessagesLoading,
  error: messagesError,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
} = useSessionMessages(projectDir, sessionId)

const isLoading = isSessionLoading || isMessagesLoading
const error = sessionError || messagesError
```

Flatten pages into a single message array:
```typescript
const allMessages = useMemo(
  () => pagesData?.pages.flatMap(page => page.messages) ?? [],
  [pagesData]
)

const totalMessages = pagesData?.pages[0]?.total ?? 0
```

Update `filteredMessages` to use `allMessages` instead of `session?.messages`:
```typescript
const filteredMessages = useMemo(
  () => allMessages.length > 0 ? filterMessages(allMessages, viewMode) : [],
  [allMessages, viewMode]
)
```

Wire Virtuoso's `endReached` to load more:
```typescript
<Virtuoso
  data={filteredMessages}
  endReached={() => {
    if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage()
    }
  }}
  // ... rest of existing props
/>
```

Add a loading indicator in the Footer component:
```typescript
Footer: () => (
  <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
    {isFetchingNextPage ? (
      <span>Loading more messages...</span>
    ) : (
      <>
        {totalMessages} messages
        {!hasNextPage && totalMessages > allMessages.length && ' (all loaded)'}
        {viewMode === 'compact' && hiddenCount > 0 && (
          <> &bull; {hiddenCount} hidden in compact view</>
        )}
      </>
    )}
  </div>
)
```

Update `hiddenCount` to use `allMessages`:
```typescript
const hiddenCount = allMessages.length - filteredMessages.length
```

Keep `session` usage for `session.metadata.toolCallCount` in the footer — that's still fetched from the full session endpoint. Alternatively, the tool call count can come from `sessionInfo.toolCallCount` (already fetched via `useSessionDetail`).

**Step 3: Update export handlers to use allMessages**

The export functions (`handleExportHtml`, `handleExportMarkdown`, etc.) currently use `session.messages`. Update them to use `allMessages`, or keep the full `useSession` call as a fallback for exports (since exports need all messages anyway).

A pragmatic approach: keep `useSession` but only trigger it when the user clicks export. For now, keep it as-is — the export will use whatever messages are loaded. If the user wants to export the full session, the full `useSession` is still available as a fallback.

**Step 4: Generate TypeScript types**

Run: `cargo test -p vibe-recall-core -- test_that_generates_types` (or whatever generates the ts-rs types)

Or manually ensure `PaginatedMessages` gets exported. Check that `src/types/generated/PaginatedMessages.ts` exists after running `cargo test`.

**Step 5: Verify the frontend builds**

Run: `cd frontend && pnpm build` (or however the frontend builds)
Expected: No TypeScript errors.

**Step 6: Manual test**

Start the dev server, open a large session (100+ messages), verify:
- Initial load shows first 100 messages quickly
- Scrolling to the bottom triggers loading of the next page
- "Loading more messages..." appears during fetch
- All messages eventually load

**Step 7: Commit**

```bash
git add src/hooks/use-session-messages.ts src/components/ConversationView.tsx src/types/generated/PaginatedMessages.ts
git commit -m "feat: paginated message loading with infinite scroll"
```

---

## Task 4: Tail-First Loading (Most Recent Messages First)

Users almost always want the latest messages. Instead of offset=0 (start of conversation), the initial load should show the **last N** messages and let users scroll **up** to load older ones.

**Files:**
- Modify: `src/hooks/use-session-messages.ts` (reverse pagination direction)
- Modify: `src/components/ConversationView.tsx` (set Virtuoso to start at bottom, load upward)

**Step 1: Update the hook to load from the tail**

In `src/hooks/use-session-messages.ts`:

Change `initialPageParam` to be computed from total. The first query doesn't know the total yet, so use a sentinel value:

```typescript
export function useSessionMessages(projectDir: string | null, sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', projectDir, sessionId],
    queryFn: async ({ pageParam }) => {
      if (!projectDir || !sessionId) throw new Error('projectDir and sessionId are required')

      if (pageParam === 'initial') {
        // First request: fetch total count first, then load last PAGE_SIZE messages
        // Use a large offset to get the tail; the backend clamps to available messages
        const probe = await fetchMessages(projectDir, sessionId, 0, 0)
        const total = probe.total
        const tailOffset = Math.max(0, total - PAGE_SIZE)
        return fetchMessages(projectDir, sessionId, tailOffset, PAGE_SIZE)
      }

      return fetchMessages(projectDir, sessionId, pageParam as number, PAGE_SIZE)
    },
    initialPageParam: 'initial' as string | number,
    getNextPageParam: (lastPage) => {
      // "Next" page is actually older messages (lower offset)
      if (lastPage.offset === 0) return undefined // already at the beginning
      const prevOffset = Math.max(0, lastPage.offset - PAGE_SIZE)
      return prevOffset
    },
    enabled: !!projectDir && !!sessionId,
  })
}
```

Wait — this is tricky with `useInfiniteQuery` since pages load in order and the flattened array needs to be chronological. A simpler approach:

**Alternative: Use `getPreviousPageParam` for upward scrolling**

```typescript
export function useSessionMessages(projectDir: string | null, sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', projectDir, sessionId],
    queryFn: async ({ pageParam }) => {
      if (!projectDir || !sessionId) throw new Error('projectDir and sessionId are required')

      if (pageParam === -1) {
        // Initial load: probe total, then fetch the last PAGE_SIZE
        // Use limit=0 to get total cheaply, but our API still parses the whole file.
        // Instead, fetch the last page directly using a large offset.
        // We don't know the total yet, so fetch with offset=0, limit=1 to get total.
        const probe = await fetchMessages(projectDir, sessionId, 0, 1)
        const tailOffset = Math.max(0, probe.total - PAGE_SIZE)
        return fetchMessages(projectDir, sessionId, tailOffset, PAGE_SIZE)
      }

      return fetchMessages(projectDir, sessionId, pageParam, PAGE_SIZE)
    },
    initialPageParam: -1,
    getNextPageParam: () => undefined, // no downward pagination needed initially
    getPreviousPageParam: (firstPage) => {
      if (firstPage.offset === 0) return undefined
      const prevOffset = Math.max(0, firstPage.offset - PAGE_SIZE)
      return prevOffset
    },
    enabled: !!projectDir && !!sessionId,
  })
}
```

**Step 2: Update ConversationView for upward scrolling**

Flatten pages in correct order (previous pages come before current):
```typescript
const allMessages = useMemo(
  () => pagesData?.pages.flatMap(page => page.messages) ?? [],
  [pagesData]
)
```

Since `useInfiniteQuery` with `getPreviousPageParam` prepends pages, the order should be chronological automatically.

Wire Virtuoso's `startReached` instead of `endReached`:
```typescript
<Virtuoso
  data={filteredMessages}
  startReached={() => {
    if (hasPreviousPage && !isFetchingPreviousPage) {
      fetchPreviousPage()
    }
  }}
  initialTopMostItemIndex={filteredMessages.length - 1}
  followOutput="smooth"
  // ... rest of existing props
/>
```

Extract the new variables from the hook:
```typescript
const {
  data: pagesData,
  isLoading: isMessagesLoading,
  error: messagesError,
  fetchPreviousPage,
  hasPreviousPage,
  isFetchingPreviousPage,
} = useSessionMessages(projectDir, sessionId)
```

Update the Header component to show loading state:
```typescript
Header: () => (
  isFetchingPreviousPage ? (
    <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
      Loading older messages...
    </div>
  ) : hasPreviousPage ? (
    <div className="h-6" />
  ) : (
    <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
      Beginning of conversation
    </div>
  )
)
```

**Step 3: Handle initial scroll position**

Set `initialTopMostItemIndex` to the last message so the user sees the most recent content:
```typescript
initialTopMostItemIndex={filteredMessages.length - 1}
```

**Step 4: Verify builds and test manually**

Run: `pnpm build` (frontend)
Expected: No errors.

Manual test:
- Open a 200+ message session
- Should land at the **bottom** (most recent messages visible)
- Scrolling up should trigger "Loading older messages..."
- Older messages appear above

**Step 5: Commit**

```bash
git add src/hooks/use-session-messages.ts src/components/ConversationView.tsx
git commit -m "feat: tail-first loading — show latest messages first, scroll up for older"
```

---

## Task 5: Backend Optimization — Skip Full Parse for Pagination

The current `parse_session_paginated` still parses the entire file, then slices. For large files this is wasteful. This task optimizes the parser to skip messages outside the requested window.

> **Note:** This is a follow-up optimization. Tasks 1-4 deliver the user-facing improvement. This task reduces server-side CPU/memory for very large sessions.

**Files:**
- Modify: `crates/core/src/parser.rs` (add early-termination / count-only mode)

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_parse_session_count_only() {
    let path = fixtures_path().join("large_session.jsonl");
    let count = count_session_messages(&path).await.unwrap();
    assert_eq!(count, 200);
}
```

**Step 2: Implement `count_session_messages`**

A lightweight function that counts JSONL lines matching `"type":"user"` or `"type":"assistant"` etc., without full JSON parsing. Use `memmem::Finder` for SIMD pre-filter (per project rules):

```rust
/// Count total messages in a session file without full parsing.
///
/// Uses SIMD byte scanning to count lines with known message types,
/// avoiding JSON deserialization entirely.
pub async fn count_session_messages(file_path: &Path) -> Result<usize, ParseError> {
    let file = File::open(file_path)
        .await
        .map_err(|e| ParseError::io(file_path, e))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut count = 0;

    while let Some(line_result) = lines.next_line().await.map_err(|e| ParseError::io(file_path, e))? {
        let line = line_result.trim();
        if line.is_empty() { continue; }
        // Count any valid JSON line with a "type" field
        // Full parse_session determines the actual message count after filtering
        // This is a rough count — the paginated endpoint uses the exact count from full parse
        count += 1;
    }

    Ok(count)
}
```

Actually, this won't match the real message count (since the parser filters meta messages, merges thinking-only messages, etc.). The simpler and correct approach: keep the full parse for now, but cache the result. A more impactful optimization would be to **cache parsed sessions in memory** with an LRU cache keyed by `(file_path, file_mtime)`. This avoids re-parsing on subsequent page loads.

**Step 3: Add in-memory LRU cache for parsed sessions**

This is the real win for pagination: parse once, serve many pages from cache.

```rust
// In crates/server/src/state.rs or a new crates/server/src/cache.rs
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

pub struct SessionCache {
    cache: Mutex<LruCache<String, (std::time::SystemTime, ParsedSession)>>,
}

impl SessionCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
        }
    }

    pub fn get(&self, path: &str, mtime: std::time::SystemTime) -> Option<ParsedSession> {
        let mut cache = self.cache.lock().unwrap();
        if let Some((cached_mtime, session)) = cache.get(path) {
            if *cached_mtime == mtime {
                return Some(session.clone());
            }
        }
        None
    }

    pub fn insert(&self, path: String, mtime: std::time::SystemTime, session: ParsedSession) {
        let mut cache = self.cache.lock().unwrap();
        cache.put(path, (mtime, session));
    }
}
```

Add to `AppState` and use in the paginated endpoint handler.

> **Scope note:** This is a larger refactor. If the earlier tasks already make the UX acceptable, defer this to a separate plan. The initial `parse_session_paginated` slicing approach is correct and simple — caching is an optimization for repeated page loads of the same session.

**Step 4: Commit (if implemented)**

```bash
git add crates/server/src/cache.rs crates/server/src/state.rs crates/server/src/routes/sessions.rs
git commit -m "perf: add LRU cache for parsed sessions to avoid re-parsing on pagination"
```

---

## Summary

| Task | What | Impact |
|------|------|--------|
| 1 | Gzip compression | ~80% smaller responses, one-liner |
| 2 | Paginated backend endpoint | Serve 100 msgs instead of 1000+ |
| 3 | Frontend infinite scroll | Show content immediately, load on scroll |
| 4 | Tail-first loading | Show latest messages first (what users want) |
| 5 | Session cache (optional) | Avoid re-parsing on page 2, 3, etc. |

Tasks 1-4 are the core deliverables. Task 5 is a follow-up optimization.
