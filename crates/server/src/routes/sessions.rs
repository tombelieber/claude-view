// crates/server/src/routes/sessions.rs
//! Session retrieval and listing endpoints.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_core::{ParsedSession, SessionInfo};
use vibe_recall_db::git_correlation::GitCommit;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Filter and Sort Enums
// ============================================================================

/// Valid filter values for GET /api/sessions
const VALID_FILTERS: &[&str] = &["all", "has_commits", "high_reedit", "long_session"];

/// Valid sort values for GET /api/sessions
const VALID_SORTS: &[&str] = &["recent", "tokens", "prompts", "files_edited", "duration"];

/// Query parameters for GET /api/sessions
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SessionsListQuery {
    /// Filter: all (default), has_commits, high_reedit, long_session
    pub filter: Option<String>,
    /// Sort: recent (default), tokens, prompts, files_edited, duration
    pub sort: Option<String>,
    /// Pagination limit (default 50)
    pub limit: Option<i64>,
    /// Pagination offset (default 0)
    pub offset: Option<i64>,
}

/// Response for GET /api/sessions with pagination
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionsListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub filter: String,
    pub sort: String,
}

// ============================================================================
// Session Detail Types (Step 21)
// ============================================================================

/// Extended session detail with commits (for GET /api/sessions/:id)
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    /// Base session info
    #[serde(flatten)]
    pub info: SessionInfo,
    /// Linked commits with tier
    pub commits: Vec<CommitWithTier>,
    /// Derived metrics
    pub derived_metrics: DerivedMetrics,
}

/// A commit linked to a session with its confidence tier
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CommitWithTier {
    pub hash: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Tier 1 = high confidence (commit skill), Tier 2 = medium (during session)
    pub tier: i32,
}

impl From<(GitCommit, i32, String)> for CommitWithTier {
    fn from((commit, tier, _evidence): (GitCommit, i32, String)) -> Self {
        Self {
            hash: commit.hash,
            message: commit.message,
            author: commit.author,
            timestamp: commit.timestamp,
            branch: commit.branch,
            tier,
        }
    }
}

/// Derived metrics calculated from atomic units
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct DerivedMetrics {
    /// Tokens per prompt: (total_input + total_output) / user_prompt_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_prompt: Option<f64>,
    /// Re-edit rate: reedited_files_count / files_edited_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reedit_rate: Option<f64>,
    /// Tool density: tool_call_count / api_call_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_density: Option<f64>,
    /// Edit velocity: files_edited_count / (duration_seconds / 60)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_velocity: Option<f64>,
    /// Read-to-edit ratio: files_read_count / files_edited_count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_to_edit_ratio: Option<f64>,
}

impl From<&SessionInfo> for DerivedMetrics {
    fn from(s: &SessionInfo) -> Self {
        Self {
            tokens_per_prompt: s.tokens_per_prompt(),
            reedit_rate: s.reedit_rate(),
            tool_density: s.tool_density(),
            edit_velocity: s.edit_velocity(),
            read_to_edit_ratio: s.read_to_edit_ratio(),
        }
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/sessions - List all sessions with filter/sort (Step 20).
///
/// Filters:
/// - `all` (default): no filter
/// - `has_commits`: commit_count > 0
/// - `high_reedit`: reedit_rate > 0.2
/// - `long_session`: duration_seconds > 1800 (30 minutes)
///
/// Sorts:
/// - `recent` (default): first_message_at DESC
/// - `tokens`: (total_input + total_output) DESC
/// - `prompts`: user_prompt_count DESC
/// - `files_edited`: files_edited_count DESC
/// - `duration`: duration_seconds DESC
///
/// Returns 400 with valid options list for invalid filter/sort.
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionsListQuery>,
) -> ApiResult<Json<SessionsListResponse>> {
    let filter = query.filter.unwrap_or_else(|| "all".to_string());
    let sort = query.sort.unwrap_or_else(|| "recent".to_string());
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // Validate filter
    if !VALID_FILTERS.contains(&filter.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid filter '{}'. Valid options: {}",
            filter,
            VALID_FILTERS.join(", ")
        )));
    }

    // Validate sort
    if !VALID_SORTS.contains(&sort.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid sort '{}'. Valid options: {}",
            sort,
            VALID_SORTS.join(", ")
        )));
    }

    // Fetch all projects with sessions
    let projects = state.db.list_projects().await?;

    // Flatten sessions from all projects
    let mut all_sessions: Vec<SessionInfo> = projects
        .into_iter()
        .flat_map(|p| p.sessions)
        .collect();

    // Apply filter
    all_sessions = match filter.as_str() {
        "has_commits" => all_sessions
            .into_iter()
            .filter(|s| s.commit_count > 0)
            .collect(),
        "high_reedit" => all_sessions
            .into_iter()
            .filter(|s| {
                s.reedit_rate().map(|r| r > 0.2).unwrap_or(false)
            })
            .collect(),
        "long_session" => all_sessions
            .into_iter()
            .filter(|s| s.duration_seconds > 1800)
            .collect(),
        _ => all_sessions, // "all" - no filter
    };

    // Apply sort
    match sort.as_str() {
        "tokens" => {
            all_sessions.sort_by(|a, b| {
                let a_tokens = a.total_input_tokens.unwrap_or(0) + a.total_output_tokens.unwrap_or(0);
                let b_tokens = b.total_input_tokens.unwrap_or(0) + b.total_output_tokens.unwrap_or(0);
                b_tokens.cmp(&a_tokens)
            });
        }
        "prompts" => {
            all_sessions.sort_by(|a, b| b.user_prompt_count.cmp(&a.user_prompt_count));
        }
        "files_edited" => {
            all_sessions.sort_by(|a, b| b.files_edited_count.cmp(&a.files_edited_count));
        }
        "duration" => {
            all_sessions.sort_by(|a, b| b.duration_seconds.cmp(&a.duration_seconds));
        }
        _ => {
            // "recent" - sort by modified_at DESC (already sorted from DB, but ensure)
            all_sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        }
    }

    let total = all_sessions.len();

    // Apply pagination
    let sessions: Vec<SessionInfo> = all_sessions
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(SessionsListResponse {
        sessions,
        total,
        filter,
        sort,
    }))
}

/// GET /api/sessions/:id - Get extended session detail (Step 21).
///
/// Returns session with:
/// - All atomic units (files_read, files_edited arrays)
/// - Derived metrics (tokens_per_prompt, reedit_rate, etc.)
/// - Linked commits with tier
pub async fn get_session_detail(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<SessionDetail>> {
    // Find session across all projects
    let projects = state.db.list_projects().await?;
    let session = projects
        .into_iter()
        .flat_map(|p| p.sessions)
        .find(|s| s.id == session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    // Get linked commits
    let commits_raw = state.db.get_commits_for_session(&session_id).await?;
    let commits: Vec<CommitWithTier> = commits_raw.into_iter().map(Into::into).collect();

    // Calculate derived metrics
    let derived_metrics = DerivedMetrics::from(&session);

    Ok(Json(SessionDetail {
        info: session,
        commits,
        derived_metrics,
    }))
}

/// GET /api/session/:project_dir/:session_id - Get a parsed session by ID.
///
/// Returns the full parsed session with all messages and metadata.
/// The project_dir is the URL-encoded project directory name (can contain slashes).
/// The session_id is the UUID of the session.
pub async fn get_session(
    State(_state): State<Arc<AppState>>,
    Path((project_dir, session_id)): Path<(String, String)>,
) -> ApiResult<Json<ParsedSession>> {
    // Decode the project directory (URL-encoded, may contain slashes)
    let project_dir_decoded = urlencoding::decode(&project_dir)
        .map_err(|_| ApiError::ProjectNotFound(project_dir.clone()))?
        .into_owned();

    // Get the Claude projects directory and construct the session path
    let projects_dir = vibe_recall_core::claude_projects_dir()?;
    let session_path = projects_dir
        .join(&project_dir_decoded)
        .join(&session_id)
        .with_extension("jsonl");

    // Check if the session file exists
    if !session_path.exists() {
        return Err(ApiError::SessionNotFound(format!(
            "{}/{}",
            project_dir_decoded, session_id
        )));
    }

    // Parse and return the session
    let session = vibe_recall_core::parse_session(&session_path).await?;
    Ok(Json(session))
}

/// Create the sessions routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session_detail))
        .route("/session/{project_dir}/{session_id}", get(get_session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::path::PathBuf;
    use tower::ServiceExt;
    use vibe_recall_core::{Message, SessionMetadata, ToolCounts};
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
            files_read: vec!["a.rs".to_string()],
            files_edited: vec!["b.rs".to_string()],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 0,
        }
    }

    #[test]
    fn test_parsed_session_serialization() {
        let session = ParsedSession {
            messages: vec![
                Message::user("Hello Claude!"),
                Message::assistant("Hello! How can I help?"),
            ],
            metadata: SessionMetadata {
                total_messages: 2,
                tool_call_count: 0,
            },
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"totalMessages\":2"));
    }

    #[test]
    fn test_session_path_construction() {
        let project_dir = "Users-user-dev-myproject";
        let session_id = "abc123-def456";

        let base = PathBuf::from("/Users/user/.claude/projects");
        let session_path = base
            .join(project_dir)
            .join(session_id)
            .with_extension("jsonl");

        assert_eq!(
            session_path.to_string_lossy(),
            "/Users/user/.claude/projects/Users-user-dev-myproject/abc123-def456.jsonl"
        );
    }

    // ========================================================================
    // GET /api/sessions tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["sessions"].as_array().unwrap().is_empty());
        assert_eq!(json["filter"], "all");
        assert_eq!(json["sort"], "recent");
    }

    #[tokio::test]
    async fn test_list_sessions_with_data() {
        let db = test_db().await;

        let session = make_session("sess-1", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_sessions_invalid_filter() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?filter=invalid").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"].as_str().unwrap().contains("invalid"));
        assert!(json["details"].as_str().unwrap().contains("all, has_commits"));
    }

    #[tokio::test]
    async fn test_list_sessions_invalid_sort() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?sort=invalid").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"].as_str().unwrap().contains("invalid"));
        assert!(json["details"].as_str().unwrap().contains("recent, tokens"));
    }

    #[tokio::test]
    async fn test_list_sessions_filter_has_commits() {
        let db = test_db().await;

        // Session without commits
        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.commit_count = 0;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        // Session with commits
        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.commit_count = 3;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?filter=has_commits").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_filter_high_reedit() {
        let db = test_db().await;

        // Session with low reedit rate (1/10 = 0.1)
        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.files_edited_count = 10;
        session1.reedited_files_count = 1;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        // Session with high reedit rate (5/10 = 0.5)
        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.files_edited_count = 10;
        session2.reedited_files_count = 5;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?filter=high_reedit").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_filter_long_session() {
        let db = test_db().await;

        // Short session (10 minutes)
        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.duration_seconds = 600;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        // Long session (1 hour)
        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.duration_seconds = 3600;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?filter=long_session").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_sort_tokens() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.total_input_tokens = Some(1000);
        session1.total_output_tokens = Some(500);
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.total_input_tokens = Some(10000);
        session2.total_output_tokens = Some(5000);
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?sort=tokens").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // sess-2 should be first (more tokens)
        assert_eq!(json["sessions"][0]["id"], "sess-2");
        assert_eq!(json["sessions"][1]["id"], "sess-1");
    }

    #[tokio::test]
    async fn test_list_sessions_sort_duration() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.duration_seconds = 600;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.duration_seconds = 3600;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?sort=duration").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // sess-2 should be first (longer duration)
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_pagination() {
        let db = test_db().await;

        // Insert 5 sessions
        for i in 0..5 {
            let session = make_session(&format!("sess-{}", i), "project-a", 1700000000 + i);
            db.insert_session(&session, "project-a", "Project A")
                .await
                .unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?limit=2&offset=1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 5); // Total count before pagination
        assert_eq!(json["sessions"].as_array().unwrap().len(), 2); // Only 2 returned
    }

    // ========================================================================
    // GET /api/sessions/:id tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_session_detail() {
        let db = test_db().await;

        let session = make_session("sess-123", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/sess-123").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["id"], "sess-123");
        assert!(json["commits"].is_array());
        assert!(json["derivedMetrics"].is_object());
        // Note: tokensPerPrompt requires turns table data which we don't insert in tests.
        // The tokens come from the turns aggregate, not from session.total_input_tokens.
        // Since we have files_edited_count=5 and reedited_files_count=2, reeditRate should be 0.4
        assert!(json["derivedMetrics"]["reeditRate"].is_number());
        assert_eq!(json["derivedMetrics"]["reeditRate"], 0.4);
    }

    #[tokio::test]
    async fn test_get_session_detail_not_found() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"].as_str().unwrap().contains("nonexistent"));
    }

    #[test]
    fn test_derived_metrics_calculation() {
        let session = make_session("test", "project", 1700000000);
        let metrics = DerivedMetrics::from(&session);

        // (10000 + 5000) / 10 = 1500.0
        assert_eq!(metrics.tokens_per_prompt, Some(1500.0));
        // 2 / 5 = 0.4
        assert_eq!(metrics.reedit_rate, Some(0.4));
        // 50 / 20 = 2.5
        assert_eq!(metrics.tool_density, Some(2.5));
        // 5 / (600 / 60) = 0.5
        assert_eq!(metrics.edit_velocity, Some(0.5));
        // 20 / 5 = 4.0
        assert_eq!(metrics.read_to_edit_ratio, Some(4.0));
    }
}
