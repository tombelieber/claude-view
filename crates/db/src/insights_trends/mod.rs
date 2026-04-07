//! Database queries for the insights trends endpoint (Phase 7).
//!
//! Provides time-series aggregation queries for:
//! - Metric trends (re-edit rate, session count, lines, cost-per-line, prompts)
//! - Category evolution (code/support/thinking work distribution over time)
//! - Activity heatmap (day-of-week x hour session density and efficiency)

mod insights;
mod queries;
mod types;

#[cfg(test)]
mod tests;

pub use insights::{
    calculate_trend_stats, generate_category_insight, generate_heatmap_insight,
    generate_metric_insight,
};
pub use types::{CategoryDataPoint, HeatmapCell, InsightsTrendsResponse, MetricDataPoint};
