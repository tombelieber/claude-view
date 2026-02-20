---
status: pending
date: 2026-02-10
---

# Tech Debt Audit & Remediation Plan

## Executive Summary

A comprehensive audit of the Rust backend (4 agents: SQL, data pipeline, type safety, indexing pipeline) identified **22 distinct issues** spanning data correctness, structural fragility, and code quality. The most critical finding is the **3-way SQL UPDATE divergence**: three copies of the deep-index UPDATE statement have drifted to 43/47/50 parameters respectively, with different column sets:

| Path | Params | Missing columns vs complete `_tx` path |
|------|--------|----------------------------------------|
| Rusqlite (`UPDATE_SESSION_DEEP_SQL`) | 47 | `lines_added`, `lines_removed`, `loc_source` |
| Sqlx non-tx (`update_session_deep_fields`) | 43 | `lines_added`, `lines_removed`, `loc_source`, `ai_lines_added`, `ai_lines_removed`, `work_type`, `git_branch` |
| Sqlx tx (`update_session_deep_fields_tx`) | 50 | None (complete) |

The rusqlite path (production, file-based DBs) is missing 3 LOC columns. The 43-param sqlx non-tx path (used nowhere in production but called by some test code) is the worst offender, missing 7 columns. This means `lines_added`, `lines_removed`, and `loc_source` are **never populated in production**, silently returning zeros for LOC metrics.

The second critical class of bugs is **INSERT OR IGNORE on turns/invocations without DELETE on re-index**: when a session's JSONL file grows and triggers re-indexing, new turns are appended but old turns are never removed or updated. Combined with missing foreign keys on these tables (neither `turns.session_id` nor `invocations.session_id` has ANY FK to `sessions(id)`), stale session deletion leaves orphaned rows in `turns` and `invocations`.

Fixing these issues requires 4 focused phases. Phases 1 and 2 can start in parallel. Phase 1 is the largest (~400 LOC, core write path) and may require 1-2 sessions; the other phases are each completable in a single session.

---

## Prioritized Findings

### P0 -- Ship Blockers / Active Data Bugs

#### P0-1: 3-way SQL UPDATE divergence (production data silently wrong)

**Bug:** Three copies of the deep-index UPDATE statement have drifted apart:

| Path | Params | Missing columns |
|------|--------|-----------------|
| Rusqlite (`UPDATE_SESSION_DEEP_SQL`, `indexer_parallel.rs:33-82`) | 47 | `lines_added`, `lines_removed`, `loc_source` |
| Sqlx non-tx (`update_session_deep_fields`, `queries.rs:565-709`) | 43 | `lines_added`, `lines_removed`, `loc_source`, `ai_lines_added`, `ai_lines_removed`, `work_type`, `git_branch` |
| Sqlx tx (`update_session_deep_fields_tx`, `queries.rs:1918-2081`) | 50 | None (complete) |

Since production always uses the rusqlite path (file-based DB), `lines_added`, `lines_removed`, and `loc_source` are **never populated in production**. The rusqlite path DOES correctly write `work_type` (?44), `ai_lines_added` (?42), and `ai_lines_removed` (?43). The 43-param sqlx non-tx path is the worst offender, missing 7 columns including `work_type` and `git_branch`.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:33-82` -- `UPDATE_SESSION_DEEP_SQL` const: 47 params, sets `ai_lines_added` (?42), `ai_lines_removed` (?43), `work_type` (?44), but **does not set** `lines_added`, `lines_removed`, or `loc_source`
- `crates/db/src/queries.rs:1972-2025` -- `update_session_deep_fields_tx`: 50 params (complete), includes `lines_added` (?42), `lines_removed` (?43), `loc_source` (?44), `ai_lines_added` (?45), `ai_lines_removed` (?46), `work_type` (?47)
- `crates/db/src/queries.rs:614-659` -- `update_session_deep_fields`: 43 params, missing `lines_added`, `lines_removed`, `loc_source`, `ai_lines_added`, `ai_lines_removed`, `work_type`, `git_branch`

**Impact:** LOC metrics (`lines_added`, `lines_removed`, `loc_source`) show zeros for all users in production. The 43-param path also loses `work_type` and `git_branch`, though this path is currently only used in test code (in-memory DBs).

**Fix:** Unify all 3 SQL paths into a single source of truth (see Phase 1).

#### P0-2: Turns/invocations never deleted on re-index (stale data accumulates)

**Bug:** `INSERT_TURN_SQL` uses `INSERT OR IGNORE` (line 84-96). When a session is re-indexed (file grew), `parse_bytes` produces a fresh set of turns including old ones with the same UUIDs. `INSERT OR IGNORE` silently skips duplicates but **never updates** changed rows and **never deletes** turns that were removed by compaction. Over time, re-indexed sessions accumulate stale turn data.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:84-96` -- `INSERT_TURN_SQL` with `INSERT OR IGNORE`
- `crates/db/src/indexer_parallel.rs:98-102` -- `INSERT_INVOCATION_SQL` with `INSERT OR IGNORE`
- No `DELETE FROM turns WHERE session_id = ?` before re-inserting

**Impact:** Token usage stats (aggregated from `turns` table) over-count for compacted sessions. Invocation counts drift upward over time.

**Fix:** Add `DELETE FROM turns WHERE session_id = ?` and `DELETE FROM invocations WHERE session_id = ?` before inserting, scoped to each session being re-indexed (see Phase 2).

#### P0-3: Periodic loop passes `registry=None` to Pass 2 (invocations never classified)

**Bug:** The periodic sync loop in `main.rs:227` calls `pass_2_deep_index(&idx_db, None, ...)`. With `registry=None`, the classify branch at `indexer_parallel.rs:1745-1770` produces an empty `Vec`, so no invocations are inserted for sessions discovered or changed after initial startup.

**Evidence:**
- `crates/server/src/main.rs:227` -- `pass_2_deep_index(&idx_db, None, |_, _| {})`
- `crates/db/src/indexer_parallel.rs:1745-1770` -- classify block returns `Vec::new()` when registry is `None`

**Impact:** Tools/skills usage analytics are incomplete for any session indexed after the initial startup pass. The longer the server runs, the more invocations are missing.

**Fix:** Pass the registry (stored in `AppState`) to the periodic loop (see Phase 3).

### P1 -- Structural Risk (Will Cause Bugs on Next Change)

#### P1-1: 42+ positional args make wrong-order bugs undetectable

**Bug:** `update_session_deep_fields` takes 42 positional parameters of the same type (`i32`, `i64`, `Option<i64>`). Swapping two adjacent args (e.g., `api_error_count` and `api_retry_count`) compiles without error but silently writes data to the wrong column.

**Evidence:**
- `crates/db/src/queries.rs:565-611` -- 42 args, all numeric
- `crates/db/src/indexer_parallel.rs:1886-1934` -- 47 positional `rusqlite::params![]`
- `crates/db/src/indexer_parallel.rs:2079-2130` -- 50 positional `.bind()` calls

**Impact:** Any future column addition requires updating 3 SQL statements, 3 parameter lists, and 14+ call sites in tests. Risk of silent data corruption.

**Fix:** Replace positional args with a `DeepIndexFields` struct (see Phase 1).

#### P1-2: `turns` and `invocations` tables have NO foreign key to sessions

**Bug:** Neither `turns.session_id` nor `invocations.session_id` has **any foreign key reference** to `sessions(id)` -- not even a non-CASCADE one. They are bare `TEXT NOT NULL` columns with no referential integrity. When `remove_stale_sessions` deletes sessions, their turns and invocations become orphaned rows with no cleanup path.

**Evidence:**
- `crates/db/src/migrations.rs:90-95` -- `CREATE TABLE turns`: `session_id TEXT NOT NULL` (no FK at all, no REFERENCES)
- `crates/db/src/migrations.rs:66-71` -- `CREATE TABLE invocations`: `session_id TEXT NOT NULL` (no FK to sessions -- the only FK is `invocable_id REFERENCES invocables(id)`)
- `crates/db/src/queries.rs:1617-1658` -- `remove_stale_sessions` only deletes from `sessions` and `indexer_state`
- Compare: `session_commits` at line 138 **does** have `REFERENCES sessions(id) ON DELETE CASCADE`

**Impact:** Orphan rows inflate token stats, tool counts, and DB size. No cleanup path exists. Also no referential integrity -- a turn could reference a non-existent session.

**Fix:** Add CASCADE FKs via migration (see Phase 2).

#### P1-3: `summary` vs `summary_text` column confusion

**Bug:** The schema has two summary columns: `summary` (set by Pass 1 from `sessions-index.json`) and `summary_text` (set by Pass 2 from JSONL summary lines). Some query paths read `summary`, others read `summary_text`. The frontend `SessionInfo.summary` field maps to the Pass 1 `summary` column, while `SessionInfo.summary_text` maps to the Pass 2 column. They can contain different values for the same session.

**Evidence:**
- `crates/db/src/queries.rs:258` -- Pass 1 upsert writes `summary`
- `crates/db/src/queries.rs:653` / `indexer_parallel.rs:71` -- Pass 2 writes `summary_text`
- `crates/core/src/types.rs:201` -- `SessionInfo.summary: Option<String>`
- `crates/core/src/types.rs:268` -- `SessionInfo.summary_text: Option<String>`
- `crates/db/src/queries.rs:2351` -- `into_session_info` maps both independently

**Impact:** UI shows inconsistent summaries depending on which field is read. Two columns storing conceptually the same thing (session summary).

**Fix:** Consolidate in Phase 4 -- use `summary_text` (from actual JSONL parsing) as authoritative, fall back to `summary` (from index.json) when not deep-indexed.

#### P1-4: TOCTOU window on file reads during Claude Code writes (accepted risk)

**Observation:** The re-index check compares stored `file_size`/`file_mtime` against current `stat()` results (lines 1620-1638), then later mmaps and parses the file (lines 1691-1727). Between the filter-phase stat and the mmap, Claude Code may still be writing to the JSONL file.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:1620-1638` -- stat() for change detection (filter phase)
- `crates/db/src/indexer_parallel.rs:1691-1727` -- mmap in spawn_blocking, with its own stat at line 1696

**Current mitigations (already correct):**
- The **stored** `file_size`/`file_mtime` already comes from inside `spawn_blocking` (lines 1696-1706), NOT from the filter phase. This is correct — the stored metadata reflects the file state at actual parse time.
- The filter-phase stat is only used for the "should we re-index?" decision, not for stored metadata.
- If a partial read occurs (truncated last JSON line), `json_parse_failures` is logged, and the session is re-indexed on the next periodic cycle when the file has grown further.

**Impact:** Rare transient `json_parse_failures` warnings; session data is stale for one sync cycle. Not a data loss bug. Self-correcting on next periodic cycle.

**Fix:** None needed -- accepted risk. The polling-based change detection inherently has this window, and the current code handles it correctly. No code change required.

#### P1-5: `file_hash` column is dead

**Bug:** The `file_hash` column exists in the sessions table schema (migration line 18) but is never written to or read from by any code path.

**Evidence:**
- `crates/db/src/migrations.rs:18` -- `file_hash TEXT`
- No other file references `file_hash` (only 1 grep result, in migrations)

**Impact:** Wasted schema space. Potential confusion for future developers.

**Fix:** Drop column in Phase 4 migration.

### P2 -- Code Quality / Maintenance Burden

#### P2-1: `turn_durations_ms` parsed but never stored as array

**Bug:** `ExtendedMetadata.turn_durations_ms` (line 167) collects all turn durations during parsing, but only the aggregates (`turn_duration_avg_ms`, `turn_duration_max_ms`, `turn_duration_total_ms`) are stored in the database. The raw array is computed and then discarded.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:167` -- `pub turn_durations_ms: Vec<u64>`
- `crates/db/src/indexer_parallel.rs:1864-1871` -- Aggregates computed, raw vec dropped

**Impact:** Minor memory waste during parsing. If we later want per-turn duration histograms, we'd need to re-parse. Not a bug per se -- the aggregates are correct.

**Fix:** Low priority. Document the intentional aggregation. Consider storing as JSON column in a future feature pass if histograms are needed.

#### P2-2: 332 `.unwrap()`/`.expect()` in server crate (41 in indexer)

**Bug:** Large number of unwrap calls in non-test code. Most are on `serde_json::to_string` (infallible for simple types) or `.ok().flatten()` patterns that are safe. However, some are on `row.try_get()` results or iterator operations that could panic on malformed data.

**Evidence:**
- 332 occurrences across 20 files in `crates/server/src/`
- 41 occurrences in `crates/db/src/indexer_parallel.rs`
- Notable risky ones: `meta.turn_durations_ms.iter().max().unwrap()` at `indexer_parallel.rs:1868` (safe due to `.is_empty()` guard but brittle)

**Impact:** Potential panics on malformed input. Mostly safe in practice due to upstream guards.

**Fix:** Audit and replace the riskiest ones (iterator `.unwrap()` without adjacent guard, `row.try_get().unwrap()`). Full cleanup is P2 -- do incrementally.

#### P2-3: `ExtendedMetadata` struct exists but not used as function parameter

**Bug:** `ExtendedMetadata` (lines 128-194) is a well-structured type that contains all the fields passed as positional arguments to `update_session_deep_fields`. It is populated during parsing but then destructured into 40+ individual arguments for the SQL call.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:128-194` -- `ExtendedMetadata` struct definition
- `crates/db/src/indexer_parallel.rs:1886-1934` -- Fields destructured into positional params

**Impact:** Missed opportunity. The struct already exists and is the natural parameter type for the UPDATE function.

**Fix:** This is the core of Phase 1 -- use `DeepIndexFields` (a superset of `ExtendedMetadata` plus computed fields) as the parameter type.

#### P2-4: `last_message_at` not set in Pass 1 for some code paths

**Bug:** Pass 1 (`insert_session`) uses `session.modified_at` for `last_message_at` (line 277). This is the file mtime from the filesystem, which is a reasonable proxy. However, after Pass 2 deep-indexes the session, it may discover the actual last message timestamp from JSONL and write it via `COALESCE(?47, last_message_at)`. If the JSONL has no timestamp (empty session), `last_message_at` retains the file mtime value, which is fine.

**Evidence:**
- `crates/db/src/queries.rs:277` -- `.bind(session.modified_at)` for `last_message_at`
- `crates/db/src/indexer_parallel.rs:80` -- `last_message_at = COALESCE(?47, last_message_at)`

**Impact:** Minor. File mtime is a reasonable fallback. The COALESCE pattern correctly preserves it when Pass 2 finds no timestamp.

**Fix:** Low priority. Document the intentional fallback behavior.

#### P2-5: JSON array fields use `unwrap_or_default()` on deserialization

**Bug:** `serde_json::from_str(&self.files_touched).unwrap_or_default()` (line 2323) silently returns an empty vec if the stored JSON is malformed. This is defensive but masks data corruption.

**Evidence:**
- `crates/db/src/queries.rs:2323-2330` -- `unwrap_or_default()` on 4 JSON array fields
- `crates/db/src/indexer_parallel.rs:1849-1856` -- `unwrap_or_else(|_| "[]".to_string())` on serialization

**Impact:** No crash risk. But malformed data is silently swallowed. In practice, `serde_json::to_string` on `Vec<String>` never fails, so the serialization side is safe.

**Fix:** Low priority. Add a `tracing::warn!` on deserialization failure so corruption is logged, not silent.

#### P2-6: 12+ API fields fetched but never displayed in frontend

**Bug:** Several `SessionInfo` fields populated by the backend are not consumed by any frontend component: `queue_enqueue_count`, `queue_dequeue_count`, `file_snapshot_count`, `hook_blocked_count`, `api_retry_count`, `bash_progress_count`, `hook_progress_count`, `mcp_progress_count`, among others.

**Evidence:**
- `crates/db/src/indexer_parallel.rs:183-188` -- `queue_enqueue_count`, `queue_dequeue_count`, `file_snapshot_count` in `ExtendedMetadata` but not in `SessionInfo`
- `crates/core/src/types.rs:256-266` -- Fields exist on `SessionInfo` but no frontend component references them

**Impact:** Wasted bandwidth and DB storage. Not a correctness issue.

**Fix:** Low priority. Keep the fields -- they may be used in future analytics views. Do not remove without checking frontend plans.

---

## Implementation Phases

### Phase 1: Struct-Based Deep Index Fields

**Goal:** Eliminate the 3-way SQL divergence and 42+ positional arg fragility by introducing a `DeepIndexFields` struct as the single source of truth for deep-index UPDATE operations.

**Files to modify:**
| File | Change |
|------|--------|
| `crates/db/src/indexer_parallel.rs` | Define `DeepIndexFields` struct (extend `ExtendedMetadata` with computed fields). Replace `UPDATE_SESSION_DEEP_SQL` const with a method on `DeepIndexFields` that generates params. Update rusqlite write loop. |
| `crates/db/src/queries.rs` | Replace `update_session_deep_fields` (42 args) with `update_session_deep(&self, id: &str, fields: &DeepIndexFields)`. Replace `update_session_deep_fields_tx` (50 args) similarly. Single SQL string shared between both. |
| `crates/db/src/queries.rs` (tests) | Update 14 test call sites to construct `DeepIndexFields` instead of positional args. |

**Approach:**
1. Define `DeepIndexFields` in `indexer_parallel.rs` with all 50 fields. **Important:** The fields come from multiple source structs — `ExtendedMetadata` does NOT contain `lines_added`, `lines_removed`, or `git_branch`:
   - Most fields come from `ExtendedMetadata` (via `result.parse_result.deep`): `turn_count`, `tool_counts`, `ai_lines_added`, `ai_lines_removed`, `summary_text`, etc.
   - `lines_added`, `lines_removed` come from `ParseResult` (via `result.parse_result.lines_added`), NOT from `ExtendedMetadata` — these are computed at the `ParseResult` level from Edit/Write tool_use blocks
   - `loc_source` is a constant: `1` (tool-call estimate) -- hardcode in the constructor or accept as parameter
   - `git_branch` comes from `ParseResult` (via `result.parse_result.git_branch`)
   - `primary_model` is computed from `result.parse_result.turns` via `compute_primary_model()`
   - `file_size`, `file_mtime` come from the `DeepIndexResult` wrapper
   - `commit_count` is computed from `extract_commit_skill_invocations()`
   - `work_type` is computed from `classify_work_type()`
   - Turn duration aggregates (`dur_avg`, `dur_max`, `dur_total`) are computed from `turn_durations_ms`
2. Add a `to_rusqlite_params(&self) -> Vec<rusqlite::types::Value>` method to generate the parameter array.
3. Write a single `const DEEP_UPDATE_SQL: &str` that includes ALL 50 columns. Both rusqlite and sqlx paths must use this same const.
4. Replace `update_session_deep_fields` and `update_session_deep_fields_tx` with functions that accept `&DeepIndexFields`.
5. Update the rusqlite write loop in `pass_2_deep_index` to construct `DeepIndexFields` from `ParseResult` + `ExtendedMetadata` + computed values, then call the single method.
6. Update `write_results_sqlx` similarly.

**Estimated scope:** 3 files, ~400 LOC changed (mostly deletions of duplicated SQL and parameter lists). This is the largest phase and touches the core write path -- allow 1-2 sessions for implementation + testing. Also commit the Prevention Rules to `CLAUDE.md` as part of this phase's PR.

**Test strategy:**
- Run `cargo test -p claude-view-db -- update_session_deep` to verify all existing tests pass with the new struct-based API.
- Add a new test that verifies `lines_added`, `lines_removed`, `loc_source`, and `work_type` are round-tripped through the rusqlite path.
- Verify column count matches between SQL string and struct field count at compile time (const assert or test).
- **Critical:** Existing tests use in-memory SQLite databases, which take the `write_results_sqlx` fallback path (not the rusqlite production path). You MUST add at least one integration test that uses a **file-based** temporary database to exercise the rusqlite write path (`pass_2_deep_index` with `db.is_file_based() == true`). Without this, the production code path remains untested.

**Risk:** Medium-high. Touches the core write path. The 3 previously-missing LOC columns in the rusqlite path must be verified via a file-based DB test, not just the sqlx path that existing tests cover. Regression risk for the sqlx path is mitigated by existing test coverage (14 test call sites).

---

### Phase 2: Re-Index Safety (DELETE + CASCADE)

**Goal:** Ensure re-indexing a session produces correct data by deleting stale turns/invocations before re-inserting, and adding CASCADE FKs so session deletion auto-cleans child tables.

**Files to modify:**
| File | Change |
|------|--------|
| `crates/db/src/migrations.rs` | New migration: recreate `turns` and `invocations` tables with `ON DELETE CASCADE` FK. Drop and recreate (SQLite doesn't support `ALTER TABLE ... ADD CONSTRAINT`). |
| `crates/db/src/indexer_parallel.rs` | Add `DELETE FROM turns WHERE session_id = ?` and `DELETE FROM invocations WHERE session_id = ?` before INSERT loops in both rusqlite and sqlx write paths. |
| `crates/db/src/queries.rs` | Add `DELETE FROM turns/invocations WHERE session_id = ?` in `write_results_sqlx` path. Remove `remove_stale_sessions`'s manual cleanup comment (CASCADE handles it now). |

**Approach:**
1. Write a new migration (next version number) that:
   - Creates `turns_new` with full schema including `session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE`
   - Copies data: `INSERT INTO turns_new SELECT * FROM turns`
   - Drops `turns`, renames `turns_new` to `turns`
   - Recreates indexes: `idx_turns_session`, `idx_turns_model`
   - Same for `invocations`: add `session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE`. Keep the existing `invocable_id TEXT NOT NULL REFERENCES invocables(id)` **without CASCADE** -- invocables are never deleted (they are only disabled via the `status` column), so CASCADE is unnecessary and could mask bugs if invocables were accidentally deleted
   - Recreates indexes: `idx_invocations_invocable`, `idx_invocations_session`, `idx_invocations_timestamp`
   - **WARNING:** Use explicit `CREATE TABLE` DDL with full constraints, NOT `CREATE TABLE ... AS SELECT`
2. In the **rusqlite write loop** (`indexer_parallel.rs`, around line 1936-1941), before the turn/invocation INSERT loop for each session, add prepared statements:
   ```sql
   DELETE FROM turns WHERE session_id = ?1
   DELETE FROM invocations WHERE session_id = ?1
   ```
3. In the **sqlx fallback write path** (`write_results_sqlx` in `indexer_parallel.rs`, around line 2139-2151), add the same DELETE statements before calling `batch_insert_invocations_tx` and before the turns insert loop.
4. **Keep `INSERT OR IGNORE`** -- do NOT convert to plain `INSERT`. `INSERT OR IGNORE` serves as a safety net in case the DELETE is accidentally skipped or fails. The cost of the duplicate check is negligible compared to the risk of a hard failure on duplicate PKs.

**Estimated scope:** 3 files, ~100 LOC added (migration with explicit DDL + 2 delete statements per write path).

**Test strategy:**
- Add test: insert a session with turns, re-index with different turns, verify old turns are gone and new turns are present.
- Add test: delete a session via `remove_stale_sessions`, verify turns and invocations are also deleted (CASCADE).
- Run `cargo test -p claude-view-db`.

**Risk:** Low-medium. The migration recreates tables, which requires copying data. For large databases, this is a one-time cost at startup. The DELETE-before-INSERT pattern is standard and well-understood.

---

### Phase 3: Periodic Loop Registry Fix

**Goal:** Fix the periodic sync loop to pass the registry so invocations are classified for sessions indexed after initial startup.

**Files to modify:**
| File | Change |
|------|--------|
| `crates/server/src/main.rs` | Pass `registry` (from `registry_holder`) to `pass_2_deep_index` in the periodic loop. |

**Note:** P1-4 (TOCTOU on file reads) was analyzed and found to be already handled correctly -- the stored `file_size`/`file_mtime` metadata already comes from inside `spawn_blocking`, not from the filter phase. No code change needed for TOCTOU. See P1-4 finding for details.

**Approach:**
1. **Ownership fix:** `idx_registry` is consumed by `Some(idx_registry)` at `main.rs:174` (moved into `run_background_index`). After `run_background_index` returns, `idx_registry` no longer exists in the closure scope. The periodic loop at line 214+ cannot access it. Fix: clone `registry_holder` a second time before the spawn:
   ```rust
   let idx_registry = registry_holder.clone();       // for run_background_index
   let periodic_registry = registry_holder.clone();   // NEW: for periodic loop
   tokio::spawn(async move {
       // ... run_background_index(... Some(idx_registry) ...) consumes idx_registry
       // ... after run_background_index returns, the registry is stored inside
       //     the shared Arc<RwLock<Option<Registry>>> (line 2252-2256)
       // ... periodic loop uses periodic_registry (same underlying Arc):
   ```
2. In the periodic loop (around `main.rs:227`), read the registry from the holder. **Important:** `RwLockReadGuard` is not `Send`, so it cannot be held across an `.await` point. Clone the `Option<Registry>` out and drop the guard immediately:
   ```rust
   // Clone registry out of the lock (HashMap clone, ~100 entries, cheap)
   let registry_clone = periodic_registry.read().unwrap().clone();
   match pass_2_deep_index(&idx_db, registry_clone.as_ref(), |_, _| {}).await {
   ```
   This pattern: (a) acquires read lock, (b) clones `Option<Registry>`, (c) drops the guard (temporary is dropped at end of statement), (d) passes `Option<&Registry>` to `pass_2_deep_index` which is exactly what it accepts.

**Estimated scope:** 1 file, ~5 LOC changed.

**Test strategy:**
- Manual: start server, create a new Claude session, wait for periodic sync, verify invocations appear in the database.
- Verify by querying: `SELECT COUNT(*) FROM invocations WHERE session_id = '<new-session>'` should be > 0.
- Existing tests are unaffected (they call `pass_2_deep_index` directly with an explicit registry).

**Risk:** Low. The registry clone is cheap (it's a HashMap of ~100 entries). The `RwLock` is only read, never written after startup.

---

### Phase 4: Schema Cleanup

**Goal:** Remove dead columns, consolidate summary fields, clean up confusion.

**Files to modify:**
| File | Change |
|------|--------|
| `crates/db/src/migrations.rs` | New migration: drop `file_hash` column, add comment clarifying `summary` vs `summary_text` semantics. |
| `crates/db/src/queries.rs` | Add `COALESCE(s.summary_text, s.summary) AS summary` in read queries so the frontend gets the best available summary. |
| `crates/core/src/types.rs` | Remove `summary_text` from `SessionInfo` (merge into `summary`). |

**Approach:**
1. **`file_hash` removal:** New migration drops the column. SQLite requires table recreation with explicit DDL (NOT `CREATE TABLE ... AS SELECT`, which drops all constraints, defaults, and indexes):
   ```sql
   -- Step 1: Create new table with FULL schema (all constraints, defaults, checks)
   CREATE TABLE sessions_new (
       id TEXT PRIMARY KEY,
       project_id TEXT NOT NULL,
       -- ... all columns EXCEPT file_hash, with all NOT NULL / DEFAULT / CHECK constraints ...
   );
   -- Step 2: Copy data (explicit column list, omitting file_hash)
   INSERT INTO sessions_new (id, project_id, ...) SELECT id, project_id, ... FROM sessions;
   -- Step 3: Swap
   DROP TABLE sessions;
   ALTER TABLE sessions_new RENAME TO sessions;
   -- Step 4: Recreate ALL indexes (they are dropped with the old table)
   CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
   CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);
   CREATE INDEX IF NOT EXISTS idx_sessions_project_branch ON sessions(project_id, git_branch);
   CREATE INDEX IF NOT EXISTS idx_sessions_sidechain ON sessions(is_sidechain);
   ```
   **WARNING:** Never use `CREATE TABLE ... AS SELECT` in SQLite migrations -- it silently drops all constraints, defaults, indexes, and foreign keys from the new table.
2. **Summary consolidation:** In all SELECT queries that read `summary`, change to `COALESCE(s.summary_text, s.summary) AS summary`. This gives the JSONL-parsed summary (more accurate) when available, falling back to the index.json summary. Remove `summary_text` from `SessionInfo` and `SessionRow`.
3. **Frontend:** Audit complete -- minimal scope:
   - `src/types/generated/SessionInfo.ts` and `src/types/generated/SessionDetail.ts` both have `summaryText?: string | null` -- these are **auto-generated by ts-rs** and will be regenerated automatically when the Rust `SessionInfo` struct changes.
   - `src/components/CompactSessionTable.test.tsx:50` sets `summaryText: null` in a test fixture -- remove this line.
   - **0 actual components** read `.summaryText` (grep confirmed). No runtime code changes needed.

**Estimated scope:** 3-4 Rust files (~60 LOC changed) + 1 test fixture line removal + ts-rs regeneration.

**Test strategy:**
- Run full `cargo test` (cross-crate changes).
- Verify migration applies cleanly on a fresh DB and on an existing DB with data.
- Check frontend renders summaries correctly after the field merge.

**Risk:** Low. Purely additive migration + query changes. The COALESCE pattern is safe -- if both are NULL, result is NULL.

---

## Verification Checklist

After all phases are complete, run these checks:

### Automated
- [ ] `cargo test -p claude-view-db` -- all DB tests pass
- [ ] `cargo test -p claude-view-server` -- all server tests pass
- [ ] `cargo test -p claude-view-core` -- all core tests pass
- [ ] `cargo build --release` -- no warnings
- [ ] `bun run build` -- frontend builds without errors
- [ ] **Full pipeline integration test** (added in Phase 1): construct `DeepIndexFields`, write via rusqlite path to a temp file-based DB, read back via `into_session_info`, assert ALL 50 fields match expected values (including `lines_added`, `lines_removed`, `loc_source`, `work_type`, `git_branch`)

### Manual Data Integrity
- [ ] Start server, let it complete initial indexing
- [ ] Query: `SELECT COUNT(*) FROM sessions WHERE lines_added > 0` -- should be > 0 (P0-1 fix verified)
- [ ] Query: `SELECT COUNT(*) FROM sessions WHERE work_type IS NOT NULL` -- should be > 0 (P0-1 fix verified)
- [ ] Re-index a session (modify its JSONL): verify turn count matches JSONL content exactly (P0-2 fix verified)
- [ ] Delete a session file, re-run Pass 1: verify `SELECT COUNT(*) FROM turns WHERE session_id = '<deleted>'` returns 0 (P1-2 CASCADE verified)
- [ ] Wait for periodic sync cycle: verify new session's invocations are classified (P0-3 fix verified)
- [ ] Query: `SELECT id, summary FROM sessions WHERE summary IS NOT NULL LIMIT 5` -- summaries should be coherent (P1-3 fix verified)
- [ ] Query: `SELECT COUNT(*) FROM sessions WHERE file_hash IS NOT NULL` -- should error (column dropped) or return 0 (P1-5 fix verified)

### Regression
- [ ] Dashboard page loads and shows correct token stats
- [ ] Session detail page shows correct tool counts
- [ ] Settings > Data Status shows correct session/project counts
- [ ] "Rebuild Index" button works and re-populates all data correctly

---

## Prevention Rules

Add these rules to `CLAUDE.md` **as part of the Phase 1 PR** (not deferred). Prevention rules must land with the code they protect, so future implementers see them immediately:

### Single Source of Truth for SQL

Every SQL statement that appears in more than one code path must be defined as a single `const` or generated by a single function. Never duplicate SQL strings between rusqlite and sqlx paths.

```rust
// WRONG -- two copies of the same UPDATE, will drift
const RUSQLITE_UPDATE: &str = "UPDATE sessions SET a = ?1, b = ?2 ...";
// in queries.rs:
sqlx::query("UPDATE sessions SET a = ?1, b = ?2, c = ?3 ...") // drifted!

// RIGHT -- single SQL const, used by both paths
const DEEP_UPDATE_SQL: &str = "UPDATE sessions SET a = ?1, b = ?2, c = ?3 ...";
// rusqlite: conn.prepare(DEEP_UPDATE_SQL)
// sqlx: sqlx::query(DEEP_UPDATE_SQL)
```

### Struct Parameters Over Positional Args

Any function with more than 8 parameters must accept a struct. This prevents wrong-order bugs and makes call sites self-documenting.

```rust
// WRONG -- 42 positional args of the same type
fn update_session_deep_fields(&self, id: &str, count_a: i32, count_b: i32, count_c: i32, ...)

// RIGHT -- struct with named fields
fn update_session_deep(&self, id: &str, fields: &DeepIndexFields) -> DbResult<()>
```

### DELETE Before Re-INSERT for Child Tables

When re-indexing a session, always `DELETE FROM child_table WHERE session_id = ?` before inserting new rows. `INSERT OR IGNORE` is only safe for immutable data; JSONL sessions are mutable (they grow and compact).

### CASCADE Foreign Keys on Session Children

Every table with a `session_id` column must have `REFERENCES sessions(id) ON DELETE CASCADE`. This ensures session deletion automatically cleans up child rows. Verify in code review.

### Periodic Loops Must Have Feature Parity

Any background/periodic loop that calls an indexing function must pass the same parameters as the initial startup call. If the initial call gets `registry`, the periodic call must too. Grep for `None` being passed where the initial call passes `Some(...)`.

---

## Dependencies Between Phases

```
Phase 1 (struct fields) ─── no dependency ───> can start immediately
Phase 2 (re-index safety) ─ no dependency ───> can start immediately
Phase 3 (periodic loop) ─── depends on Phase 1 (uses DeepIndexFields struct)
Phase 4 (schema cleanup) ── depends on Phase 2 (migration ordering)
```

Phases 1 and 2 can be done in parallel. Phase 3 should follow Phase 1. Phase 4 should follow Phase 2 (to batch migrations).
