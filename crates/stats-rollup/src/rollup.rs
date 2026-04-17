//! Stage C: aggregate a window of `SessionStats` into a `PeriodStats`.
//!
//! Phase 1 signature takes `&[&SessionStats]`. Phase 4 will extend to
//! `&[RollupInput<'_> { stats, flags }]` once `SessionFlags` lands and
//! cost / lines / commits / reedit fields become derivable.
//!
//! Current derivable fields (from `SessionStats` alone):
//!   * `session_count` = `items.len()`
//!   * `total_tokens` = Σ (input + output + cache_read + cache_creation)
//!   * `prompt_count` = Σ `user_prompt_count`
//!   * `file_count`   = Σ `files_edited_count`
//!   * `duration_sum_ms` = Σ (`duration_seconds` × 1000)
//!   * `duration_count`  = `items.len()`
//!
//! Phase 4 fields stay zero — see `period.rs` docstring.
//!
//! The `_bucket` arg is currently unused. Phase 4 uses it to route each
//! session to the correct pre-aggregated row when rollup writers persist
//! per-day / per-week / per-month buckets; at the `rollup()` layer the
//! aggregate semantics are bucket-agnostic pointwise sum.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

use claude_view_core::session_stats::SessionStats;
use claude_view_session_parser::RollupVersion;

use crate::bucket::Bucket;
use crate::period::PeriodStats;

/// Aggregate a window of sessions into a `PeriodStats`.
///
/// Phase 1 impl; see module docstring for which fields are populated.
pub fn rollup(items: &[&SessionStats], _rollup_v: RollupVersion, _bucket: Bucket) -> PeriodStats {
    let mut acc = PeriodStats::EMPTY;
    for s in items {
        acc.session_count += 1;
        acc.total_tokens += s.total_input_tokens
            + s.total_output_tokens
            + s.cache_read_tokens
            + s.cache_creation_tokens;
        acc.prompt_count += u64::from(s.user_prompt_count);
        acc.file_count += u64::from(s.files_edited_count);
        acc.duration_sum_ms += u64::from(s.duration_seconds) * 1000;
        acc.duration_count += 1;
    }
    acc
}
