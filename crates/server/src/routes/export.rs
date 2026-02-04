//! Export endpoint for session data (JSON and CSV).

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_core::SessionInfo;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Export format query parameter.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ExportQuery {
    /// Export format: "json" (default) or "csv"
    pub format: Option<String>,
}

/// Exported session data for JSON format (A5.2 schema).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ExportedSession {
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub modified_at: i64,
    pub duration_seconds: u32,
    pub user_prompt_count: u32,
    pub api_call_count: u32,
    pub tool_call_count: u32,
    pub files_read_count: u32,
    pub files_edited_count: u32,
    pub reedited_files_count: u32,
    pub commit_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reedit_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_prompt: Option<f64>,
}

impl From<&SessionInfo> for ExportedSession {
    fn from(s: &SessionInfo) -> Self {
        Self {
            id: s.id.clone(),
            project: s.project.clone(),
            project_path: s.project_path.clone(),
            modified_at: s.modified_at,
            duration_seconds: s.duration_seconds,
            user_prompt_count: s.user_prompt_count,
            api_call_count: s.api_call_count,
            tool_call_count: s.tool_call_count,
            files_read_count: s.files_read_count,
            files_edited_count: s.files_edited_count,
            reedited_files_count: s.reedited_files_count,
            commit_count: s.commit_count,
            total_input_tokens: s.total_input_tokens,
            total_output_tokens: s.total_output_tokens,
            reedit_rate: s.reedit_rate(),
            tokens_per_prompt: s.tokens_per_prompt(),
        }
    }
}

/// JSON export response wrapper.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub sessions: Vec<ExportedSession>,
    pub exported_at: i64,
    pub total_count: usize,
}

/// GET /api/export/sessions - Export all sessions.
///
/// Query parameters:
/// - format: "json" (default) or "csv"
///
/// JSON format returns structured data per A5.2 spec.
/// CSV format returns RFC 4180 compliant CSV with proper escaping.
pub async fn export_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Response> {
    let format = query.format.unwrap_or_else(|| "json".to_string());

    // Validate format
    if format != "json" && format != "csv" {
        return Err(ApiError::BadRequest(format!(
            "Invalid format '{}'. Valid options: json, csv",
            format
        )));
    }

    // Fetch all sessions from all projects
    let projects = state.db.list_projects().await?;
    let sessions: Vec<&SessionInfo> = projects
        .iter()
        .flat_map(|p| p.sessions.iter())
        .collect();

    let exported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match format.as_str() {
        "csv" => {
            let csv = build_csv(&sessions);
            Ok((
                [
                    (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"sessions.csv\"",
                    ),
                ],
                csv,
            )
                .into_response())
        }
        _ => {
            // JSON format
            let exported: Vec<ExportedSession> = sessions.iter().map(|s| (*s).into()).collect();
            let response = ExportResponse {
                total_count: exported.len(),
                sessions: exported,
                exported_at,
            };
            Ok(Json(response).into_response())
        }
    }
}

/// Build CSV output per A5.3 spec with proper RFC 4180 escaping.
fn build_csv(sessions: &[&SessionInfo]) -> String {
    let mut csv = String::new();

    // Header row
    csv.push_str("id,project,projectPath,modifiedAt,durationSeconds,userPromptCount,apiCallCount,toolCallCount,filesReadCount,filesEditedCount,reeditedFilesCount,commitCount,totalInputTokens,totalOutputTokens,reeditRate,tokensPerPrompt\n");

    for s in sessions {
        // Escape fields that may contain special characters
        let id = escape_csv_field(&s.id);
        let project = escape_csv_field(&s.project);
        let project_path = escape_csv_field(&s.project_path);

        // Calculate derived metrics
        let reedit_rate = s.reedit_rate().map(|r| format!("{:.4}", r)).unwrap_or_default();
        let tokens_per_prompt = s.tokens_per_prompt().map(|t| format!("{:.2}", t)).unwrap_or_default();
        let total_input = s.total_input_tokens.map(|t| t.to_string()).unwrap_or_default();
        let total_output = s.total_output_tokens.map(|t| t.to_string()).unwrap_or_default();

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            id,
            project,
            project_path,
            s.modified_at,
            s.duration_seconds,
            s.user_prompt_count,
            s.api_call_count,
            s.tool_call_count,
            s.files_read_count,
            s.files_edited_count,
            s.reedited_files_count,
            s.commit_count,
            total_input,
            total_output,
            reedit_rate,
            tokens_per_prompt,
        ));
    }

    csv
}

/// Escape a CSV field per RFC 4180.
///
/// If the field contains comma, double quote, newline, or pipe, wrap in double quotes
/// and escape any internal double quotes by doubling them.
fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') || field.contains('|') {
        // Wrap in quotes and escape internal quotes
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Create the export routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/export/sessions", get(export_sessions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_core::ToolCounts;
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            file_path: format!("/path/{}.jsonl", id),
            modified_at,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Last msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(10),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            summary_text: None,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
        }
    }

    #[tokio::test]
    async fn test_export_json_empty() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/export/sessions").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalCount"], 0);
        assert!(json["sessions"].as_array().unwrap().is_empty());
        assert!(json["exportedAt"].is_number());
    }

    #[tokio::test]
    async fn test_export_json_with_data() {
        let db = test_db().await;

        let session = make_session("sess-1", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/export/sessions?format=json").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalCount"], 1);

        let sessions = json["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], "sess-1");
        assert_eq!(sessions[0]["project"], "project-a");
        assert_eq!(sessions[0]["durationSeconds"], 600);
        assert_eq!(sessions[0]["userPromptCount"], 10);
    }

    #[tokio::test]
    async fn test_export_csv_empty() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/export/sessions?format=csv").await;

        assert_eq!(status, StatusCode::OK);
        // Should have header row only
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("id,project,"));
    }

    #[tokio::test]
    async fn test_export_csv_with_data() {
        let db = test_db().await;

        let session = make_session("sess-1", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/export/sessions?format=csv").await;

        assert_eq!(status, StatusCode::OK);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2); // Header + 1 data row
        assert!(lines[1].starts_with("sess-1,"));
    }

    #[tokio::test]
    async fn test_export_invalid_format() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/export/sessions?format=xml").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"].as_str().unwrap().contains("xml"));
        assert!(json["details"].as_str().unwrap().contains("json, csv"));
    }

    #[test]
    fn test_escape_csv_field_simple() {
        assert_eq!(escape_csv_field("hello"), "hello");
        assert_eq!(escape_csv_field("no special chars"), "no special chars");
    }

    #[test]
    fn test_escape_csv_field_with_comma() {
        assert_eq!(escape_csv_field("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_escape_csv_field_with_quote() {
        assert_eq!(escape_csv_field("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_field_with_newline() {
        assert_eq!(escape_csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_escape_csv_field_with_pipe() {
        // Pipes in paths like /Users|something
        assert_eq!(escape_csv_field("a|b"), "\"a|b\"");
    }

    #[test]
    fn test_escape_csv_field_combined() {
        // Multiple special chars including quotes
        assert_eq!(
            escape_csv_field("a,b\"c\nd"),
            "\"a,b\"\"c\nd\""
        );
    }
}
