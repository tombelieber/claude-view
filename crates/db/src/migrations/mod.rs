//! Inline SQL migrations for the claude-view database schema.
//!
//! We use simple inline migrations rather than sqlx migration files
//! because the schema is small and self-contained.
//!
//! Split into themed sub-modules in CQRS Phase 2 PR 2.0 (per project
//! "decompose, don't monolith" rule — the previous single-file
//! `migrations.rs` was 2,670 lines, well past the 600-line hard stop):
//!
//! | Sub-module     | Migrations | Purpose                                                              |
//! |----------------|-----------:|----------------------------------------------------------------------|
//! | [`core`]       | 1–20       | Schema bootstrap (sessions, indexer_state, turns, theme-4 foundation)|
//! | [`indexer`]    | 21–48      | Facets, fluency, hooks, reports, derived session fields, integrity   |
//! | [`features`]   | 49–63      | Archive, hooks dedup, model catalog, entrypoint, CQRS Phase 0 drops  |
//! | [`rollups`]    | (empty)    | Stub — populated by `stats-rollup-derive` in CQRS Phase 4            |
//!
//! Migrations are applied in order via `migrations()` below. The runner
//! in `lib.rs` enumerates with `(i, sql) in migrations().iter().enumerate()`
//! and uses `version = i + 1`. Adding a new migration: append to the
//! appropriate sub-module's `MIGRATIONS` array (or to `rollups::MIGRATIONS`
//! once Phase 4 lands the derive macro). NEVER insert in the middle.

mod core;
mod features;
mod indexer;
mod rollups;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

/// Canonical migration sequence in apply order.
///
/// Version N corresponds to index N-1 in the returned slice. Concatenates
/// the four sub-module slices on first call; cached for the process
/// lifetime via `OnceLock`.
///
/// Order across sub-modules MUST be preserved: `core` → `indexer` →
/// `features` → `rollups`. Any new migration is appended to the
/// appropriate sub-module's `MIGRATIONS` array; the trailing module
/// (currently `rollups`) is the only one that may grow.
pub fn migrations() -> &'static [&'static str] {
    static ALL: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v = Vec::with_capacity(
            core::MIGRATIONS.len()
                + indexer::MIGRATIONS.len()
                + features::MIGRATIONS.len()
                + rollups::MIGRATIONS.len(),
        );
        v.extend_from_slice(core::MIGRATIONS);
        v.extend_from_slice(indexer::MIGRATIONS);
        v.extend_from_slice(features::MIGRATIONS);
        v.extend_from_slice(rollups::MIGRATIONS);
        v
    })
}
