# TODO: Refactor Large Files

> Tracked: 2026-02-23. 20 files over 1,000 lines.

## Priority Targets (db crate — top 3)

| File | Lines | Notes |
|------|------:|-------|
| `crates/db/src/indexer_parallel.rs` | 4,562 | Largest file in codebase. Split indexing phases into submodules. |
| `crates/db/src/snapshots.rs` | 3,247 | Snapshot logic could separate read/write/diff concerns. |
| `crates/db/src/git_correlation.rs` | 3,159 | Git analysis logic could split by correlation strategy. |

## Secondary Targets

| File | Lines | Crate | Notes |
|------|------:|-------|-------|
| `crates/server/src/routes/insights.rs` | 2,080 | server | Split insight endpoints into per-category modules. |
| `crates/server/src/routes/terminal.rs` | 1,823 | server | |
| `crates/db/src/migrations.rs` | 1,785 | db | Naturally large (SQL). Low urgency. |
| `crates/core/src/discovery.rs` | 1,770 | core | |
| `crates/server/src/live/manager.rs` | 1,715 | server | |
| `crates/core/src/live_parser.rs` | 1,659 | core | |
| `crates/server/src/routes/sessions.rs` | 1,531 | server | |
| `crates/server/src/routes/hooks.rs` | 1,462 | server | |
| `crates/server/src/routes/stats.rs` | 1,363 | server | |
| `crates/core/src/parser.rs` | 1,325 | core | |
| `crates/core/src/accumulator.rs` | 1,287 | core | |
| `crates/core/src/types.rs` | 1,223 | core | Naturally large (type defs). Low urgency. |
| `src/lib/export-html.ts` | 1,206 | frontend | |
| `crates/core/src/registry.rs` | 1,144 | core | |
| `crates/db/src/queries/dashboard.rs` | 1,132 | db | |
| `crates/search/src/lib.rs` | 1,058 | search | |
| `crates/server/src/routes/system.rs` | 1,038 | server | |

## By Crate

- **db** (5 files) — worst offenders, start here
- **server** (6 files) — route files can split into submodules
- **core** (6 files) — `migrations.rs` and `types.rs` are naturally large
- **search** (1 file)
- **frontend** (1 file)
