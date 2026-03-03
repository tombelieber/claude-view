# Smart Search: Google-Level Search for Claude Sessions

**Date:** 2026-02-28
**Status:** Design (supersedes Phase 2 search plans)
**Supersedes:** `2026-02-27-search-phase2-regex-grep-design.md`, `2026-02-27-search-phase2-implementation.md`

## Design Principle

> One box. Type what you remember. Find it.

No toggles. No modes. No decisions for the user. The query engine figures out intent automatically. Every search input in the app looks and behaves identically.

---

## 1. UX: Unified Search Input

### One component, everywhere

A single `<SearchInput>` component used across all search contexts. Same icon, same placeholder, same behavior. The only variable is an implicit `scope` prop injected behind the scenes.

```
┌──────────────────────────────────────────┐
│  🔍  Search conversations...        ⌘K   │
└──────────────────────────────────────────┘
```

### Context table

| Context | Scope | Behavior |
|---|---|---|
| **CommandPalette** (`Cmd+K`) | none | Global smart search. Scrollable results + "View all N results" button → navigates to `/search`. |
| **Header bar** | none | Global smart search. `Enter` navigates to `/search?q=...`. |
| **SearchResults page** (`/search`) | none | Full smart search with pagination. |
| **ConversationView** (`Cmd+F`) | `session:<id>` | Smart search scoped to current session. Match counter, `▲`/`▼` jump, scroll-to-match. |
| **HistoryView** | *(client-side)* | **Keeps existing client-side filter.** Shared `<SearchInput>` visually, but wired to local metadata filtering — NOT the smart search API. Rationale: data is already loaded, instant filtering is better UX for metadata (project name, skills). |
| **LiveFilterBar** | *(client-side)* | Same as HistoryView — keeps client-side filter on live session metadata. |

### Why HistoryView stays client-side

HistoryView filters already-loaded session metadata (project name, skills, file paths). Switching to server-side smart search would:
- Add ~50ms latency to what is currently instant
- Change behavior: search message content instead of metadata (different user intent)
- No precedent for replacing instant client-side filtering with server-side in a desktop tool

The shared `<SearchInput>` component unifies the visual design. The wiring differs by context.

### Zero-state (before typing)

Show recent searches with relative timestamps. Already exists in CommandPalette — keep as-is.

### Loading behavior

200ms debounce (existing). No spinner unless response takes >300ms. For a local Tantivy index, responses are sub-10ms — user never sees a loading indicator.

### Hidden power features (no UI, discoverable by habit)

- `project:claude-view` — scope to project
- `branch:main` — scope to branch
- `model:opus` — filter by model
- `after:2026-02-01` / `before:2026-02-28` — date range
- `"exact phrase"` — still works (boosts phrase signal even higher), but unnecessary since phrase matching is always on
- `session:<uuid>` — scope to specific session (used internally by Cmd+F)

---

## 2. UX: Result Cards

Users recognize conversations by context, not by session ID. The snippet IS the result.

### Card layout

```
┌─────────────────────────────────────────────────────┐
│  claude-view • main              2 hours ago        │
│                                                     │
│  ...we need to [deploy to production] tonight       │
│  before the release window closes. The CI pipeline  │
│  passed all checks so we should be good to...       │
│                                                     │
│  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄  │
│  ...the [production] [deploy] script failed with    │
│  exit code 1. Check the Docker config...            │
│                                                     │
│  47 matches across 12 turns              View all → │
└─────────────────────────────────────────────────────┘
```

### Changes from current

| Current | New |
|---|---|
| 1 snippet (top match only) | Top 2 snippets visible, rest behind "View all →" |
| Fuzzy matches not highlighted | Fixed: snippet generator uses exact terms (see §4 Snippet Strategy) |
| Expand to see all matches inline | "View all →" navigates to session with search pre-populated |

### "View all →" behavior

Navigates to `/sessions/:id?q=deploy+to+production`. ConversationView opens with Cmd+F bar auto-populated and scrolled to first match. Same smart search engine, scoped to session.

### CommandPalette results

Not hard-capped. Scrollable list with "View all N results" button at bottom that navigates to `/search?q=...`.

---

## 3. UX: In-Session Cmd+F

### Layout

```
┌─ ConversationView ──────────────────────────────────┐
│ ┌─────────────────────────────────────────────────┐ │
│ │  🔍  deploy timeout          3 of 12    ▲  ▼  ✕│ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│  ┌─ assistant ─────────────────────────────────┐    │
│  │  The [deploy] [timeout] was caused by a     │    │
│  │  misconfigured health check interval...     │ ◀── scrolled here
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### Behavior

- `Cmd+F` opens floating search bar at top of ConversationView
- Same `<SearchInput>` component
- Implicit scope: `session:<current_session_id>` — injected automatically
- API call: `GET /api/search?q=deploy+timeout&scope=session:abc-123`
- Results: highlighted `<mark>` tags on matching messages
- Match counter: `"3 of 12"` with `▲`/`▼` buttons (or `Enter`/`Shift+Enter`)
- Virtuoso `scrollToIndex` (using `firstItemIndex + matchIndex` for virtual index)
- `Esc` or `✕` closes bar and clears highlights
- Entry from search results: `?q=...` in URL → bar auto-opens with query pre-filled

### What it does NOT do

- No separate client-side search logic
- No `useInSessionSearch` hook — reuse `useSearch` with scope param
- No separate search index — same Tantivy engine, same API endpoint

---

## 4. Query Engine Architecture

### Decision tree

```
User input
    │
    ├─ UUID? ──────────────▶ Direct session lookup (existing, unchanged)
    │
    ├─ Regex detected AND ──▶ Grep engine (raw JSONL, ripgrep core crates)
    │  smart search got 0?     Only as FALLBACK, not first choice.
    │
    └─ Everything else ────▶ Smart search (multi-signal BooleanQuery)
```

### Auto-regex strategy: Smart-first, grep-fallback

**Why not auto-detect silently:** No search engine auto-detects regex by pattern matching. VS Code has an explicit toggle. GitHub uses `/regex/` delimiters. A heuristic like "contains `.*`" has false positives (user searching for code text containing metacharacters).

**The safer approach:**
1. Always run smart search first
2. If smart search returns 0 results AND the input contains regex metacharacters (`.*`, `\b`, `\d`, `\w`, `\s`, `[a-`, `(?:`, `^...$`), auto-retry with grep engine
3. If grep finds results, show them with a subtle note: "Showing regex matches"
4. If grep also finds nothing, show "No results"

This means: normal queries never accidentally hit grep. Only zero-result queries with regex-looking input get the fallback. Zero false positives for the 95% case.

### Multi-signal query (the core change)

For input `deploy to production` (after qualifier extraction):

```rust
// Outer query: qualifiers (Must) + text signals (Must wrapper)
BooleanQuery {
    Must: [qualifier TermQueries...]        // project:, branch:, session:, etc.
    Must: inner_text_query                  // at least one text signal must match
}

// Inner text query: all three signals compete (Should = OR with score accumulation)
inner_text_query = BooleanQuery {
    // Signal 1: Exact phrase — highest weight
    Should(boost = PHRASE_BOOST):  PhraseQuery(["deploy", "to", "production"])

    // Signal 2: All exact terms — BM25 scored
    Should(boost = EXACT_BOOST):  BooleanQuery(Must) {
        TermQuery("deploy"), TermQuery("to"), TermQuery("production")
    }

    // Signal 3: Fuzzy terms — typo tolerance
    Should(boost = FUZZY_BOOST):  BooleanQuery(Must) {
        FuzzyTermQuery("deploy", distance=1, prefix=true),
        FuzzyTermQuery("to", distance=1, prefix=true),
        FuzzyTermQuery("production", distance=1, prefix=true)
    }
}
```

**How it ranks:** A document matching the exact phrase scores highest (all 3 signals fire). A document with all exact terms but not adjacent scores mid (signals 2+3). A document with typos scores lowest (signal 3 only). Ranking falls out naturally from score accumulation.

**Single-word queries:** No phrase signal (PhraseQuery needs ≥2 terms). Just exact + fuzzy:

```rust
BooleanQuery {
    Should(boost = EXACT_BOOST):  TermQuery("deploy")
    Should(boost = FUZZY_BOOST):  FuzzyTermQuery("deploy", distance=1, prefix=true)
}
```

### Boost weight strategy

**The weights are tunable constants, not magic numbers.** Defined in `crates/search/src/query.rs`:

```rust
/// Boost weights for multi-signal scoring.
/// Invariant: PHRASE > EXACT > FUZZY (verified by integration tests).
const PHRASE_BOOST: f32 = 3.0;
const EXACT_BOOST: f32 = 1.5;
const FUZZY_BOOST: f32 = 0.5;
```

**Why these initial values:**
- The only hard requirement is ordering: phrase > exact > fuzzy
- Starting values based on Elasticsearch's default `tie_breaker` ratios
- Must be validated with integration tests on representative session data
- Tests assert ranking ORDER (not specific scores): phrase match result ranks above exact-terms result ranks above fuzzy result

**Proven pattern:** Elasticsearch `multi_match` with `type: "best_fields"` uses weighted signal combination. Algolia uses ordered ranking rules (typo > geo > words > proximity > attribute > exact > custom). MeiliSearch uses tiered matching. The pattern of weighted multi-signal scoring is industry standard.

### Recency tiebreaker

**Not a scoring boost — a secondary sort.** When relevance scores are close, newer results float up.

```rust
sessions.sort_by(|a, b| {
    let score_ratio = (a.best_score / b.best_score).min(b.best_score / a.best_score);
    if score_ratio > 0.9 {
        // Scores within 10% of each other — tiebreak by recency
        b.modified_at.cmp(&a.modified_at)
    } else {
        // Clear relevance winner — ignore recency
        b.best_score.partial_cmp(&a.best_score).unwrap_or(Equal)
    }
});
```

**Why percentage-based (10%) instead of absolute threshold:**
- BM25 scores are unbounded positive floats. An absolute threshold like `0.1` is meaningless — it could be huge or tiny depending on corpus size and term frequency.
- Percentage-based: `score_ratio > 0.9` means "scores within 10% of each other" regardless of magnitude. Proven approach — Elasticsearch's `function_score` uses decay functions on dates.

**Why NOT multiplicative decay:** `score × recency_factor` would let a recent mediocre match outrank an old perfect match. The user explicitly chose "relevance first, recency as tiebreaker only."

### Snippet generation strategy

**Tantivy limitation:** `FuzzyTermQuery::query_terms()` is a no-op (inherits empty default from the `Query` trait). Passing a `FuzzyTermQuery` to `SnippetGenerator::create()` produces zero highlights.

**Correct approach:** Build the snippet query from the **original user input terms** only, using `PhraseQuery` + `TermQuery` — never `FuzzyTermQuery`. This is actually what the current code already does (separate `QueryParser` re-parse for snippets).

```rust
// Snippet query: built from original terms, NOT from the multi-signal search query
let snippet_query = if tokens.len() >= 2 {
    // PhraseQuery highlights the exact phrase when found
    let phrase = PhraseQuery::new(terms.clone());
    Box::new(phrase) as Box<dyn Query>
} else {
    // Single term
    Box::new(TermQuery::new(terms[0].clone(), IndexRecordOption::WithFreqs))
};
let snippet_gen = SnippetGenerator::create(&searcher, &*snippet_query, content_field)?;
```

**What gets highlighted:** Exact term matches and exact phrase matches. Fuzzy-matched terms (typo variants) appear in the snippet context but are NOT highlighted. This is acceptable — Google doesn't highlight typo variants in snippets either. The user sees the right result; the highlighting marks the exact words they typed.

### `session:` qualifier

New qualifier added to `known_keys`. Produces `TermQuery(session_id_field, uuid, Occur::Must)`.

```rust
let known_keys = ["project", "branch", "model", "role", "skill", "session", "after", "before"];
```

Used internally by Cmd+F: `GET /api/search?q=deploy&scope=session:abc-123-def`.

### Date qualifiers: `after:` / `before:`

Parse `YYYY-MM-DD` dates, convert to unix timestamp, emit `RangeQuery` on the `timestamp` fast field.

```rust
"after"  => RangeQuery::new_i64_bounds("timestamp", Bound::Excluded(ts), Bound::Unbounded)
"before" => RangeQuery::new_i64_bounds("timestamp", Bound::Unbounded, Bound::Excluded(ts))
```

Requires adding `RangeQuery` to tantivy imports and `use chrono::NaiveDate` (not currently imported in `query.rs`).

---

## 5. Grep Engine (Regex Fallback)

### When it fires

Only as a fallback: smart search returns 0 results AND input contains regex metacharacters.

### Detection heuristic

```rust
fn has_regex_metacharacters(input: &str) -> bool {
    let patterns = [".*", r"\b", r"\d", r"\w", r"\s", "[a-", "(?:", "^$"];
    patterns.iter().any(|p| input.contains(p))
}
```

### Architecture

Separate from Tantivy. Searches raw `.jsonl` files in `~/.claude/projects/` using ripgrep core crates (`grep-matcher`, `grep-regex`, `grep-searcher`).

- **Endpoint:** `GET /api/grep?pattern=...&project=...&limit=...`
- **Engine:** Per-thread `RegexMatcher` construction (avoids contention). Validate regex once upfront; clone pattern string; build per thread inside `std::thread::scope`.
- **Response type:** `GrepResponse` — separate from `SearchResponse` (different structure: line-level matches, not session-level).
- **Parallelism:** `Semaphore` bounded to `available_parallelism()`. Scoped threads, no rayon.
- **UTF-8 safety:** `char_indices().nth(500)` for truncation, never raw byte slicing.

### Integration with smart search

When grep fallback fires:
1. Smart search returns 0 results
2. `has_regex_metacharacters(query)` returns true
3. Frontend calls `/api/grep` with the same query
4. Grep results displayed with a subtle note: "Showing regex matches"
5. Result cards use `GrepResults` component (line-level, not snippet-level)

This is handled in the frontend `useSearch` hook — not a backend concern. The backend exposes two separate endpoints; the frontend decides when to fall back.

---

## 6. Schema Changes

### New qualifier: `session:`

No schema change needed. `session_id` is already `STRING | STORED`. The qualifier produces a `TermQuery` filter.

### New qualifiers: `after:` / `before:`

No schema change needed. `timestamp` is already `i64 | FAST | STORED`. Date qualifiers produce `RangeQuery` on the existing field.

### Schema version

**No version bump needed.** No new fields, no tokenizer changes, no re-index required. The changes are purely in query construction (`query.rs`), not in the index structure.

---

## 7. What Changes From Current Code

### Backend (`crates/search/src/query.rs`) — THE core change

| Current | New |
|---|---|
| Fuzzy OR phrase (mutually exclusive) | Multi-signal: phrase + exact + fuzzy simultaneously |
| Single `FuzzyTermQuery` per term | Three signals combined in nested `BooleanQuery(Should)` |
| No recency in ranking | Percentage-based recency tiebreaker (10% threshold) |
| 5 known qualifier keys | 8 keys: + `session`, `after`, `before` |
| Snippet from `QueryParser` re-parse | Snippet from `PhraseQuery`/`TermQuery` (same approach, explicit construction) |

### Backend (new file: `crates/search/src/grep.rs`)

New grep engine using ripgrep core crates. Separate endpoint `/api/grep`.

### Frontend

| Current | New |
|---|---|
| 4 different search inputs with different behavior | Shared `<SearchInput>` component (visual only) |
| CommandPalette hard-caps at 5 results | Scrollable results + "View all N results" |
| No Cmd+F in ConversationView | Cmd+F → smart search scoped to session |
| HistoryView: client-side filter | **Unchanged** — keeps client-side filter |
| `useSearch` hook | Same hook, new `scope` param support |
| No `useGrep` hook | New `useGrep` hook for regex fallback |
| No regex fallback logic | `useSearch` → 0 results + regex detected → auto-call `useGrep` |

### Files untouched

- `crates/search/src/lib.rs` — schema unchanged (no version bump)
- `crates/search/src/indexer.rs` — write path unchanged
- `crates/search/src/types.rs` — response types unchanged (maybe add `topMatches: Vec<MatchHit>` for 2-snippet display)

---

## 8. What This Design Does NOT Include

- **Semantic search / embeddings** — Would require an embedding model + vector index. Over-engineered for a local tool. Fuzzy + phrase covers 95% of "I vaguely remember" searches.
- **Autocomplete / suggestions** — Small corpus (personal sessions), not worth the complexity. The 200ms debounce + instant results IS the autocomplete.
- **"Did you mean..."** — Fuzzy matching already handles typos silently. Explicit correction UI is redundant.
- **Stemming / lemmatization** — Tantivy's default tokenizer (lowercase + split on non-alphanumeric) is sufficient. Stemming risks false matches ("running" → "run" → matches "database run" when user meant "running shoes").
- **N-gram tokenizer** — Would bloat the index for marginal partial-match benefit. Prefix matching via `FuzzyTermQuery(prefix=true)` covers typeahead.

---

## 9. Prove-It Audit Results

This design was audited against three pillars (root cause, proven at scale, alternatives considered). Results:

| Claim | Verdict | Detail |
|---|---|---|
| Multi-signal BooleanQuery | **Pass** | Elasticsearch `multi_match`, Algolia tiered matching, MeiliSearch ranking rules |
| Boost weights | **Conditional** | Initial values 3.0/1.5/0.5 need integration test validation. Must assert ordering: phrase > exact > fuzzy |
| Recency tiebreaker | **Pass (revised)** | Changed from absolute threshold (0.1) to percentage-based (10%). Proven: ES decay functions |
| Auto-regex | **Pass (revised)** | Changed from silent auto-detect to smart-first + grep-fallback on zero results. Zero false positives |
| `session:` qualifier | **Pass** | Standard Tantivy/Lucene filtered query |
| Snippet generation | **Pass (revised)** | Tantivy `FuzzyTermQuery::query_terms()` is a no-op. Snippets built from exact terms only. Limitation documented. |
| Unified search input | **Pass (revised)** | Visual component shared. HistoryView keeps client-side wiring. |

### Tantivy API verification (confirmed against v0.22.1)

- `BoostQuery::new(Box<dyn Query>, f32)` — wraps any query, delegates `query_terms()`
- `BooleanQuery` with only `Should` clauses — OR semantics, at least one must match
- `PhraseQuery::new(Vec<Term>)` — direct construction without QueryParser
- `SnippetGenerator::create(&Searcher, &dyn Query, Field)` — accepts any Query, but `FuzzyTermQuery` yields zero terms (no-op `query_terms()`)
