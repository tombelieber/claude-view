//! Shared helpers for rollup-backed insights endpoints (PR 4.5).
//!
//! Both `/api/insights/models` and `/api/insights/projects` follow the
//! same shape:
//!
//! 1. Resolve the `[from, to)` range (reuses the existing time-range
//!    resolver).
//! 2. Parse the bucket (daily/weekly/monthly) with a safe default.
//! 3. Scan the appropriate rollup table and aggregate by dim key in
//!    memory.
//! 4. Sort descending by `total_tokens` + clamp to `limit`.
//!
//! This module owns the bucket parsing so the handlers stay focused on
//! their dim-specific aggregation. The legacy GROUP BY escape hatch
//! (`CLAUDE_VIEW_USE_LEGACY_STATS_READ`) was retired in CQRS Phase 7.g
//! once rollup coverage was complete.

use claude_view_core::EffectiveRangeMeta;
use claude_view_stats_rollup::Bucket;

/// Hard cap on rows returned after sort + limit. Bounds response size
/// irrespective of input `limit` — a client asking for `limit=100_000`
/// can't force a 100k-row JSON payload.
pub const ROWS_RETURNED_CAP: usize = 500;

/// Default `limit` when the client omits it. Keeps small payloads the
/// common case; UI typically shows <= 20 rows.
pub const DEFAULT_LIMIT: usize = 100;

/// Parse the `bucket` query param to a `Bucket`. Accepts case-
/// insensitive variants; defaults to `Daily`. Unknown values are
/// coerced to `Daily` rather than 400 — §13 endpoints never fail
/// purely because a client passed a typo.
pub fn parse_bucket(raw: Option<&str>) -> Bucket {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("weekly") => Bucket::Weekly,
        Some("monthly") => Bucket::Monthly,
        _ => Bucket::Daily,
    }
}

/// Label a `Bucket` back to its serialised form for the response
/// metadata.
pub const fn bucket_label(b: Bucket) -> &'static str {
    match b {
        Bucket::Daily => "daily",
        Bucket::Weekly => "weekly",
        Bucket::Monthly => "monthly",
    }
}

/// Common shape of the per-dimension aggregate. Handlers fold typed
/// rollup rows into this and then serialise into their public
/// response types.
#[derive(Debug, Clone, Copy, Default)]
pub struct DimAggregate {
    pub session_count: u64,
    pub total_tokens: u64,
    pub prompt_count: u64,
    pub duration_sum_ms: u64,
    pub duration_count: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub commit_count: u64,
}

impl DimAggregate {
    /// Mean duration in seconds. Matches the display-layer convention
    /// for every rollup-shape `_sum` / `_count` pair: the boundary
    /// layer divides; associativity stays pure inside the merge.
    pub fn avg_duration_seconds(&self) -> f64 {
        if self.duration_count == 0 {
            0.0
        } else {
            (self.duration_sum_ms as f64) / (self.duration_count as f64) / 1000.0
        }
    }
}

/// Cap a caller-supplied `limit` into `[1, ROWS_RETURNED_CAP]` with
/// `DEFAULT_LIMIT` when absent.
pub fn clamp_limit(input: Option<u32>) -> usize {
    let requested = input.unwrap_or(DEFAULT_LIMIT as u32) as usize;
    requested.clamp(1, ROWS_RETURNED_CAP)
}

/// Resolve the `[from, to)` range for a rollup read. Helpers in
/// `routes::insights::*` use this to keep the time-range plumbing in
/// one place — the handler body then only needs to think about the
/// dim aggregation.
pub fn resolved_range_to_unix(range: &EffectiveRangeMeta) -> (i64, i64) {
    // Rollup queries use half-open `[start, end)` — our callers treat
    // `to` as inclusive-end like the legacy shape, so add one second
    // so the last-day rollup bucket is not clipped.
    (range.from, range.to + 1)
}
