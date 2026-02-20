---
status: approved
date: 2026-02-18
theme: "Search & Discovery"
---

# Full-Text Search — Design

> **Problem:** Users accumulate thousands of sessions with rich conversation content, but the only way to find a specific session is by scrolling, filtering metadata, or searching the truncated 200-char preview. If you remember something Claude said — or something you asked — there's no way to find it.

## Decisions Made

| Decision | Choice | Why |
|----------|--------|-----|
| Search engine | **Tantivy** (embedded Rust) | De facto standard for embedded Rust search. 14.6K stars, used by Meilisearch/Quickwit/ParadeDB. BM25 ranking, fuzzy, facets built in. No separate process. |
| Index granularity | **One doc per message** | Enables message-level hit positions, per-message BM25 scoring, manageable doc sizes |
| What to index | **User + assistant + tool call inputs** | Tool results excluded (40-60% of JSONL size, mostly noise). Covers what you asked, what Claude said, and what code it wrote. |
| When to index | **During Pass 2 (deep indexer)** | Piggybacks on existing JSONL parsing. No extra file reads. Incremental via mtime detection. |
| UI entry points | **Cmd+K (global) + search bar (scoped)** | Cmd+K for "find anything fast." Search bar narrows to current context. |
| Result format | **Session-grouped with expandable message hits** | Like GitHub code search — session-level cards, click to expand individual matches with highlighted snippets. |
| Query features | **All five levels** | Plain text, exact phrase, qualifiers, fuzzy, regex. Tantivy provides all except regex for free; regex via RegexQuery. |
| In-session search | **Client-side** | Messages already loaded in React state. No network round trip needed. |

---

## Architecture

```
                    ┌─────────────┐
                    │  Tantivy    │
                    │  Index      │
                    │  (~500MB)   │
                    └──────▲──────┘
                           │ write (Pass 2)
                           │ read (query)
┌──────────┐    ┌──────────┴──────────┐    ┌────────────┐
│  JSONL   │───►│  Deep Indexer       │───►│  SQLite    │
│  Files   │    │  (existing Pass 2)  │    │  (metadata)│
└──────────┘    └─────────────────────┘    └────────────┘
                           │
                           ▼
                ┌─────────────────────┐
                │  GET /api/search    │
                │  Axum route         │
                └──────────┬──────────┘
                           │
              ┌────────────┴────────────┐
              │                         │
     ┌────────▼────────┐    ┌──────────▼──────────┐
     │  Cmd+K Modal    │    │  Scoped Search Bar  │
     │  (global)       │    │  (context-aware)    │
     └─────────────────┘    └─────────────────────┘
```

**Storage**: Tantivy index at `<cache_dir>/claude-view/search-index/` (macOS: `~/Library/Caches/claude-view/search-index/`, Linux: `~/.cache/claude-view/search-index/`). Resolved via `dirs::cache_dir().join("claude-view").join("search-index")` — same base as the existing SQLite DB at `<cache_dir>/claude-view/claude-view.db`. Rebuilt from JSONL if deleted.

**Data flow**: Deep indexer parses JSONL → inserts messages into Tantivy (same pass) → search API queries Tantivy → returns session-grouped results with highlighted snippets.

---

## Tantivy Schema

Each message is a Tantivy document (~400K documents for 3K sessions):

| Field | Builder Call (Tantivy 0.22) | Purpose |
|-------|---------------------------|---------|
| `session_id` | `add_text_field("session_id", STRING \| STORED)` | Group results by session, delete-by-session for re-index |
| `project` | `add_text_field("project", STRING \| STORED)` | Qualifier: `project:claude-view` — maps from `SessionInfo.project` |
| `branch` | `add_text_field("branch", STRING \| STORED)` | Qualifier: `branch:feature/auth` — maps from `SessionInfo.git_branch: Option<String>` (use `""` for None) |
| `model` | `add_text_field("model", STRING \| STORED)` | Qualifier: `model:opus` — maps from `SessionInfo.primary_model: Option<String>` (use `""` for None) |
| `role` | `add_text_field("role", STRING \| STORED)` | Qualifier: `role:user` — map `Role::User` → `"user"`, `Role::Assistant` → `"assistant"`, `Role::ToolUse` → `"tool"`. Skip `ToolResult`, `System`, `Progress`, `Summary` (noise, not user-relevant) |
| `content` | `add_text_field("content", TEXT \| STORED)` | The actual message text — tokenized for full-text BM25 search |
| `turn_number` | `add_u64_field("turn_number", FAST \| STORED)` | For display in results ("turn 3") — derived from incrementing counter over `search_messages` during the write phase (1-based). Not a field on `Message`. |
| `timestamp` | `add_i64_field("timestamp", FAST \| STORED)` | For `after:`/`before:` range queries — derived from `Message.timestamp: Option<String>` (ISO-8601). Must parse to unix seconds via `chrono::DateTime::parse_from_rfc3339().timestamp()`. Use `0` if None (excluded from range queries). |
| `skills` | `add_text_field("skills", STRING \| STORED)` (multi-valued) | Qualifier: `skill:commit` |

**Note on Tantivy 0.22 types**: `STRING` and `TEXT` are `TextOptions` presets — `STRING` indexes the field as a single untokenized term (for exact-match qualifiers), `TEXT` tokenizes and indexes for full-text search with BM25 scoring. `FAST` and `STORED` are flags, not separate types. `FAST` enables columnar storage for fast range queries and sorting. `STORED` means the field value is retrievable from search results.

### Concrete schema builder (verbatim-compilable)

```rust
use tantivy::schema::*;

pub fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Untokenized string fields for exact-match qualifiers and grouping
    schema_builder.add_text_field("session_id", STRING | STORED);
    schema_builder.add_text_field("project", STRING | STORED);
    schema_builder.add_text_field("branch", STRING | STORED);
    schema_builder.add_text_field("model", STRING | STORED);
    schema_builder.add_text_field("role", STRING | STORED);

    // Full-text field — tokenized, BM25-ranked, stored for snippet generation
    schema_builder.add_text_field("content", TEXT | STORED);

    // Numeric fast fields for range queries and display
    schema_builder.add_u64_field("turn_number", FAST | STORED);
    schema_builder.add_i64_field("timestamp", FAST | STORED);

    // Multi-valued string field — one doc can have multiple skills
    schema_builder.add_text_field("skills", STRING | STORED);

    schema_builder.build()
}
```

### Why one doc per message, not per session

- **Snippet extraction**: Tantivy highlights the exact match position within the document. Per-message docs give precise "turn 3, user said X" results.
- **Granular ranking**: BM25 scores per-message. A session with 1 strong match ranks higher than a session with a weak match spread across 50 turns.
- **Memory**: Individual messages fit in memory during search. Whole sessions (some are 7M tokens) would blow up result serialization.

### Estimated index stats

| Metric | Estimate |
|--------|----------|
| Documents | ~400K |
| Index size | ~400–600MB |
| Query latency | <10ms |

---

## API Design

### `GET /api/search`

**Parameters**:

| Param | Type | Default | Example |
|-------|------|---------|---------|
| `q` | string (required) | — | `"JWT authentication"`, `project:claude-view auth` |
| `scope` | string | `all` | `all`, `project:claude-view`, `session:abc123` |
| `limit` | int | `20` | Max sessions returned |
| `offset` | int | `0` | Pagination |

**Response**:

```json
{
  "query": "JWT authentication",
  "totalSessions": 3,
  "totalMatches": 12,
  "elapsedMs": 4.2,
  "sessions": [
    {
      "sessionId": "abc123",
      "project": "claude-view",
      "branch": "feature/auth",
      "modifiedAt": 1739600000,
      "matchCount": 4,
      "bestScore": 12.7,
      "topMatch": {
        "role": "user",
        "turnNumber": 3,
        "snippet": "Add <mark>JWT authentication</mark> to the login endpoint...",
        "timestamp": 1739598000
      },
      "matches": [
        {
          "role": "user",
          "turnNumber": 3,
          "snippet": "Add <mark>JWT authentication</mark> to the login...",
          "timestamp": 1739598000
        },
        {
          "role": "assistant",
          "turnNumber": 4,
          "snippet": "...implement <mark>JWT authentication</mark> using the...",
          "timestamp": 1739598060
        }
      ]
    }
  ]
}
```

**Design choices**:

- `topMatch` always present — for collapsed view (one snippet per session)
- `matches` array — for expanded view (all hits in session)
- `<mark>` tags in snippets — Tantivy's snippet generator does this natively
- `scope` param — frontend sets automatically based on context
- `bestScore` — BM25 score of the top match, for inter-session ranking

### Query parsing (server-side, Rust)

The `q` string is parsed into:

| Input | Parsed as |
|-------|-----------|
| `JWT authentication` | Tantivy `QueryParser` — matches either word |
| `"JWT authentication"` | Phrase query — matches exact sequence |
| `project:claude-view auth` | Facet filter on project + text query on "auth" |
| `role:user "fix the bug"` | Facet filter on role + phrase query |
| `authentcation~` | Fuzzy query (edit distance 1) |
| `/auth.*middleware/` | Tantivy `RegexQuery` |
| `after:2026-02-01 JWT` | Range filter on timestamp + text query |

Qualifier parsing: strip `key:value` pairs from query string, convert to facet/range filters, pass remaining text to `QueryParser`.

---

## Frontend Components

### Scoping Model

```
┌─────────────────────┬──────────────────────────┬─────────────────────────┐
│  You're on...       │  Search bar              │  Cmd+K                  │
├─────────────────────┼──────────────────────────┼─────────────────────────┤
│  History view       │  [all] Search sessions…  │  Same (global)          │
│  Project view       │  [project] Search…       │  Global (all sessions)  │
│  Session detail     │  [session] Find in…      │  Global (all sessions)  │
│  Live / Mission Ctrl│  [live] Search active…   │  Global (all sessions)  │
└─────────────────────┴──────────────────────────┴─────────────────────────┘
```

Search bar is context-scoped. Cmd+K is always global. No "search everywhere" link needed — Cmd+K is the escape hatch.

### Component Breakdown

**`useSearch` hook** (`src/hooks/use-search.ts`)
- Debounced query (200ms) → `GET /api/search`. **Note: React Query v5 has no built-in debounce.** Implement manually: `useState` for raw query, `useEffect` + `setTimeout(200ms)` to produce `debouncedQuery`, then pass `debouncedQuery` as part of the `useQuery` key with `enabled: debouncedQuery.length > 0`.
- Manages: `query`, `results`, `isLoading`, `scope`, `selectedIndex`
- Keyboard navigation: arrow keys move `selectedIndex`, Enter opens session
- Scope auto-set from current route (`useLocation()` — react-router-dom v7)
- React Query (`@tanstack/react-query` v5) for caching and deduplication. Follow existing pattern: query key as array of primitives `['search', debouncedQuery, scope]`, set `staleTime` and `gcTime` (NOT `cacheTime` — renamed in v5).
- **Reuse existing Zustand state**: `useAppStore` already has `recentSearches`, `addRecentSearch()`, `isCommandPaletteOpen`, `openCommandPalette()`, `closeCommandPalette()`. Do NOT duplicate this state in the hook.

**`SearchBar`** (`src/components/SearchBar.tsx`) — new component, but **modifies existing `Header.tsx`**
- Replaces the existing search button in `Header.tsx` (lines 108-118) that currently calls `openCommandPalette()`.
- Always visible in header as an `<input>`, not just a button.
- Scope chip: colored pill showing current context, click to remove (goes global)
- Results in dropdown below (max-height, scrollable)
- `Escape` clears and closes. `ArrowDown` moves into results.
- On session detail page: switches to in-session mode (highlight-and-scroll)

```
┌──────────────────────────────────────────────────┐
│  🔍  [claude-view]  Search this project...       │
└──────────────────────────────────────────────────┘
```

**`CommandPalette`** (`src/components/CommandPalette.tsx`) — **ALREADY EXISTS (228 lines)**
- Existing implementation handles: Cmd+K toggle, portal rendering, keyboard nav (arrow keys, Enter, Escape), recent searches from Zustand, and navigation to `/search?q=`.
- **Modify, don't recreate.** Changes needed: replace client-side `filterSessions` call with Tantivy-backed `useSearch` hook results, render `SearchResultCard` components instead of current `SessionCard` items, add Tantivy result metadata (match count, snippets with `<mark>` tags).
- **Keyboard shortcut coordination**: `App.tsx` (line 25-35) already registers global `Cmd+K` on `window`. `MissionControlPage.tsx` (line 119-131) adds a capture-phase override to suppress the global handler. Do NOT add a third competing listener — reuse the existing Zustand `openCommandPalette()`/`closeCommandPalette()` actions.

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │  🔍  Search all sessions...            ⌘K    │  │
│  └───────────────────────────────────────────────┘  │
│                                                     │
│  Recent searches                                    │
│    "JWT authentication"                             │
│    "fix login bug"                                  │
│                                                     │
│  ─────────────────────────────────────────────────  │
│                                                     │
│  Results (as you type)                              │
│  ┌─────────────────────────────────────────────┐   │
│  │  claude-view · feature/auth · 4 matches     │   │
│  │  "Add JWT authentication to the login..."   │   │
│  └─────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────┐   │
│  │  project-a · main · 6 matches                  │   │
│  │  "...middleware handles JWT auth..."         │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

**`SearchResultCard`** (`src/components/SearchResultCard.tsx`)
- Shared by both SearchBar dropdown and CommandPalette
- Session-level: project, branch, match count, date, top snippet
- Expandable: click "N matches" to show all message-level hits
- Each match: role icon (user/assistant/tool), turn number, highlighted snippet
- Click a match → opens session detail, scrolled to that message

```
┌─────────────────────────────────────────────────────┐
│  claude-view · feature/auth · 4 matches    Feb 15   │
│  "Add <mark>JWT authentication</mark> to the..."    │
│                                                      │
│  ▸ Show all 4 matches                                │
└─────────────────────────────────────────────────────┘
```

Expanded:

```
│  ▾ 4 matches                                         │
│                                                      │
│  👤 User · turn 3                                    │
│  "Add <mark>JWT authentication</mark> to the login   │
│   endpoint and make sure it validates..."            │
│                                                      │
│  🤖 Assistant · turn 4                               │
│  "I'll implement <mark>JWT authentication</mark>     │
│   using the jsonwebtoken crate..."                   │
│                                                      │
│  🔧 Edit · turn 6                                    │
│  "...pub fn verify_<mark>jwt</mark>(...) →           │
│   Result<Claims..."                                  │
│                                                      │
│  👤 User · turn 12                                   │
│  "The <mark>JWT</mark> validation is failing on      │
│   expired tokens"                                    │
```

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+K` | Open/close command palette |
| `Ctrl+F` (session page) | Focus search bar (in-session mode) — add to existing `ConversationView.tsx` keydown handler (lines 166-185), do NOT add a separate `window` listener |
| `Escape` | Close palette / clear search bar |
| `↑↓` | Navigate results |
| `Enter` | Open selected result |
| `Cmd+Enter` | Open in new tab |

---

## In-Session Search

When on a session detail page, the search bar switches to find-in-conversation mode.

```
┌──────────────────────────────────────────────────────────┐
│  🔍  [session]  Find in conversation...    3/17  ▲ ▼    │
└──────────────────────────────────────────────────────────┘
```

- Match counter: `3/17` — currently focused / total
- Nav arrows (or `Enter`/`Shift+Enter`) to cycle through matches
- All matches get subtle background highlight
- Active match gets stronger highlight + viewport scrolls to it
- Debounced (150ms)

**Client-side only** — messages are already loaded in React state from `/api/sessions/:id/parsed`. String matching over an in-memory array is instant. No Tantivy round trip needed.

**Hook**: `useInSessionSearch(messages, query)` → `{ matches, activeIndex, next(), prev(), totalCount }`

---

## Indexing Integration

### Where it plugs in

The deep indexer in `crates/db/src/indexer_parallel.rs` already:
1. Opens each JSONL file
2. Parses every message via `claude_view_db::indexer_parallel::parse_bytes()` → returns `ParseResult`
3. Extracts 60+ fields from `ParseResult`
4. Writes to SQLite

**Critical architectural constraint**: `parse_bytes()` returns `ParseResult` which contains `Vec<RawTurn>` — but `RawTurn` is a **compact token-accounting struct** (uuid, seq, model_id, input/output tokens). It has **no `.messages` field, no `.content`, no `.role`**. Message text content is NOT available from `ParseResult`. To get actual message content (role, text, timestamp) for Tantivy indexing, a different parser path is needed.

**Integration strategy (two-phase approach)**:

1. **Extend `ParseResult`** to collect message content during `parse_bytes()`. Add a new field:
   ```rust
   pub struct ParseResult {
       // ... existing fields ...
       /// Collected message content for search indexing (role, content, timestamp_unix)
       pub search_messages: Vec<SearchableMessage>,
   }

   pub struct SearchableMessage {
       pub role: String,      // "user", "assistant", "tool" — mapped from Role enum
       pub content: String,
       pub timestamp: Option<i64>,  // parsed from ISO-8601 to unix seconds
   }
   ```
   This is populated inside `parse_bytes()` during the existing line-by-line parse, filtering to only `Role::User`, `Role::Assistant`, and `Role::ToolUse`. Skip `ToolResult` (noisy output), `System`, `Progress`, and `Summary`. The ISO-8601 timestamp string is converted to unix i64 using `chrono::DateTime::parse_from_rfc3339().timestamp()`.

   **Memory impact**: Collecting all message text in `search_messages` means the full content of every indexed message for every session in a batch is held in memory between the parallel parse phase and the sequential write phase. For a batch of ~100 sessions (the deep indexer's batch size), this is manageable. For a full re-index of 3K sessions, the indexer processes in batches — `search_messages` for each batch is dropped after Tantivy writes complete in the write phase, keeping peak memory proportional to batch size, not total corpus size. If memory pressure becomes a concern, the batch size can be reduced or `search_messages` can be written to Tantivy eagerly per-session in the write loop and cleared immediately after.

2. **Write to Tantivy in the sequential write phase** (not the parallel parse phase). After the parallel parse tasks complete and results are collected, the write phase (`spawn_blocking` closure at line ~1931) iterates over all `DeepIndexResult`s and writes to SQLite. **Add Tantivy writes in this same sequential loop**:
   - `delete_term(session_id)` — remove old docs for re-indexed session
   - `add_document()` for each `SearchableMessage`
   - One `commit()` after the entire batch completes

   This avoids `IndexWriter` lock contention (Tantivy has a single writer per index) and mirrors the existing SQLite transaction pattern. Since the write phase runs in a single `spawn_blocking` closure, `IndexWriter` can be passed as `&mut IndexWriter` — no `Mutex` needed. The writer is created once at startup (via `Index::writer()`), stored in the `SearchIndex` struct, and borrowed mutably only in this sequential write phase.

3. **Thread `project` through `DeepIndexResult`**. Currently `get_sessions_needing_deep_index()` returns only `(id, file_path)` — it does NOT return `project`. Fix: extend the SQL query to also SELECT `project`, and add `project: String` to `DeepIndexResult`. Similarly, `primary_model` is only computed in the write phase via `compute_primary_model()` — it is available there but NOT during parsing.

**Key type mappings in the write phase**:
- `session_id`: from `DeepIndexResult.id`
- `project`: from `DeepIndexResult.project` (new field — requires extending `get_sessions_needing_deep_index()`)
- `git_branch`: from `DeepIndexResult.parse_result.git_branch: Option<String>`
- `primary_model`: from `compute_primary_model(&result.parse_result.turns)` (already computed in write phase at line ~1995)
- Messages: from `DeepIndexResult.parse_result.search_messages: Vec<SearchableMessage>` (new field)
- Turn number: tracked as an incrementing counter over `search_messages` (1-based index as messages are iterated)

**Role filtering** (applied inside `parse_bytes()` when collecting `search_messages`):
- Include: `Role::User` → `"user"`, `Role::Assistant` → `"assistant"`, `Role::ToolUse` → `"tool"`
- Exclude: `ToolResult`, `System`, `Progress`, `Summary`

### Incremental updates

The indexer already detects changed files via `file_mtime_at_index`:
- File mtime unchanged → skip (already indexed in both SQLite and Tantivy)
- File mtime changed → re-parse → delete old Tantivy docs for that `session_id` → insert new docs

### Rebuild from scratch

If the Tantivy index is deleted or corrupted:
- On next startup, the indexer detects all sessions need re-indexing (no Tantivy docs exist)
- Full re-index runs as normal during Pass 2
- No special rebuild logic needed — it's just "everything has changed"

### Index location

`<cache_dir>/claude-view/search-index/` — alongside the existing SQLite DB at `<cache_dir>/claude-view/claude-view.db`. Use `dirs::cache_dir().join("claude-view").join("search-index")` in Rust. On macOS this is `~/Library/Caches/claude-view/search-index/`, on Linux `~/.cache/claude-view/search-index/`.

---

## Backend Implementation

### Crate dependency fix (pre-requisite)

`crates/search/Cargo.toml` is missing `serde` and `ts-rs`. Add before implementation:

```toml
# Add to [dependencies] in crates/search/Cargo.toml
serde = { workspace = true }
ts-rs = { workspace = true }
```

### New files

| File | Purpose |
|------|---------|
| `crates/search/src/lib.rs` | Tantivy schema definition, index open/create (replaces current stub: `pub fn placeholder() {}`) |
| `crates/search/src/indexer.rs` | `index_session()`, `delete_session()`, `commit()` |
| `crates/search/src/query.rs` | Query parsing (qualifiers, phrase, fuzzy, regex), execute, snippet extraction |
| `crates/search/src/types.rs` | `SearchResponse`, `SessionHit`, `MatchHit` response types |
| `crates/server/src/routes/search.rs` | `GET /api/search` Axum handler |

### Modified files

| File | Change |
|------|--------|
| `crates/search/Cargo.toml` | Add `serde` and `ts-rs` workspace dependencies |
| `crates/core/src/types.rs` (or new file in core) | Add `SearchableMessage` struct; extend `ParseResult` with `search_messages: Vec<SearchableMessage>` |
| `crates/db/src/indexer_parallel.rs` | (1) Collect `search_messages` during `parse_bytes()` line-by-line parse. (2) Extend `get_sessions_needing_deep_index()` to also SELECT `project`. (3) Add `project: String` to `DeepIndexResult`. (4) Pass `Arc<SearchIndex>` into `pass_2_deep_index()`. (5) Add Tantivy batch write (delete+insert+commit) in the sequential write phase alongside SQLite writes. |
| `crates/server/src/state.rs` | Add `pub search_index: Arc<SearchIndex>` field to `AppState` struct |
| `crates/server/src/state.rs` | Update `AppState::new()` (line ~70) and `AppState::new_with_indexing()` (line ~95) and `AppState::new_with_indexing_and_registry()` (line ~119) |
| `crates/server/src/lib.rs` | Update `create_app_with_git_sync()` (line ~103) and `create_app_full()` (line ~154) — both construct `AppState { ... }` directly |
| `crates/server/src/routes/mod.rs` | Add `pub mod search;` declaration and `.nest("/api", search::router())` in `api_routes()` |
| `src/types/generated/index.ts` | Manually add re-exports: `export * from './SearchResponse'`, `export * from './SessionHit'`, `export * from './MatchHit'` |

| `crates/server/src/routes/jobs.rs` | Update test helper `AppState { ... }` struct literal (1 site, in `#[cfg(test)]`) |
| `crates/server/src/routes/terminal.rs` | Update test helper `AppState { ... }` struct literals (2 sites, in `#[cfg(test)]`) |

**Critical: AppState has 7 construction sites (not 5) — ALL must include the new `search_index` field or the code won't compile:**
1. `state.rs` — `AppState::new()` (~line 70)
2. `state.rs` — `AppState::new_with_indexing()` (~line 95)
3. `state.rs` — `AppState::new_with_indexing_and_registry()` (~line 119)
4. `lib.rs` — `create_app_with_git_sync()` (~line 103)
5. `lib.rs` — `create_app_full()` (~line 154)
6. `routes/jobs.rs` — test helper (~line 65, `#[cfg(test)]`)
7. `routes/terminal.rs` — test helpers (~lines 974, 1143, `#[cfg(test)]`)

### Axum handler pattern (copy from existing routes)

The search route handler must follow the established pattern:

```rust
// crates/server/src/routes/search.rs

use crate::error::ApiResult;
use crate::AppState;
use axum::{extract::{Query, State}, routing::get, Json, Router};
use serde::Deserialize;
use std::sync::Arc;
use claude_view_search::types::SearchResponse;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub scope: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/search", get(search_handler))
}

pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let q = query.q.as_deref().unwrap_or("").trim();
    if q.is_empty() {
        return Err(crate::error::ApiError::BadRequest(
            "query parameter 'q' is required".to_string(),
        ));
    }
    // ... Tantivy query execution ...
}
```

**Note**: All URL query params are **snake_case** (matching every other endpoint in the codebase). No camelCase in URLs.

### Rust types

```rust
// crates/search/src/types.rs
// Requires: serde = { workspace = true }, ts-rs = { workspace = true } in crates/search/Cargo.toml

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub total_sessions: usize,
    pub total_matches: usize,
    pub elapsed_ms: f64,
    pub sessions: Vec<SessionHit>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionHit {
    pub session_id: String,
    pub project: String,
    pub branch: Option<String>,
    pub modified_at: i64,
    pub match_count: usize,
    pub best_score: f32,
    pub top_match: MatchHit,
    pub matches: Vec<MatchHit>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct MatchHit {
    pub role: String,
    pub turn_number: u64,
    pub snippet: String,  // contains <mark> tags
    pub timestamp: i64,
}
```

---

## Performance Targets

| Metric | Target | Basis |
|--------|--------|-------|
| Index build (full, 3K sessions) | <60s | Piggybacks on existing deep index pass |
| Index build (incremental, 1 session) | <500ms | Single JSONL parse + Tantivy insert |
| Query latency (plain text) | <10ms | Tantivy benchmark data |
| Query latency (phrase + facets) | <20ms | Phrase queries are Tantivy's strength |
| Query latency (regex) | <100ms | Regex scans are slower by nature |
| Snippet generation | <5ms per session | Tantivy native snippet generator |
| Index size on disk | <600MB | ~400K docs, text + facets |
| Cmd+K time-to-first-result | <300ms | 200ms debounce + <10ms query + render |
| In-session search | <5ms | Client-side string match, no network |

---

## Phasing

### Phase 1 — Core text search (Day 1)

| Item | Layer | What |
|------|-------|------|
| Tantivy schema + crate | Backend | Build out `crates/search/` with schema |
| Index during Pass 2 | Backend | Pipe parsed messages into Tantivy during deep indexing |
| `GET /api/search` | Backend | Endpoint with plain text + exact phrase + scope |
| Upgrade `CommandPalette` | Frontend | **Modify existing** `src/components/CommandPalette.tsx` — replace client-side filtering with Tantivy `useSearch` hook, add `SearchResultCard` rendering |
| Upgrade `SearchResults` page | Frontend | **Modify existing** `src/components/SearchResults.tsx` — replace `useAllSessions` + `src/lib/search.ts` with Tantivy `useSearch` hook |
| `SearchBar` scoped mode | Frontend | **New** component in header (replaces existing button in `Header.tsx`), scope chips, dropdown results |
| `SearchResultCard` | Frontend | **New** shared result component with expandable matches |
| Update `src/types/generated/index.ts` | Frontend | Manually add `export * from './SearchResponse'`, `export * from './SessionHit'`, `export * from './MatchHit'` |

### Phase 2 — Polish & power features

| Item | What |
|------|------|
| Qualifiers | Parse `project:`, `branch:`, `model:`, `role:`, `skill:`, `after:`, `before:` — **Note:** `src/lib/search.ts` already implements client-side qualifier parsing for `project:`, `path:`, `skill:`, `after:`, `before:`, regex, phrase. Port this logic to Rust or delegate to Tantivy's QueryParser. |
| Fuzzy matching | Tantivy edit-distance fuzzy (`~` suffix) |
| Regex | `/pattern/` → `RegexQuery` |
| Recent searches | Already implemented in Zustand (`useAppStore.recentSearches`, `addRecentSearch()`). Just ensure Tantivy search calls `addRecentSearch()` on submit. |
| In-session search | Client-side Ctrl+F with highlight-and-scroll |

### Phase 3 — Intelligent search (future)

| Item | What |
|------|------|
| Semantic search | Local embeddings via `ort` (ONNX) or LLM API for query expansion |
| "Find similar sessions" | Vector similarity on session embeddings |
| Natural language queries | "sessions where I fixed auth bugs last week" → structured query |

---

## Accessibility

| Requirement | Implementation |
|-------------|---------------|
| Keyboard navigable | Arrow keys, Enter, Escape all work without mouse |
| Focus visible | `focus-visible:ring-2 focus-visible:ring-blue-500` on all interactive elements |
| `aria-expanded` | On search bar when dropdown is open |
| `aria-live="polite"` | On result count ("3 sessions, 12 matches") for screen readers |
| `aria-activedescendant` | Tracks keyboard-selected result in list |
| `role="combobox"` | On search input with `role="listbox"` on results |
| `role="option"` | On each result card |
| Reduced motion | No animations if `prefers-reduced-motion` is set |

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Index not built yet | Search bar shows "Indexing in progress..." with progress indicator. Cmd+K shows same. |
| Index corrupted | Auto-detect on startup, trigger full rebuild. Show "Rebuilding search index..." |
| Query syntax error | Fall back to plain text search (treat qualifiers as literal text). No error shown to user. |
| No results | "No sessions match" with suggestions: "Try removing quotes for broader search" or "Search all sessions" link if scoped |
| Search API timeout (>5s) | "Search is taking longer than expected. Try a simpler query." |
| Very large result set (>100 sessions) | Paginate. Show "Showing 20 of 147 sessions. Load more." |

---

## Existing Code to Reuse or Replace

These files already exist and the plan must integrate with them, not recreate from scratch:

| File | Current State | Plan Action |
|------|--------------|-------------|
| `src/components/CommandPalette.tsx` (228 lines) | Fully implemented: Cmd+K, keyboard nav, recent searches, Zustand integration | **Modify**: replace client-side `filterSessions` with Tantivy `useSearch` results |
| `src/components/SearchResults.tsx` (110 lines) | Page-level search with client-side filtering via `useAllSessions` | **Modify**: replace with Tantivy-backed `useSearch` hook |
| `src/lib/search.ts` (183 lines) | Client-side `parseQuery`/`filterSessions` with `project:`, `path:`, `skill:`, `after:`, `before:`, regex, phrase | **Dead code after Phase 1** — both consumers (`SearchResults.tsx`, `CommandPalette.tsx`) are migrated to Tantivy-backed `useSearch`. Port qualifier parsing logic to Rust. Remove `search.ts` when in-session search lands in Phase 2 (in-session search uses a dedicated `useInSessionSearch` hook with simple string matching, not this file's `filterSessions`). |
| `src/store/app-store.ts` | Zustand: `recentSearches`, `isCommandPaletteOpen`, `addRecentSearch()`, `openCommandPalette()`, `closeCommandPalette()` | **Reuse as-is** |
| `src/components/Header.tsx` (line 108-118) | Search button that calls `openCommandPalette()` | **Replace** button with `SearchBar` input component |
| `src/App.tsx` (line 25-35) | Global `Cmd+K` listener on `window` | **Keep** — already wired to Zustand `openCommandPalette()` |
| `src/pages/MissionControlPage.tsx` (line 119-131) | Capture-phase `Cmd+K` override | **Keep** — do not conflict |
| `src/components/ConversationView.tsx` (line 166-185) | Existing `keydown` handler for `Cmd+Shift+E/P` | **Extend** with `Ctrl+F` for in-session search |

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Plan said `AppState` is in `crates/server/src/lib.rs` — actual: `crates/server/src/state.rs` with 5 construction sites | Blocker | Updated "Modified files" table to list `state.rs` and all 5 construction sites explicitly |
| 2 | No hook point for search indexing — `parse_bytes()` doesn't expose session metadata | Blocker | Added detailed integration strategy: pass `Arc<SearchIndex>` into `pass_2_deep_index()`, call inside existing per-session closure |
| 3 | `crates/search/Cargo.toml` missing `serde` and `ts-rs` dependencies | Blocker | Added "Crate dependency fix" pre-requisite section with exact lines to add |
| 4 | Wrong data directory: plan said `~/.claude-view/`, actual: `dirs::cache_dir().join("claude-view")` | Blocker | Fixed all path references to use `<cache_dir>/claude-view/search-index/` with platform-specific paths |
| 5 | `CommandPalette.tsx` already fully exists (228 lines) — plan treated as net-new | Blocker | Changed to "Modify existing" with specific changes needed |
| 6 | `SearchResults.tsx` already exists with client-side filtering | Blocker | Changed Phase 1 to "Upgrade SearchResults page" instead of creating new |
| 7 | `src/types/generated/index.ts` is hand-maintained — new types won't be importable without manual update | Blocker | Added explicit step in "Modified files" to add re-exports |
| 8 | Route registration is in `routes/mod.rs`, not `main.rs` | Blocker | Fixed "Modified files" table to reference `routes/mod.rs` with `pub mod search;` + `.nest()` call |
| 9 | Field name mismatches: `branch` vs `git_branch`, `model` vs `primary_model` | Warning | Updated Tantivy schema table with actual field names and mappings |
| 10 | `Message.timestamp` is `Option<String>` (ISO-8601), not `i64` | Warning | Added conversion note to schema table: parse via `chrono::DateTime::parse_from_rfc3339().timestamp()` |
| 11 | `turn_number` doesn't exist on `Message` — from `RawTurn.seq` | Warning | Updated schema table: derived from message sequence counter, not a Message field |
| 12 | `Role` enum has 7 values, plan maps to 3 without specifying which to skip | Warning | Added explicit role filtering: index `User`, `Assistant`, `ToolUse`; skip `ToolResult`, `System`, `Progress`, `Summary` |
| 13 | Existing client-side search engine at `src/lib/search.ts` already has most "Phase 2" qualifiers | Warning | Added note to Phase 2 qualifiers about porting existing client-side logic |
| 14 | Zustand store already owns search state — plan risked duplicating it | Warning | Added explicit "reuse existing Zustand state" note to `useSearch` hook spec |
| 15 | `Header.tsx` modification needed — not purely additive | Warning | Updated `SearchBar` spec to note it replaces existing button |
| 16 | Multiple keyboard shortcut listeners could conflict | Warning | Added coordination notes for Cmd+K (App.tsx, MissionControlPage.tsx) and Ctrl+F (ConversationView.tsx) |
| 17 | React Query v5 has no built-in debounce | Warning | Added explicit debounce implementation notes to `useSearch` hook spec |
| 18 | Recent searches already in Zustand — plan's Phase 2 said "persist in localStorage" | Warning | Updated Phase 2 to reference existing Zustand `addRecentSearch()` |
| 19 | Added "Existing Code to Reuse or Replace" section | — | New section documenting all existing files the plan must integrate with |
| 20 | Added Axum handler pattern with concrete code | — | New section showing exact handler signature, query params struct, error handling |

**Round 2 — Adversarial Review Fixes (score: 61→85)**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 21 | `RawTurn` has no `.messages: Vec<Message>` — entire integration strategy based on non-existent field | Blocker | Completely rewrote integration strategy: extend `ParseResult` with `search_messages: Vec<SearchableMessage>` collected during `parse_bytes()` |
| 22 | `project` not in scope in `pass_2_deep_index()` — `get_sessions_needing_deep_index()` doesn't return it | Blocker | Added requirement to extend SQL query to SELECT project, add `project: String` to `DeepIndexResult` |
| 23 | Missing `use serde::Deserialize;` and `use claude_view_search::types::SearchResponse;` in handler code block | Blocker | Added both imports to the code block |
| 24 | Plan claimed 5 AppState construction sites; actual is 7 (2 test helpers in `jobs.rs` and `terminal.rs` missed) | Blocker | Updated count to 7, listed all sites with file/line references, added `jobs.rs` and `terminal.rs` to modified files |
| 25 | Tantivy writes in parallel parse tasks would cause `IndexWriter` lock contention | Important | Moved Tantivy writes to sequential write phase — batch delete+insert+commit alongside SQLite transaction |
| 26 | `primary_model` not computed at parse phase, only available in write phase | Important | Documented that `compute_primary_model()` is called in write phase — Tantivy writes happen there too |
| 27 | `turn_number` derivation from `RawTurn.seq` misleading — `RawTurn` has no messages | Minor | Changed to incrementing counter over `search_messages` in the write phase. Removed `RawTurn.seq` analogy from schema table. |

**Round 3 — Final Precision Fixes (score: ~85→100)**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 28 | Tantivy schema table used informal type notation (`STRING`, `TEXT`, `U64`, `I64`, `Fast`) that could mislead — these are `TextOptions` presets and flags, not separate types | Important | Replaced schema table "Tantivy Type" column with "Builder Call (Tantivy 0.22)" showing exact `add_text_field`/`add_u64_field`/`add_i64_field` calls with proper flag syntax. Added explanatory note on `STRING` vs `TEXT` vs `FAST` vs `STORED` semantics. |
| 29 | No concrete, compilable schema builder code — informal table left room for API misuse | Important | Added "Concrete schema builder (verbatim-compilable)" code block with `use tantivy::schema::*`, `Schema::builder()`, and all 9 field definitions matching the table exactly. |
| 30 | `IndexWriter` requires exclusive access (`&mut`) but plan didn't mention this constraint | Important | Added note to write phase section: since sequential `spawn_blocking` runs single-threaded, `&mut IndexWriter` suffices — no `Mutex` needed. Writer created once at startup, stored in `SearchIndex`, borrowed mutably only in write phase. |
| 31 | `turn_number` schema table row still referenced `RawTurn.seq` analogy, inconsistent with key type mappings section | Minor | Removed `(analogous to RawTurn.seq)` from schema table. Now consistently says "incrementing counter over `search_messages`" in both schema table and key type mappings. |
| 32 | `src/lib/search.ts` lifecycle unclear — plan said "Keep for fallback / in-session search" but both consumers migrate away in Phase 1 | Minor | Clarified: dead code after Phase 1, removed when in-session search lands in Phase 2. In-session search uses dedicated `useInSessionSearch` hook, not `filterSessions`. |
| 33 | `SearchableMessage` memory impact unaddressed — collecting all message text in `ParseResult` could be significant for large batches | Minor | Added memory impact note: peak memory proportional to batch size (not corpus size) because `search_messages` is dropped after each batch's Tantivy write. Batch size is tunable if memory pressure occurs. |
