//! Pointwise-addition merge primitive over `PeriodStats`.
//!
//! Associative + commutative for all integer fields (machine-integer
//! addition in `u64` wrap-free domain — realistic cost/token counts never
//! approach `u64::MAX`). The `reedit_rate_sum: f64` field is addition-
//! based too; IEEE-754 double-precision addition is *nearly* but not
//! strictly associative in general, however the proptest generator in
//! `tests/associativity.rs` keeps the value range bounded such that
//! every sum lies within the f64 exact-add region.
//!
//! `EMPTY` is the identity: `merge(&a, &EMPTY) == a` for every `a`.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

use crate::period::PeriodStats;

/// Pointwise sum of two `PeriodStats` windows.
///
/// Forms a commutative monoid with `PeriodStats::EMPTY` as identity.
/// Proof lives in `tests/associativity.rs` as proptest property laws
/// (1000 random cases each for associativity, commutativity, identity).
pub fn merge(a: &PeriodStats, b: &PeriodStats) -> PeriodStats {
    PeriodStats {
        session_count: a.session_count + b.session_count,
        total_tokens: a.total_tokens + b.total_tokens,
        total_cost_cents: a.total_cost_cents + b.total_cost_cents,
        prompt_count: a.prompt_count + b.prompt_count,
        file_count: a.file_count + b.file_count,
        lines_added: a.lines_added + b.lines_added,
        lines_removed: a.lines_removed + b.lines_removed,
        commit_count: a.commit_count + b.commit_count,
        duration_sum_ms: a.duration_sum_ms + b.duration_sum_ms,
        duration_count: a.duration_count + b.duration_count,
        reedit_rate_sum: a.reedit_rate_sum + b.reedit_rate_sum,
        reedit_rate_count: a.reedit_rate_count + b.reedit_rate_count,
    }
}
