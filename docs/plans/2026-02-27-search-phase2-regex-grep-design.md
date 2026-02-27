---
status: deprecated
date: 2026-02-27
theme: "Search & Discovery"
superseded_by: 2026-02-28-smart-search-design.md
---

# ~~Search Phase 2: Regex Grep + Remaining Search Features~~

> **DEPRECATED:** Superseded by [Smart Search Design](2026-02-28-smart-search-design.md) which replaces the toggle-based regex approach with a multi-signal Google-style search experience.

**Context:** Shippable audit on 2026-02-27 confirmed the full-text search module (crates/search/) is production-grade with zero regressions after PR #14. This design covers all remaining Phase 2 search features, with regex grep as the primary new capability.

## Background — Audit Results

### What Passed (Phase 1 — SHIP IT)

| Check | Status |
|-------|--------|
| Plan Compliance | All Phase 1 items delivered |
| Wiring Integrity | 8/8 paths verified (pipeline → API → frontend) |
| Production Hardening | 0 blockers, 3 minor warnings |
| Build & Test | 45/45 Rust tests pass, TS clean, frontend builds |
| Regression from PR #14 | None — deleted `indexer.rs` had zero references from search crate |

### What's Pending (This Plan)

Consolidated from `2026-02-18-full-text-search-design.md` Phase 2, `2026-02-22-unified-search-design.md`, and `2026-02-22-unified-search.md`:

| # | Item | Source | Current State |
|---|------|--------|---------------|
| **1** | **Regex grep search (raw JSONL)** | New (this design) | Not started — PRIMARY FEATURE |
| **2** | `after:`/`before:` date range qualifiers | Phase 2 design | Not implemented. Tantivy `timestamp` field has `FAST` flag, needs `RangeQuery` |
| **3** | In-session search (Ctrl+F client-side) | Phase 2 design | Not implemented. Needs `useInSessionSearch` hook + highlight-and-scroll |
| **4** | `SearchBar` scoped mode in header | Phase 1 design (deferred) | Not implemented. Header still uses button → CommandPalette |
| **5** | History page search → Tantivy wiring | Unified search impl Task 4 | **Partially done** — `search_session_ids` field exists in `SessionFilterParams` but route handler not wired |

### What's Explicitly OUT of Scope

| Item | Why |
|------|-----|
| `git_root` / `first_message_at` in search index | By design — session-level metadata, not message-level content |
| Semantic search / embeddings (Phase 3) | Needs new infra (ONNX/vector store), deferred to future cycle |
| "Find similar sessions" (Phase 3) | Needs session embeddings, deferred |
| Natural language queries (Phase 3) | Needs LLM query expansion, deferred |

---

## Feature 1: Regex Grep Search (Primary)

### Problem

User has 2.6 GB / 4,737 JSONL files in `~/.claude/projects/`. When searching for patterns like `auth.*middleware`, the existing Tantivy search fails because:
1. Tantivy tokenizes content — multi-word regex patterns break across token boundaries
2. Tantivy only indexes user/assistant/tool text — system prompts, JSON keys, raw tool output are invisible
3. User currently uses VSCode's Cmd+Shift+F across the JSONL directory as workaround

### Decision: Approach A — Rust-native `grep` crate

**Chosen over:**
- Approach B (spawn `rg` CLI): Requires users to install ripgrep, violates "no external dependencies" principle
- Approach C (Tantivy `RegexQuery`): Only searches indexed terms, not raw content. Fundamentally wrong tool.

**Who uses this at scale:** The `grep` crate IS ripgrep's core engine (same author: BurntSushi). Used by ripgrep (45K+ stars), VSCode (spawns ripgrep for "Find in Files"), GitHub code search, Sourcegraph.

### Performance Baseline (measured on user's machine)

```
Corpus: 2.6 GB, 4,737 JSONL files, avg 570 KB/file
Fixed string "brainstorming":  742 matches in 174ms
Regex "auth.*middleware":      541 matches in 190ms
Engine: ripgrep, parallel, mmap
```

Both are under 200ms — faster than the frontend debounce timer.

### Architecture

```
┌──────────────────────────────────────────────────────┐
│  Search Bar / Cmd+K                                   │
│  ┌────────────────────────┐  ┌───┐ ┌───┐ ┌───┐      │
│  │  auth.*middleware       │  │Aa │ │Ab│  │.* │      │
│  └────────────────────────┘  └───┘ └───┘ └───┘      │
│                              case  word   REGEX       │
│                                          TOGGLE       │
└───────────────┬──────────────────────────────────────┘
                │
        ┌───────┴───────┐
        │ Regex ON?     │
        ├── No ─────────┼──► Tantivy search (existing)
        │               │    → Session-grouped + BM25 ranked
        └── Yes ────────┘
                │
                ▼
        ┌───────────────┐
        │ GET /api/grep │
        │  Axum route   │
        └───────┬───────┘
                │
                ▼
        ┌───────────────────────┐
        │  grep crate           │
        │  (ripgrep core)       │
        │  parallel file scan   │
        │  mmap + SIMD          │
        │  ~/.claude/projects/  │
        └───────┬───────────────┘
                │
                ▼
        Line-level results:
        project > session_id : line_num : content
```

### UI Design — VSCode-style Toggle

The existing search bar gets three toggle buttons (like VSCode):

```
┌─────────────────────────────────────────────────────────┐
│  🔍  [  auth.*middleware              ]  [Aa] [Ab] [.*] │
└─────────────────────────────────────────────────────────┘
                                            │     │    │
                                         Case  Word  Regex
                                         Match Match Toggle
```

**Toggle states:**
- `.*` OFF (default): Tantivy search — existing behavior, session-grouped results
- `.*` ON: Raw grep — line-level results, ripgrep engine against JSONL files

**When regex toggle is ON, the results panel switches to a VSCode-style tree view:**

```
┌─────────────────────────────────────────────────────────┐
│  3,247 results in 541 sessions (189ms)                  │
│                                                         │
│  ▾ claude-view (245 results)                            │
│    ▾ session abc123 · Feb 27                            │
│      L142  "role":"user","content":"add auth middleware  │
│      L389  ...implement JWT auth.*middleware pattern...  │
│      L512  "role":"assistant"..."auth middleware done"   │
│    ▸ session def456 · Feb 26  (12 results)              │
│    ▸ session ghi789 · Feb 25  (3 results)               │
│                                                         │
│  ▸ my-other-project (296 results)                       │
│                                                         │
│  ▸ Load more sessions...                                │
└─────────────────────────────────────────────────────────┘
```

**Interaction:**
- Click a line → opens session detail, scrolls to that position in conversation
- Sessions sorted by recency (newest first), NOT by relevance (no BM25 for grep)
- Lines within a session sorted by line number ascending
- Collapsible project groups → session groups → line hits (3-level tree)
- Highlighted match text in each line (regex match group highlighted with `<mark>`)

### API Design

#### `GET /api/grep`

| Param | Type | Default | Example |
|-------|------|---------|---------|
| `pattern` | string (required) | — | `auth.*middleware` |
| `case_sensitive` | bool | `false` | `true` |
| `whole_word` | bool | `false` | `true` |
| `limit` | int | `500` | Max total lines returned |
| `project` | string | — | Scope to one project |

**Response:**

```json
{
  "pattern": "auth.*middleware",
  "totalMatches": 3247,
  "totalSessions": 541,
  "elapsedMs": 189.2,
  "truncated": true,
  "results": [
    {
      "sessionId": "abc123",
      "project": "claude-view",
      "projectPath": "/Users/tom/dev/claude-view",
      "modifiedAt": 1740600000,
      "matches": [
        {
          "lineNumber": 142,
          "content": "{\"role\":\"user\",\"content\":\"add auth middleware...\"}",
          "matchStart": 34,
          "matchEnd": 53
        }
      ]
    }
  ]
}
```

### Backend Implementation

**New crate dependency** in `crates/search/Cargo.toml`:

```toml
grep-regex = "0.1"    # ripgrep's regex matcher
grep-searcher = "0.1" # ripgrep's parallel file searcher
grep-matcher = "0.1"  # trait definitions
```

Or alternatively, use the higher-level `grep` crate if available. Check crates.io for the exact package names from BurntSushi's ripgrep workspace.

**New files:**
- `crates/search/src/grep.rs` — grep engine wrapper (pattern compile, file scan, result collection)
- `crates/search/src/grep_types.rs` — `GrepResponse`, `GrepSessionHit`, `GrepLineMatch` response types
- `crates/server/src/routes/grep.rs` — `GET /api/grep` Axum handler

**Key implementation details:**
- Use `~/.claude/projects/` as base directory (same as existing discovery)
- Parallel file scanning (bounded by `available_parallelism()` — matches CLAUDE.md rule)
- mmap for large files (matches CLAUDE.md rule: "parse directly, never `.to_vec()`")
- Session ID extracted from filename (strip `.jsonl` extension)
- Project extracted from parent directory name, decoded via `resolve_project_path()` (matches CLAUDE.md rule)
- Results capped at `limit` lines to prevent browser OOM
- Regex compilation happens once, reused across all files
- Report `truncated: true` if total matches exceed limit

### Frontend Implementation

**Modified files:**
- `src/hooks/use-search.ts` — add `useGrep` hook (or extend `useSearch` with mode param)
- `src/components/CommandPalette.tsx` — add regex toggle button, switch result rendering
- `src/components/SearchResults.tsx` — add grep result tree view component
- `src/components/Header.tsx` — add toggle buttons to search bar (if SearchBar exists)
- `src/types/generated/index.ts` — add `GrepResponse`, `GrepSessionHit`, `GrepLineMatch` exports

**New files:**
- `src/components/GrepResults.tsx` — VSCode-style collapsible tree view for grep results

**State management:**
- Add `isRegexMode: boolean` to search state (Zustand or local)
- Toggle persists within session (not across page reloads — VSCode behavior)
- When regex mode ON: call `/api/grep` instead of `/api/search`
- Debounce: 300ms for regex (slightly longer — regex compilation + full scan)

---

## Feature 2: `after:`/`before:` Date Range Qualifiers

Add to existing Tantivy query parser in `crates/search/src/query.rs`.

**Implementation:**
- Parse `after:2026-02-01` and `before:2026-02-28` from query string
- Convert date string to unix timestamp via `chrono::NaiveDate::parse_from_str`
- Create `RangeQuery` on the `timestamp` field (already `FAST` indexed)
- Add to `sub_queries` as `Occur::Must` filter

**Scope:** ~50 lines in `query.rs` (add to `parse_query_string` + qualifier handling block at line 310-318).

---

## Feature 3: In-Session Search (Ctrl+F)

Client-side only — messages already loaded in React state.

**New hook:** `useInSessionSearch(messages, query)` returns `{ matches, activeIndex, next(), prev(), totalCount }`

**UI:**
- Ctrl+F on session detail page → focus search input (mini-bar at top of conversation)
- Match counter: `3/17` with ▲ ▼ nav arrows
- All matches get subtle background highlight (`bg-yellow-100 dark:bg-yellow-900/30`)
- Active match gets stronger highlight + viewport scrolls to it
- Enter = next, Shift+Enter = prev, Escape = close
- Debounced 150ms

**Modified files:**
- `src/components/ConversationView.tsx` — add Ctrl+F handler (extend existing keydown at lines 166-185), render mini search bar
- `src/hooks/use-in-session-search.ts` — new hook

---

## Feature 4: SearchBar Scoped Mode

Replace the search button in Header.tsx with an always-visible `<input>` with scope chip.

**Deferred — lower priority than features 1-3.** The existing Cmd+K → CommandPalette workflow works. This is a UX polish item.

---

## Feature 5: History Page → Tantivy Wiring

**Partially done:** `search_session_ids: Option<Vec<String>>` already exists in `SessionFilterParams` (dashboard.rs:17). The SQL `IN (...)` clause is implemented (dashboard.rs:306).

**Remaining:** Wire the Tantivy call in `crates/server/src/routes/sessions.rs` — when `q` param is present, call `SearchIndex::search()` to get session IDs, then pass to `SessionFilterParams.search_session_ids`. Falls back to SQLite LIKE if Tantivy unavailable.

**Implementation plan exists:** `docs/plans/2026-02-22-unified-search.md` Task 4 has the exact code. Just needs execution.

---

## Implementation Priority

| Order | Feature | Effort | Impact |
|-------|---------|--------|--------|
| **1** | Regex grep (Feature 1) | Medium (new crate + API + UI) | High — replaces VSCode workaround |
| **2** | History → Tantivy wiring (Feature 5) | Small (code exists in plan) | High — unifies search behavior |
| **3** | `after:`/`before:` qualifiers (Feature 2) | Small (~50 lines) | Medium |
| **4** | In-session Ctrl+F (Feature 3) | Medium (new hook + UI) | Medium |
| **5** | SearchBar scoped mode (Feature 4) | Small | Low — polish |

---

## Technical Constraints (from CLAUDE.md)

- **Parallelism:** `Semaphore` bounded to `available_parallelism()` for grep file scanning
- **mmap:** Parse directly, never `.to_vec()` — grep crate supports mmap natively
- **Path decoding:** Use `resolve_project_path()` for Claude Code directory names
- **Startup:** Server binds port before any background work — grep is on-demand, no startup cost
- **Frontend:** No shadcn/ui CSS vars. Use explicit Tailwind + `dark:` variants for toggle buttons and tree view
- **Radix UI:** Use `@radix-ui/react-toggle` for the regex/case/word toggle buttons if available
- **Testing:** Test only changed crates. `cargo test -p claude-view-search` for search changes.

---

## Existing Code References

| File | Relevance |
|------|-----------|
| `crates/search/src/lib.rs` | Schema, SearchIndex, SEARCH_SCHEMA_VERSION (currently 6) |
| `crates/search/src/query.rs` | Query parser — add `after:`/`before:` here |
| `crates/search/src/indexer.rs` | SearchDocument struct — unchanged for grep |
| `crates/search/src/types.rs` | SearchResponse types — add GrepResponse types alongside |
| `crates/server/src/routes/search.rs` | Existing search endpoint — grep gets its own route |
| `crates/server/src/routes/mod.rs` | Route registration — add `pub mod grep;` |
| `crates/db/src/indexer_parallel.rs:2366-2449` | Tantivy write pipeline — unchanged |
| `crates/db/src/queries/dashboard.rs:17,306` | `search_session_ids` field — already wired for Feature 5 |
| `src/hooks/use-search.ts` | Existing search hook — add grep mode or new `useGrep` hook |
| `src/components/CommandPalette.tsx:6,70` | Uses `useSearch` — add toggle + grep result rendering |
| `src/components/SearchResults.tsx` | Search results page — add grep tree view |
| `docs/plans/2026-02-22-unified-search.md` | Task 4 has exact code for Feature 5 wiring |
| `docs/plans/2026-02-18-full-text-search-design.md` | Original Phase 2 spec for features 2-4 |
