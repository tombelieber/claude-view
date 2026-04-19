//! Phase 1 — associative-monoid merge primitives over session windows.
//! Phase 4 — `StatsCore` + `#[derive(RollupTable)]` expansion into 15
//! typed rollup structs, `CREATE TABLE` statements, and sqlx I/O fns.
//!
//! Version types (`RollupVersion`, `ROLLUP_VERSION`) are re-exported
//! from `claude-view-session-parser` so there's exactly one canonical
//! ROLLUP_VERSION in the workspace.
//!
//! Bumping `ROLLUP_VERSION` there forces rollup-only recompute without
//! re-running Stages A/B.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md
//! §2.2 (Phase 1 scaffold) + §6.2 (Phase 4 macro expansion)`.

pub mod bucket;
pub mod dashboard;
pub mod merge;
pub mod period;
pub mod rollup;
pub mod stats_core;

pub use bucket::Bucket;
pub use claude_view_session_parser::{RollupVersion, ROLLUP_VERSION};
pub use dashboard::sum_global_stats_in_range;
pub use merge::merge;
pub use period::PeriodStats;
pub use rollup::rollup;
pub use stats_core::StatsCore;
