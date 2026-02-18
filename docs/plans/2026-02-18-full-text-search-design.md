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

**Storage**: Tantivy index at `~/.claude-view/search-index/`. Rebuilt from JSONL if deleted.

**Data flow**: Deep indexer parses JSONL → inserts messages into Tantivy (same pass) → search API queries Tantivy → returns session-grouped results with highlighted snippets.

---

## Tantivy Schema

Each message is a Tantivy document (~400K documents for 3K sessions):

| Field | Tantivy Type | Stored | Indexed | Purpose |
|-------|-------------|--------|---------|---------|
| `session_id` | `STRING` | Yes | Facet | Group results by session, delete-by-session for re-index |
| `project` | `STRING` | Yes | Facet | Qualifier: `project:claude-view` |
| `branch` | `STRING` | Yes | Facet | Qualifier: `branch:feature/auth` |
| `model` | `STRING` | Yes | Facet | Qualifier: `model:opus` |
| `role` | `STRING` | Yes | Facet | Qualifier: `role:user` — values: `user`, `assistant`, `tool` |
| `content` | `TEXT` | Yes | Full-text (BM25) | The actual message text — tokenized, searchable |
| `turn_number` | `U64` | Yes | Fast | For display in results ("turn 3") |
| `timestamp` | `I64` | Yes | Fast | For `after:`/`before:` range queries and result ordering |
| `skills` | `STRING` (multi) | Yes | Facet | Qualifier: `skill:commit` |

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
- Debounced query (200ms) → `GET /api/search`
- Manages: `query`, `results`, `isLoading`, `scope`, `selectedIndex`
- Keyboard navigation: arrow keys move `selectedIndex`, Enter opens session
- Scope auto-set from current route (`useLocation()`)
- React Query for caching and deduplication

**`SearchBar`** (`src/components/SearchBar.tsx`)
- Always visible in header
- Scope chip: colored pill showing current context, click to remove (goes global)
- Results in dropdown below (max-height, scrollable)
- `Escape` clears and closes. `ArrowDown` moves into results.
- On session detail page: switches to in-session mode (highlight-and-scroll)

```
┌──────────────────────────────────────────────────┐
│  🔍  [claude-view]  Search this project...       │
└──────────────────────────────────────────────────┘
```

**`CommandPalette`** (`src/components/CommandPalette.tsx`)
- Modal overlay, always global scope
- Portal-rendered (escapes any parent context)
- `Escape` or click-outside closes
- Shows recent searches when input is empty
- Arrow keys navigate results, `Enter` opens
- `Cmd+K` toggles open/close

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
| `Ctrl+F` (session page) | Focus search bar (in-session mode) |
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
2. Parses every message via `parse_bytes()`
3. Extracts 60+ fields
4. Writes to SQLite

**Addition**: After step 2, also write each message to Tantivy. Same parsed data, no extra file reads.

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

`~/.claude-view/search-index/` — alongside the existing SQLite DB at `~/.claude-view/sessions.db`.

---

## Backend Implementation

### New files

| File | Purpose |
|------|---------|
| `crates/search/src/lib.rs` | Tantivy schema definition, index open/create |
| `crates/search/src/indexer.rs` | `index_message()`, `delete_session()`, `commit()` |
| `crates/search/src/query.rs` | Query parsing (qualifiers, phrase, fuzzy, regex), execute, snippet extraction |
| `crates/search/src/types.rs` | `SearchResult`, `SessionHit`, `MatchHit` response types |
| `crates/server/src/routes/search.rs` | `GET /api/search` Axum handler |

### Modified files

| File | Change |
|------|--------|
| `crates/db/src/indexer_parallel.rs` | Add Tantivy writes during deep index pass |
| `crates/server/src/main.rs` | Open Tantivy index at startup, add to `AppState`, register `/api/search` route |
| `crates/server/src/lib.rs` | Add `SearchIndex` to `AppState` struct |

### Rust types

```rust
// crates/search/src/types.rs

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
| `CommandPalette` (Cmd+K) | Frontend | Global search modal, result cards, keyboard nav |
| `SearchBar` scoped mode | Frontend | Header bar with scope chips, dropdown results |
| `SearchResultCard` | Frontend | Shared result component with expandable matches |

### Phase 2 — Polish & power features

| Item | What |
|------|------|
| Qualifiers | Parse `project:`, `branch:`, `model:`, `role:`, `skill:`, `after:`, `before:` |
| Fuzzy matching | Tantivy edit-distance fuzzy (`~` suffix) |
| Regex | `/pattern/` → `RegexQuery` |
| Recent searches | Persist last 20 searches in localStorage |
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
