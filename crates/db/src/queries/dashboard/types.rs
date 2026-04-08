// crates/db/src/queries/dashboard/types.rs
// Public types for dashboard queries — filter params, activity aggregation structs.

/// Parameters for filtered, paginated session queries.
/// All fields are optional — omitted fields apply no filter.
pub struct SessionFilterParams {
    pub q: Option<String>,
    pub search_session_ids: Option<Vec<String>>, // pre-resolved from Tantivy
    pub branches: Option<Vec<String>>,
    pub models: Option<Vec<String>>,
    pub has_commits: Option<bool>,
    pub has_skills: Option<bool>,
    pub min_duration: Option<i64>,
    pub min_files: Option<i64>,
    pub min_tokens: Option<i64>,
    pub high_reedit: Option<bool>,
    pub time_after: Option<i64>,
    pub time_before: Option<i64>,
    pub project: Option<String>,
    pub show_archived: Option<bool>,
    pub sort: String, // "recent", "tokens", "prompts", "files_edited", "duration"
    pub limit: i64,   // default 30
    pub offset: i64,  // default 0
}

/// A single point in the activity histogram.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ActivityPoint {
    pub date: String,
    pub count: i64,
    /// Total duration in seconds for this bucket (used by CalendarHeatmap).
    #[serde(rename = "totalSeconds")]
    pub total_seconds: i64,
}

/// Project-level aggregation for the activity page.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActivityRow {
    pub project_path: String,
    pub display_name: String,
    pub session_count: i64,
    pub total_seconds: i64,
    pub total_cost_usd: f64,
}

/// Summary stats for the activity page.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummaryRow {
    pub total_seconds: i64,
    pub session_count: i64,
    pub total_tool_calls: i64,
    pub total_agent_spawns: i64,
    pub total_mcp_calls: i64,
    pub unique_skills: i64,
    pub longest_session_id: Option<String>,
    pub longest_session_seconds: i64,
    pub longest_session_project: Option<String>,
    pub longest_session_title: Option<String>,
}

/// Full server-side activity response (replaces client-side aggregation).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RichActivityResponse {
    pub histogram: Vec<ActivityPoint>,
    pub bucket: String,
    pub projects: Vec<ProjectActivityRow>,
    pub summary: ActivitySummaryRow,
    pub total: i64,
}
