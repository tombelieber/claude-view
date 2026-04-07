// crates/server/src/insights/mod.rs
//! Insight generation for the contributions page.
//!
//! This module provides plain-English insights based on contribution metrics.
//! Insights help users understand their productivity patterns and identify
//! areas for improvement.
//!
//! ## Insight Categories
//!
//! - **Fluency**: How active the user has been compared to previous period
//! - **Output**: Productivity level and peak activity days
//! - **Effectiveness**: Quality metrics (commit rate, re-edit rate)
//! - **Model**: Which models perform best for different tasks
//! - **Learning Curve**: Improvement in prompting over time
//! - **Branch**: Human/AI contribution balance per branch
//! - **Skill**: Impact of using skills on output quality
//! - **Uncommitted**: Warnings about uncommitted work

mod generators;
#[cfg(test)]
mod tests;
mod types;

pub use generators::*;
pub use types::*;
