---
status: done
date: 2026-02-03
---

# Deep Index Performance Optimizations

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce first-run deep indexing time from ~10.6s to ~4-5s for 658 sessions, and fix stale-data correctness bug for modified sessions.

**Architecture:** Three independent optimizations to the Pass 2 deep indexing pipeline: (1) wrap all per-session DB writes in a single outer transaction to eliminate ~2000 implicit fsyncs, (2) SIMD pre-filter `progress`/`queue-operation`/`file-history-snapshot`/`saved_hook_context` lines to skip full JSON parsing for ~65% of lines, (3) detect modified JSONL files via mtime+size to re-index sessions whose files changed since last deep index.

**Tech Stack:** Rust, sqlx (SQLite), memchr memmem, tokio

---

## Task 1: Transaction Batching for Pass 2 DB Writes

Currently each session's `update_session_deep_fields` is an implicit transaction (1 fsync), and each of `batch_insert_invocations`, `batch_upsert_models`, `batch_insert_turns` opens its own `BEGIN/COMMIT` (1 fsync each). With 658 sessions × ~3-4 transactions = ~2000+ fsyncs.

**Approach:** Collect parse results from spawned tasks, then write them all in a single transaction on the main task. This separates the parallel parse phase (CPU-bound) from the sequential write phase (I/O-bound).

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:799-1055` (pass_2_deep_index)
- Modify: `crates/db/src/queries.rs:485-615` (add transaction-accepting variants)
- Test: `cargo test -p claude-view-db -- indexer_parallel`

**Step 1: Write the failing test**

Add to `crates/db/src/indexer_parallel.rs` tests module:

```rust
#[tokio::test]
async fn test_pass_2_writes_in_single_transaction() {
    // Setup: create temp DB, insert 3 sessions via pass_1
    // Run pass_2_deep_index
    // Assert: all 3 sessions have deep_indexed_at set
    // Key: if transaction batching works, this is functionally identical
    // to the existing test — we're testing that the refactor doesn't break anything.
    // The perf improvement is structural (fewer fsyncs), not behavioral.
}
```

This test is identical to the existing `test_run_background_index_full_pipeline` — the refactor must preserve behavior. Use the existing test as the regression gate.

**Step 2: Run existing tests to establish green baseline**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All tests pass (current baseline).

**Step 3: Refactor pass_2_deep_index — collect-then-write pattern**

In `crates/db/src/indexer_parallel.rs`, restructure `pass_2_deep_index()`:

1. **Parse phase** (parallel, unchanged): Each spawned task does mmap + `parse_bytes()` + invocation classification. But instead of writing to DB inside the task, return the results through the join handle.

2. **Write phase** (sequential, new): After all tasks complete, open ONE transaction and write all results.

Change the spawned task return type from `Ok::<(), String>(())` to return the parse results:

```rust
// Return type for each spawned task
struct DeepIndexResult {
    session_id: String,
    file_path: String,
    parse_result: ParseResult,
    classified_invocations: Vec<(String, i64, String, String, String, i64)>,
}
```

Then after `for handle in handles`:

```rust
// Write phase: single transaction for all sessions
let mut tx = db.pool().begin().await.map_err(|e| format!("Begin tx: {}", e))?;

for result in completed_results {
    // update_session_deep_fields_tx(&mut tx, ...)
    // batch_insert_invocations_tx(&mut tx, ...)
    // batch_upsert_models_tx(&mut tx, ...)
    // batch_insert_turns_tx(&mut tx, ...)
}

tx.commit().await.map_err(|e| format!("Commit tx: {}", e))?;
```

This requires adding `_tx` variants of the write functions in `queries.rs` that accept `&mut SqliteConnection` instead of using `self.pool()`. The existing functions can delegate to the `_tx` variants for backward compatibility.

**Step 4: Add _tx variants to queries.rs**

Add these functions that accept a transaction reference instead of using the pool:

```rust
pub async fn update_session_deep_fields_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    // ... same params as update_session_deep_fields ...
) -> DbResult<()> {
    // Same SQL, but execute on &mut *tx instead of self.pool()
}
```

Do the same for `batch_insert_invocations_tx`, `batch_upsert_models_tx`, `batch_insert_turns_tx`. The existing batch functions already use internal transactions — the `_tx` variants should NOT open their own `BEGIN/COMMIT` since the caller provides the transaction.

**Step 5: Run tests to verify**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add crates/db/src/indexer_parallel.rs crates/db/src/queries.rs
git commit -m "perf(db): batch all Pass 2 writes in single transaction

Collect parse results from parallel tasks, then write in one BEGIN/COMMIT.
Eliminates ~2000 implicit fsyncs on first run (658 sessions × 3-4 txns each)."
```

---

## Task 2: SIMD Pre-Filter for Lightweight Line Types

`progress` lines are ~37% of all JSONL lines (78,691 / 210k). For these lines, we only need to increment a subtype counter (agent/bash/hook/mcp). Similarly, `queue-operation`, `file-history-snapshot`, and `saved_hook_context` only need simple field extraction or counting. Full `serde_json::from_slice::<Value>()` is overkill for all of these.

**Approach:** Before the full JSON parse, use `memmem::Finder` to detect line type from raw bytes. For lightweight types, extract the needed subtype field with a second SIMD scan and skip the JSON parse entirely.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:253-516` (parse_bytes)
- Test: `cargo test -p claude-view-db -- indexer_parallel`

**Step 1: Write the failing test**

Add a test that verifies SIMD-parsed progress lines produce identical counts to full-JSON-parsed lines:

```rust
#[test]
fn test_simd_prefilter_matches_full_parse() {
    // Craft a JSONL blob with known progress/queue/snapshot lines
    // plus user/assistant lines that MUST still get full parse
    let data = br#"{"type":"progress","uuid":"p1","data":{"type":"agent_progress"}}
{"type":"progress","uuid":"p2","data":{"type":"bash_progress"}}
{"type":"progress","uuid":"p3","data":{"type":"hook_progress"}}
{"type":"progress","uuid":"p4","data":{"type":"mcp_progress"}}
{"type":"progress","uuid":"p5","data":{"type":"waiting_for_task"}}
{"type":"queue-operation","uuid":"q1","operation":"enqueue"}
{"type":"queue-operation","uuid":"q2","operation":"dequeue"}
{"type":"file-history-snapshot","uuid":"f1","snapshot":{}}
{"type":"saved_hook_context","uuid":"s1","content":["ctx"]}
{"type":"user","uuid":"u1","message":{"role":"user","content":"hello"}}
{"type":"assistant","uuid":"a1","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"hi"}]}}
"#;

    let result = parse_bytes(data);
    let diag = &result.diagnostics;

    // Progress subtypes
    assert_eq!(result.deep.agent_spawn_count, 1);
    assert_eq!(result.deep.bash_progress_count, 1);
    assert_eq!(result.deep.hook_progress_count, 1);
    assert_eq!(result.deep.mcp_progress_count, 1);
    assert_eq!(diag.lines_progress, 5);

    // Queue
    assert_eq!(result.deep.queue_enqueue_count, 1);
    assert_eq!(result.deep.queue_dequeue_count, 1);

    // File snapshot
    assert_eq!(result.deep.file_snapshot_count, 1);

    // Hook context
    assert_eq!(diag.lines_hook_context, 1);

    // User + assistant still parsed correctly
    assert_eq!(diag.lines_user, 1);
    assert_eq!(diag.lines_assistant, 1);

    // JSON parse attempts should be ONLY for user + assistant (2), not all 11
    assert_eq!(diag.json_parse_attempts, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db -- test_simd_prefilter_matches_full_parse`
Expected: FAIL — `json_parse_attempts` will be 11, not 2 (currently parses every line).

**Step 3: Implement SIMD pre-filter in parse_bytes**

In `parse_bytes()`, add type-detection finders at the top (alongside existing finders):

```rust
// SIMD type detectors — check raw bytes before JSON parse
let type_progress = memmem::Finder::new(b"\"type\":\"progress\"");
let type_queue_op = memmem::Finder::new(b"\"type\":\"queue-operation\"");
let type_file_snap = memmem::Finder::new(b"\"type\":\"file-history-snapshot\"");
let type_hook_ctx = memmem::Finder::new(b"\"type\":\"saved_hook_context\"");

// Progress subtype detectors
let subtype_agent = memmem::Finder::new(b"\"type\":\"agent_progress\"");
let subtype_bash = memmem::Finder::new(b"\"type\":\"bash_progress\"");
let subtype_hook = memmem::Finder::new(b"\"type\":\"hook_progress\"");
let subtype_mcp = memmem::Finder::new(b"\"type\":\"mcp_progress\"");

// Queue operation detectors
let op_enqueue = memmem::Finder::new(b"\"operation\":\"enqueue\"");
let op_dequeue = memmem::Finder::new(b"\"operation\":\"dequeue\"");
```

Then in the line loop, BEFORE the `serde_json::from_slice` call:

```rust
// SIMD fast path: lightweight types that don't need full JSON parse
if type_progress.find(line).is_some() {
    diag.lines_progress += 1;
    // Extract timestamp if present (reuse existing SIMD timestamp helper)
    if let Some(ts) = extract_timestamp_from_bytes(line) {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() { first_timestamp = Some(ts); }
        last_timestamp = Some(ts);
    }
    if subtype_agent.find(line).is_some() {
        result.deep.agent_spawn_count += 1;
    } else if subtype_bash.find(line).is_some() {
        result.deep.bash_progress_count += 1;
    } else if subtype_hook.find(line).is_some() {
        result.deep.hook_progress_count += 1;
    } else if subtype_mcp.find(line).is_some() {
        result.deep.mcp_progress_count += 1;
    }
    continue;
}

if type_queue_op.find(line).is_some() {
    diag.lines_queue_op += 1;
    if op_enqueue.find(line).is_some() {
        result.deep.queue_enqueue_count += 1;
    } else if op_dequeue.find(line).is_some() {
        result.deep.queue_dequeue_count += 1;
    }
    continue;
}

if type_file_snap.find(line).is_some() {
    diag.lines_file_snapshot += 1;
    result.deep.file_snapshot_count += 1;
    continue;
}

if type_hook_ctx.find(line).is_some() {
    diag.lines_hook_context += 1;
    continue;
}

// Full JSON parse only for user, assistant, system, summary, unknown
diag.json_parse_attempts += 1;
let value: serde_json::Value = match serde_json::from_slice(line) { ... };
```

**Important:** The `system` type is NOT pre-filtered because it needs full JSON to extract `durationMs`, `retryAttempt`, and `preventedContinuation` fields — these are numeric/boolean values that SIMD string matching can't reliably extract. Similarly, `summary` needs full JSON for the summary text. `user` and `assistant` need full JSON for content extraction and token tracking.

**Note on timestamp extraction:** The SIMD fast path needs timestamps for `first_timestamp` / `last_timestamp`. Add a helper `extract_timestamp_from_bytes(line: &[u8]) -> Option<i64>` that uses `memmem::Finder` to find `"timestamp":"` and extract the ISO 8601 string, then parse it. If this is too complex, accept that SIMD-skipped lines won't contribute to timestamp tracking — `user` and `assistant` lines (which ARE still full-parsed) already provide first/last timestamps. Simpler option: skip timestamp extraction on fast-path lines. The first `user` and last `assistant` line will capture the session time bounds anyway.

**Step 4: Run tests to verify**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All tests pass, including the new `test_simd_prefilter_matches_full_parse`.

**Step 5: Run the existing golden fixture tests**

Run: `cargo test -p claude-view-db -- golden`
Expected: All pass. The golden fixtures should produce identical results whether lines are SIMD-skipped or full-parsed.

**Step 6: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "perf(parser): SIMD pre-filter progress/queue/snapshot lines

Skip full JSON parse for ~65% of JSONL lines (progress, queue-operation,
file-history-snapshot, saved_hook_context). Use memmem::Finder to detect
line type and extract subtypes from raw bytes. Only user, assistant,
system, and summary lines get full serde_json::from_slice."
```

---

## Task 3: Mtime Check for Modified Sessions (Correctness Fix)

Currently, once a session has `deep_indexed_at` set, it's never re-indexed even if the JSONL file grows (e.g., user continues a conversation). This means stale metrics for active sessions.

**Approach:** When querying sessions needing deep index, also check if the file's mtime or size has changed since `deep_indexed_at`. Store `file_size` and `file_mtime` in the sessions table at deep-index time, then compare on next startup.

**Files:**
- Modify: `crates/db/src/migrations.rs` (add migration for file_size_at_index, file_mtime_at_index columns)
- Modify: `crates/db/src/queries.rs:619-630` (update get_sessions_needing_deep_index)
- Modify: `crates/db/src/queries.rs:485-527` (update update_session_deep_fields to store file size/mtime)
- Modify: `crates/db/src/indexer_parallel.rs:838-878` (pass file metadata to write phase)
- Test: `cargo test -p claude-view-db -- indexer_parallel`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_modified_session_gets_reindexed() {
    // Setup: create temp DB + temp JSONL file with 2 user lines
    // Run pass_2_deep_index → session gets deep_indexed_at
    // Append a new line to the JSONL file (simulating continued conversation)
    // Run pass_2_deep_index again
    // Assert: session was re-indexed (turn_count increased)
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db -- test_modified_session_gets_reindexed`
Expected: FAIL — session is NOT re-indexed because `deep_indexed_at IS NOT NULL` and `parse_version` hasn't changed.

**Step 3: Add migration**

Add migration (next number in sequence) to add columns:

```sql
ALTER TABLE sessions ADD COLUMN file_size_at_index INTEGER;
ALTER TABLE sessions ADD COLUMN file_mtime_at_index INTEGER;
```

**Step 4: Update update_session_deep_fields to store file metadata**

Add `file_size: i64` and `file_mtime: i64` parameters to `update_session_deep_fields` (and the `_tx` variant from Task 1). Store them in the new columns.

**Step 5: Update get_sessions_needing_deep_index**

Change the query to also return `file_size_at_index` and `file_mtime_at_index`:

```sql
SELECT id, file_path, file_size_at_index, file_mtime_at_index
FROM sessions
WHERE deep_indexed_at IS NULL
   OR parse_version < ?1
```

Then in `pass_2_deep_index`, before parsing each session, `stat()` the file and compare size+mtime. If both match, skip the session. If either changed OR the columns are NULL, parse it.

This approach avoids changing the SQL query to do the comparison (which would require stat'ing all files upfront). Instead, the filtering happens in Rust after the query returns.

**Step 6: Run tests**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All pass, including the new test.

**Step 7: Bump CURRENT_PARSE_VERSION**

Bump `CURRENT_PARSE_VERSION` from 1 to 2. This forces a one-time re-index of all sessions so they get the new `file_size_at_index` and `file_mtime_at_index` columns populated. After that, only modified sessions will be re-indexed.

**Step 8: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/queries.rs crates/db/src/indexer_parallel.rs
git commit -m "fix(db): re-index sessions when JSONL file changes

Store file_size and file_mtime at deep-index time. On subsequent startups,
compare against current file metadata and re-parse if changed.

Fixes stale metrics for sessions where the user continued the conversation
after the last deep index."
```

---

## Future: sonic-rs JSON Parser (Not in Scope)

`sonic-rs` is a SIMD-accelerated JSON parser that's API-compatible with `serde_json::from_slice` and takes `&[u8]` (no mmap conflict). Benchmarks show 2-3x throughput improvement.

**Why deferred:** After Tasks 1-3, the SIMD pre-filter already skips ~65% of lines. The remaining ~35% (user/assistant/system/summary) still need full JSON parse, but the absolute time is reduced to ~2-3s. Swapping to sonic-rs would save another ~1-1.5s on first run — diminishing returns. Revisit if profiling shows JSON parse is still the dominant bottleneck after these optimizations.

**Risk:** sonic-rs is younger than serde_json, less ecosystem adoption. Would need testing across malformed JSONL edge cases.
