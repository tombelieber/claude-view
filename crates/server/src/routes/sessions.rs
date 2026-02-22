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
use claude_view_core::accumulator::SessionAccumulator;
use claude_view_core::{ParsedSession, SessionInfo};
use claude_view_db::git_correlation::GitCommit;

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
    /// Filter: all (default), has_commits, high_reedit, long_session (kept for backward compat)
    pub filter: Option<String>,
    /// Sort: recent (default), tokens, prompts, files_edited, duration
    pub sort: Option<String>,
    /// Pagination limit (default 30)
    pub limit: Option<i64>,
    /// Pagination offset (default 0)
    pub offset: Option<i64>,
    /// Text search across preview, last_message, project name
    pub q: Option<String>,
    // New multi-facet filters
    /// Comma-separated list of branches to filter by
    pub branches: Option<String>,
    /// Comma-separated list of models to filter by
    pub models: Option<String>,
    /// Filter sessions with commits (true) or without (false)
    pub has_commits: Option<bool>,
    /// Filter sessions with skills (true) or without (false)
    pub has_skills: Option<bool>,
    /// Minimum duration in seconds
    pub min_duration: Option<i64>,
    /// Minimum number of files edited
    pub min_files: Option<i64>,
    /// Minimum total tokens (input + output)
    pub min_tokens: Option<i64>,
    /// Filter sessions with high re-edit rate (> 0.2)
    pub high_reedit: Option<bool>,
    /// Filter sessions after this timestamp (unix seconds)
    pub time_after: Option<i64>,
    /// Filter sessions before this timestamp (unix seconds)
    pub time_before: Option<i64>,
}

/// Response for GET /api/sessions with pagination
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionsListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub has_more: bool,
    pub filter: String,
    pub sort: String,
}

/// Response for GET /api/sessions/activity
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionActivityResponse {
    pub activity: Vec<claude_view_db::ActivityPoint>,
    pub bucket: String,
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
    #[ts(type = "number")]
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
// Paginated Messages Query
// ============================================================================

/// Query parameters for GET /api/session/:project_dir/:session_id/messages
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SessionMessagesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
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
    let limit = query.limit.unwrap_or(30);
    let offset = query.offset.unwrap_or(0);

    // Validate filter (kept for backward compat — legacy single-value filter)
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

    // Map legacy filter param to the new structured params
    let has_commits = match (query.has_commits, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "has_commits") => Some(true),
        _ => None,
    };
    let high_reedit = match (query.high_reedit, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "high_reedit") => Some(true),
        _ => None,
    };
    let min_duration = match (query.min_duration, filter.as_str()) {
        (Some(v), _) => Some(v),
        (None, "long_session") => Some(1800),
        _ => None,
    };

    // Resolve text query via Tantivy (if available and q is present)
    let search_session_ids = if let Some(ref q_text) = query.q {
        let q_trimmed = q_text.trim();
        if q_trimmed.is_empty() {
            None
        } else {
            // Try to get search index
            let search_index = state.search_index.read().ok().and_then(|guard| guard.clone());
            match search_index {
                Some(idx) => {
                    const TANTIVY_SESSION_LIMIT: usize = 10_000;
                    match idx.search(q_trimmed, None, TANTIVY_SESSION_LIMIT, 0) {
                        Ok(response) => {
                            let ids: Vec<String> = response.sessions.into_iter().map(|s| s.session_id).collect();
                            if ids.len() >= TANTIVY_SESSION_LIMIT {
                                tracing::warn!(
                                    query = q_trimmed,
                                    limit = TANTIVY_SESSION_LIMIT,
                                    "Tantivy session limit saturated — results may be incomplete"
                                );
                            }
                            Some(ids)
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, query = q_trimmed, "Tantivy search failed, falling back to LIKE");
                            None // Fall back to SQLite LIKE
                        }
                    }
                }
                None => None, // Index not ready, fall back to SQLite LIKE
            }
        }
    } else {
        None
    };

    let params = claude_view_db::SessionFilterParams {
        q: query.q,
        search_session_ids,
        branches: query.branches.map(|s| s.split(',').map(|b| b.trim().to_string()).collect()),
        models: query.models.map(|s| s.split(',').map(|m| m.trim().to_string()).collect()),
        has_commits,
        has_skills: query.has_skills,
        min_duration,
        min_files: query.min_files,
        min_tokens: query.min_tokens,
        high_reedit,
        time_after: query.time_after,
        time_before: query.time_before,
        sort: sort.clone(),
        limit,
        offset,
    };

    let (sessions, total) = state.db.query_sessions_filtered(&params).await?;
    let has_more = (offset + limit) < total as i64;

    Ok(Json(SessionsListResponse {
        sessions,
        total,
        has_more,
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

/// GET /api/sessions/:id/parsed — Get full parsed session by ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
pub async fn get_session_parsed(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<ParsedSession>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    // NOTE: There is a small TOCTOU window between exists() and parse_session().
    // If the file is deleted in that window, parse_session returns ParseError (different
    // error message). This is acceptable — filesystem ops are inherently racy, and
    // the exists() check provides a cleaner "Session not found" for the common case.
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let session = claude_view_core::parse_session(&path).await?;
    Ok(Json(session))
}

/// GET /api/sessions/:id/messages — Get paginated messages by session ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
pub async fn get_session_messages_by_id(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<claude_view_core::PaginatedMessages>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let result = claude_view_core::parse_session_paginated(&path, limit, offset).await?;
    Ok(Json(result))
}
/// GET /api/sessions/:id/rich — Parse JSONL on demand via `SessionAccumulator` and return
/// rich session data (tokens, cost, cache status, sub-agents, progress items, etc.).
///
/// This endpoint bridges historical sessions with the same rich data shape used by
/// Live Monitor, enabling a unified session detail view.
pub async fn get_session_rich(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<claude_view_core::accumulator::RichSessionData>> {
    // 1. Look up session file path from DB
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    // 2. Snapshot the current pricing table (clone inside read lock, then drop lock)
    let pricing = state.pricing.read().expect("pricing lock poisoned").clone();

    // 3. Parse JSONL through SessionAccumulator (blocking I/O → spawn_blocking)
    let rich_data = tokio::task::spawn_blocking(move || {
        SessionAccumulator::from_file(&path, &pricing)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?
    .map_err(|e| ApiError::Internal(format!("Parse error: {e}")))?;

    Ok(Json(rich_data))
}

/// DEPRECATED: Use `GET /api/sessions/:id/parsed` instead.
/// Kept for backward compatibility. Will be removed in v0.6.
///
/// The `project_dir` parameter is now ignored — path resolution is DB-based.
#[deprecated(note = "Use get_session_parsed instead")]
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path((_project_dir, session_id)): Path<(String, String)>,
) -> ApiResult<Json<ParsedSession>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let session = claude_view_core::parse_session(&path).await?;
    Ok(Json(session))
}

/// DEPRECATED: Use `GET /api/sessions/:id/messages` instead.
/// Kept for backward compatibility. Will be removed in v0.6.
///
/// The `project_dir` parameter is now ignored — path resolution is DB-based.
#[deprecated(note = "Use get_session_messages_by_id instead")]
pub async fn get_session_messages(
    State(state): State<Arc<AppState>>,
    Path((_project_dir, session_id)): Path<(String, String)>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<claude_view_core::PaginatedMessages>> {
    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let result = claude_view_core::parse_session_paginated(&path, limit, offset).await?;
    Ok(Json(result))
}

/// GET /api/branches - Get distinct list of branch names across all sessions.
///
/// Returns a sorted array of unique branch names found in the database.
/// Excludes sessions without a branch (NULL git_branch).
pub async fn list_branches(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<String>>> {
    // Fetch all projects with sessions
    let projects = state.db.list_projects().await?;

    // Collect all unique branch names
    let mut branches: Vec<String> = projects
        .into_iter()
        .flat_map(|p| p.sessions)
        .filter_map(|s| s.git_branch)
        .collect();

    // Sort and deduplicate
    branches.sort();
    branches.dedup();

    Ok(Json(branches))
}

/// GET /api/sessions/:id/hook-events — Fetch stored hook events for a historical session.
async fn get_session_hook_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    match claude_view_db::hook_events_queries::get_hook_events(&state.db, &session_id).await {
        Ok(events) => {
            let json_events: Vec<serde_json::Value> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "timestamp": e.timestamp,
                        "eventName": e.event_name,
                        "toolName": e.tool_name,
                        "label": e.label,
                        "group": e.group_name,
                        "context": e.context,
                    })
                })
                .collect();
            Json(serde_json::json!({ "hookEvents": json_events }))
        }
        Err(e) => Json(serde_json::json!({ "hookEvents": [], "error": e.to_string() })),
    }
}

/// GET /api/sessions/activity — Activity histogram for sparkline chart.
pub async fn session_activity(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SessionActivityResponse>> {
    let (activity, bucket) = state.db.session_activity_histogram().await?;
    Ok(Json(SessionActivityResponse { activity, bucket }))
}

/// Create the sessions routes router.
#[allow(deprecated)] // Legacy /session/ routes kept for backward compat until v0.6
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/activity", get(session_activity))
        .route("/sessions/{id}", get(get_session_detail))
        .route("/sessions/{id}/parsed", get(get_session_parsed))
        .route("/sessions/{id}/messages", get(get_session_messages_by_id))
        .route("/sessions/{id}/rich", get(get_session_rich))
        .route("/sessions/{id}/hook-events", get(get_session_hook_events))
        .route("/session/{project_dir}/{session_id}", get(get_session))
        .route("/session/{project_dir}/{session_id}/messages", get(get_session_messages))
        .route("/branches", get(list_branches))
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
    use claude_view_core::{Message, SessionMetadata, ToolCounts};
    use claude_view_db::Database;

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
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
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
    // New multi-facet filter tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_sessions_filter_by_branches() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.git_branch = Some("main".to_string());
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.git_branch = Some("feature/auth".to_string());
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let mut session3 = make_session("sess-3", "project-a", 1700000200);
        session3.git_branch = Some("fix/bug".to_string());
        db.insert_session(&session3, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?branches=main,feature/auth").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 2);
        let ids: Vec<&str> = json["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"sess-1"));
        assert!(ids.contains(&"sess-2"));
        assert!(!ids.contains(&"sess-3"));
    }

    #[tokio::test]
    async fn test_list_sessions_filter_by_models() {
        // TODO: This test is currently skipped because insert_session() doesn't persist
        // primary_model to the database. This is a pre-existing bug that needs to be fixed
        // in the db crate's insert_session SQL query.
        //
        // Once fixed, uncomment the test below.

        /*
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.primary_model = Some("claude-opus-4".to_string());
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.primary_model = Some("claude-sonnet-4".to_string());
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?models=claude-opus-4").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-1");
        */
    }

    #[tokio::test]
    async fn test_list_sessions_filter_has_skills() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.skills_used = vec!["git".to_string()];
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.skills_used = vec![];
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?has_skills=true").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-1");
    }

    #[tokio::test]
    async fn test_list_sessions_filter_min_duration() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.duration_seconds = 300; // 5 minutes
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.duration_seconds = 2400; // 40 minutes
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?min_duration=1800").await; // 30 minutes

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_filter_min_files() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.files_edited_count = 2;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.files_edited_count = 10;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?min_files=5").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_filter_min_tokens() {
        // TODO: This test is currently skipped because insert_session() doesn't persist
        // token counts to the database (only deep_index_session does via aggregation).
        // This is a pre-existing limitation of the test helper.
        //
        // Once we add proper token persistence or use deep_index_session in tests,
        // uncomment the test below.

        /*
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.total_input_tokens = Some(1000);
        session1.total_output_tokens = Some(500);
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.total_input_tokens = Some(50000);
        session2.total_output_tokens = Some(25000);
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions?min_tokens=10000").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
        */
    }

    #[tokio::test]
    async fn test_list_sessions_filter_time_range() {
        let db = test_db().await;

        let session1 = make_session("sess-1", "project-a", 1700000000); // Jan 2024
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let session2 = make_session("sess-2", "project-a", 1720000000); // Jul 2024
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let session3 = make_session("sess-3", "project-a", 1740000000); // Dec 2024
        db.insert_session(&session3, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        // Filter for sessions between Feb 2024 and Nov 2024
        let (status, body) = do_get(app, "/api/sessions?time_after=1710000000&time_before=1730000000").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-2");
    }

    #[tokio::test]
    async fn test_list_sessions_multiple_filters_combined() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.git_branch = Some("main".to_string());
        session1.commit_count = 3;
        session1.duration_seconds = 2400;
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.git_branch = Some("feature/auth".to_string());
        session2.commit_count = 0;
        session2.duration_seconds = 2400;
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let mut session3 = make_session("sess-3", "project-a", 1700000200);
        session3.git_branch = Some("main".to_string());
        session3.commit_count = 5;
        session3.duration_seconds = 600;
        db.insert_session(&session3, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        // Filter: main branch AND has commits AND duration >= 30 mins
        let (status, body) = do_get(app, "/api/sessions?branches=main&has_commits=true&min_duration=1800").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["sessions"][0]["id"], "sess-1");
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

    // ========================================================================
    // PaginatedMessages serialization test
    // ========================================================================

    #[test]
    fn test_paginated_messages_serialization() {
        use claude_view_core::PaginatedMessages;
        let result = PaginatedMessages {
            messages: vec![
                Message::user("Hello"),
                Message::assistant("Hi"),
            ],
            total: 100,
            offset: 0,
            limit: 2,
            has_more: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"total\":100"));
        assert!(json.contains("\"hasMore\":true"));
    }

    // ========================================================================
    // GET /api/branches tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_branches_empty() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/branches").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_branches_with_data() {
        let db = test_db().await;

        let mut session1 = make_session("sess-1", "project-a", 1700000000);
        session1.git_branch = Some("main".to_string());
        db.insert_session(&session1, "project-a", "Project A")
            .await
            .unwrap();

        let mut session2 = make_session("sess-2", "project-a", 1700000100);
        session2.git_branch = Some("feature/auth".to_string());
        db.insert_session(&session2, "project-a", "Project A")
            .await
            .unwrap();

        let mut session3 = make_session("sess-3", "project-a", 1700000200);
        session3.git_branch = Some("main".to_string()); // Duplicate
        db.insert_session(&session3, "project-a", "Project A")
            .await
            .unwrap();

        let mut session4 = make_session("sess-4", "project-a", 1700000300);
        session4.git_branch = None; // No branch - should be excluded
        db.insert_session(&session4, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/branches").await;

        assert_eq!(status, StatusCode::OK);
        let branches: Vec<String> = serde_json::from_str(&body).unwrap();
        assert_eq!(branches.len(), 2); // Only "feature/auth" and "main"
        assert_eq!(branches, vec!["feature/auth", "main"]); // Alphabetically sorted
    }

    // ========================================================================
    // GET /api/sessions/:id/parsed tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_session_parsed_not_in_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent/parsed").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_session_parsed_file_gone() {
        let db = test_db().await;
        let mut session = make_session("parsed-test", "proj", 1700000000);
        session.file_path = "/nonexistent/path.jsonl".to_string();
        db.insert_session(&session, "proj", "Project").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/parsed-test/parsed").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_session_parsed_success() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("success-test.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
        ).unwrap();

        let mut session = make_session("parsed-ok", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/parsed-ok/parsed").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let messages = json["messages"].as_array().expect("Response should contain messages array");
        assert!(!messages.is_empty(), "Fixture should produce at least one parsed message");
    }

    // ========================================================================
    // GET /api/sessions/:id/messages tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_session_messages_by_id_not_in_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent/messages?limit=10&offset=0").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_session_messages_by_id_file_gone() {
        let db = test_db().await;
        let mut session = make_session("msg-test", "proj", 1700000000);
        session.file_path = "/nonexistent/path.jsonl".to_string();
        db.insert_session(&session, "proj", "Project").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/msg-test/messages?limit=10&offset=0").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_session_messages_by_id_success() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("msg-success.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
        ).unwrap();

        let mut session = make_session("msg-ok", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/msg-ok/messages?limit=10&offset=0").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let messages = json["messages"].as_array().expect("Response should contain messages array");
        assert!(!messages.is_empty(), "Fixture should produce at least one parsed message");
        assert!(json["total"].as_u64().unwrap() > 0, "Total should reflect the fixture message count");
    }

    // ========================================================================
    // Legacy endpoint backward-compat regression test
    // ========================================================================

    #[tokio::test]
    async fn test_legacy_get_session_still_works() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("legacy-test.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","message":{"content":"Hello"},"timestamp":"2026-01-01T00:00:00Z"}"#,
        ).unwrap();

        let mut session = make_session("legacy-ok", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project").await.unwrap();

        let app = build_app(db);
        // Legacy endpoint: project_dir is now ignored, path comes from DB
        let (status, body) = do_get(app, "/api/session/proj/legacy-ok").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let messages = json["messages"].as_array().expect("should contain messages");
        assert!(!messages.is_empty(), "Fixture should produce at least one parsed message");
    }

    // ========================================================================
    // GET /api/sessions/activity tests
    // ========================================================================

    #[tokio::test]
    async fn test_session_activity() {
        let db = test_db().await;
        let session = make_session("sess-activity", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/activity").await;

        assert_eq!(status, StatusCode::OK);
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(resp["activity"].is_array());
        assert!(resp["bucket"].is_string());
        let activity = resp["activity"].as_array().unwrap();
        assert!(!activity.is_empty());
        assert!(activity[0]["date"].is_string());
        assert!(activity[0]["count"].is_number());
    }

    // ========================================================================
    // GET /api/sessions/:id/hook-events tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_hook_events_empty() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, body) = do_get(app, "/api/sessions/nonexistent/hook-events").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["hookEvents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_hook_events_with_data() {
        let db = test_db().await;

        // Insert session first (FK reference)
        let session = make_session("hook-test", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        // Insert hook events
        let events = vec![
            claude_view_db::HookEventRow {
                timestamp: 1000,
                event_name: "SessionStart".into(),
                tool_name: None,
                label: "Waiting for first prompt".into(),
                group_name: "needs_you".into(),
                context: None,
            },
            claude_view_db::HookEventRow {
                timestamp: 1001,
                event_name: "PreToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Running: git status".into(),
                group_name: "autonomous".into(),
                context: Some(r#"{"command":"git status"}"#.into()),
            },
        ];
        claude_view_db::hook_events_queries::insert_hook_events(&db, "hook-test", &events)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/hook-test/hook-events").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let hook_events = json["hookEvents"].as_array().unwrap();
        assert_eq!(hook_events.len(), 2);

        // Verify camelCase serialization
        assert_eq!(hook_events[0]["eventName"], "SessionStart");
        assert_eq!(hook_events[0]["group"], "needs_you");
        assert!(hook_events[0]["toolName"].is_null());

        assert_eq!(hook_events[1]["eventName"], "PreToolUse");
        assert_eq!(hook_events[1]["toolName"], "Bash");
        assert_eq!(hook_events[1]["label"], "Running: git status");
        assert!(hook_events[1]["context"].as_str().unwrap().contains("git status"));
    }
}
