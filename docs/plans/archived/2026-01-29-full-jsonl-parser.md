---
status: done
date: 2026-01-29
---

# Full JSONL Parser Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the SIMD-selective parser (which extracts ~40% of data from only `user` and `assistant` lines) with a full JSON parser that extracts data from all 7 JSONL line types, adds `ParseDiagnostics` for observability, fixes the SIMD spacing bug, and enables automatic re-indexing via `parse_version`.

**Architecture:** Full `serde_json::from_slice` on every line. Profiling shows Rust can full-parse 10GB in ~0.6s — well within the 10s budget. The parser processes lines in a `match` on `type` field, dispatching to type-specific extractors. A `ParseDiagnostics` struct counts every classification and failure for observability. A `parse_version` column enables automatic re-indexing when the parser logic changes.

**Tech Stack:** Rust (serde_json, chrono, memmap2, memchr), SQLite (sqlx), TypeScript (ts-rs generated types)

**Reference spec:** `docs/plans/2026-01-29-jsonl-parser-spec.md` — sections referenced as §N throughout.

---

### Task 1: Schema Migration — New Columns + `parse_version`

Add Migration 9 with new aggregate columns for system/progress/summary data, `parse_version` for re-indexing, and detail tables for per-turn and per-event data.

**Files:**
- Modify: `crates/db/src/migrations.rs` (append to `MIGRATIONS` array)

**Step 1: Write the failing test**

Add a test to `crates/db/src/migrations.rs` in the `tests` module that asserts the new columns and tables exist.

```rust
#[tokio::test]
async fn test_migration9_full_parser_columns_exist() {
    let pool = setup_db().await;

    // Check new session columns
    let columns: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM pragma_table_info('sessions')"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(names.contains(&"parse_version"), "Missing parse_version");
    assert!(names.contains(&"turn_duration_avg_ms"), "Missing turn_duration_avg_ms");
    assert!(names.contains(&"turn_duration_max_ms"), "Missing turn_duration_max_ms");
    assert!(names.contains(&"turn_duration_total_ms"), "Missing turn_duration_total_ms");
    assert!(names.contains(&"api_error_count"), "Missing api_error_count");
    assert!(names.contains(&"api_retry_count"), "Missing api_retry_count");
    assert!(names.contains(&"compaction_count"), "Missing compaction_count");
    assert!(names.contains(&"hook_blocked_count"), "Missing hook_blocked_count");
    assert!(names.contains(&"agent_spawn_count"), "Missing agent_spawn_count");
    assert!(names.contains(&"bash_progress_count"), "Missing bash_progress_count");
    assert!(names.contains(&"hook_progress_count"), "Missing hook_progress_count");
    assert!(names.contains(&"mcp_progress_count"), "Missing mcp_progress_count");
    assert!(names.contains(&"summary_text"), "Missing summary_text");
    assert!(names.contains(&"total_input_tokens"), "Missing total_input_tokens");
    assert!(names.contains(&"total_output_tokens"), "Missing total_output_tokens");
    assert!(names.contains(&"cache_read_tokens"), "Missing cache_read_tokens");
    assert!(names.contains(&"cache_creation_tokens"), "Missing cache_creation_tokens");
    assert!(names.contains(&"thinking_block_count"), "Missing thinking_block_count");
}

#[tokio::test]
async fn test_migration9_detail_tables_exist() {
    let pool = setup_db().await;

    // Verify turn_metrics table
    let columns: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM pragma_table_info('turn_metrics')"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
    assert!(names.contains(&"session_id"));
    assert!(names.contains(&"turn_seq"));
    assert!(names.contains(&"duration_ms"));
    assert!(names.contains(&"input_tokens"));
    assert!(names.contains(&"model"));

    // Verify api_errors table
    let columns: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM pragma_table_info('api_errors')"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
    assert!(names.contains(&"session_id"));
    assert!(names.contains(&"timestamp_unix"));
    assert!(names.contains(&"retry_attempt"));
}

#[tokio::test]
async fn test_migration9_parse_version_default() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('pv-test', 'proj', '/tmp/pv.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64,) = sqlx::query_as(
        "SELECT parse_version FROM sessions WHERE id = 'pv-test'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0, "parse_version default should be 0");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p db -- migrations::tests::test_migration9 -v`
Expected: FAIL — columns and tables don't exist yet.

**Step 3: Write the migration SQL**

Append these to the `MIGRATIONS` array in `crates/db/src/migrations.rs`:

```rust
// Migration 9: Full JSONL parser schema (Phase 3.5)
// 9a: Token aggregates on sessions
r#"ALTER TABLE sessions ADD COLUMN total_input_tokens INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN total_output_tokens INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN cache_creation_tokens INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN thinking_block_count INTEGER NOT NULL DEFAULT 0;"#,
// 9b: System line aggregates on sessions
r#"ALTER TABLE sessions ADD COLUMN turn_duration_avg_ms INTEGER;"#,
r#"ALTER TABLE sessions ADD COLUMN turn_duration_max_ms INTEGER;"#,
r#"ALTER TABLE sessions ADD COLUMN turn_duration_total_ms INTEGER;"#,
r#"ALTER TABLE sessions ADD COLUMN api_error_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN api_retry_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN compaction_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN hook_blocked_count INTEGER NOT NULL DEFAULT 0;"#,
// 9c: Progress line aggregates on sessions
r#"ALTER TABLE sessions ADD COLUMN agent_spawn_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN bash_progress_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN hook_progress_count INTEGER NOT NULL DEFAULT 0;"#,
r#"ALTER TABLE sessions ADD COLUMN mcp_progress_count INTEGER NOT NULL DEFAULT 0;"#,
// 9d: Summary + parse_version
r#"ALTER TABLE sessions ADD COLUMN summary_text TEXT;"#,
r#"ALTER TABLE sessions ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;"#,
// 9e: Detail tables
r#"
CREATE TABLE IF NOT EXISTS turn_metrics (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_seq INTEGER NOT NULL,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    model TEXT,
    PRIMARY KEY (session_id, turn_seq)
);
"#,
r#"
CREATE TABLE IF NOT EXISTS api_errors (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,
    retry_attempt INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    retry_in_ms REAL NOT NULL DEFAULT 0.0
);
"#,
r#"CREATE INDEX IF NOT EXISTS idx_api_errors_session_id ON api_errors(session_id);"#,
// 9f: Partial index for re-indexing
r#"CREATE INDEX IF NOT EXISTS idx_sessions_needs_reindex ON sessions(id, file_path) WHERE parse_version < 1;"#,
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p db -- migrations::tests::test_migration9 -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs
git commit -m "feat(db): add migration 9 — full parser schema with parse_version and detail tables"
```

---

### Task 2: Add `ParseDiagnostics` Struct

Zero-cost observability counters that track every classification and failure in the parser.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` (add struct after `ParseResult`)

**Step 1: Write the failing test**

```rust
#[test]
fn test_parse_diagnostics_default_zeroes() {
    let diag = ParseDiagnostics::default();
    assert_eq!(diag.lines_total, 0);
    assert_eq!(diag.lines_user, 0);
    assert_eq!(diag.lines_assistant, 0);
    assert_eq!(diag.lines_system, 0);
    assert_eq!(diag.lines_progress, 0);
    assert_eq!(diag.lines_queue_op, 0);
    assert_eq!(diag.lines_summary, 0);
    assert_eq!(diag.lines_file_snapshot, 0);
    assert_eq!(diag.lines_unknown_type, 0);
    assert_eq!(diag.json_parse_failures, 0);
    assert_eq!(diag.content_not_array, 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p db -- tests::test_parse_diagnostics_default -v`
Expected: FAIL — `ParseDiagnostics` not defined.

**Step 3: Add the struct**

Add to `crates/db/src/indexer_parallel.rs` after the `ParseResult` struct definition (line ~125):

```rust
/// Parse diagnostic counters — per session, not per line.
/// Zero per-line overhead: stack-allocated integers incremented in existing branches.
/// See spec §9.
#[derive(Debug, Clone, Default)]
pub struct ParseDiagnostics {
    // Line classification
    pub lines_total: u32,
    pub lines_empty: u32,
    pub lines_user: u32,
    pub lines_assistant: u32,
    pub lines_system: u32,
    pub lines_progress: u32,
    pub lines_queue_op: u32,
    pub lines_summary: u32,
    pub lines_file_snapshot: u32,
    pub lines_unknown_type: u32,

    // Parse outcomes
    pub json_parse_attempts: u32,
    pub json_parse_failures: u32,
    pub content_not_array: u32,
    pub tool_use_missing_name: u32,

    // Extraction outcomes
    pub tool_use_blocks_found: u32,
    pub file_paths_extracted: u32,
    pub timestamps_extracted: u32,
    pub timestamps_unparseable: u32,
    pub turns_extracted: u32,

    // Bytes
    pub bytes_total: u64,
}
```

Also add `diagnostics: ParseDiagnostics` to `ParseResult`:

```rust
pub struct ParseResult {
    pub deep: ExtendedMetadata,
    pub raw_invocations: Vec<RawInvocation>,
    pub turns: Vec<claude_view_core::RawTurn>,
    pub models_seen: Vec<String>,
    pub diagnostics: ParseDiagnostics,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p db -- tests::test_parse_diagnostics_default -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat(parser): add ParseDiagnostics struct for observability counters"
```

---

### Task 3: Add New Metric Fields to `ExtendedMetadata`

Extend `ExtendedMetadata` with token aggregates, system metrics, progress metrics, and summary text.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — `ExtendedMetadata` struct

**Step 1: Write the failing test**

```rust
#[test]
fn test_extended_metadata_new_fields_default() {
    let meta = ExtendedMetadata::default();
    assert_eq!(meta.total_input_tokens, 0);
    assert_eq!(meta.total_output_tokens, 0);
    assert_eq!(meta.cache_read_tokens, 0);
    assert_eq!(meta.cache_creation_tokens, 0);
    assert_eq!(meta.thinking_block_count, 0);
    assert_eq!(meta.api_error_count, 0);
    assert_eq!(meta.compaction_count, 0);
    assert_eq!(meta.agent_spawn_count, 0);
    assert!(meta.summary_text.is_none());
    assert!(meta.turn_durations_ms.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p db -- tests::test_extended_metadata_new_fields -v`
Expected: FAIL

**Step 3: Add fields to `ExtendedMetadata`**

Add these fields to the `ExtendedMetadata` struct in `crates/db/src/indexer_parallel.rs`:

```rust
    // Token aggregates (from assistant lines)
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_block_count: u32,

    // System line metrics
    pub turn_durations_ms: Vec<u64>,
    pub api_error_count: u32,
    pub api_retry_count: u32,
    pub compaction_count: u32,
    pub hook_blocked_count: u32,

    // Progress line metrics
    pub agent_spawn_count: u32,
    pub bash_progress_count: u32,
    pub hook_progress_count: u32,
    pub mcp_progress_count: u32,

    // Summary text (from summary lines)
    pub summary_text: Option<String>,

    // Queue operations
    pub queue_enqueue_count: u32,
    pub queue_dequeue_count: u32,

    // File history snapshots
    pub file_snapshot_count: u32,
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p db -- tests::test_extended_metadata_new_fields -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat(parser): add token, system, progress, and summary fields to ExtendedMetadata"
```

---

### Task 4: Create Golden Test Fixtures

Create JSONL fixture files containing all 7 line types, edge cases, and spacing variants. These are the ground truth for parser correctness.

**Files:**
- Create: `crates/db/tests/golden_fixtures/complete_session.jsonl`
- Create: `crates/db/tests/golden_fixtures/edge_cases.jsonl`
- Create: `crates/db/tests/golden_fixtures/spacing_variants.jsonl`
- Create: `crates/db/tests/golden_fixtures/empty_session.jsonl`
- Create: `crates/db/tests/golden_fixtures/text_only_session.jsonl`

**Step 1: Create fixture directory**

Run: `mkdir -p crates/db/tests/golden_fixtures`

**Step 2: Create `complete_session.jsonl`**

One example of every line type with realistic field values. This is the primary correctness fixture.

```jsonl
{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","sessionId":"sess-golden","message":{"role":"user","content":"Read and fix auth.rs"},"version":"1.0.0","cwd":"/project"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"2026-01-28T10:01:00Z","sessionId":"sess-golden","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"thinking","thinking":"Let me read the file first"},{"type":"tool_use","id":"tu1","name":"Read","input":{"file_path":"/project/src/auth.rs"}}],"usage":{"input_tokens":1500,"output_tokens":150,"cache_read_input_tokens":500,"cache_creation_input_tokens":100}}}
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":"2026-01-28T10:02:00Z","sessionId":"sess-golden","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn authenticate() { todo!() }"}]}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":"2026-01-28T10:03:00Z","sessionId":"sess-golden","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"I'll fix the authentication function."},{"type":"tool_use","id":"tu2","name":"Edit","input":{"file_path":"/project/src/auth.rs","old_string":"todo!()","new_string":"validate_token(token)"}},{"type":"tool_use","id":"tu3","name":"Edit","input":{"file_path":"/project/src/auth.rs","old_string":"fn authenticate()","new_string":"fn authenticate(token: &str)"}}],"usage":{"input_tokens":2000,"output_tokens":200,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
{"type":"system","uuid":"s1","timestamp":"2026-01-28T10:03:05Z","sessionId":"sess-golden","subtype":"turn_duration","durationMs":5000,"isMeta":true,"isSidechain":false}
{"type":"progress","uuid":"p1","timestamp":"2026-01-28T10:03:10Z","sessionId":"sess-golden","toolUseID":"tu-hook","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"lint-check","command":"eslint --fix"}}
{"type":"queue-operation","uuid":"q1","timestamp":"2026-01-28T10:03:15Z","sessionId":"sess-golden","operation":"enqueue","content":"next task"}
{"type":"queue-operation","uuid":"q2","timestamp":"2026-01-28T10:03:20Z","sessionId":"sess-golden","operation":"dequeue"}
{"type":"summary","uuid":"sum1","timestamp":"2026-01-28T10:04:00Z","sessionId":"sess-golden","summary":"Fixed authentication bug in auth.rs","leafUuid":"a2"}
{"type":"file-history-snapshot","uuid":"fhs1","timestamp":"2026-01-28T10:04:05Z","sessionId":"sess-golden","messageId":"a2","snapshot":{"trackedFileBackups":{"/project/src/auth.rs":"backup-hash"},"timestamp":"2026-01-28T10:04:05Z"},"isSnapshotUpdate":false}
```

**Step 3: Create `edge_cases.jsonl`**

Covers: string content (not array), tool_use missing name, api_error, compact_boundary, agent_progress, unknown line type, invalid JSON.

```jsonl
{"type":"assistant","uuid":"e1","timestamp":"2026-01-28T10:00:00Z","sessionId":"sess-edge","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":"Just a string, not an array","usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"assistant","uuid":"e2","timestamp":"2026-01-28T10:01:00Z","sessionId":"sess-edge","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"tool_use","input":{"file_path":"/foo"}},{"type":"tool_use","name":"Read","input":{"file_path":"/bar"}}],"usage":{"input_tokens":200,"output_tokens":100}}}
{"type":"system","uuid":"e3","timestamp":"2026-01-28T10:02:00Z","sessionId":"sess-edge","subtype":"api_error","error":{"message":"overloaded"},"retryAttempt":1,"maxRetries":3,"retryInMs":1500.0,"isMeta":true,"isSidechain":false}
{"type":"system","uuid":"e4","timestamp":"2026-01-28T10:03:00Z","sessionId":"sess-edge","subtype":"compact_boundary","content":"Conversation compacted","compactMetadata":{"trigger":"auto","preTokens":150000},"isMeta":true,"isSidechain":false}
{"type":"progress","uuid":"e5","timestamp":"2026-01-28T10:04:00Z","sessionId":"sess-edge","toolUseID":"tu-agent","data":{"type":"agent_progress","agentId":"agent-1","prompt":"Explore the codebase","message":{"message":{"model":"claude-haiku-4-20250514","usage":{"input_tokens":500,"output_tokens":50}}}}}
{"type":"totally_new_type","uuid":"e6","timestamp":"2026-01-28T10:05:00Z","sessionId":"sess-edge"}
this is not valid JSON at all
```

**Step 4: Create `spacing_variants.jsonl`**

Three user lines with different JSON spacing styles. Tests that the parser handles both compact and spaced formats.

```jsonl
{"type":"user","timestamp":"2026-01-28T10:00:00Z","message":{"content":"compact format"}}
{"type": "user","timestamp":"2026-01-28T10:01:00Z","message":{"content":"spaced type"}}
{ "type" : "user" , "timestamp" : "2026-01-28T10:02:00Z" , "message" : { "content" : "fully spaced" } }
```

**Step 5: Create `empty_session.jsonl`**

Empty file (0 bytes).

**Step 6: Create `text_only_session.jsonl`**

Multiple user/assistant exchanges, zero tool_use blocks.

```jsonl
{"type":"user","timestamp":"2026-01-28T10:00:00Z","message":{"content":"What is Rust?"}}
{"type":"assistant","timestamp":"2026-01-28T10:01:00Z","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"Rust is a systems programming language."}],"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"user","timestamp":"2026-01-28T10:02:00Z","message":{"content":"How does ownership work?"}}
{"type":"assistant","timestamp":"2026-01-28T10:03:00Z","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"Every value has a single owner."}],"usage":{"input_tokens":200,"output_tokens":100}}}
```

**Step 7: Commit**

```bash
git add crates/db/tests/golden_fixtures/
git commit -m "test(parser): add golden test fixtures for all 7 JSONL line types"
```

---

### Task 5: Rewrite `parse_bytes` — Full JSON Parse Every Line

Replace the SIMD-selective parser with full JSON parse on every line. Dispatches to type-specific extractors via `match` on the `type` field. Populates `ParseDiagnostics`. Fixes spacing bug (§8 silent failure #13-14).

This is the core task. The new parser handles all 7 line types from spec §3-§7.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — rewrite `parse_bytes()` function

**Step 1: Write golden tests for the complete session fixture**

Add to `crates/db/src/indexer_parallel.rs` tests module:

```rust
#[test]
fn test_golden_complete_session() {
    let data = include_bytes!("../tests/golden_fixtures/complete_session.jsonl");
    let result = parse_bytes(data);
    let diag = &result.diagnostics;

    // Line counts (spec §10 fixture 1)
    assert_eq!(diag.lines_total, 10);
    assert_eq!(diag.lines_user, 2);
    assert_eq!(diag.lines_assistant, 2);
    assert_eq!(diag.lines_system, 1);
    assert_eq!(diag.lines_progress, 1);
    assert_eq!(diag.lines_queue_op, 2);
    assert_eq!(diag.lines_summary, 1);
    assert_eq!(diag.lines_file_snapshot, 1);
    assert_eq!(diag.lines_unknown_type, 0);
    assert_eq!(diag.json_parse_failures, 0);

    // Core metrics
    assert_eq!(result.deep.user_prompt_count, 2);
    assert_eq!(result.deep.api_call_count, 2);
    assert_eq!(result.deep.tool_call_count, 3); // 1 Read + 2 Edit
    assert_eq!(result.deep.files_read_count, 1);
    assert_eq!(result.deep.files_edited_count, 1); // unique
    assert_eq!(result.deep.reedited_files_count, 1); // auth.rs edited 2x

    // Tokens
    assert_eq!(result.deep.total_input_tokens, 3500);
    assert_eq!(result.deep.total_output_tokens, 350);
    assert_eq!(result.deep.cache_read_tokens, 500);
    assert_eq!(result.deep.cache_creation_tokens, 100);

    // Thinking
    assert_eq!(result.deep.thinking_block_count, 1);

    // System
    assert_eq!(result.deep.turn_durations_ms, vec![5000]);
    assert_eq!(result.deep.compaction_count, 0);
    assert_eq!(result.deep.api_error_count, 0);

    // Progress
    assert_eq!(result.deep.hook_progress_count, 1);
    assert_eq!(result.deep.agent_spawn_count, 0);

    // Summary
    assert_eq!(result.deep.summary_text.as_deref(), Some("Fixed authentication bug in auth.rs"));

    // Queue
    assert_eq!(result.deep.queue_enqueue_count, 1);
    assert_eq!(result.deep.queue_dequeue_count, 1);

    // File snapshot
    assert_eq!(result.deep.file_snapshot_count, 1);
}

#[test]
fn test_golden_edge_cases() {
    let data = include_bytes!("../tests/golden_fixtures/edge_cases.jsonl");
    let result = parse_bytes(data);
    let diag = &result.diagnostics;

    assert_eq!(diag.lines_unknown_type, 1);       // "totally_new_type"
    assert_eq!(diag.json_parse_failures, 1);       // invalid JSON line
    assert_eq!(diag.content_not_array, 1);         // string content on e1
    assert_eq!(diag.tool_use_missing_name, 1);     // nameless tool_use on e2
    assert_eq!(result.deep.api_error_count, 1);    // api_error system line
    assert_eq!(result.deep.compaction_count, 1);   // compact_boundary
    assert_eq!(result.deep.agent_spawn_count, 1);  // agent_progress
}

#[test]
fn test_golden_spacing_variants() {
    let data = include_bytes!("../tests/golden_fixtures/spacing_variants.jsonl");
    let result = parse_bytes(data);

    // ALL THREE must be counted — this is the SIMD spacing bug fix
    assert_eq!(result.diagnostics.lines_user, 3);
    assert_eq!(result.deep.user_prompt_count, 3);
}

#[test]
fn test_golden_empty_session() {
    let data = include_bytes!("../tests/golden_fixtures/empty_session.jsonl");
    let result = parse_bytes(data);

    assert_eq!(result.diagnostics.lines_total, 0);
    assert_eq!(result.deep.user_prompt_count, 0);
    assert_eq!(result.deep.api_call_count, 0);
    assert_eq!(result.deep.duration_seconds, 0);
}

#[test]
fn test_golden_text_only() {
    let data = include_bytes!("../tests/golden_fixtures/text_only_session.jsonl");
    let result = parse_bytes(data);

    assert_eq!(result.deep.user_prompt_count, 2);
    assert_eq!(result.deep.api_call_count, 2);
    assert_eq!(result.deep.tool_call_count, 0);
    assert_eq!(result.deep.files_read_count, 0);
    assert_eq!(result.deep.total_input_tokens, 300);
    assert_eq!(result.deep.total_output_tokens, 150);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p db -- tests::test_golden -v`
Expected: FAIL — parse_bytes doesn't populate diagnostics or new metrics.

**Step 3: Rewrite `parse_bytes`**

Replace the entire `parse_bytes` function body in `crates/db/src/indexer_parallel.rs`. The key architectural change: instead of SIMD pre-filtering by `"type":"user"` / `"type":"assistant"` patterns, we do `serde_json::from_slice::<serde_json::Value>(line)` on every non-empty line, then `match` on the `type` field.

The new function should:

1. Initialize `ParseDiagnostics` and all metric accumulators
2. For each non-empty line:
   a. Try `serde_json::from_slice::<serde_json::Value>(line)` — on failure, increment `diag.json_parse_failures`, continue
   b. Extract `type` field as string
   c. `match type`:
      - `"user"` → increment `diag.lines_user`, extract content for last_message, extract skills
      - `"assistant"` → increment `diag.lines_assistant`, extract tool_use blocks (count tools, file paths, raw invocations), extract token usage, extract turn data, count thinking blocks
      - `"system"` → increment `diag.lines_system`, dispatch on `subtype`: `turn_duration` (push to `turn_durations_ms`), `api_error` (increment counters, extract retry info), `compact_boundary` / `microcompact_boundary` (increment compaction), `stop_hook_summary` (check `preventedContinuation`)
      - `"progress"` → increment `diag.lines_progress`, dispatch on `data.type`: `agent_progress` (increment `agent_spawn_count`), `bash_progress`, `hook_progress`, `mcp_progress`, `waiting_for_task`
      - `"queue-operation"` → increment `diag.lines_queue_op`, check `operation` for enqueue/dequeue counts
      - `"summary"` → increment `diag.lines_summary`, extract `summary` text
      - `"file-history-snapshot"` → increment `diag.lines_file_snapshot`
      - unknown → increment `diag.lines_unknown_type`
   d. Extract timestamp (both ISO8601 and Unix integer — spec §12)
3. Finalize: compute aggregates (duration, dedup files, etc.)
4. Return `ParseResult` with `diagnostics` populated

**Important implementation details:**
- Content polymorphism (§13): when extracting tool_use from assistant, check if `.message.content` is `Value::Array` or `Value::String`. If string, increment `diag.content_not_array` and skip tool extraction (not an error).
- Tool names: if a `tool_use` block has no `name` field, increment `diag.tool_use_missing_name` and skip it.
- Token aggregation: sum `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` from each assistant message's `.message.usage`.
- Keep existing SIMD finders for `extract_first_text_content` and `extract_skills_from_line` (these still work on user lines for last_message and skills — they're called on lines already identified as `"user"` type).
- For the `extract_raw_invocations` call, pass the already-parsed `Value` instead of re-parsing from bytes. Refactor `extract_raw_invocations` to accept `&Value` instead of `&[u8]`.

**Step 4: Run golden tests to verify they pass**

Run: `cargo test -p db -- tests::test_golden -v`
Expected: PASS

**Step 5: Run ALL existing tests to verify no regressions**

Run: `cargo test -p db -v`
Expected: ALL PASS — the old test assertions still hold because the new parser extracts a superset of the old data.

**Step 6: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat(parser): rewrite parse_bytes for full 7-type JSONL extraction with diagnostics"
```

---

### Task 6: Update `update_session_deep_fields` for New Columns

Add the new metric fields to the DB update query so they get persisted after deep indexing.

**Files:**
- Modify: `crates/db/src/queries.rs` — `update_session_deep_fields()` function

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_update_session_deep_fields_new_metrics() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a session
    db.insert_session_from_index("s1", "proj", "Proj", "/path", "/tmp/s1.jsonl", "preview", None, 0, 0, None, false, 100).await.unwrap();

    // Update with new metrics
    db.update_session_deep_fields(
        "s1", "last msg", 5, 2, 3, 1, 1, "[]", "[]",
        10, 8, 15, "[]", "[]", 5, 3, 1, 900, 2,
        // New fields:
        5000, 500, 200, 50, 3,    // tokens + thinking
        100, 200, 300,            // turn durations
        1, 2, 1, 0,              // system metrics
        2, 5, 3, 1,              // progress metrics
        Some("Session summary"), // summary
        1,                       // parse_version
    ).await.unwrap();

    // Verify new columns
    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, Option<String>, i64) =
        sqlx::query_as(
            "SELECT total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens, thinking_block_count, api_error_count, compaction_count, agent_spawn_count, hook_progress_count, summary_text, parse_version FROM sessions WHERE id = 's1'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

    assert_eq!(row.0, 5000);
    assert_eq!(row.1, 500);
    assert_eq!(row.9.as_deref(), Some("Session summary"));
    assert_eq!(row.10, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p db -- tests::test_update_session_deep_fields_new_metrics -v`
Expected: FAIL — function signature doesn't accept new params.

**Step 3: Update the function signature and SQL**

Extend `update_session_deep_fields` in `crates/db/src/queries.rs` to accept and persist all new fields:
- `total_input_tokens: i64`, `total_output_tokens: i64`, `cache_read_tokens: i64`, `cache_creation_tokens: i64`, `thinking_block_count: i32`
- `turn_duration_avg_ms: Option<i64>`, `turn_duration_max_ms: Option<i64>`, `turn_duration_total_ms: Option<i64>`
- `api_error_count: i32`, `api_retry_count: i32`, `compaction_count: i32`, `hook_blocked_count: i32`
- `agent_spawn_count: i32`, `bash_progress_count: i32`, `hook_progress_count: i32`, `mcp_progress_count: i32`
- `summary_text: Option<&str>`
- `parse_version: i32`

Update the SQL `UPDATE` statement to `SET` all new columns.

**Step 4: Update the caller in `pass_2_deep_index`**

In the `pass_2_deep_index` function, compute the turn duration aggregates from `meta.turn_durations_ms` and pass all new fields to the updated `update_session_deep_fields`.

```rust
// Compute turn duration aggregates
let (dur_avg, dur_max, dur_total) = if meta.turn_durations_ms.is_empty() {
    (None, None, None)
} else {
    let total: u64 = meta.turn_durations_ms.iter().sum();
    let max = *meta.turn_durations_ms.iter().max().unwrap();
    let avg = total / meta.turn_durations_ms.len() as u64;
    (Some(avg as i64), Some(max as i64), Some(total as i64))
};
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p db -- tests::test_update_session_deep_fields_new_metrics -v`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/db/src/queries.rs crates/db/src/indexer_parallel.rs
git commit -m "feat(db): persist new parser metrics (tokens, system, progress, summary, parse_version)"
```

---

### Task 7: Update `get_sessions_needing_deep_index` for `parse_version`

Change the query to also re-index sessions with `parse_version < CURRENT_PARSE_VERSION`.

**Files:**
- Modify: `crates/db/src/queries.rs` — `get_sessions_needing_deep_index()`
- Modify: `crates/db/src/indexer_parallel.rs` — add `CURRENT_PARSE_VERSION` constant

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_reindex_on_parse_version_bump() {
    let (_tmp, claude_dir) = setup_test_claude_dir();
    let db = Database::new_in_memory().await.unwrap();

    // Run full pipeline
    pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    let first = pass_2_deep_index(&db, None, |_, _| {}).await.unwrap();
    assert_eq!(first, 1);

    // Simulate old parse_version (parser was upgraded)
    sqlx::query("UPDATE sessions SET parse_version = 0")
        .execute(db.pool())
        .await
        .unwrap();

    // Should re-index because parse_version < CURRENT_PARSE_VERSION
    let second = pass_2_deep_index(&db, None, |_, _| {}).await.unwrap();
    assert_eq!(second, 1, "Should re-index when parse_version is stale");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p db -- tests::test_reindex_on_parse_version -v`
Expected: FAIL — query only checks `deep_indexed_at IS NULL`.

**Step 3: Implement**

In `crates/db/src/indexer_parallel.rs`:
```rust
/// Current parse version. Bump when parser logic changes to trigger re-indexing.
pub const CURRENT_PARSE_VERSION: i32 = 1;
```

In `crates/db/src/queries.rs`, update the query:
```rust
pub async fn get_sessions_needing_deep_index(&self) -> DbResult<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, file_path FROM sessions WHERE deep_indexed_at IS NULL OR parse_version < ?1",
    )
    .bind(crate::indexer_parallel::CURRENT_PARSE_VERSION)
    .fetch_all(self.pool())
    .await?;
    Ok(rows)
}
```

Also update `update_session_deep_fields` to set `parse_version = CURRENT_PARSE_VERSION` in the SQL.

**Step 4: Run test to verify it passes**

Run: `cargo test -p db -- tests::test_reindex_on_parse_version -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries.rs crates/db/src/indexer_parallel.rs
git commit -m "feat(db): re-index sessions when parse_version is stale"
```

---

### Task 8: Update `SessionInfo` Type + Frontend Types

Add the new metric fields to `SessionInfo` in core types so the API exposes them. Regenerate TypeScript types.

**Files:**
- Modify: `crates/core/src/types.rs` — `SessionInfo` struct
- Modify: `crates/db/src/queries.rs` — `SessionRow` and `From<SessionRow> for SessionInfo`

**Step 1: Add fields to `SessionInfo`**

Add to `crates/core/src/types.rs` `SessionInfo` struct:

```rust
    // Full parser metrics (Phase 3.5)
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_block_count: u32,
    pub turn_duration_avg_ms: Option<u64>,
    pub turn_duration_max_ms: Option<u64>,
    pub api_error_count: u32,
    pub compaction_count: u32,
    pub agent_spawn_count: u32,
    pub summary_text: Option<String>,
    pub parse_version: u32,
```

**Step 2: Update `SessionRow` in queries.rs**

Add matching columns to the `SessionRow` struct and the SELECT query in `list_projects()`.

**Step 3: Update `From<SessionRow> for SessionInfo`**

Map the new fields from the DB row to the API type.

**Step 4: Regenerate TypeScript types**

Run: `cargo test -p core -- export_bindings --ignored 2>/dev/null; echo "TypeScript types generated"`

Or if there's a ts-rs export test:
Run: `cargo test -p core -- ts_export`

**Step 5: Verify compile**

Run: `cargo check && cd src && npx tsc --noEmit`
Expected: No errors.

**Step 6: Commit**

```bash
git add crates/core/src/types.rs crates/db/src/queries.rs src/types/generated/
git commit -m "feat(api): expose full parser metrics in SessionInfo type"
```

---

### Task 9: Add Observability Logging

Add `tracing` log statements for parse diagnostics — per-session on anomaly, per-run summary.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — after `parse_bytes` returns in `pass_2_deep_index`

**Step 1: Add per-session anomaly logging**

After `parse_bytes` returns in `pass_2_deep_index`, add:

```rust
let diag = &parse_result.diagnostics;
if diag.json_parse_failures > 0 || diag.lines_unknown_type > 0 {
    tracing::warn!(
        session_id = %id,
        parse_failures = diag.json_parse_failures,
        unknown_types = diag.lines_unknown_type,
        total_lines = diag.lines_total,
        "Parse anomalies detected"
    );
}
```

**Step 2: Add per-run summary logging**

After all tasks complete in `pass_2_deep_index`, log aggregate stats.

**Step 3: Verify logging works**

Run: `RUST_LOG=debug cargo test -p db -- tests::test_golden_edge -v 2>&1 | grep -i anomal`
Expected: Warning about parse anomalies (1 unknown type, 1 parse failure).

**Step 4: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat(parser): add tracing logs for parse anomalies and indexing summary"
```

---

### Task 10: Run Full Test Suite and Verify

**Step 1: Run all backend tests**

Run: `cargo test`
Expected: ALL PASS (224+ existing tests + new golden/migration tests).

**Step 2: Run frontend compile check**

Run: `cd /Users/user/dev/@example-org/claude-view && npx tsc --noEmit`
Expected: No TypeScript errors.

**Step 3: Run the server to verify startup**

Run: `cargo run -p server -- --claude-dir ~/.claude 2>&1 | head -20`
Expected: Server starts, background indexing runs, re-indexes sessions with `parse_version = 0`.

**Step 4: Commit any final fixups**

If any tests need adjustments, fix and commit.

**Step 5: Update PROGRESS.md and plan status**

Update `docs/plans/PROGRESS.md` to add Phase 3.5 entry.
Update this plan's YAML frontmatter from `draft` to `done`.

```bash
git add docs/plans/
git commit -m "docs: mark full JSONL parser plan as done, update PROGRESS.md"
```

---

## Summary

| Task | Description | Key Files |
|------|-------------|-----------|
| 1 | Schema migration — new columns + parse_version + detail tables | `migrations.rs` |
| 2 | `ParseDiagnostics` struct | `indexer_parallel.rs` |
| 3 | New fields on `ExtendedMetadata` | `indexer_parallel.rs` |
| 4 | Golden test fixtures (all 7 line types) | `tests/golden_fixtures/*.jsonl` |
| 5 | **Rewrite `parse_bytes` — full JSON parse** | `indexer_parallel.rs` |
| 6 | Update DB persist for new metrics | `queries.rs`, `indexer_parallel.rs` |
| 7 | `parse_version` re-index trigger | `queries.rs`, `indexer_parallel.rs` |
| 8 | `SessionInfo` type + TypeScript regen | `types.rs`, `queries.rs`, `generated/` |
| 9 | Observability logging | `indexer_parallel.rs` |
| 10 | Full verification | All |

**Parse version:** `0` → `1` (triggers automatic re-index of all sessions on first startup after deploy)
