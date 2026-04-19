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

    /// Zero-valued `StatsCore` — the identity under pointwise sum.
    pub const ZERO: Self = Self {
        session_count: 0,
        total_tokens: 0,
        total_cost_cents: 0,
        prompt_count: 0,
        file_count: 0,
        lines_added: 0,
        lines_removed: 0,
        commit_count: 0,
        commit_insertions: 0,
        commit_deletions: 0,
        duration_sum_ms: 0,
        duration_count: 0,
        reedit_rate_sum: 0.0,
        reedit_rate_count: 0,
    };

    /// Compute the delta `StatsCore` to apply to rollup tables for a
    /// `SessionStats` observation, given the previous observation for
    /// the same session (if any).
    ///
    /// When `old` is `None` (first observation), every field is the
    /// full absolute value from `new`. `session_count = 1` (a new
    /// session joins the rollup).
    ///
    /// When `old` is `Some` (re-indexed session), each cumulative field
    /// is `new - old` via `saturating_sub` — producing zero if `new`
    /// regressed (shouldn't happen in practice, but guards against
    /// parser bugs). `session_count = 0` so the same session isn't
    /// double-counted across observations.
    ///
    /// Phase 4 fields populated from `SessionFlags` stay at 0 here;
    /// Phase 5's flag-fold writer emits a separate `FlagDelta` that
    /// Stage C applies on top.
    pub fn delta_from(
        new: &claude_view_core::session_stats::SessionStats,
        old: Option<&claude_view_core::session_stats::SessionStats>,
    ) -> Self {
        let new_total_tokens = total_tokens(new);
        let (
            session_count,
            total_tokens_delta,
            prompt_count_delta,
            duration_ms_delta,
            duration_count_delta,
        ) = match old {
            None => (
                1u64,
                new_total_tokens,
                new.user_prompt_count as u64,
                (new.duration_seconds as u64) * 1000,
                if new.duration_seconds > 0 { 1 } else { 0 },
            ),
            Some(o) => {
                let old_total_tokens = total_tokens(o);
                (
                    0u64,
                    new_total_tokens.saturating_sub(old_total_tokens),
                    (new.user_prompt_count as u64).saturating_sub(o.user_prompt_count as u64),
                    ((new.duration_seconds as u64) * 1000)
                        .saturating_sub((o.duration_seconds as u64) * 1000),
                    // `duration_count` counts sessions that
                    // contributed >0 duration — not incremental.
                    // First emit only (matches session_count).
                    0,
                )
            }
        };

        Self {
            session_count,
            total_tokens: total_tokens_delta,
            // total_cost_cents needs pricing lookup — Phase 4 skeleton
            // leaves this 0; Phase 4b cost-wiring fills it when the
            // per-model pricing table is plumbed.
            total_cost_cents: 0,
            prompt_count: prompt_count_delta,
            // file_count + lines_*, commit_*, reedit_* come from
            // Phase 5 SessionFlags fold — stay 0 in Phase 4.
            file_count: 0,
            lines_added: 0,
            lines_removed: 0,
            commit_count: 0,
            commit_insertions: 0,
            commit_deletions: 0,
            duration_sum_ms: duration_ms_delta,
            duration_count: duration_count_delta,
            reedit_rate_sum: 0.0,
            reedit_rate_count: 0,
        }
    }
}

/// Sum of every token-carrying field on `SessionStats`. Helper to keep
/// Phase 4's rollup arithmetic in one place — if a new token type
/// lands on the parser side, update it here.
fn total_tokens(s: &claude_view_core::session_stats::SessionStats) -> u64 {
    s.total_input_tokens + s.total_output_tokens + s.cache_read_tokens + s.cache_creation_tokens
}
