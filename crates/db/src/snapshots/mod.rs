// crates/db/src/snapshots/mod.rs
//! Contribution snapshot queries and aggregation.
//!
//! This module provides:
//! - Snapshot CRUD operations
//! - Time range aggregation queries
//! - Daily snapshot generation (for the nightly job)
//!
//! ## Snapshot Table Schema
//!
//! The `contribution_snapshots` table stores pre-aggregated daily metrics:
//! - `date` - YYYY-MM-DD format
//! - `project_id` - NULL for global aggregates
//! - `branch` - NULL for project-wide aggregates
//! - Metrics: sessions_count, ai_lines_added/removed, commits_count, etc.

mod aggregation;
mod branches;
mod breakdowns;
mod contributions;
mod generation;
pub(crate) mod helpers;
mod rates;
mod session_detail;
mod tests;
mod trends;
pub mod types;

pub use types::{
    AggregatedContributions, BranchBreakdown, BranchSession, ContributionSnapshot, DailyTrendPoint,
    FileImpact, LearningCurve, LearningCurvePeriod, LinkedCommit, ModelBreakdown, ModelStats,
    SessionContribution, SkillStats, SnapshotStats, TimeRange, UncommittedWork,
};
