//! Trend calculations and index metadata management.
//!
//! This module implements:
//! - Week period bounds (current week, previous week)
//! - TrendMetric calculation with delta and delta_percent
//! - Week-over-week trend aggregations
//! - index_metadata CRUD operations

mod index_metadata;
mod time_periods;
mod trend_queries;
mod types;

#[cfg(test)]
mod tests;

pub use time_periods::*;
pub use types::*;
