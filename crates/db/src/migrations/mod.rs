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
//! | [`features`]   | 49–66      | Archive, hooks dedup, model catalog, entrypoint, CQRS Phase 0–3 DDL  |
//! | [`rollups`]    | 67–81      | 15 typed rollup tables (CQRS Phase 4, proc-macro generated)          |
//! | [`events`]     | 82–        | Event-sourcing tables (CQRS Phase 5: `session_action_log`)           |
//!
//! Migrations are applied in order via `migrations()` below. The runner
//! in `lib.rs` enumerates with `(i, sql) in migrations().iter().enumerate()`
//! and uses `version = i + 1`. Adding a new migration: append to the
//! current **trailing** sub-module's `MIGRATIONS` array (as of Phase 5,
//! that is `events`). If a new phase needs a semantically distinct sub-
//! module, create it AFTER the current trailing module and update the
//! concat + doc table below. NEVER insert in the middle — prod databases
//! track applied versions by index, and a shift would reapply migrations
//! out of order.

mod core;
mod events;
mod features;
mod indexer;
mod rollups;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

/// Canonical migration sequence in apply order.
///
/// Version N corresponds to index N-1 in the returned slice. Concatenates
/// the sub-module slices on first call; cached for the process lifetime
/// via `OnceLock`.
///
/// Order across sub-modules MUST be preserved: `core` → `indexer` →
/// `features` → `rollups` → `events`. Any new migration is appended to
/// the trailing sub-module's `MIGRATIONS` array; the trailing module
/// (currently `events`) is the only one that may grow. If a Phase 6+
/// sub-module ships, append it AFTER `events` and update this comment.
pub fn migrations() -> &'static [&'static str] {
    static ALL: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v = Vec::with_capacity(
            core::MIGRATIONS.len()
                + indexer::MIGRATIONS.len()
                + features::MIGRATIONS.len()
                + rollups::MIGRATIONS.len()
                + events::MIGRATIONS.len(),
        );
        v.extend_from_slice(core::MIGRATIONS);
        v.extend_from_slice(indexer::MIGRATIONS);
        v.extend_from_slice(features::MIGRATIONS);
        v.extend_from_slice(rollups::MIGRATIONS);
        v.extend_from_slice(events::MIGRATIONS);
        v
    })
}
