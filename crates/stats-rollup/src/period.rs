//! Per-bucket aggregate view over a window of `SessionStats`.
//!
//! `PeriodStats` is the output shape of Stage C (`rollup`) and the carrier
//! type of `merge`. Every numeric field forms a commutative monoid under
//! addition with `EMPTY` as identity — this is what `merge` relies on.
//!
//! Phase 1 fields populated from `SessionStats` alone:
//!   * `session_count`, `total_tokens`, `prompt_count`, `file_count`,
//!     `duration_sum_ms`, `duration_count`
//!
//! Phase 4 fields filled from `SessionFlags` via `RollupInput { stats, flags }`:
//!   * `total_cost_cents`, `lines_added`, `lines_removed`, `commit_count`,
//!     `reedit_rate_sum`, `reedit_rate_count`
//!
//! Rationale for the `_sum` + `_count` pair on reedit rate: averages aren't
//! associative, but *sum-of-rates paired with count* is — callers compute
//! `mean = sum / count` only at the display boundary.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

use serde::{Deserialize, Serialize};

/// Aggregate window of session metrics.
///
/// All numeric fields are non-negative counters and therefore form a
/// commutative monoid under addition. `PeriodStats::EMPTY` is the
/// identity element — `merge(&a, &EMPTY) == a` for every `a`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeriodStats {
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_cost_cents: u64,
    pub prompt_count: u64,
    pub file_count: u64,
    /// Phase 4: filled from `SessionFlags`. Stays 0 in Phase 1.
    pub lines_added: u64,
    /// Phase 4: filled from `SessionFlags`. Stays 0 in Phase 1.
    pub lines_removed: u64,
    /// Phase 4: filled from `SessionFlags`. Stays 0 in Phase 1.
    pub commit_count: u64,
    pub duration_sum_ms: u64,
    pub duration_count: u64,
    /// Phase 4: running sum of per-session reedit ratios. Display layer
    /// computes `reedit_rate_sum / reedit_rate_count` as the mean.
    pub reedit_rate_sum: f64,
    /// Phase 4: count of sessions that contributed to `reedit_rate_sum`.
    pub reedit_rate_count: u64,
}

impl PeriodStats {
    /// Identity element of the `merge` monoid. All numeric fields zero.
    pub const EMPTY: Self = Self {
        session_count: 0,
        total_tokens: 0,
        total_cost_cents: 0,
        prompt_count: 0,
        file_count: 0,
        lines_added: 0,
        lines_removed: 0,
        commit_count: 0,
        duration_sum_ms: 0,
        duration_count: 0,
        reedit_rate_sum: 0.0,
        reedit_rate_count: 0,
    };
}
