# Unified Pipeline Analytics Redesign

**Date:** 2026-02-25
**Status:** Ready for execution — Section 1 (Pipeline) + Section 2 (Queries) + Section 3 (Impl Plan) complete
**Branch:** `worktree-mobile-remote`

---

## Table of Contents

1. [Context & Problem](#context--problem)
2. [Investigation Summary](#investigation-summary)
3. [What Was Already Fixed (Before This Design)](#what-was-already-fixed)
4. [What Remains Broken](#what-remains-broken)
5. [Design Decision: Store Everything](#design-decision-store-everything)
6. [Section 1: Pipeline Write Layer (APPROVED)](#section-1-pipeline-write-layer)
7. [Section 2: Query Layer Redesign (PENDING)](#section-2-query-layer-redesign)
8. [Prove-It Audit Trail](#prove-it-audit-trail)
9. [Key Learnings](#key-learnings)
10. [Next Steps](#next-steps)

---

## Context & Problem

### What happened

The `worktree-mobile-remote` branch introduced a **unified single-pass indexing pipeline** (`scan_and_index_all()`) to replace the old two-pass pipeline (`pass_1_read_indexes` → `pass_2_deep_index`). The old pipeline was marked `#[deprecated]`.

The new pipeline correctly:
- Parses every JSONL file (extracting turns, models, metadata)
- Computes `primary_model` from turn data
- Writes session-level aggregates to the `sessions` table via `upsert_parsed_session()`

The new pipeline **silently dropped**:
- Writing per-turn data to the `turns` table (`batch_insert_turns_tx()` never called)
- Writing model metadata to the `models` table (`batch_upsert_models_tx()` never called)
- Writing invocations to the `invocations` table
- All Tantivy full-text search indexing (no `SearchIndex` parameter in function signature)

### Why main works fine

Main's old pipeline (`pass_2_deep_index`) called `write_results_sqlx()` which executed `batch_insert_turns_tx()`, `batch_upsert_models_tx()`, and `batch_insert_invocations_tx()` inside a single transaction, then wrote search batches to Tantivy. The new pipeline replaced all of this with a single `upsert_parsed_session()` call.

### User's requirement

> "This design is NOT ENOUGH, NOT SUFFICIENT. I want this app to be zero data loss, no guessing. You MUST reflect real stat/data from the raw data."

Decision: **Store everything the parser extracts.** The JSONL is the source of truth, and every piece of data the parser can extract must be persisted to the DB — not just what current queries need, but everything for future analytics.

---

## Investigation Summary

### Systematic debugging traced two regressions:

**Regression 1: Activity Page Stats Cards (NOT a regression)**
- Activity page was newly built on this branch (4 commits: `00142f31` → `0981ea0c`)
- `SummaryStats` component only rendered 4 cards; the API already had tool/agent/MCP/skills data
- Root cause: incomplete new feature, not a regression from main
- Fixed: Added 4 more cards (Tools, Agents, MCP, Skills) with aggregation

**Regression 2: Contributions "By Model" empty data (REAL regression)**
- `get_model_breakdown()` in `snapshots.rs` used CTEs joining `turns` table
- `turns` table was empty because `scan_and_index_all()` never writes to it
- `sessions.primary_model` was correctly populated (parser writes it directly)
- Fixed: Rewrote `get_model_breakdown()` to query `sessions.primary_model` instead of `turns`

### Full audit of turns table consumers found 3 more broken queries:

| Query | File | Endpoint | Status |
|-------|------|----------|--------|
| `get_all_models()` | `queries/models.rs:38` | `GET /api/models` | **BROKEN** — `LEFT JOIN turns` returns 0 counts |
| `get_token_stats()` | `queries/models.rs:57` | `GET /api/stats/tokens` | **BROKEN** — `FROM turns` returns all zeros |
| Insights `primary_model` subquery | `routes/insights.rs:406` | `GET /api/insights` | **BROKEN** — subquery from `turns` returns NULL |
| `get_model_breakdown()` | `snapshots.rs` | `GET /api/contributions` | **FIXED** — already rewritten to use `sessions` |
| `get_trends_for_periods()` | `trends.rs` | `GET /api/trends` | **OK** — already reads from `sessions` |

### Full audit also discovered:

- **Full-text search is completely non-functional** on this branch. `scan_and_index_all()` has no `SearchIndex` parameter. The old `pass_2_deep_index` was the only function that wrote to Tantivy. Search silently returns no results.
- **`backfill_primary_models()`** runs at startup but is a no-op (new pipeline writes `primary_model` directly).

---

## What Was Already Fixed

Before this design session, these patches were applied:

1. **Activity Stats Cards** — `activity-utils.ts`, `SummaryStats.tsx`, `ActivityPage.tsx`
   - Added `totalToolCalls`, `totalAgentSpawns`, `totalMcpCalls`, `uniqueSkills` to `ActivitySummary`
   - Added 4 new cards with icons

2. **By Model Query** — `snapshots.rs`
   - Rewrote `get_model_breakdown()` to use `sessions.primary_model` with session-level token aggregates
   - Removed `turns` table dependency
   - All 15 contributions tests pass

---

## What Remains Broken

### Pipeline layer (Section 1 — designed, approved)
- `turns` table not populated → 3 broken queries
- `models` table not updated → `/api/models` incomplete
- `invocations` table not populated
- Search index not populated → full-text search broken
- No transaction atomicity per session (3 separate DB calls can leave partial state)
- N sessions = N individual transactions = N WAL fsyncs (slow)

### Query layer (Section 2 — pending design)
- `get_all_models()` still joins `turns` table
- `get_token_stats()` still reads from `turns` table
- Insights `primary_model` still subqueries `turns` table
- These need redesign, not just patching, since infrastructure changed

---

## Design Decision: Store Everything

### What the parser extracts vs what gets stored

| Data | Parser extracts it? | Old pipeline stored it? | New pipeline stores it? |
|------|-------------------|----------------------|----------------------|
| Session aggregates (63 fields) | Yes | Yes | Yes |
| Per-turn data (uuid, model_id, tokens, seq, timestamp) | Yes (`ParseResult.turns`) | Yes (`batch_insert_turns_tx`) | **NO — dropped** |
| Model metadata (id, provider, family) | Yes (`ParseResult.models_seen`) | Yes (`batch_upsert_models_tx`) | **NO — dropped** |
| Invocations (tool/skill usage) | Yes (`ParseResult.raw_invocations`) | Yes (`batch_insert_invocations_tx`) | **NO — dropped** |
| Search messages (full text) | Yes (`ParseResult.search_messages`) | Yes (Tantivy `index_session`) | **NO — dropped** |

Decision: **All 5 categories must be persisted.** The parser already does the work — the write step was simply omitted.

### What's truly unique to the `turns` table (not derivable from `sessions`)

- Per-model token split within a multi-model session (e.g., 70% Opus / 30% Haiku)
- Turn-level timestamps and sequence ordering
- Turn UUIDs and parent relationships
- Per-turn cache behavior (cache_read vs cache_creation per API call)

---

## Section 1: Pipeline Write Layer

**Status: APPROVED after 3 rounds of prove-it audit**

### Architecture: 3-Phase Pipeline

```
scan_and_index_all(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<&SearchIndex>,       // NEW — was missing
    on_file_done: F,
) → Result<(usize, usize), String>

CURRENT_PARSE_VERSION = 13                    // BUMP from 12

Phase 1: PARSE (parallel, CPU-bound, zero I/O writes)
┌──────────────────────────────────────────────────────────┐
│  Semaphore(available_parallelism)                        │
│  for each .jsonl file (tokio::spawn):                    │
│    stat() → staleness check against in-memory HashMap    │
│      skip if mtime+size match AND parse_version >= 13    │
│      UNLESS search.needs_full_reindex → force all        │
│    spawn_blocking(parse_file_bytes)  // mmap, zero-copy  │
│    build IndexedSession {                                │
│      parsed: ParsedSession,          // 63 fields        │
│      turns: Vec<RawTurn>,            // per-turn data    │
│      models_seen: Vec<String>,       // model IDs        │
│      invocations: Vec<ClassifiedInvocation>,             │
│      search_messages: Vec<SearchableMessage>,            │
│      cwd: Option<String>,                                │
│      git_root: Option<String>,                           │
│    }                                                     │
│  → push to Vec<IndexedSession>                           │
└──────────────────────────────────────────────────────────┘
         │
         ▼  Vec<IndexedSession> (~200MB peak for 1000 sessions)
Phase 2: SQLITE WRITE (sequential, chunked, single writer)
┌──────────────────────────────────────────────────────────┐
│  Dedup models across all sessions → HashMap<String, _>   │
│  for chunk in indexed_sessions.chunks(200):              │
│    BEGIN IMMEDIATE                                       │
│    for each session in chunk:                            │
│      DELETE FROM turns WHERE session_id = ?              │
│      DELETE FROM invocations WHERE session_id = ?        │
│      UPSERT sessions (63 params)                        │
│      UPDATE topology (session_cwd, git_root)             │
│      INSERT INTO turns (per-turn, loop)                  │
│      INSERT INTO invocations (per-invocation, loop)      │
│    UPSERT models (deduped, typically 3-6 rows)           │
│    COMMIT                                                │
│    // yield between chunks — live manager can write      │
└──────────────────────────────────────────────────────────┘
         │
         ▼  SQLite committed successfully
Phase 3: SEARCH INDEX (sequential, after SQLite success)
┌──────────────────────────────────────────────────────────┐
│  if search_index.is_some():                              │
│    for each session with search_messages:                │
│      search.index_session(session_id, messages)          │
│    search.commit()    // single Tantivy commit           │
│    if search.needs_full_reindex:                         │
│      search.mark_schema_synced()                         │
│  // If Tantivy fails: log warning, next startup rebuilds │
└──────────────────────────────────────────────────────────┘
```

### Key Design Decisions

**Why `BEGIN IMMEDIATE` (not EXCLUSIVE):**
WAL mode (confirmed at `lib.rs:113`) allows concurrent readers regardless of writer state. `IMMEDIATE` acquires write lock upfront (no mid-transaction `SQLITE_BUSY` upgrade) while letting readers proceed. `EXCLUSIVE` would block readers — unnecessary in WAL mode.
*Citation: SQLite docs by D. Richard Hipp — "WAL mode readers never conflict with writers."*

**Why DELETE-then-INSERT (not INSERT OR IGNORE):**
Re-indexing a session whose JSONL changed means the set of turns may have shrunk (compaction removes old entries). `INSERT OR IGNORE` with UUID PK would leave stale turns from a previous parse. `DELETE WHERE session_id = ?` + fresh INSERT is the only correct approach for replace-all semantics.
*Citation: PostgreSQL, CockroachDB docs recommend DELETE+INSERT over MERGE/UPSERT for set-replacement.*

**Why batch collect → single write (not per-task write):**
SQLite's write bottleneck is `fsync` at `COMMIT`. N transactions = N fsyncs (~60 writes/sec individually vs ~50,000/sec in batch). Parse is CPU-bound and embarrassingly parallel; write is I/O-bound and inherently sequential in SQLite.
*Citation: SQLite FAQ — "50,000 INSERTs/sec in a transaction, ~60/sec individually." Fossil SCM uses this exact pattern.*

**Why chunk at 200 sessions:**
Single transaction blocking write lock for the entire batch could starve the live session manager (`parse_tail()` → `upsert_partial_session()`). `busy_timeout` is 30s (`lib.rs:115`), but chunks of 200 sessions complete in ~100ms (10,000 INSERTs at SQLite's in-txn speed). Inter-chunk yield lets live manager writes slip through.

**Why Phase 3 search AFTER Phase 2 SQLite:**
1. Matches old pipeline's proven ordering (`write_results_sqlx` → search batch)
2. If SQLite fails, don't pollute search with entries for unpersisted sessions
3. Tantivy crash recovery is built-in: `needs_full_reindex` flag + `mark_schema_synced()` pattern (verified by test at `search/src/lib.rs:836`)

**Why parse version bump (12 → 13):**
Without bumping, sessions already indexed at v12 pass the staleness check and are skipped. Their turns/models/invocations/search would never be populated. Bump forces full re-index of all sessions so every table gets backfilled.

### Memory Budget

| Component | Per session | 1000 sessions |
|-----------|-----------|---------------|
| `ParsedSession` (63 fields + JSON strings) | ~2KB | ~2MB |
| `Vec<RawTurn>` (~50 turns × 200 bytes) | ~10KB | ~10MB |
| `Vec<ClassifiedInvocation>` | ~1KB | ~1MB |
| `Vec<SearchableMessage>` (full text) | ~200KB | ~200MB |
| **Total** | ~213KB | **~213MB** |

Raw JSONL files are NOT held in memory — mmap is used (`indexer_parallel.rs:566`, kernel page cache manages it). 213MB peak is acceptable for a dev machine (Chrome tabs use more).

### Data Integrity Invariants (enforced by code path, not runtime checks)

After write, these must hold:
```
sessions.turn_count == COUNT(*) FROM turns WHERE session_id = ?
sessions.total_input_tokens == SUM(input_tokens) FROM turns WHERE session_id = ?
sessions.primary_model == MODE(model_id) FROM turns WHERE session_id = ?
```

Both the session aggregates and the turn-level data come from the same `ParseResult` — the parser is the single source of truth.

### Function Signature Changes

```rust
// OLD (current branch)
pub async fn scan_and_index_all<F>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    on_file_done: F,
) -> Result<(usize, usize), String>

// NEW
pub async fn scan_and_index_all<F>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,  // NEW — Arc, not &ref
    registry: Option<Arc<Registry>>,                              // NEW
    on_file_done: F,
) -> Result<(usize, usize), String>
```

### Callers to update (4 total)

1. `crates/server/src/main.rs:336` — startup scan
2. `crates/server/src/main.rs:388` — periodic re-scan
3. `crates/server/src/routes/sync.rs:342` — manual sync endpoint
4. `crates/server/src/live/manager.rs:733` — live manager catch-up

---

## Section 2: Query Layer Redesign

**Status: DESIGNED**

### Principle

Queries should use the **most efficient source** that provides the required granularity:
- **`sessions` table** (denormalized aggregates): Fast path for total counts. One row per session.
- **`turns` table** (per-turn data): Required for per-model breakdowns within multi-model sessions.
- **`models` table** (metadata): Model provider/family info, first/last seen timestamps.

### Design Decisions (answers to Section 2 questions)

**Q1: Should `get_all_models()` use turns or sessions?**

**Answer: Keep using `turns`.** The current query is correct — it just needs turns to be populated (which Section 1 handles). Using `sessions.primary_model` would only count sessions where a model was the *most-used*, missing secondary usage. The `LEFT JOIN turns` query counts every session where the model appeared, which is more accurate for a "models overview" page.

The query remains unchanged:
```sql
SELECT m.id, m.provider, m.family, m.first_seen, m.last_seen,
       COUNT(t.uuid) as total_turns,
       COUNT(DISTINCT t.session_id) as total_sessions
FROM models m
LEFT JOIN turns t ON t.model_id = m.id
GROUP BY m.id
ORDER BY total_turns DESC
```

**Q2: Should `get_token_stats()` use sessions or turns?**

**Answer: Rewrite to use `sessions`.** The current query aggregates total tokens across all turns — no per-model split. For simple totals, the `sessions` table is faster (fewer rows) and the denormalized values are identical (same parser source). The `turns` table would be needed only for per-model token breakdown, which is a different query.

New query:
```sql
SELECT
    COALESCE(SUM(total_input_tokens), 0),
    COALESCE(SUM(total_output_tokens), 0),
    COALESCE(SUM(cache_read_tokens), 0),
    COALESCE(SUM(cache_creation_tokens), 0),
    COALESCE(SUM(turn_count), 0),
    COUNT(*)
FROM valid_sessions
```

Why `valid_sessions` instead of `sessions`: Filters out sidechain/orphan sessions, consistent with other analytics views. The `turn_count` column replaces `COUNT(*)` from turns (equivalent — parser writes both from the same data). `COUNT(*)` on sessions gives session count directly.

**Q3: Should insights use `s.primary_model` directly?**

**Answer: Yes.** Replace the expensive correlated subquery with the denormalized column. The subquery:
```sql
COALESCE(
    (SELECT model_id FROM turns t
     WHERE t.session_id = s.id
     GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1),
    NULL
) as primary_model
```
becomes simply:
```sql
s.primary_model
```

The parser writes `primary_model` to sessions directly (`indexer_parallel.rs:2888`). The subquery is O(N) correlated queries for N sessions and computes the same value the parser already wrote. The `COALESCE(..., NULL)` was also a no-op.

**Q4: Any new queries to build?**

**Answer: No.** The pipeline fix (Section 1) restores all data that was dropped. The existing query set covers all current UI needs. New queries (e.g., per-model token breakdown) can be built later when the UI needs them — the data will be in the `turns` table waiting.

### Summary of Query Changes

| Query | File | Action | Reason |
|-------|------|--------|--------|
| `get_all_models()` | `queries/models.rs:38` | **NO CHANGE** | Correct query, just needs populated `turns` table (Section 1) |
| `get_token_stats()` | `queries/models.rs:57` | **REWRITE** | Switch from `turns` to `valid_sessions` — faster, same data for aggregates |
| Insights `primary_model` | `routes/insights.rs:406` | **SIMPLIFY** | Replace correlated subquery with `s.primary_model` |
| `get_model_breakdown()` | `snapshots.rs` | **NO CHANGE** | Already fixed in prior session |
| `get_trends_for_periods()` | `trends.rs` | **NO CHANGE** | Already reads from sessions |

---

## Section 3: Implementation Plan

**Status: READY FOR EXECUTION**

### Task ordering

Tasks are grouped into 3 phases with dependencies shown. Within each phase, tasks are ordered by dependency.

### Phase 1: Pipeline Write Layer (8 tasks)

**Task 1: Bump `CURRENT_PARSE_VERSION` to 13**
- File: `crates/db/src/indexer_parallel.rs:34`
- Change: `12` → `13`
- Why: Forces re-index of all sessions so turns/models/invocations/search get backfilled
- Test: Existing staleness check at line 2823 uses `>= CURRENT_PARSE_VERSION`

**Task 2: Add parameters to `scan_and_index_all` signature**
- File: `crates/db/src/indexer_parallel.rs:2715`
- Add: `search_index: Option<&claude_view_search::SearchIndex>`, `registry: Option<&claude_view_core::Registry>`
- Dependency: `claude-view-db` already depends on `claude-view-search` (confirmed in `crates/db/Cargo.toml:10`)
- The `registry` is needed to classify `raw_invocations` into `(source_file, byte_offset, invocable_id, ...)` tuples for the invocations table

**Task 3: Refactor parse phase — collect `IndexedSession` structs instead of writing per-task**
- File: `crates/db/src/indexer_parallel.rs` (inside `scan_and_index_all`)
- Currently: Each spawned task calls `db.upsert_parsed_session()` individually (line 2993)
- After: Each task returns an `IndexedSession` struct; parent collects into `Vec<IndexedSession>`
- New struct:
```rust
struct IndexedSession {
    parsed: ParsedSession,
    turns: Vec<claude_view_core::RawTurn>,
    models_seen: Vec<String>,
    classified_invocations: Vec<(String, i64, String, String, String, i64)>,
    search_messages: Vec<claude_view_core::SearchableMessage>,
    cwd: Option<String>,
    git_root: Option<String>,
}
```
- Move `db.upsert_parsed_session()` and `db.update_session_topology()` calls OUT of the spawned task
- The spawned task becomes pure: parse + build struct + return
- Add invocation classification (using `registry`) inside the spawned task (CPU work, same as old `pass_2_deep_index` at line 2254)

**Task 4: Implement Phase 2 — chunked SQLite write**
- File: `crates/db/src/indexer_parallel.rs` (new code after Task 3's collect)
- After all spawned tasks complete, iterate `indexed_sessions.chunks(200)`:
  - `BEGIN IMMEDIATE`
  - For each session in chunk:
    - `DELETE FROM turns WHERE session_id = ?`
    - `DELETE FROM invocations WHERE session_id = ?`
    - `UPSERT sessions` (existing `execute_upsert_parsed_session`)
    - `UPDATE topology` (existing `update_session_topology`)
    - `INSERT INTO turns` (existing `batch_insert_turns_tx`)
    - `INSERT INTO invocations` (existing `batch_insert_invocations_tx`)
  - Dedup models across chunk → `batch_upsert_models_tx`
  - `COMMIT`
- Reuse existing `_tx` functions (they take `&mut Transaction`, no BEGIN/COMMIT)
- Need a new `upsert_parsed_session_tx` variant that takes a transaction instead of calling `self.pool().begin()`

**Task 5: Implement Phase 3 — search index write**
- File: `crates/db/src/indexer_parallel.rs` (new code after Task 4's SQLite write)
- After SQLite commit succeeds, write search messages to Tantivy
- Port the search indexing logic from old `pass_2_deep_index` (lines 2365-2448):
  - Convert `SearchableMessage` → `SearchDocument` (adding session_id, project, branch, model, skills, preview, last_message)
  - Add summary document per session
  - Call `search.index_session(session_id, &docs)` per session
  - Single `search.commit()` at end
  - `search.reader.reload()` for immediate visibility
  - `search.mark_schema_synced()` if full reindex was triggered
- Handle `needs_full_reindex` flag: if search index schema is stale, force all sessions through search (even if SQLite staleness check would skip them)

**Task 6: Update 4 callers of `scan_and_index_all`**
- `crates/server/src/main.rs:336` — startup scan: pass `search_index` from `search_index_holder`, `registry` from `idx_registry`
- `crates/server/src/main.rs:388` — periodic re-scan: same holders
- `crates/server/src/routes/sync.rs:342` — manual sync: pass from `state.search_index`, `state.registry`
- `crates/server/src/live/manager.rs:733` — live manager overflow: pass from `manager.search_index`, `manager.registry` (need to verify these are on `LiveSessionManager`)
- Each caller must lock the `RwLock<Option<Arc<SearchIndex>>>` and extract `Option<&SearchIndex>`

**Task 7: Remove `backfill_primary_models` call and function**
- Remove call at `crates/server/src/main.rs:359-363`
- Remove function at `crates/db/src/queries/system.rs:65`
- Redundant: pipeline writes `primary_model` directly, and version bump forces re-index of all sessions

**Task 8: ~~Remove deprecated `pass_2_deep_index` function~~ DEFERRED**
- **DEFERRED per Round 4 audit:** 20+ test callers of `pass_2_deep_index` make removal a multi-hour migration, not a cleanup task
- Keep deprecated code + tests untouched for now
- Schedule as separate cleanup task in a future session

### Phase 2: Query Layer Fixes (3 tasks)

**Task 9: Rewrite `get_token_stats()` to use `valid_sessions`**
- File: `crates/db/src/queries/models.rs:57`
- Replace `FROM turns` with `FROM valid_sessions`
- Map: `SUM(turn_count)` → `turns_count`, `COUNT(*)` → `sessions_count`
- Test: Hit `GET /api/stats/tokens`, verify non-zero values

**Task 10: Simplify insights `primary_model` subquery**
- File: `crates/server/src/routes/insights.rs:406-411`
- Replace the 5-line correlated subquery with `s.primary_model`
- Must match the `LightSession` struct field name and position in SELECT
- Test: Hit `GET /api/insights`, verify `primary_model` populated on returned sessions

**Task 11: Verify `get_all_models()` works (no code change)**
- File: `crates/db/src/queries/models.rs:38`
- No code change needed — query is correct, just needs turns data from Phase 1
- Test: Hit `GET /api/models`, verify `total_turns > 0` and `total_sessions > 0`

### Phase 3: Verification & Cleanup (3 tasks)

**Task 12: Run targeted test suites**
- `cargo test -p claude-view-db` — all DB tests (turns populated, models written)
- `cargo test -p claude-view-server` — API endpoint tests
- `cargo test -p claude-view-core` — only if core changes were needed (unlikely)
- Existing 15 contributions tests must still pass

**Task 13: Manual browser verification**
- Start server, wait for indexing to complete
- Verify each analytics page has non-zero data:
  - Activity page: 8 stat cards populated
  - Models page (`/api/models`): models listed with turn/session counts
  - Token stats (`/api/stats/tokens`): non-zero token totals
  - Insights page (`/api/insights`): `primary_model` populated on sessions
  - Contributions ("By Model"): model breakdown chart populated
  - Search: type a query, verify results returned
- Check console for errors or warnings

**Task 14: Commit and verify clean build**
- `cargo build` — no warnings from removed deprecated code
- `cargo clippy` — no new lints
- Commit with descriptive message covering both pipeline and query fixes

### Dependency Graph

```
Task 1 (version bump)
  ↓
Task 2 (signature change)
  ↓
Task 3 (collect phase)
  ↓
Task 4 (SQLite write phase)    Task 9 (token stats rewrite)
  ↓                              ↓
Task 5 (search write phase)    Task 10 (insights simplify)
  ↓                              ↓
Task 6 (update callers)        Task 11 (verify get_all_models)
  ↓
Task 7 (remove backfill)
  ↓
Task 8 (remove deprecated, optional)
  ↓
Task 12 (tests) ← depends on Phase 1 + Phase 2
  ↓
Task 13 (browser verification)
  ↓
Task 14 (commit)
```

Phase 1 tasks are sequential (each builds on the previous).
Phase 2 tasks are independent of each other but need Phase 1 complete to verify.
Phase 3 depends on both Phase 1 and Phase 2.

### Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Memory spike with 1000+ sessions collected in Vec | Peak ~213MB (see Memory Budget), acceptable for dev machine. Could add streaming if needed. |
| Chunked transactions blocking live manager | 200-session chunks complete in ~100ms. `busy_timeout` is 30s. Inter-chunk yield. |
| Search index corruption on crash mid-write | Tantivy has built-in recovery. `needs_full_reindex` flag + `mark_schema_synced()` after commit. |
| `upsert_parsed_session_tx` doesn't exist yet | Need to extract from current `upsert_parsed_session()` — move pool().begin()/commit() to caller |
| `LiveSessionManager` may not have search/registry access | Verify struct fields; may need to add them to constructor |

---

## Prove-It Audit Trail

### Round 1 (v2 design)

Found 3 issues:
- `BEGIN EXCLUSIVE` wrong for WAL mode → fixed to `BEGIN IMMEDIATE`
- Memory estimate wrong (didn't account for `search_messages`) → revised
- Chunking should be built-in, not deferred → added 200/chunk

### Round 2 (v3 design)

Found 1 structural error:
- Phase 1 search indexing wrong — Tantivy `IndexWriter` serializes internally, and `scan_and_index_all` had no `SearchIndex` parameter
- Corrected to 3-phase: parse → SQLite → Tantivy

### Round 3 (final 3-phase design)

Found 2 gaps:
- **Parse version bump missing** — without 12→13 bump, existing sessions never get turns populated
- **Search index completely broken on branch** — regression from dropping `SearchIndex` parameter; `needs_full_reindex` flag not propagated

After fixing both: **all 7 claims pass at high confidence.**

### Round 4 (consolidated prove-it audit)

Found 4 issues that must be resolved before execution:

1. **BLOCKER: Defer Task 8 entirely** — `pass_2_deep_index` has 20+ test callers. Removing it is a multi-hour test migration, not a cleanup task. Keep deprecated code + tests untouched.

2. **HIGH: `Option<&SearchIndex>` signature won't work** — holding `std::sync::RwLock` guard across 30s+ async scan blocks `clear_cache`. Changed to `Option<Arc<SearchIndex>>` — callers clone the Arc cheaply, no lock held during scan.

3. **HIGH: LiveSessionManager has neither search_index nor registry fields** — Must add both fields to struct + constructor + call site in `lib.rs:152`. Folded into Task 6.

4. **MEDIUM: `upsert_parsed_session_tx` concern incorrect** — `execute_upsert_parsed_session` is already generic over `Executor<'e, Database = Sqlite>`. Just pass `&mut *tx` directly. No new function needed.

5. **MEDIUM: `on_file_done` callback timing** — Must fire during Phase 2 write loop (after DB commit), not Phase 1 parse collect.

6. **LOW: Memory estimate ~200KB/session is a floor** — Outlier sessions could be 1-10MB from search_messages. Added total byte count logging.

7. **LOW: Sidechain filtering asymmetry** — token stats excludes sidechains (uses `valid_sessions`), models page includes them (uses `turns`). Documented as intentional design choice.

---

## Key Learnings

### 1. "Unified pipeline" didn't mean "complete pipeline"

The refactor replaced the old two-pass pipeline with a single-pass that only wrote sessions. It was a simplification that accidentally dropped 4 write targets (turns, models, invocations, search). The parser still extracted everything — only the write step was incomplete.

### 2. `Option`/`undefined` silently absorbs gaps

When `turns` is empty, queries return `0` or `NULL` — not errors. `LEFT JOIN` returns the left side with NULLs. `SUM()` on an empty table returns `NULL`, which `COALESCE(..., 0)` catches. Everything looks "fine" — just with zero values. This is why the broken analytics weren't caught until manual inspection.

### 3. Parse + Write should be separate phases

Mixing CPU-bound parsing with I/O-bound DB writes in the same task loses both parallelism (SQLite serializes writes) and atomicity (partial writes on failure). Separating them is textbook producer-consumer.

### 4. Transaction mode matters in WAL

`BEGIN EXCLUSIVE` blocks readers in WAL mode unnecessarily. `BEGIN IMMEDIATE` is the correct choice — acquires write lock upfront (no SQLITE_BUSY upgrade) while allowing concurrent reads.

### 5. Version bump is essential for schema changes

When the write format changes (new tables populated), `CURRENT_PARSE_VERSION` must bump so the staleness check forces re-indexing. Without this, only sessions whose JSONL files happen to change will get the new data.

### 6. Full-text search was silently broken

`scan_and_index_all()` never received a `SearchIndex` parameter, so Tantivy was never written to. The old `pass_2_deep_index` (deprecated) was the only writer. This wasn't caught because search returning no results looks the same as "no matching content."

---

## Next Steps

### Ready for execution

All 3 sections are designed and approved. Execute the implementation plan in Section 3:

1. **Phase 1** (Tasks 1-8): Pipeline write layer — bump version, refactor `scan_and_index_all` into 3 phases, update callers, remove dead code
2. **Phase 2** (Tasks 9-11): Query layer fixes — rewrite `get_token_stats`, simplify insights query, verify `get_all_models`
3. **Phase 3** (Tasks 12-14): Verification — run tests, browser verification, commit

### Testing strategy

- Existing 15 contributions tests must still pass
- Verify `turns` table has rows after `scan_and_index_all`
- Verify `models` table updated after indexing
- Verify search returns results after indexing
- Verify re-index idempotency (index twice, same result)
- Verify parse version bump forces re-index
- Manual: hit every analytics endpoint, verify non-zero data

---

## Files Referenced

### Pipeline
- `crates/db/src/indexer_parallel.rs` — `scan_and_index_all()` (line 2712), `ParsedSession` (line 40), `parse_file_bytes` (line 617)
- `crates/db/src/queries/sessions.rs` — `UPSERT_SESSION_SQL` (line 16), `execute_upsert_parsed_session` (line 122)
- `crates/db/src/queries/row_types.rs` — `batch_insert_turns_tx` (line 381), `batch_upsert_models_tx` (line 346)
- `crates/db/src/lib.rs` — WAL mode (line 113), busy_timeout 30s (line 115)
- `crates/search/src/lib.rs` — `needs_full_reindex` (line 106), `mark_schema_synced` (line 264)

### Queries (broken, need redesign)
- `crates/db/src/queries/models.rs` — `get_all_models()` (line 38), `get_token_stats()` (line 57)
- `crates/server/src/routes/insights.rs` — primary_model subquery (line 406)
- `crates/db/src/snapshots.rs` — `get_model_breakdown()` (already fixed)

### Callers of scan_and_index_all (need signature update)
- `crates/server/src/main.rs:336` — startup
- `crates/server/src/main.rs:388` — periodic re-scan
- `crates/server/src/routes/sync.rs:342` — manual sync
- `crates/server/src/live/manager.rs:733` — live manager

### Frontend (already fixed)
- `src/lib/activity-utils.ts` — `computeSummary()` aggregation
- `src/components/activity/SummaryStats.tsx` — 8 stat cards
- `src/pages/ActivityPage.tsx` — skeleton placeholders
