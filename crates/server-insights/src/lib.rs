//! Insight generation for the contributions page.
//!
//! This module provides plain-English insights based on contribution metrics.
//! Insights help users understand their productivity patterns and identify
//! areas for improvement.

mod generators;
#[cfg(test)]
mod tests;
mod types;

pub use generators::*;
pub use types::*;
