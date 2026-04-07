//! Response types for the insights trends endpoint.

use serde::Serialize;
use ts_rs::TS;

/// Time-series data point for metric trends.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MetricDataPoint {
    pub date: String,
    pub value: f64,
}

/// Category evolution data point.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CategoryDataPoint {
    pub date: String,
    pub code_work: f64,
    pub support_work: f64,
    pub thinking_work: f64,
}

/// Activity heatmap cell.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HeatmapCell {
    pub day_of_week: u8,
    pub hour_of_day: u8,
    #[ts(type = "number")]
    pub sessions: i64,
    pub avg_reedit_rate: f64,
}

/// Full trends response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InsightsTrendsResponse {
    pub metric: String,
    pub data_points: Vec<MetricDataPoint>,
    pub average: f64,
    pub trend: f64,
    pub trend_direction: String,
    pub insight: String,

    pub category_evolution: Option<Vec<CategoryDataPoint>>,
    pub category_insight: Option<String>,
    pub classification_required: bool,

    pub activity_heatmap: Vec<HeatmapCell>,
    pub heatmap_insight: String,

    pub period_start: String,
    pub period_end: String,
    #[ts(type = "number")]
    pub total_sessions: i64,
}
