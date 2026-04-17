//! Rollup time-bucket granularity.
//!
//! PR 1.3 accepts this as a `rollup()` argument but does not yet branch on
//! it — Phase 1 aggregates are bucket-agnostic (pointwise sum). Phase 4
//! consumes the `Bucket` when rollup writers need to decide per-row
//! boundaries (e.g. midnight UTC vs Monday-start-of-week) to route each
//! `SessionStats` into the correct pre-aggregated row.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

/// Rollup bucket granularity.
///
/// Ordered from narrowest (`Daily`) to widest (`Monthly`). Consumers must
/// not assume `Monthly` == 30×`Daily` — calendar-month length varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Daily,
    Weekly,
    Monthly,
}
