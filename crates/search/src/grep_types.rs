use serde::Serialize;
use ts_rs::TS;

/// Response from the `/api/grep` endpoint (regex search over raw JSONL files).
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepResponse {
    pub pattern: String,
    pub total_matches: usize,
    pub total_sessions: usize,
    pub elapsed_ms: f64,
    pub truncated: bool,
    pub results: Vec<GrepSessionHit>,
}

/// One session that matched the grep pattern.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepSessionHit {
    pub session_id: String,
    pub project: String,
    pub project_path: String,
    pub modified_at: i64,
    pub matches: Vec<GrepLineMatch>,
}

/// One matching line within a session JSONL file.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepLineMatch {
    pub line_number: usize,
    pub content: String,
    pub match_start: usize,
    pub match_end: usize,
}
