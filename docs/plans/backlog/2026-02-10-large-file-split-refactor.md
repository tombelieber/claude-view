---
status: pending
date: 2026-02-10
scope: phase-a-only
---

# Large File Split Refactor — Reduce Merge Conflict Surface Area

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split 5 oversized Rust files (16,200+ combined lines) into focused domain modules to eliminate merge conflicts during rebase/merge from main.

**Architecture:** Each large file becomes a directory module (`queries.rs` → `queries/mod.rs` + sub-files). Rust allows multiple `impl Database` blocks across files — the compiler merges them. Zero runtime difference, zero behavior change. The compiler enforces correctness: if a method or type is misplaced, it won't compile.

**Tech Stack:** Rust 2021 edition, traditional `mod.rs` directory module style (matching existing `routes/mod.rs` pattern), sqlx/serde/ts_rs.

---

## Audit Summary

| File | Lines | Methods | Types | Tests | Phase |
|------|-------|---------|-------|-------|-------|
| `crates/db/src/queries.rs` | 4,562 | 67 | 16 | 29 (~1,312 lines) | A |
| `crates/db/src/migrations.rs` | 1,528 | 0 | 0 | ~1,000 lines | B |
| `crates/db/src/indexer_parallel.rs` | 3,524 | ~20 | ~15 | ~2,200 lines | C (Draft) |
| `crates/db/src/git_correlation.rs` | 3,342 | ~15 | ~8 | ~2,025 lines | D (Draft) |
| `crates/db/src/snapshots.rs` | 3,282 | ~20 | ~16 | ~500 lines | E (Draft) |

**Why these 5?** They're all in `crates/db/` — the single crate that every feature branch touches. Splitting them means branch A (adding dashboard queries) no longer conflicts with branch B (adding classification queries) even when both rebase on the same main.

---

## Cross-Reference Audit (queries.rs)

Before splitting, these are the **only external consumers** of `queries.rs` internals:

**`lib.rs` re-exports (13 types):**
```rust
pub use queries::AIGenerationStats;
pub use queries::BranchCount;
pub use queries::IndexerEntry;
pub use queries::InvocableWithCount;
pub use queries::ModelWithStats;
pub use queries::StatsOverview;
pub use queries::TokensByModel;
pub use queries::TokensByProject;
pub use queries::TokenStats;
pub use queries::StorageStats;
pub use queries::HealthStats;
pub use queries::HealthStatus;
pub use queries::ClassificationStatus;
```

**`indexer_parallel.rs` imports (4 _tx functions):**
```rust
crate::queries::update_session_deep_fields_tx()   // line ~2089
crate::queries::batch_insert_invocations_tx()      // line ~2163
crate::queries::batch_upsert_models_tx()           // line ~2177
crate::queries::batch_insert_turns_tx()            // line ~2190
```

**Server crate:** No direct imports. All access goes through `lib.rs` re-exports.

---

## Phase A: Split `queries.rs` (4,562 → 10 files + 7 test files)

**Rationale:** Highest-value split. 67 methods across 8 domains, every feature branch touches this file.

### Target structure

```
crates/db/src/queries/
├── mod.rs              # Module declarations + re-exports only (~50 lines)
├── types.rs            # 13 public structs/enums (BranchCount, TokenStats, etc.)
├── sessions.rs         # Session CRUD, indexer state, deep fields (11 methods)
├── invocables.rs       # Invocable + Invocation CRUD, stats overview (5 methods)
├── models.rs           # Model + Turn CRUD, token stats (4 methods)
├── dashboard.rs        # Dashboard stats, project summaries, branches, top invocables (12 methods)
├── classification.rs   # Classification job + index run CRUD (19 methods)
├── system.rs           # Storage stats, health stats, classification status, reset (10 methods)
├── ai_generation.rs    # AI generation stats (1 method)
└── row_types.rs        # Internal row types (SessionRow, ClassificationJobRow, IndexRunRow) + 4 _tx helpers
```

> **Why `types.rs`?** Without it, `mod.rs` holds 13 type definitions (~250 lines). Every new type added by any feature requires editing `mod.rs`, making it a conflict hotspot. With `types.rs`, `mod.rs` shrinks to ~50 lines (module declarations + re-exports) and almost never needs editing when adding features.

### Complete method → file mapping (67 methods + 4 free functions)

#### `sessions.rs` (11 methods)

| Line | Method |
|------|--------|
| 246 | `insert_session` |
| 371 | `list_projects` |
| 477 | `get_indexer_state` |
| 496 | `get_all_indexer_states` |
| 520 | `update_indexer_state` |
| 550 | `insert_session_from_index` |
| 625 | `update_session_deep_fields` |
| 799 | `get_sessions_needing_deep_index` |
| 818 | `mark_all_sessions_for_reindex` |
| 1666 | `remove_stale_sessions` |
| 2442 | `update_session_classification` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use chrono::Utc;
use std::collections::HashMap;
use claude_view_core::{ProjectInfo, SessionInfo, ToolCounts};
use super::row_types::SessionRow;
use super::IndexerEntry;
// NOTE: _tx functions are NOT used by sessions.rs — they're only used by indexer_parallel.rs
```

#### `invocables.rs` (5 methods)

| Line | Method |
|------|--------|
| 834 | `upsert_invocable` |
| 869 | `batch_insert_invocations` |
| 885 | `list_invocables_with_counts` |
| 909 | `batch_upsert_invocables` |
| 947 | `get_stats_overview` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use super::{InvocableWithCount, StatsOverview};
// NOTE: batch_insert_invocations_tx is NOT called by invocables.rs — only by indexer_parallel.rs
```

#### `models.rs` (4 methods)

| Line | Method |
|------|--------|
| 982 | `batch_upsert_models` |
| 999 | `batch_insert_turns` |
| 1014 | `get_all_models` |
| 1033 | `get_token_stats` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use claude_view_core::RawTurn;
use super::{ModelWithStats, TokenStats};
// NOTE: _tx functions are NOT called by models.rs — only by indexer_parallel.rs
```

#### `dashboard.rs` (12 methods)

| Line | Method |
|------|--------|
| 1076 | `list_project_summaries` |
| 1120 | `list_sessions_for_project` |
| 1238 | `list_branches_for_project` |
| 1263 | `all_top_invocables_by_kind` (private) |
| 1292 | `all_top_invocables_by_kind_with_range` (private) |
| 1325 | `partition_invocables_by_kind` (private fn) |
| 1354 | `get_dashboard_stats` |
| 1485 | `get_dashboard_stats_with_range` |
| 1630 | `get_all_time_metrics` |
| 2083 | `get_session_count` |
| 2092 | `get_project_count` |
| 2102 | `get_commit_count` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use chrono::Utc;
use claude_view_core::{
    DashboardStats, DayActivity, ProjectStat, ProjectSummary,
    SessionDurationStat, SessionInfo, SessionsPage, SkillStat, ToolCounts,
};
use super::row_types::SessionRow;
use super::BranchCount;
```

#### `classification.rs` (19 methods)

| Line | Method |
|------|--------|
| 1714 | `create_classification_job` |
| 1740 | `get_active_classification_job` |
| 1750 | `update_classification_job_progress` |
| 1779 | `complete_classification_job` |
| 1803 | `cancel_classification_job` |
| 1821 | `fail_classification_job` |
| 1841 | `get_recent_classification_jobs` |
| 1855 | `create_index_run` |
| 1877 | `complete_index_run` |
| 1907 | `fail_index_run` |
| 1927 | `get_recent_index_runs` |
| 1938 | `get_unclassified_sessions` |
| 1959 | `get_all_sessions_for_classification` |
| 1978 | `count_unclassified_sessions` |
| 1988 | `count_all_sessions` |
| 1998 | `count_classified_sessions` |
| 2008 | `batch_update_session_classifications` |
| 2042 | `get_classification_job` |
| 2053 | `get_last_completed_classification_job` |
| 2063 | `recover_stale_classification_jobs` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use chrono::Utc;
use super::row_types::{ClassificationJobRow, IndexRunRow};
```

#### `system.rs` (10 methods)

| Line | Method |
|------|--------|
| 2110 | `get_oldest_session_date` |
| 2124 | `get_storage_counts` |
| 2143 | `get_database_size` |
| 2154 | `set_session_primary_model` |
| 2165 | `backfill_primary_models` |
| 2190 | `get_storage_stats` |
| 2223 | `get_health_stats` |
| 2269 | `calculate_health_status` (private fn) |
| 2295 | `get_classification_status` |
| 2377 | `reset_all_data` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use chrono::Utc;
use super::{StorageStats, HealthStats, HealthStatus, ClassificationStatus};
```

#### `ai_generation.rs` (1 method)

| Line | Method |
|------|--------|
| 2481 | `get_ai_generation_stats` |

**Imports needed:**
```rust
use crate::{Database, DbResult};
use super::{AIGenerationStats, TokensByModel, TokensByProject};
```

#### `row_types.rs` (3 structs + 5 impls + 4 free functions)

| Line | Item |
|------|------|
| 2624 | `struct ClassificationJobRow` + `FromRow` impl + `into_classification_job()` |
| 2685 | `struct IndexRunRow` + `FromRow` impl + `into_index_run()` |
| 3022 | `struct SessionRow` + `FromRow` impl + `into_session_info()` |
| 2742 | `pub fn update_session_deep_fields_tx()` |
| 2908 | `pub fn batch_insert_invocations_tx()` |
| 2938 | `pub fn batch_upsert_models_tx()` |
| 2973 | `pub fn batch_insert_turns_tx()` |

**Imports needed:**
```rust
use crate::DbResult;  // NOTE: Database is NOT needed — row_types has no impl Database blocks
use chrono::Utc;
use serde_json;
use sqlx::Row;
use claude_view_core::{
    parse_model_id, RawTurn, SessionInfo, ToolCounts,
    ClassificationJob, ClassificationJobStatus,
    IndexRun, IndexRunType, IndexRunStatus,
};
```

> **Audit note:** The original import list was missing 6 types from `claude_view_core` needed by `ClassificationJobRow::into_classification_job()` and `IndexRunRow::into_index_run()`, plus `serde_json` needed by `SessionRow::into_session_info()`. These were caught by the visibility audit agent. The compiler would also catch these immediately.

---

### Pre-Flight Checklist (run before starting Phase A)

```bash
# 1. Create backup branch (safety net for total rollback)
git checkout -b backup/pre-queries-refactor
git checkout -  # return to working branch

# 2. Record baseline test results
cargo test -p claude-view-db 2>&1 | tail -3  # save expected pass count

# 3. Record baseline line count
wc -l crates/db/src/queries.rs  # should be 4,562
```

### Line Count Verification (run after EACH domain extraction in A4–A10)

```bash
# After cutting code from mod.rs → {domain}.rs, verify no lines were lost:
git diff --stat crates/db/src/queries/
# Expected: "2 files changed, N insertions(+), N deletions(-)"
# If insertions != deletions, you lost or duplicated code — STOP and investigate
```

---

### Task A1: Atomic rename queries.rs → queries/mod.rs

**Files:**
- Rename: `crates/db/src/queries.rs` → `crates/db/src/queries/mod.rs`

**Step 1: Create directory and move file**

```bash
mkdir -p crates/db/src/queries
git mv crates/db/src/queries.rs crates/db/src/queries/mod.rs
```

**Step 2: Verify it compiles (zero changes to code)**

```bash
cargo check -p claude-view-db
```

Expected: Clean compile. Rust resolves `mod queries;` to `queries/mod.rs` identically.

**Step 3: Run tests**

```bash
cargo test -p claude-view-db -- queries 2>&1 | tail -5
```

Expected: All 29 queries tests pass.

**Step 4: Commit**

```bash
git add crates/db/src/queries/
git commit -m "refactor(db): rename queries.rs to queries/mod.rs (no code changes)"
```

### Task A2: Create row_types.rs (internal types + _tx helpers)

**Files:**
- Create: `crates/db/src/queries/row_types.rs`
- Modify: `crates/db/src/queries/mod.rs` (remove moved items, add `pub(crate) mod row_types;`)

**Step 1: Add module declaration to top of mod.rs**

Add after the existing `use` statements:

```rust
pub(crate) mod row_types;
```

**Step 2: Create row_types.rs**

Cut these blocks from `mod.rs` and paste into `row_types.rs`:
- `ClassificationJobRow` struct + `FromRow` impl + `into_classification_job` (lines 2624–2681)
- `IndexRunRow` struct + `FromRow` impl + `into_index_run` (lines 2685–2731)
- `SessionRow` struct + `FromRow` impl + `into_session_info` (lines 3022–3245)
- All 4 `_tx` free functions (lines 2742–3018)

Add at the top:

```rust
use crate::DbResult;
use chrono::Utc;
use sqlx::Row;
use claude_view_core::{parse_model_id, RawTurn, SessionInfo, ToolCounts};
```

Make all items `pub(crate)` — they're internal to the db crate.

**Step 3: Update references in mod.rs**

Internal calls to `_tx` functions become `row_types::batch_insert_invocations_tx(...)` etc.
Or add re-exports in mod.rs:

```rust
// Re-export _tx functions so indexer_parallel.rs path doesn't change
pub use row_types::{
    update_session_deep_fields_tx,
    batch_insert_invocations_tx,
    batch_upsert_models_tx,
    batch_insert_turns_tx,
};
```

This preserves `crate::queries::update_session_deep_fields_tx` for `indexer_parallel.rs`.

**Step 4: Verify compile + test**

```bash
cargo check -p claude-view-db && cargo test -p claude-view-db
```

**Step 5: Commit**

```bash
git add crates/db/src/queries/
git commit -m "refactor(db): extract row types and _tx helpers to queries/row_types.rs"
```

### Task A3: Extract tests to per-domain test files

> **Why per-domain?** A single `queries_test.rs` becomes a new merge conflict hotspot — every feature branch appends tests to the same file. Per-domain test files match the source split: branch A adds dashboard tests to `queries_dashboard_test.rs`, branch B adds classification tests to `queries_classification_test.rs` — zero conflict.

**Files:**
- Create: `crates/db/tests/queries_sessions_test.rs`
- Create: `crates/db/tests/queries_invocables_test.rs`
- Create: `crates/db/tests/queries_models_test.rs`
- Create: `crates/db/tests/queries_dashboard_test.rs`
- Create: `crates/db/tests/queries_classification_test.rs`
- Create: `crates/db/tests/queries_system_test.rs`
- Create: `crates/db/tests/queries_ai_generation_test.rs`
- Create: `crates/db/tests/queries_shared.rs` (shared `make_session` helper)
- Modify: `crates/db/src/queries/mod.rs` (remove `#[cfg(test)] mod tests` block, ~1,312 lines)

**Step 1: Move test block**

Cut the entire `#[cfg(test)] mod tests { ... }` block (lines 3250–4562) from mod.rs.

Distribute the 29 tests into domain-specific files. Each file uses the crate's public API:

```rust
//! Integration tests for Database {domain} query methods.

use claude_view_db::Database;
use claude_view_core::{SessionInfo, ToolCounts};

mod queries_shared;
use queries_shared::make_session;

// ... domain-specific test functions
```

The shared `make_session` helper goes in `queries_shared.rs`:

```rust
//! Shared test helpers for queries integration tests.

use claude_view_core::{SessionInfo, ToolCounts};

pub fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
    // ... exact copy from the original test block
}
```

Note: Integration tests use the crate's public API (`claude_view_db::Database`), not `super::`. Adjust any `super::` references to use the crate name.

**Step 2: Verify ALL test files pass**

```bash
cargo test -p claude-view-db 2>&1 | tail -10
```

Expected: All 29 tests pass across the domain-specific files.

**Step 3: Create safety tag (rollback point before domain splits)**

```bash
git tag refactor-queries-foundation
```

**Step 4: Commit**

```bash
git add crates/db/src/queries/mod.rs crates/db/tests/queries_*.rs
git commit -m "refactor(db): extract 29 queries tests to per-domain test files (~1,312 lines)

Distribute tests to match source domain split:
- queries_sessions_test.rs
- queries_invocables_test.rs
- queries_models_test.rs
- queries_dashboard_test.rs
- queries_classification_test.rs
- queries_system_test.rs
- queries_ai_generation_test.rs
- queries_shared.rs (make_session helper)

Per-domain test files prevent merge conflicts when two branches add tests."
```

### Tasks A4–A9: Move methods domain by domain

Each task follows the same pattern:

1. Create `crates/db/src/queries/{domain}.rs`
2. Add `mod {domain};` to `queries/mod.rs`
3. Cut the methods from `mod.rs`, paste into the new file
4. Add `use crate::{Database, DbResult};` + domain-specific imports
5. `cargo check -p claude-view-db && cargo test -p claude-view-db`
6. Commit

**Task A4:** Move 11 session methods → `sessions.rs`
**Task A5:** Move 5 invocable methods → `invocables.rs`
**Task A6:** Move 4 model/turn methods → `models.rs`
**Task A7:** Move 12 dashboard methods → `dashboard.rs`
**Task A8:** Move 19 classification methods → `classification.rs`
**Task A9:** Move 10 system methods (8 pub + 1 private fn + 1 private method) → `system.rs`
**Task A10:** Move 1 AI generation method → `ai_generation.rs`

### Task A10.5: Extract types to queries/types.rs

> **Why:** Reduces `mod.rs` from ~350 lines → ~50 lines. New types go into `types.rs` (append-only, isolated), not `mod.rs` (which holds module wiring). This eliminates the last conflict hotspot.

**Files:**
- Create: `crates/db/src/queries/types.rs`
- Modify: `crates/db/src/queries/mod.rs` (move 13 type definitions out, add `mod types; pub use types::*;`)

**Step 1: Create types.rs**

Move these 13 types from `mod.rs` into `types.rs`:
- `BranchCount`, `IndexerEntry`, `InvocableWithCount`, `ModelWithStats`
- `TokenStats`, `TokensByModel`, `TokensByProject`, `AIGenerationStats`
- `StorageStats`, `HealthStats`, `HealthStatus`, `ClassificationStatus`, `StatsOverview`

Include their derive macros and any associated impls.

**Step 2: Update mod.rs**

Replace the type definitions with:
```rust
mod types;
pub use types::*;
```

**Step 3: Verify lib.rs re-exports still work**

```bash
cargo check -p claude-view-db && cargo check -p claude-view-server
```

The `pub use queries::BranchCount` in `lib.rs` resolves through: `queries/mod.rs` → `pub use types::*` → `types.rs::BranchCount`. No changes needed to `lib.rs`.

**Step 4: Commit**

```bash
git add crates/db/src/queries/
git commit -m "refactor(db): extract 13 public types to queries/types.rs

Reduces mod.rs from ~350 lines → ~50 lines (module declarations only).
New types are added to types.rs, not mod.rs — avoids conflict hotspot."
```

### Task A11: Final cleanup and full verification

**Step 1: Verify mod.rs is now just module declarations + re-exports**

After all moves, `mod.rs` should contain only:
- Module declarations (`mod sessions;`, `mod dashboard;`, `mod types;`, etc.)
- `pub use types::*;` (re-export all public types)
- Re-exports for `_tx` functions
- Approximately **50 lines total**

**Step 2: Full verification**

```bash
# Compile all dependent crates
cargo check -p claude-view-db
cargo check -p claude-view-server

# Run ALL db tests (unit + integration)
cargo test -p claude-view-db

# Run server tests
cargo test -p claude-view-server

# Clippy
cargo clippy -p claude-view-db -- -D warnings
```

**Step 3: Final commit**

```bash
git add crates/db/src/queries/ crates/db/tests/queries_*.rs
git commit -m "refactor(db): complete queries.rs split — 4,562 lines → 10 focused modules

queries/mod.rs:            ~50 lines (module declarations + re-exports only)
queries/types.rs:          ~250 lines (13 public types)
queries/sessions.rs:       ~580 lines (11 methods)
queries/invocables.rs:     ~200 lines (5 methods)
queries/models.rs:         ~140 lines (4 methods)
queries/dashboard.rs:      ~640 lines (12 methods)
queries/classification.rs: ~370 lines (19 methods)
queries/system.rs:         ~400 lines (10 methods)
queries/ai_generation.rs:  ~140 lines (1 method)
queries/row_types.rs:      ~420 lines (3 row types + 4 _tx helpers)
tests/queries_*_test.rs:  ~1,312 lines (29 tests across 7 domain files)

Zero behavior change. All existing tests pass."
```

---

## Phase B: Extract migrations.rs tests (1,528 → ~500 + ~1,000) — DEFERRED

> **Status: Deferred.** The audit found that `pub use migrations::MIGRATIONS` leaks internal SQL as public API. Migration tests are rarely edited concurrently, so the ROI of extracting them is low. Revisit only if migrations.rs becomes a conflict hotspot.

### Task B1: Move migration tests to integration test file

**Files:**
- Create: `crates/db/tests/migration_test.rs`
- Modify: `crates/db/src/migrations.rs` (remove `#[cfg(test)] mod tests`)
- Modify: `crates/db/src/lib.rs` (add `pub use migrations::MIGRATIONS;`)

**Step 1: Make MIGRATIONS publicly accessible**

Add to `crates/db/src/lib.rs`:
```rust
pub use migrations::MIGRATIONS;
```

Change `crates/db/src/migrations.rs`:
```rust
// Was: pub const MIGRATIONS (already pub, but module was private)
// The module is `mod migrations;` (private), so we need the re-export in lib.rs
```

**Step 2: Create integration test file**

Move `#[cfg(test)] mod tests { ... }` (lines 509–1528) to `crates/db/tests/migration_test.rs`.

Replace `super::MIGRATIONS` with `claude_view_db::MIGRATIONS`.

Replace `setup_db()` helper — it manually runs migrations, so it needs access to `MIGRATIONS`. With the re-export, use `claude_view_db::MIGRATIONS`.

**Step 3: Verify**

```bash
cargo test -p claude-view-db --test migration_test 2>&1 | tail -10
```

**Step 4: Commit**

```bash
git add crates/db/
git commit -m "refactor(db): extract migration tests to crates/db/tests/migration_test.rs (~1,000 lines)"
```

---

## Phase C: Split `indexer_parallel.rs` (3,524 → 4 files) — DRAFT, NEEDS AUDIT

### Target structure

```
crates/db/src/indexer_parallel/
├── mod.rs        # Re-exports, CURRENT_PARSE_VERSION, SQL constants
├── types.rs      # ParsedSession, ParseDiagnostics, FileData, TimestampValue
├── parser.rs     # parse_bytes(), SIMD scanning, JSONL line extraction
└── pipeline.rs   # pass_1_read_indexes, pass_2_deep_index, run_background_index
```

Extract ~1,200 lines of tests to `crates/db/tests/indexer_parallel_test.rs`.

### Tasks C1–C4

Same atomic pattern: `git mv` → scaffold → move domain → compile → test → commit.

> **Status: Draft.** This phase lacks method-to-file mappings, exact line numbers, and import lists. Requires the same level of audit as Phase A before execution. Test line count is ~2,200 (not ~1,200 as originally estimated).

---

## Phase D: Split `git_correlation.rs` (3,342 → 5 files) — DRAFT, NEEDS AUDIT

### Target structure

```
crates/db/src/git_correlation/
├── mod.rs        # Re-exports, shared types
├── scan.rs       # scan_repo_commits, get_commit_diff_stats, DiffStats
├── matching.rs   # tier1_match, tier2_match, correlate_session
├── sync.rs       # run_git_sync, get_sessions_for_git_sync
└── db.rs         # impl Database: batch_upsert_commits, update_diff_stats
```

Extract ~2,025 lines of tests to `crates/db/tests/git_correlation_test.rs`.

> **Status: Draft.** This phase lacks method-to-file mappings, exact line numbers, and import lists. Requires the same level of audit as Phase A before execution. Test line count is ~2,025 (not ~1,000 as originally estimated).

---

## Phase E: Split `snapshots.rs` (3,282 → 4 files) — DRAFT, NEEDS AUDIT

### Target structure

```
crates/db/src/snapshots/
├── mod.rs          # Re-exports, TimeRange, shared types
├── queries.rs      # get_aggregated_contributions, trends, breakdowns
├── generation.rs   # generate_daily_snapshot, generate_missing_snapshots
└── metrics.rs      # get_reedit_rate, get_commit_rate, get_total_prompts
```

Extract ~500 lines of tests to `crates/db/tests/snapshots_test.rs`.

> **Status: Draft.** This phase lacks method-to-file mappings, exact line numbers, and import lists. Requires the same level of audit as Phase A before execution.

---

## Execution Order

```
Phase A (queries.rs)     ← DO FIRST, highest value
  └── Phase B (migrations.rs)  ← quick win, 15 min
      ├── Phase C (indexer_parallel.rs)  ← independent
      ├── Phase D (git_correlation.rs)   ← independent
      └── Phase E (snapshots.rs)         ← independent
```

Phases C, D, E can be parallelized across sessions.

## Verification Checklist (after each phase)

```bash
cargo check -p claude-view-db        # db crate compiles
cargo check -p claude-view-server    # server crate compiles (depends on db)
cargo test -p claude-view-db         # all db tests pass
cargo test -p claude-view-server     # all server tests pass
cargo clippy -p claude-view-db -- -D warnings  # no new warnings
```

## Import Strategy

All 67 query methods remain `impl Database` methods. **Consuming code does NOT need import changes** — `db.method_name()` works identically before and after the split. Rust merges `impl Database` blocks from all sub-files at compile time.

Only **types** are re-exported from `queries/mod.rs` via `pub use types::*;` to preserve existing `use claude_view_db::BranchCount` paths in `lib.rs`.

The 4 `_tx` free functions are re-exported from `queries/mod.rs` via `pub use row_types::{...}` to preserve `crate::queries::function_name` paths used by `indexer_parallel.rs`.

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| Visibility errors (`pub` vs `pub(crate)`) | High | Compiler catches immediately. Fix iteratively. |
| Import path breaks in `indexer_parallel.rs` | Medium | Re-export `_tx` functions from `queries/mod.rs` to preserve paths |
| Test helper `make_session()` not accessible | Low | Lives in `queries_shared.rs`, imported by all domain test files |
| `SessionRow` needed by both `sessions.rs` and `dashboard.rs` | Certain | Lives in `row_types.rs`, both import via `super::row_types::SessionRow` |
| Circular imports between sub-modules | Zero | All sub-modules depend on `mod.rs` types only, never on each other |
| Each commit is independently revertable | By design | `git revert <hash>` for any single step |
| Lines lost during manual cut/paste | Medium | **Line count verification** after each extraction (see pre-flight checklist) |
| Missing imports in sub-files | Medium | **Compiler catches immediately** — 8 missing imports were pre-identified and fixed in this plan |

## Rollback Strategy

| Scenario | Command | Result |
|----------|---------|--------|
| Total abort (undo everything) | `git reset --hard backup/pre-queries-refactor` | Back to original state |
| Abort after test extraction | `git reset --hard refactor-queries-foundation` | Back to after A3 |
| Undo one domain extraction | `git revert <commit-hash>` | Methods return to mod.rs |
| Rebase conflict too complex | `git rebase --abort` | Back to pre-rebase state |
