---
status: superseded
date: 2026-01-27
superseded_by: 2026-01-27-phase2-parallel-indexing-and-registry.md
---

# Startup UX + Parallel Indexing Design

> Instant startup by reading Claude Code's own `sessions-index.json` (<10ms), with optional deep JSONL parsing for extended metadata in background.

## Problem

Current startup reads 807 MB of JSONL (542 sessions, 10 projects) sequentially before starting the server. Users see no feedback for ~83 seconds. Target users may have 10-30 GB of session data. At current throughput (~10 MB/s), 30 GB = 50 minutes. Unusable.

**Root cause:** `extract_session_metadata()` parses every line of every JSONL file to extract metadata that Claude Code has **already computed and stored** in `sessions-index.json`.

## Discovery: Claude Code's Built-In Data Sources

Claude Code maintains pre-computed metadata files that eliminate the need to parse raw JSONL for most fields.

### Source 1: `sessions-index.json` (per project)

**Location:** `~/.claude/projects/<encoded-path>/sessions-index.json`

Each project has an index file with session metadata. Example entry:

```json
{
  "sessionId": "8a662442-4584-4700-b6b1-707d5a524355",
  "fullPath": "/Users/.../<uuid>.jsonl",
  "fileMtime": 1769364547212,
  "firstPrompt": "some feedback abt the @docs/plans/...",
  "summary": "Claude-view UI: Sidebar paths, SPA nav, active indicators",
  "messageCount": 60,
  "created": "2026-01-25T16:42:56.852Z",
  "modified": "2026-01-25T17:18:30.718Z",
  "gitBranch": "main",
  "projectPath": "/Users/user/dev/@myorg/claude-view",
  "isSidechain": false
}
```

**Fields available for free (no JSONL parsing):**

| Field | Source | Notes |
|-------|--------|-------|
| `sessionId` | UUID | Unique session identifier |
| `fullPath` | Absolute path | Direct path to JSONL file |
| `fileMtime` | Epoch ms | File modification timestamp |
| `firstPrompt` | Truncated string | First user message (= our `preview`) |
| `summary` | Claude-generated | Human-readable session summary (NEW — not available from JSONL parsing) |
| `messageCount` | Integer | Total messages in session |
| `created` | ISO timestamp | Session creation time |
| `modified` | ISO timestamp | Last activity time |
| `gitBranch` | String | Git branch when session was active (NEW) |
| `projectPath` | Absolute path | Original project directory |
| `isSidechain` | Boolean | Whether this is a sub-agent session (NEW) |

### Source 2: `stats-cache.json` (global)

**Location:** `~/.claude/stats-cache.json`

Pre-aggregated daily activity stats across all projects:

```json
{
  "version": 1,
  "lastComputedDate": "2026-01-26",
  "dailyActivity": [
    { "date": "2025-11-10", "messageCount": 10, "sessionCount": 1, "toolCallCount": 3 }
  ]
}
```

### Source 3: `history.jsonl` (global prompt history)

**Location:** `~/.claude/history.jsonl`

Every user prompt with timestamp and project:

```json
{ "display": "how to start ngrok login", "timestamp": 1760357006294, "project": "/Users/.../mvp-app" }
```

---

## Revised Solution: Index-First Architecture

### Before vs After

```
BEFORE: Parse 807 MB of JSONL (83s) → extract metadata → start server
AFTER:  Read ~10 JSON files (~50 KB total) → start server → done
```

The entire startup bottleneck disappears. Claude Code already did the work for us.

### What We Get vs What We Still Need

| Field | From `sessions-index.json` | Still needs JSONL parsing? |
|-------|---------------------------|---------------------------|
| Session ID, path | `sessionId`, `fullPath` | No |
| Preview / first prompt | `firstPrompt` | No |
| Summary | `summary` | No (NEW — better than what we had) |
| Message count | `messageCount` | No |
| Created / Modified | `created`, `modified` | No |
| Git branch | `gitBranch` | No (NEW) |
| Is sidechain | `isSidechain` | No (NEW) |
| Project path | `projectPath` | No |
| **Tool counts** | — | **Yes** |
| **Skills used** | — | **Yes** |
| **Files touched** | — | **Yes** |
| **Last message** | — | **Yes** (index only has first prompt) |

**Conclusion:** Pass 1 is now just reading JSON index files. Pass 2 (JSONL parsing) is only needed for tool_counts, skills_used, files_touched, and last_message — fields the index doesn't provide.

### Two-Pass Architecture (Revised)

```
Pass 1: Read sessions-index.json files (10 files, ~50 KB)    → <10ms
Pass 2: Parallel mmap JSONL for tool_counts, skills, etc.    → background
```

### User Experience Timeline (Revised)

**First launch — ANY dataset size (1 GB or 30 GB):**

| Time | What user sees |
|------|---------------|
| 0.0s | Server ready, browser opens |
| **<10ms** | **Full project list with summaries, message counts, timestamps, git branches** |
| 0.5-8s | Tool counts, skills, files_touched fill in progressively (background) |

**Returning user:** Cached data from DB renders instantly. Background checks for changes.

This is a fundamentally different UX. The user sees **complete, useful data** (not skeleton placeholders) within milliseconds. The background pass fills in nice-to-have analytics fields.

---

## Core Architecture

```
main() {
  1. Open DB                                    // <1ms
  2. Start Axum server                          // <100ms
  3. Print "Ready" + URL                        // immediate
  4. tokio::spawn(background_index)             // non-blocking
}

background_index() {
  Pass 1: read_session_indexes()                // <10ms — JSON files
        → diff_against_db()                     // <1ms
        → batch insert/update DB                // <10ms
  Pass 2: parallel_deep_index(changed_files)    // background — JSONL parsing
        → mmap + parse_bytes for extended fields
        → batch update DB
}
```

### Three Startup Scenarios

| Scenario | UI behavior | Backend |
|----------|------------|---------|
| **First launch** (empty DB) | Full data at <100ms, extended fields fill in | Pass 1 reads index JSONs → Pass 2 parses JSONL |
| **Returning** (cached, no changes) | Instant from DB cache | Pass 1 diffs, finds 0 changes, skip Pass 2 |
| **Returning + changes** | Cached data + new sessions appear | Pass 1 diffs, Pass 2 for changed files only |

---

## Data Source Mapping

### `SessionInfo` Fields — Where Each Comes From

| Field | Pass 1 Source | Pass 2 Source | Notes |
|-------|--------------|---------------|-------|
| `id` | `sessionId` | — | |
| `project_id` | Derived from parent dir | — | |
| `preview` | `firstPrompt` | — | Already truncated by Claude Code |
| `summary` | `summary` | — | **NEW field** — Claude-generated |
| `message_count` | `messageCount` | — | |
| `first_message_at` | `created` | — | ISO → unix epoch |
| `last_message_at` | `modified` | — | ISO → unix epoch |
| `file_path` | `fullPath` | — | |
| `git_branch` | `gitBranch` | — | **NEW field** |
| `is_sidechain` | `isSidechain` | — | **NEW field** |
| `last_message` | — | JSONL parsing | Scan backward from EOF |
| `turn_count` | — | JSONL parsing | Count user/assistant pairs |
| `tool_counts` | — | JSONL parsing | Count Read/Edit/Write/etc. |
| `skills_used` | — | JSONL parsing | Scan user messages for /commands |
| `files_touched` | — | JSONL parsing | Extract file_path from assistant lines |
| `file_size` | `stat()` or computed | — | Not in index, quick stat call |

### Supplementary Sources

| Source | Provides | When to use |
|--------|----------|-------------|
| `stats-cache.json` | Daily activity heatmap (messages/sessions/tools per day) | Dashboard analytics |
| `history.jsonl` | All user prompts with timestamps + project | Prompt search feature |

---

## Pass 2: Deep JSONL Index (Background)

Only needed for fields NOT in `sessions-index.json`: `tool_counts`, `skills_used`, `files_touched`, `last_message`, `turn_count`.

### Pipeline (unchanged from previous design)

```
changed files[] ──┬── spawn_blocking ── mmap(file1) → parse_bytes(&[u8]) → ExtendedMetadata
                  ├── spawn_blocking ── mmap(file2) → parse_bytes(&[u8]) → ExtendedMetadata
                  └── ...  (semaphore-bounded to num_cpus)
                              │
                              ▼ (collect batch of results)
                      BEGIN TRANSACTION
                        UPDATE sessions SET tool_counts=?, skills=?, ... WHERE id=?  (×N)
                      COMMIT
```

### Optimization Stack (same as before)

1. **Parallel file processing** — `spawn_blocking` + `Semaphore` bounded to `num_cpus`
2. **Memory-mapped I/O** — `memmap2::Mmap` with fallback to `std::fs::read()`
3. **SIMD line scanning** — `memchr::memchr_iter(b'\n')` + `memmem::Finder` per line
4. **Batch DB transactions** — single `BEGIN...COMMIT`

### `parse_bytes()` — Correctness-First Line Scanner

Identical to previous design. Scan `&[u8]` line-by-line using SIMD newline detection, extract only the fields not available from `sessions-index.json`.

```rust
fn parse_bytes(data: &[u8]) -> ExtendedMetadata {
    let mut meta = ExtendedMetadata::default();
    let mut user_count = 0usize;
    let mut assistant_count = 0usize;
    let mut last_user_content: Option<String> = None;

    let user_finder = memmem::Finder::new(b"\"type\":\"user\"");
    let asst_finder = memmem::Finder::new(b"\"type\":\"assistant\"");

    for line in split_lines_simd(data) {
        if line.is_empty() { continue; }

        if user_finder.find(line).is_some() {
            user_count += 1;
            if let Some(content) = extract_content_from_line(line) {
                last_user_content = Some(content);
            }
        } else if asst_finder.find(line).is_some() {
            assistant_count += 1;
            count_tools_from_line(line, &mut meta.tool_counts);
            extract_file_paths_from_line(line, &mut meta.files_touched);
        }
    }

    meta.turn_count = user_count.min(assistant_count);
    meta.last_message = last_user_content.map(|c| truncate(&c, 200)).unwrap_or_default();
    meta
}
```

**Note:** `parse_bytes` no longer extracts `preview`, `message_count`, or timestamps — those come from `sessions-index.json` now. It only extracts `ExtendedMetadata` (tool_counts, skills, files_touched, last_message, turn_count).

### mmap Safety (unchanged)

- Claude Code appends to JSONL (never truncates)
- Maps are short-lived (milliseconds per file)
- Fallback to `std::fs::read()` on failure
- Contained in single `read_file_fast()` function

### Performance Targets (revised)

**Pass 1 (index JSON files):**

| Dataset | Files to read | Estimated time |
|---------|--------------|---------------|
| 10 projects (current) | 10 JSON files (~50 KB) | **<10ms** |
| 50 projects | 50 JSON files (~250 KB) | **<20ms** |
| 100 projects | 100 JSON files (~500 KB) | **<50ms** |

*Independent of JSONL data size. 30 GB of sessions makes no difference — the index files are tiny.*

**Pass 2 (JSONL deep parse, background):**

| Data size | Time |
|-----------|------|
| 807 MB (now) | **<1s** |
| 10 GB | **<5s** |
| 30 GB | **<10s** |

**Subsequent launches:**

| Scenario | Time |
|----------|------|
| No changes | **<10ms** (diff index JSON mtimes) |
| 5 changed sessions | **<200ms** (re-parse 5 JSONL files) |
| 20 changed sessions | **<500ms** |

---

## Progress Reporting

### Terminal (TUI)

```
🔍 vibe-recall v0.1.0

  ✓ Ready in 0.1s — 10 projects, 542 sessions
  → http://localhost:47892

  ⠋ Deep indexing 42/542 sessions...
  ✓ Deep index complete — 542 sessions (0.8s)
```

The "Ready" line now includes project/session counts because Pass 1 completes in <10ms — before the terminal even renders.

### Frontend (SSE)

`GET /api/indexing/progress` — Server-Sent Events stream:

```
event: ready
data: {"status":"ready","projects":10,"sessions":542}

event: deep-progress
data: {"status":"deep-indexing","indexed":42,"total":542}

event: done
data: {"status":"done","indexed":542,"total":542,"durationMs":800}
```

Frontend behavior:
- On connect: if Pass 1 already done (likely), immediately gets `ready` event with full counts
- During Pass 2: `deep-progress` events update progress indicator
- On `done`: final refetch to get extended fields (tool_counts, etc.)
- If Pass 2 skipped (no changes): gets `done` immediately

### Shared Indexing State

```rust
pub struct IndexingState {
    pub status: AtomicU8,           // 0=idle, 1=reading-indexes, 2=deep-indexing, 3=done, 4=error
    pub total: AtomicUsize,         // total files for deep index
    pub indexed: AtomicUsize,       // files deep-indexed so far
    pub projects_found: AtomicUsize,
    pub sessions_found: AtomicUsize,
    pub error: RwLock<Option<String>>,
}
```

---

## Schema Changes

### New fields on `sessions` table

```sql
ALTER TABLE sessions ADD COLUMN summary TEXT;
ALTER TABLE sessions ADD COLUMN git_branch TEXT;
ALTER TABLE sessions ADD COLUMN is_sidechain BOOLEAN DEFAULT FALSE;
ALTER TABLE sessions ADD COLUMN deep_indexed_at INTEGER;  -- NULL = not yet deep-indexed
```

`deep_indexed_at` tracks whether Pass 2 has run for this session. Pass 1 sets it to NULL; Pass 2 sets it to the current timestamp. This lets the API distinguish "no tool data yet" from "zero tools used."

### New `SessionInfo` fields

```rust
pub struct SessionInfo {
    // ... existing fields ...
    pub summary: Option<String>,      // NEW: Claude-generated summary
    pub git_branch: Option<String>,   // NEW: branch when session was active
    pub is_sidechain: bool,           // NEW: sub-agent session flag
    pub deep_indexed: bool,           // NEW: whether extended fields are populated
}
```

---

## Implementation Plan

### New Files

| File | Purpose |
|------|---------|
| `crates/core/src/session_index.rs` | Parse `sessions-index.json` format |
| `crates/db/src/indexer_parallel.rs` | Two-pass: read indexes + parallel JSONL deep parse |
| `crates/server/src/routes/indexing.rs` | SSE endpoint `GET /api/indexing/progress` |
| `crates/server/src/indexing_state.rs` | `IndexingState` shared atomic struct |

### Modified Files

| File | Changes |
|------|---------|
| `crates/core/src/types.rs` | Add `summary`, `git_branch`, `is_sidechain`, `deep_indexed` to `SessionInfo` |
| `crates/core/src/lib.rs` | Export `session_index` module |
| `crates/db/src/migrations.rs` | Add migration for new columns |
| `crates/db/src/queries.rs` | Update insert/update queries for new fields |
| `crates/server/src/main.rs` | Start server first, spawn background indexing |
| `crates/server/src/state.rs` | Add `Arc<IndexingState>` to `AppState` |
| `crates/server/src/lib.rs` | Register `/api/indexing/progress` route |
| `crates/db/Cargo.toml` | Add `memmap2`, `memchr` deps |
| `crates/server/Cargo.toml` | Add `tokio-stream` for SSE |

### Steps (ordered by dependency)

| Step | Depends on | Deliverable |
|------|-----------|-------------|
| 1. Add deps (`memmap2`, `memchr`, `tokio-stream`) | — | Cargo.toml changes |
| 2. `session_index.rs` — parse `sessions-index.json` | — | Struct + deserializer |
| 3. Schema migration — add `summary`, `git_branch`, `is_sidechain`, `deep_indexed_at` | — | Migration SQL |
| 4. Update `SessionInfo` type + queries | Steps 2, 3 | `types.rs`, `queries.rs` |
| 5. `IndexingState` struct | — | `indexing_state.rs` |
| 6. `indexer_parallel.rs` — `pass_1_read_indexes()` | Steps 2, 4 | Read JSON → insert/update DB |
| 7. `indexer_parallel.rs` — `read_file_fast()` + `parse_bytes()` | Step 1 | mmap + SIMD line scanner |
| 8. `indexer_parallel.rs` — `pass_2_deep_index()` | Steps 5, 7 | Parallel JSONL + batch DB |
| 9. `indexer_parallel.rs` — `run_background_index()` | Steps 6, 8 | Orchestrator |
| 10. Golden test: `parse_bytes` vs `extract_session_metadata` | Step 7 | AC-4 |
| 11. Update `AppState` + `create_app` | Step 5 | `state.rs`, `lib.rs` |
| 12. Rewrite `main.rs` | Steps 9, 11 | Server-first startup |
| 13. SSE route | Steps 5, 11 | `routes/indexing.rs` |
| 14. TUI progress | Step 9 | In `main.rs` |
| 15. All acceptance tests | Steps 1-14 | AC-1 through AC-13 |
| 16. Manual performance benchmarks | Step 15 | AC-14 |

### What Stays the Same

- `scan_files()` — still used by Pass 2 to find JSONL files for deep parsing
- `diff_against_db()` — used between Pass 1 and Pass 2
- All existing routes (`/api/projects`, `/api/health`)
- `indexer.rs` — kept as reference implementation
- `extract_session_metadata()` — kept as golden reference for correctness tests

---

## Acceptance Criteria

Each criterion is independently testable. Performance criteria are benchmarks (not CI-blocking).

### AC-1: Server starts before indexing

**Test:** Start server with empty DB. Verify `/api/health` returns 200 before any indexing completes.

```rust
#[tokio::test]
async fn test_server_starts_before_indexing() {
    let db = Database::new_in_memory().await.unwrap();
    let state = Arc::new(IndexingState::new());
    let app = create_app_with_indexing(db, state.clone());
    let (status, _) = get(app, "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state.status(), IndexingStatus::Idle);
}
```

### AC-2: Pass 1 reads `sessions-index.json` correctly

**Test:** Create temp dir with `sessions-index.json` files. Verify all fields are parsed and inserted into DB.

```rust
#[tokio::test]
async fn test_pass_1_reads_session_index() {
    let (_tmp, base) = setup_test_dir_with_session_indexes(&[
        ("-Users-test-projA", vec![
            SessionIndexEntry {
                session_id: "abc-123".into(),
                first_prompt: "Hello world".into(),
                summary: "Test session".into(),
                message_count: 42,
                git_branch: "main".into(),
                is_sidechain: false,
                created: "2026-01-25T16:42:56.852Z".into(),
                modified: "2026-01-25T17:18:30.718Z".into(),
                ..Default::default()
            },
        ]),
    ]).await;
    let db = Database::new_in_memory().await.unwrap();

    pass_1_read_indexes(&base, &db).await.unwrap();

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    let session = &projects[0].sessions[0];
    assert_eq!(session.id, "abc-123");
    assert_eq!(session.preview, "Hello world");
    assert_eq!(session.summary.as_deref(), Some("Test session"));
    assert_eq!(session.message_count, 42);
    assert_eq!(session.git_branch.as_deref(), Some("main"));
    assert!(!session.is_sidechain);
    assert!(!session.deep_indexed); // Pass 2 hasn't run yet
}
```

### AC-3: Pass 1 handles missing/malformed index files

**Test:** Verify graceful handling when `sessions-index.json` is missing, empty, or contains invalid JSON.

```rust
#[tokio::test]
async fn test_pass_1_handles_missing_index() {
    // Dir with JSONL files but no sessions-index.json
    let (_tmp, base) = setup_test_dir_jsonl_only(&["s1.jsonl", "s2.jsonl"]).await;
    let db = Database::new_in_memory().await.unwrap();

    // Should not error — falls back to stat-based skeleton
    let result = pass_1_read_indexes(&base, &db).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pass_1_handles_malformed_index() {
    let (_tmp, base) = setup_test_dir_with_raw_index(
        "-Users-test-proj",
        b"{ invalid json",
    ).await;
    let db = Database::new_in_memory().await.unwrap();

    let result = pass_1_read_indexes(&base, &db).await;
    assert!(result.is_ok()); // graceful degradation, not crash
}
```

### AC-4: Pass 2 fills extended metadata

**Test:** After Pass 1, run Pass 2. Verify `tool_counts`, `skills_used`, `files_touched`, `last_message` are populated.

```rust
#[tokio::test]
async fn test_pass_2_fills_extended_metadata() {
    let (_tmp, base) = setup_test_dir_with_index_and_jsonl(
        "-Users-test-proj",
        &session_index_entry("s1"),
        REALISTIC_SESSION_CONTENT,
    ).await;
    let db = Database::new_in_memory().await.unwrap();
    let state = IndexingState::new();

    pass_1_read_indexes(&base, &db).await.unwrap();
    pass_2_deep_index(&base, &db, &state).await.unwrap();

    let projects = db.list_projects().await.unwrap();
    let session = &projects[0].sessions[0];
    assert!(session.deep_indexed, "should be marked as deep-indexed");
    assert!(session.turn_count > 0);
    assert!(!session.last_message.is_empty());
    // tool_counts populated (specific values depend on test data)
}
```

### AC-5: `parse_bytes()` matches `extract_session_metadata()` (golden test)

**Test:** Run both parsers on the same file. Assert identical output for shared fields.

```rust
#[tokio::test]
async fn test_parse_bytes_matches_original() {
    let (_tmp, file_path) = create_realistic_session_file().await;

    let old = extract_session_metadata(&file_path).await;
    let data = std::fs::read(&file_path).unwrap();
    let new = parse_bytes(&data);

    assert_eq!(old.turn_count, new.turn_count, "turn_count mismatch");
    assert_eq!(old.tool_counts, new.tool_counts, "tool_counts mismatch");
    assert_eq!(old.last_message, new.last_message, "last_message mismatch");
    assert_eq!(old.skills_used, new.skills_used, "skills_used mismatch");
    assert_eq!(old.files_touched, new.files_touched, "files_touched mismatch");
}
```

**Golden test variants:**
- Empty file → `ExtendedMetadata::default()`
- Single user message → correct last_message
- `"type":"user"` inside message content → not double-counted
- Long lines (>1 MB single JSONL entry) → handled correctly
- Non-UTF8 bytes → graceful handling

### AC-6: mmap fallback works

```rust
#[test]
fn test_read_file_fast_returns_correct_data() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"test content").unwrap();
    let data = read_file_fast(tmp.path()).unwrap();
    assert_eq!(data.as_ref(), b"test content");
}
```

### AC-7: Batch DB writes in single transaction

```rust
#[tokio::test]
async fn test_batch_db_writes_100_sessions() {
    let (_tmp, base) = setup_test_dir_with_n_indexed_sessions(100).await;
    let db = Database::new_in_memory().await.unwrap();
    let state = IndexingState::new();

    pass_1_read_indexes(&base, &db).await.unwrap();
    pass_2_deep_index(&base, &db, &state).await.unwrap();

    let total: usize = db.list_projects().await.unwrap()
        .iter().map(|p| p.sessions.len()).sum();
    assert_eq!(total, 100);
}
```

### AC-8: Parallel processing uses multiple cores

```rust
#[tokio::test]
async fn test_parallel_deep_index_completes_quickly() {
    let (_tmp, base) = setup_test_dir_with_n_indexed_sessions(50).await;
    let db = Database::new_in_memory().await.unwrap();
    let state = IndexingState::new();
    pass_1_read_indexes(&base, &db).await.unwrap();

    let start = Instant::now();
    pass_2_deep_index(&base, &db, &state).await.unwrap();
    assert!(start.elapsed() < Duration::from_secs(5),
        "50 files should complete quickly with parallelism");
}
```

### AC-9: IndexingState tracks progress

```rust
#[tokio::test]
async fn test_indexing_state_progress() {
    let (_tmp, base) = setup_test_dir_with_n_indexed_sessions(10).await;
    let db = Database::new_in_memory().await.unwrap();
    let state = Arc::new(IndexingState::new());

    assert_eq!(state.status(), IndexingStatus::Idle);
    run_background_index(&base, &db, &state).await.unwrap();
    assert_eq!(state.status(), IndexingStatus::Done);
    assert_eq!(state.sessions_found.load(Ordering::Relaxed), 10);
    assert_eq!(state.indexed.load(Ordering::Relaxed), 10);
}
```

### AC-10: SSE endpoint streams events

```rust
#[tokio::test]
async fn test_sse_endpoint() {
    let db = Database::new_in_memory().await.unwrap();
    let state = Arc::new(IndexingState::new());
    state.status.store(3, Ordering::Relaxed); // Done

    let app = create_app_with_indexing(db, state);
    let response = app.oneshot(
        Request::builder().uri("/api/indexing/progress").body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("content-type").unwrap()
        .to_str().unwrap().contains("text/event-stream"));
}
```

### AC-11: Subsequent launches skip Pass 2

```rust
#[tokio::test]
async fn test_subsequent_launch_skips_deep_index() {
    let (_tmp, base) = setup_test_dir_with_n_indexed_sessions(5).await;
    let db = Database::new_in_memory().await.unwrap();

    let state1 = IndexingState::new();
    run_background_index(&base, &db, &state1).await.unwrap();
    assert_eq!(state1.indexed.load(Ordering::Relaxed), 5);

    let state2 = IndexingState::new();
    run_background_index(&base, &db, &state2).await.unwrap();
    assert_eq!(state2.indexed.load(Ordering::Relaxed), 0, "no changes = no re-indexing");
}
```

### AC-12: New schema fields work end-to-end

```rust
#[tokio::test]
async fn test_new_fields_in_api_response() {
    // Setup with sessions-index.json containing summary, gitBranch, isSidechain
    let (_tmp, base, db) = setup_full_test_env().await;
    let state = Arc::new(IndexingState::new());
    run_background_index(&base, &db, &state).await.unwrap();

    let app = create_app_with_indexing(db, state);
    let (status, body) = get(app, "/api/projects").await;
    assert_eq!(status, StatusCode::OK);

    let projects: Vec<ProjectInfo> = serde_json::from_str(&body).unwrap();
    let session = &projects[0].sessions[0];
    assert!(session.summary.is_some(), "summary should be present");
    assert!(session.git_branch.is_some(), "git_branch should be present");
}
```

### AC-13: Existing tests still pass

**Test:** `cargo test --workspace` passes with zero failures.

### AC-14: Performance benchmarks (not CI-blocking)

| Metric | Target | How to measure |
|--------|--------|---------------|
| Server "Ready" time | <500ms | Instrument `main()` startup |
| Pass 1 (10 projects, 542 sessions) | **<10ms** | Instrument `pass_1_read_indexes()` |
| Pass 2 (807 MB, 542 JSONL files) | <1s | Instrument `pass_2_deep_index()` |
| Subsequent launch (no changes) | **<10ms** | Instrument `run_background_index()` |
| `/api/projects` response time | <100ms | HTTP benchmark |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| `sessions-index.json` format changes | Medium | Pass 1 breaks | Version field in JSON; graceful fallback to stat-based skeleton (AC-3) |
| Index file missing for some projects | Medium | No data for that project | Fallback: stat JSONL files for skeleton data |
| Index data stale (Claude Code didn't update) | Low | Slightly wrong counts | Pass 2 JSONL parsing corrects counts; staleness is bounded |
| `parse_bytes()` diverges from original parser | Medium | Wrong extended metadata | AC-5 golden test with variants |
| mmap SIGBUS | Very Low | Crash | Fallback to `std::fs::read()` |
| `spawn_blocking` thread pool exhaustion | Low | Slowdown | Semaphore bounds to `num_cpus` |
| SQLite lock contention | Low | Slow API | WAL mode + batch writes |

---

## Future Considerations (Not in Scope)

- **API response size** — `/api/projects` returns 676 KB. Address with pagination/field filtering separately.
- **`stats-cache.json` integration** — pre-computed daily activity for dashboard heatmaps.
- **`history.jsonl` integration** — searchable prompt history across all projects.
- **`notify` file watcher** — Phase 4 of v2 roadmap. Watch for `sessions-index.json` changes.
- **Background daemon** — v3.0 potential.
