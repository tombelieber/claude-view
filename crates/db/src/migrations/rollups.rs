//! Rollup-table migrations (empty stub — populated by
//! `claude_view_stats_rollup_derive` in CQRS Phase 4).
//!
//! When Phase 4 lands the proc-macro, `#[derive(RollupTable)]` on each
//! rollup struct will append migrations here in apply order. Until then
//! this slice is intentionally empty so `migrations()` still returns the
//! pre-Phase-4 sequence verbatim.

pub const MIGRATIONS: &[&str] = &[];
