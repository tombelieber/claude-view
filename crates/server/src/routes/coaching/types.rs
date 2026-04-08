//! Request and response types for the coaching rules API.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request body for POST /api/coaching/rules.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRuleRequest {
    pub pattern_id: String,
    pub recommendation: String,
    pub title: String,
    pub impact_score: f64,
    pub sample_size: usize,
    pub scope: String, // "user" | "project"
}

/// A coaching rule parsed from a `coaching-*.md` file.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct CoachingRule {
    pub id: String,
    pub pattern_id: String,
    pub title: String,
    pub body: String,
    pub scope: String,
    pub applied_at: String,
    pub file_path: String,
}

/// Response for GET /api/coaching/rules.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct ListRulesResponse {
    pub rules: Vec<CoachingRule>,
    pub count: usize,
    pub max_rules: usize,
}

/// Response for DELETE /api/coaching/rules/{id}.
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct RemoveRuleResponse {
    pub removed: bool,
    pub id: String,
}
