---
status: approved
date: 2026-01-27
---

# SQLite Indexer + Terminal Startup UX

> Make `/api/projects` instant by caching session metadata in SQLite, with rich terminal progress on first run.

## Problem

The `/api/projects` endpoint takes **81 seconds** because it scans all 780 JSONL files (828 MB) on every request. The v2 design specifies SQLite + Tantivy for caching, but these crates are placeholders.

## Solution

Activate the `db` crate with SQLite session caching. On startup, index all sessions into SQLite with terminal progress bars (indicatif). The API then reads from DB (< 5ms) instead of scanning files.

## Parent Specs

This plan implements part of the v2 design. References:

| Spec | File | Relevant Sections |
|------|------|-------------------|
| **v2 Design** | `vibe-recall-v2-design.md` (approved) | §4 Architecture, §6 DB Schema, §9 Phase 2 |
| **Analytics** | `vibe-recall-analytics-design.md` (draft) | §8 Schema Additions (forward-compatible) |
| **Phase 1** | `vibe-recall-phase1-implementation.md` (pending) | Task 4-5 parser/discovery (already built) |

## Scope

| In scope | Out of scope (later) |
|----------|---------------------|
| SQLite sessions + indexer_state tables | Tantivy search index (v2 Phase 2) |
| Startup indexing pipeline with progress | File watcher / live updates (v2 Phase 2) |
| Terminal UX with indicatif | Tags, skills tables (v2 Phase 3) |
| `/api/projects` reads from DB | `/api/search` endpoint (v2 Phase 2) |
| Incremental re-indexing (mtime check) | Analytics columns (analytics design) |

## Architecture

```
Startup flow:
  1. Open ~/.cache/vibe-recall/vibe-recall.db
  2. Run migrations
  3. Scan ~/.claude/projects/ for .jsonl files
  4. Compare (path, mtime, size) against indexer_state
  5. Index new/changed files with progress bar
  6. Start HTTP server → "Ready" message

Request flow:
  GET /api/projects
    → SELECT * FROM sessions GROUP BY project
    → < 5ms response
```

## DB Location

`~/.cache/vibe-recall/vibe-recall.db` — XDG-compliant, safe to delete and rebuild.

## Schema

Aligned with the v2 design spec (`vibe-recall-v2-design.md` section 6), with additions
for fields the current API returns that aren't in the v2 spec.

```sql
-- Session metadata (denormalized for fast list queries)
-- Base columns from v2 spec, plus extra columns for current API contract
CREATE TABLE IF NOT EXISTS sessions (
    -- v2 spec columns
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,           -- encoded dir name (e.g., "-Users-user-dev--myorg-project")
    title TEXT,                         -- reserved for future use
    preview TEXT NOT NULL DEFAULT '',    -- first user message (truncated)
    turn_count INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,  -- number of files touched
    first_message_at INTEGER,           -- timestamp of first message
    last_message_at INTEGER,            -- timestamp of last message (used for sorting + activeCount)
    file_path TEXT NOT NULL UNIQUE,     -- full path to .jsonl file
    file_hash TEXT,                     -- reserved for content-based change detection
    indexed_at INTEGER,                 -- when this session was last indexed

    -- Additional columns for current API contract
    project_path TEXT NOT NULL DEFAULT '',        -- resolved filesystem path
    project_display_name TEXT NOT NULL DEFAULT '', -- human-readable project name
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_message TEXT NOT NULL DEFAULT '',         -- last user message (truncated)
    files_touched TEXT NOT NULL DEFAULT '[]',      -- JSON array of filenames
    skills_used TEXT NOT NULL DEFAULT '[]',        -- JSON array of skill names
    tool_counts_edit INTEGER NOT NULL DEFAULT 0,
    tool_counts_read INTEGER NOT NULL DEFAULT 0,
    tool_counts_bash INTEGER NOT NULL DEFAULT 0,
    tool_counts_write INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);

-- Indexer state: tracks per-file indexing for incremental updates
-- v2 spec has key-value, but per-file tracking is more practical
CREATE TABLE IF NOT EXISTS indexer_state (
    file_path TEXT PRIMARY KEY,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL
);
```

**Schema notes:**
- `files_touched` and `skills_used` stored as JSON arrays in TEXT columns for MVP simplicity
- The v2 spec's `skills` + `session_skills` tables are deferred to Phase 3 (skill stats feature)
- The v2 spec's `tags` table is deferred to Phase 3 (tags feature)
- The analytics design's extra columns (`duration_seconds`, `health`, etc.) are deferred — this schema is forward-compatible with `ALTER TABLE ADD COLUMN`
- `indexer_state` uses per-file tracking instead of v2's key-value design, since we need mtime/size per file for incremental detection

## Terminal UX

Using `indicatif` crate for progress bars:

**First launch:**
```
🔍 vibe-recall v0.1.0

  Scanning projects...          found 10 projects, 780 sessions (828 MB)
  Indexing sessions ━━━━━━━━━━━━━━━━━━━━ 780/780 (12.4s)

  ✓ Ready in 12.8s
  → http://localhost:47892
```

**Subsequent launch (incremental):**
```
🔍 vibe-recall v0.1.0

  Checking for changes...       3 new, 2 modified sessions
  Indexing sessions ━━━━━━━━━━━━━━━━━━━━ 5/5 (0.8s)

  ✓ Ready in 1.1s
  → http://localhost:47892
```

**No changes:**
```
🔍 vibe-recall v0.1.0

  Checking for changes...       up to date (780 sessions)

  ✓ Ready in 0.3s
  → http://localhost:47892
```

## Implementation Tasks

### Task 1: `db` crate — SQLite setup and migrations

**Files:** `crates/db/src/lib.rs`, `crates/db/src/migrations.rs`

- Create/open SQLite database at `~/.cache/vibe-recall/vibe-recall.db`
- Run schema migrations (sessions + indexer_state tables)
- Provide `Database` struct with connection pool
- Test with in-memory SQLite (`sqlite::memory:`)

**Tests:**
- `test_create_database` — creates tables, no errors
- `test_migrations_idempotent` — running twice is safe

### Task 2: `db` crate — Session CRUD operations

**Files:** `crates/db/src/queries.rs`

- `insert_session(session: &SessionInfo, project_encoded: &str, project_display_name: &str)` — upsert session
- `list_projects() -> Vec<ProjectInfo>` — query sessions grouped by project
- `get_indexer_state(file_path: &str) -> Option<IndexerEntry>` — check if file needs re-indexing
- `update_indexer_state(file_path: &str, size: i64, mtime: i64)` — mark file as indexed
- `remove_stale_sessions(valid_paths: &[String])` — delete sessions for files that no longer exist
- Serialize `files_touched`/`skills_used` as JSON text, deserialize on read

**Tests:**
- `test_insert_and_list_projects` — insert 3 sessions across 2 projects, verify grouping
- `test_upsert_session` — inserting same ID twice updates, not duplicates
- `test_indexer_state_roundtrip` — set and get indexer state
- `test_remove_stale_sessions` — removes sessions for deleted files
- `test_active_count_calculation` — activeCount uses 5-minute window
- `test_list_projects_returns_camelcase_json` — verify serialization format matches API contract

### Task 3: `core` crate — Indexer with progress callbacks

**Files:** `crates/core/src/indexer.rs`

- `Indexer` struct that orchestrates: scan → diff → parse → store
- `scan_files()` — returns list of (path, mtime, size) for all .jsonl files
- `diff_against_db(files, db)` — returns (new, modified, unchanged, deleted) sets
- `index_files(files, db, on_progress)` — parse each file, store in DB, call progress callback
- Progress callback: `Fn(indexed: usize, total: usize)` — lets caller update UI
- Uses existing `extract_session_metadata` for parsing

**Tests:**
- `test_scan_files` — finds .jsonl files in temp dir
- `test_diff_new_files` — all files are "new" on first run
- `test_diff_unchanged` — no changes when mtime/size match
- `test_diff_modified` — detects changed mtime
- `test_diff_deleted` — detects removed files
- `test_index_calls_progress` — progress callback called with correct counts

### Task 4: `server` main.rs — Terminal startup UX

**Files:** `crates/server/src/main.rs`, `crates/server/Cargo.toml`

- Add `indicatif` dependency
- On startup: create DB, run indexer with indicatif progress bar
- Display: project count, session count, total size during scan
- Progress bar during indexing with count and elapsed time
- Print "Ready" message with URL after indexing
- Add `vibe-recall-db` dependency to server crate

**No unit tests** (integration/visual — verified manually).

### Task 5: `server` routes — `/api/projects` reads from DB

**Files:** `crates/server/src/routes/projects.rs`, `crates/server/src/state.rs`

- Add `Database` to `AppState`
- Change `list_projects()` handler to call `db.list_projects()` instead of `vibe_recall_core::get_projects()`
- Ensure JSON output format is identical (same camelCase, same ISO date format)
- Keep existing `get_projects()` in core for the indexer to use internally

**Tests:**
- `test_projects_endpoint_returns_from_db` — mock DB with known data, verify JSON shape
- `test_projects_empty_db` — returns `[]` when no sessions indexed

### Task 6: Add `indicatif` to workspace

**Files:** `Cargo.toml` (workspace root), `crates/server/Cargo.toml`

- Add `indicatif = "0.17"` to workspace dependencies
- Add to server crate dependencies

## Implementation Order

```
Task 6 (add indicatif dep)
  → Task 1 (db setup)
    → Task 2 (db queries)
      → Task 3 (indexer)
        → Task 4 (terminal UX)
        → Task 5 (route swap)
```

Tasks 4 and 5 can be done in parallel after Task 3.

## Success Criteria

- [ ] First `GET /api/projects` responds in < 50ms (after indexing)
- [ ] Subsequent server starts with no changes complete in < 1 second
- [ ] Terminal shows progress bar during first-time indexing
- [ ] JSON output format identical to current API (no frontend changes needed)
- [ ] DB is at `~/.cache/vibe-recall/vibe-recall.db`
- [ ] Deleting the DB and restarting triggers full re-index
- [ ] All existing tests still pass
- [ ] New tests: ≥ 12 tests across db + core crates
