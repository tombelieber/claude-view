//! CQRS Phase 1 rollup crate — Stage C aggregation + associative-monoid merge primitives.
//!
//! PR 1.3: Phase 1 impl takes `&[&SessionStats]`. Phase 4 extends to
//! `&[RollupInput<'_> { stats, flags }]` once `SessionFlags` arrives.
//! Do NOT introduce a new `SessionStats` type; reuse the one from
//! `claude-view-core` (re-exported via `claude-view-session-parser`).
//!
//! Version types (`RollupVersion`, `ROLLUP_VERSION`) are re-exported from
//! `claude-view-session-parser` to keep a single source of truth. Bumping
//! `ROLLUP_VERSION` there forces rollup-only recompute without re-running
//! Stages A/B.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

pub mod bucket;
pub mod merge;
pub mod period;
pub mod rollup;

pub use bucket::Bucket;
pub use claude_view_session_parser::{RollupVersion, ROLLUP_VERSION};
pub use merge::merge;
pub use period::PeriodStats;
pub use rollup::rollup;
