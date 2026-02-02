---
status: pending
date: 2026-02-03
---

# Plan: rusqlite for Deep Index Write Phase

## Problem

The deep index write phase takes **3.6s** for 668 sessions. The bottleneck is
async overhead: each `sqlx::query(...).execute().await` pays a tokio context
switch cost, and we do thousands of these inside the write loop:

- 668 × `UPDATE sessions` (41 params each)
- 668 × `INSERT turns` per-row loop (varies, ~5-50 turns per session)
- 668 × `INSERT invocations` per-row loop (varies)
- 668 × `UPSERT models` per-row loop (1-3 models per session)

Multi-row VALUES batching was attempted and **made things worse** — dynamic SQL
strings can't be prepared/cached by SQLite, and the compilation cost exceeds the
async overhead saved. The fundamental issue is using an async driver for a
synchronous, in-process database.

## Solution

Use `rusqlite` (synchronous SQLite bindings) for the write phase only. Wrap the
entire write loop in `spawn_blocking` with prepared statements. This eliminates:

1. Async runtime overhead per row (~thousands of avoided awaits)
2. SQLite statement re-compilation (prepared statements reused in tight loop)
3. Connection pool contention (dedicated connection, no pool arbitration)

All read operations and API routes stay on `sqlx` — we only use rusqlite for the
hot batch write path.

## Expected Impact

| Phase | Before | After (est.) | Why |
|-------|--------|-------------|-----|
| Write | 3,584ms | 200-400ms | Prepared stmts + zero async overhead |
| Parse | 5,623ms | 5,623ms | Unchanged |
| **Total** | **9,207ms** | **~6,000ms** | ~35% reduction |

## Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `rusqlite` workspace dep with `bundled` feature |
| `crates/db/Cargo.toml` | Add `rusqlite` dependency |
| `crates/db/src/lib.rs` | Store `db_path: PathBuf` in `Database` struct, expose getter |
| `crates/db/src/indexer_parallel.rs` | Replace write phase with `spawn_blocking` + rusqlite |

## Detailed Changes

### 1. Add rusqlite dependency

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
rusqlite = { version = "0.38", features = ["bundled"] }

# crates/db/Cargo.toml
[dependencies]
rusqlite = { workspace = true }
```

Use `bundled` feature so we don't need a system SQLite — same pattern as sqlx's
`libsqlite3-sys`. Both crates link to the same C library; no conflict.

### 2. Store db_path in Database struct

Currently `Database` only stores `pool: SqlitePool`. The path is lost after
construction. We need it to open a rusqlite connection for the write phase.

```rust
// crates/db/src/lib.rs
pub struct Database {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl Database {
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}
```

Update `new()` and `new_in_memory()` constructors to store the path. For
in-memory databases (tests), store a sentinel path — the rusqlite write path
won't be used in tests since they use small datasets that go through sqlx.

### 3. Replace write phase in pass_2_deep_index

The current write phase (lines ~1090-1230 in indexer_parallel.rs) runs inside an
async context. Replace it with a single `spawn_blocking` call:

```rust
// ── Write phase (synchronous via rusqlite) ──────────────────────────
let db_path = db.db_path().to_owned();
let write_count = tokio::task::spawn_blocking(move || {
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

    // Prepare all 4 statements once
    let mut update_stmt = conn.prepare(UPDATE_SESSION_SQL)?;
    let mut insert_turn_stmt = conn.prepare(INSERT_TURN_SQL)?;
    let mut insert_invocation_stmt = conn.prepare(INSERT_INVOCATION_SQL)?;
    let mut upsert_model_stmt = conn.prepare(UPSERT_MODEL_SQL)?;

    let tx = conn.unchecked_transaction()?;

    for result in &results {
        // 1. UPDATE session deep fields (prepared, 41 params)
        update_stmt.execute(rusqlite::params![...])?;

        // 2. INSERT turns (prepared, reused per turn)
        for turn in &result.parse_result.turns {
            insert_turn_stmt.execute(rusqlite::params![...])?;
        }

        // 3. INSERT invocations (prepared, reused per invocation)
        for inv in &result.classified_invocations {
            insert_invocation_stmt.execute(rusqlite::params![...])?;
        }

        // 4. UPSERT models (prepared, reused per model)
        for model_id in &result.parse_result.models_seen {
            upsert_model_stmt.execute(rusqlite::params![...])?;
        }
    }

    tx.commit()?;
    Ok::<usize, rusqlite::Error>(results.len())
})
.await
.map_err(|e| format!("spawn_blocking join error: {}", e))?
.map_err(|e| format!("rusqlite write error: {}", e))?;
```

Key design decisions:
- **Open a separate connection** — WAL mode allows concurrent readers (sqlx) and
  one writer (rusqlite). No lock contention as long as we don't hold the write
  transaction for too long (ours takes <500ms).
- **`unchecked_transaction()`** — avoids the borrow checker issue with mutable
  refs to both transaction and statements. Safe because we commit explicitly.
- **Static SQL strings** — the 4 SQL statements are `const &str`, prepared once,
  reused thousands of times. SQLite caches the compiled bytecode.

### 4. Extract SQL as constants

Move the 4 SQL strings out of the function into `const` blocks in queries.rs or
at the top of indexer_parallel.rs:

```rust
const UPDATE_SESSION_SQL: &str = r#"
    UPDATE sessions SET
        last_message = ?2, turn_count = ?3,
        ... (same 41-param UPDATE as current)
    WHERE id = ?1
"#;

const INSERT_TURN_SQL: &str = r#"
    INSERT OR IGNORE INTO turns (
        session_id, uuid, seq, model_id, parent_uuid,
        content_type, input_tokens, output_tokens,
        cache_read_tokens, cache_creation_tokens,
        service_tier, timestamp
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
"#;

const INSERT_INVOCATION_SQL: &str = r#"
    INSERT OR IGNORE INTO invocations
        (source_file, byte_offset, invocable_id, session_id, project, timestamp)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
"#;

const UPSERT_MODEL_SQL: &str = r#"
    INSERT INTO models (id, provider, family, first_seen, last_seen)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(id) DO UPDATE SET
        last_seen = MAX(models.last_seen, excluded.last_seen)
"#;
```

### 5. Keep sqlx _tx functions for non-hot-path callers

The `batch_insert_invocations_tx`, `batch_upsert_models_tx`, and
`batch_insert_turns_tx` functions in queries.rs stay as-is. They're used by:
- `Database::batch_insert_invocations()` (API/test callers)
- `Database::batch_upsert_models()` (API/test callers)
- `Database::batch_insert_turns()` (API/test callers)

These are not in the hot path. Only `pass_2_deep_index` switches to rusqlite.

## Testing

1. `cargo test -p vibe-recall-db` — all existing tests pass (they use in-memory
   sqlx, don't hit the rusqlite path)
2. `cargo run -p vibe-recall-server` — observe `[perf]` output:
   - Write phase should drop from ~3,600ms to ~200-400ms
3. `cargo check --release` — clean release build

## Risks

| Risk | Mitigation |
|------|-----------|
| Two SQLite connections could conflict | WAL mode allows concurrent read + write |
| rusqlite + sqlx link same C library | Both use `libsqlite3-sys`; Cargo deduplicates |
| Test databases are in-memory | rusqlite path only used for file-based DBs; tests stay on sqlx |
| `bundled` feature increases build time | One-time cost; already using `libsqlite3-sys` via sqlx |

## Non-Goals

- Replacing sqlx entirely (reads are fine async)
- Optimizing the parse phase (already SIMD-filtered, 5.6s is inherent)
- simd-json swap (requires `&mut [u8]`, incompatible with zero-copy mmap)
