//! `StatsCore` — the Phase 4 rollup-shape declaration.
//!
//! This struct is the **single source of truth** for every numeric stat
//! that flows through a rollup table. `#[derive(RollupTable)]` expands
//! it into 15 typed rollup structs (5 dimensions × 3 buckets), 15
//! `CREATE TABLE` strings, and 15 × 3 I/O functions — see
//! `crates/stats-rollup-derive/src/codegen.rs`.
//!
//! Adding or removing a field here is a cross-cutting change:
//!   - `PeriodStats` in `period.rs` must be kept in sync (Phase 5 will
//!     fold the two types together; until then the merge() primitive
//!     operates on `PeriodStats` and Stage C maps at the boundary).
//!   - `associativity.rs` proptest updates to cover the new field.
//!   - Migrations must bump (new column) — the macro emits new SQL but
//!     the migration landing has to happen too.
//!
//! The reason averages use a `_sum / _count` pair: averages aren't
//! associative under pointwise merge, but `sum-of-values + count` is.
//! Display layer computes `mean = sum / count` at the boundary.

use claude_view_stats_rollup_derive::RollupTable;

/// Central rollup-shape declaration. See module doc.
#[derive(Debug, Clone, RollupTable)]
#[rollup(buckets = [daily, weekly, monthly])]
#[rollup(dimensions = [
    global,
    project(project_id: TEXT),
    branch(project_id: TEXT, branch: TEXT),
    model(model_id: TEXT),
    category(category_l1: TEXT),
])]
pub struct StatsCore {
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_cost_cents: u64,
    pub prompt_count: u64,
    pub file_count: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub commit_count: u64,
    /// D6 contribution_snapshots fold — total insertions across commits
    /// attributed to this window.
    pub commit_insertions: u64,
    /// D6 contribution_snapshots fold — total deletions across commits
    /// attributed to this window.
    pub commit_deletions: u64,
    /// Sum of per-session durations in milliseconds. Paired with
    /// `duration_count` so the display layer can compute mean without
    /// breaking associativity.
    pub duration_sum_ms: u64,
    /// Number of sessions contributing to `duration_sum_ms`.
    pub duration_count: u64,
    /// Sum of per-session reedit-ratio values. Paired with
    /// `reedit_rate_count`.
    pub reedit_rate_sum: f64,
    /// Number of sessions contributing to `reedit_rate_sum`.
    pub reedit_rate_count: u64,
}

impl StatsCore {
    /// Number of numeric fields on `StatsCore`. Hand-maintained because
    /// `size_of::<StatsCore>() / 8` would be brittle across layout
    /// changes. This constant drives the associativity proptest's
    /// field-coverage gate and can be verified in tests.
    pub const FIELD_COUNT: usize = 14;
}
