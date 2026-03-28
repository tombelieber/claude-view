// crates/server/src/routes/sessions.rs
//! Session retrieval and listing endpoints.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use claude_view_core::accumulator::SessionAccumulator;
use claude_view_core::task_files::{self, TaskItem};
use claude_view_core::{ParsedSession, SessionInfo};
use claude_view_db::git_correlation::GitCommit;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Resolve a session's JSONL file path: DB first, then live session store fallback.
///
/// Live sessions (especially IDE-spawned ones) may not be indexed in the DB yet.
/// The live session store always has the file path for any actively-monitored session.
async fn resolve_session_file_path(
    state: &AppState,
    session_id: &str,
) -> ApiResult<std::path::PathBuf> {
    let file_path = match state.db.get_session_file_path(session_id).await? {
        Some(p) => p,
        None => {
            let map = state.live_sessions.read().await;
            map.get(session_id)
                .map(|s| s.file_path.clone())
                .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?
        }
    };
    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id.to_string()));
    }
    Ok(path)
}

// ============================================================================
// Archive request/response types
// ============================================================================

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkArchiveRequest {
    pub ids: Vec<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BulkArchiveResponse {
    pub archived_count: usize,
}

// ============================================================================
// Filter and Sort Enums
// ============================================================================

/// Valid filter values for GET /api/sessions
const VALID_FILTERS: &[&str] = &["all", "has_commits", "high_reedit", "long_session"];

/// Valid sort values for GET /api/sessions
const VALID_SORTS: &[&str] = &["recent", "tokens", "prompts", "files_edited", "duration"];

/// Query parameters for GET /api/sessions
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
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
    /// Optional project filter (matches project_id or git_root)
    pub project: Option<String>,
    /// Include archived sessions (queries `sessions` table instead of `valid_sessions` view)
    pub show_archived: Option<bool>,
}

/// Response for GET /api/sessions with pagination
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionsListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
    pub has_more: bool,
    pub filter: String,
    pub sort: String,
}

/// Response for GET /api/sessions/activity
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionActivityResponse {
    pub activity: Vec<claude_view_db::ActivityPoint>,
    pub bucket: String,
    /// True total from valid_sessions (includes sessions with last_message_at=0
    /// that can't be placed on the chart axis).
    pub total: usize,
}

// ============================================================================
// Session Detail Types (Step 21)
// ============================================================================

/// Extended session detail with commits (for GET /api/sessions/:id)
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    #[serde(flatten)]
    pub info: SessionInfo,
    pub commits: Vec<CommitWithTier>,
    pub derived_metrics: DerivedMetrics,
    /// Persistent task data from ~/.claude/tasks/{sessionId}/*.json
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskItem>,
    /// Whether plan files exist for this session's slug
    pub has_plans: bool,
}

/// A commit linked to a session with its confidence tier
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
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

/// Query parameters for GET /api/sessions/:id/messages
#[derive(Debug, Deserialize, Default, utoipa::IntoParams)]
#[serde(default)]
pub struct SessionMessagesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub raw: bool,
    /// "block" → return ConversationBlock[], otherwise legacy Message[]
    pub format: Option<String>,
}

/// Paginated response for `?format=block`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedBlocks {
    pub blocks: Vec<claude_view_core::ConversationBlock>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
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
#[utoipa::path(get, path = "/api/sessions", tag = "sessions",
    params(SessionsListQuery),
    responses(
        (status = 200, description = "Paginated session list", body = SessionsListResponse),
        (status = 400, description = "Invalid filter or sort parameter"),
    )
)]
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

    // Resolve text query via unified search (same engine as /api/search).
    let search_session_ids = if let Some(ref q_text) = query.q {
        let q_trimmed = q_text.trim();
        if q_trimmed.is_empty() {
            None
        } else {
            let filters = crate::search_service::SearchFilters {
                project: query.project.clone(),
                branch: query
                    .branches
                    .as_deref()
                    .and_then(|b| b.split(',').next().map(|s| s.trim().to_string())),
                model: query
                    .models
                    .as_deref()
                    .and_then(|m| m.split(',').next().map(|s| s.trim().to_string())),
                after: query.time_after.and_then(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                }),
                before: query.time_before.and_then(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                }),
            };
            // skip_snippets=true: we only need session IDs, not highlighted text.
            match crate::search_service::execute_search(
                &state,
                q_trimmed,
                &filters,
                usize::MAX,
                0,
                true,
            )
            .await
            {
                Ok(response) => {
                    let ids: Vec<String> = response
                        .sessions
                        .into_iter()
                        .map(|s| s.session_id)
                        .collect();
                    if ids.is_empty() {
                        None
                    } else {
                        Some(ids)
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Unified search failed, falling back to LIKE");
                    None
                }
            }
        }
    } else {
        None
    };

    let params = claude_view_db::SessionFilterParams {
        q: query.q,
        search_session_ids,
        branches: query
            .branches
            .map(|s| s.split(',').map(|b| b.trim().to_string()).collect()),
        models: query
            .models
            .map(|s| s.split(',').map(|m| m.trim().to_string()).collect()),
        has_commits,
        has_skills: query.has_skills,
        min_duration,
        min_files: query.min_files,
        min_tokens: query.min_tokens,
        high_reedit,
        time_after: query.time_after,
        time_before: query.time_before,
        project: query.project,
        show_archived: query.show_archived,
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
#[utoipa::path(get, path = "/api/sessions/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session detail with commits and derived metrics", body = SessionDetail),
        (status = 404, description = "Session not found"),
    )
)]
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

    // Read persistent task files (if any)
    let tasks = task_files::claude_tasks_dir()
        .map(|dir| task_files::parse_session_tasks(&dir, &session_id))
        .unwrap_or_default();

    // Check if plan files exist for this session's slug
    let has_plans = session.slug.as_ref().is_some_and(|slug| {
        claude_view_core::plan_files::claude_plans_dir()
            .map(|dir| claude_view_core::plan_files::has_plan_files(&dir, slug))
            .unwrap_or(false)
    });

    Ok(Json(SessionDetail {
        info: session,
        commits,
        derived_metrics,
        tasks,
        has_plans,
    }))
}

/// GET /api/sessions/:id/parsed — Get full parsed session by ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
#[utoipa::path(get, path = "/api/sessions/{id}/parsed", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Full parsed session messages", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_parsed(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<ParsedSession>> {
    let path = resolve_session_file_path(&state, &session_id).await?;
    let session = claude_view_core::parse_session(&path).await?;
    Ok(Json(session))
}

/// GET /api/sessions/:id/messages — Get paginated messages by session ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
#[utoipa::path(get, path = "/api/sessions/{id}/messages", tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
        SessionMessagesQuery,
    ),
    responses(
        (status = 200, description = "Paginated session messages (block or legacy format)", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_messages_by_id(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let path = resolve_session_file_path(&state, &session_id).await?;

    if query.format.as_deref() == Some("block") {
        // Block format — use BlockAccumulator
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ApiError::Internal(format!("Read error: {e}")))?;

        let parsed = claude_view_core::block_accumulator::parse_session(&content);
        let total = parsed.blocks.len();
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(100);
        let end = std::cmp::min(offset + limit, total);
        let blocks: Vec<_> = if offset < total {
            parsed.blocks.into_iter().skip(offset).take(limit).collect()
        } else {
            vec![]
        };

        let result = PaginatedBlocks {
            blocks,
            total,
            offset,
            limit,
            has_more: end < total,
            forked_from: parsed.forked_from,
            entrypoint: parsed.entrypoint,
        };
        Ok(Json(serde_json::to_value(result).unwrap()))
    } else {
        // Legacy format — existing behavior
        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);
        let result = if query.raw {
            claude_view_core::parse_session_paginated_with_raw(&path, limit, offset).await?
        } else {
            claude_view_core::parse_session_paginated(&path, limit, offset).await?
        };
        Ok(Json(serde_json::to_value(result).unwrap()))
    }
}
/// GET /api/sessions/:id/rich — Parse JSONL on demand via `SessionAccumulator` and return
/// rich session data (tokens, cost, cache status, sub-agents, progress items, etc.).
///
/// This endpoint bridges historical sessions with the same rich data shape used by
/// Live Monitor, enabling a unified session detail view.
#[utoipa::path(get, path = "/api/sessions/{id}/rich", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Rich parsed session data with tokens, cost, and sub-agents", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_rich(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<claude_view_core::accumulator::RichSessionData>> {
    // 1. Resolve JSONL file path (DB → live session fallback)
    let path = resolve_session_file_path(&state, &session_id).await?;

    // 2. Arc-clone the pricing table (cheap, no lock needed — pricing is immutable)
    let pricing = state.pricing.clone();

    // 3. Parse JSONL through SessionAccumulator (blocking I/O → spawn_blocking)
    let rich_data =
        tokio::task::spawn_blocking(move || SessionAccumulator::from_file(&path, &pricing))
            .await
            .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?
            .map_err(|e| ApiError::Internal(format!("Parse error: {e}")))?;

    Ok(Json(rich_data))
}

/// GET /api/branches - Get distinct list of branch names across all sessions.
///
/// Returns a sorted array of unique branch names found in the database.
/// Excludes sessions without a branch (NULL git_branch).
#[utoipa::path(get, path = "/api/branches", tag = "sessions",
    responses(
        (status = 200, description = "Sorted list of unique branch names", body = Vec<String>),
    )
)]
pub async fn list_branches(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<String>>> {
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

/// GET /api/sessions/:id/hook-events — Fetch hook events for a session.
///
/// For live sessions, hook events are in memory (not flushed to SQLite until
/// SessionEnd). Check the live state first, then fall back to SQLite.
#[utoipa::path(get, path = "/api/sessions/{id}/hook-events", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Hook events for the session", body = serde_json::Value),
    )
)]
pub async fn get_session_hook_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    // Check live sessions first — hooks accumulate in memory and are only
    // flushed to SQLite on SessionEnd. Without this, the history view shows
    // hook=0 for any session that hasn't ended yet.
    {
        let sessions = state.live_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            if !session.hook.hook_events.is_empty() {
                let json_events: Vec<serde_json::Value> = session
                    .hook.hook_events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "timestamp": e.timestamp,
                            "eventName": e.event_name,
                            "toolName": e.tool_name,
                            "label": e.label,
                            "group": e.group,
                            "context": e.context,
                            "source": e.source,
                        })
                    })
                    .collect();
                return Json(serde_json::json!({ "hookEvents": json_events }));
            }
        }
    }

    // Fall back to SQLite for completed sessions
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
                        "source": e.source,
                    })
                })
                .collect();
            Json(serde_json::json!({ "hookEvents": json_events }))
        }
        Err(e) => Json(serde_json::json!({ "hookEvents": [], "error": e.to_string() })),
    }
}

/// GET /api/sessions/activity — Activity histogram for sparkline chart.
#[utoipa::path(get, path = "/api/sessions/activity", tag = "sessions",
    responses(
        (status = 200, description = "Activity histogram with date buckets and session count", body = SessionActivityResponse),
    )
)]
pub async fn session_activity(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SessionActivityResponse>> {
    let (activity, bucket) = state.db.session_activity_histogram().await?;
    let total = state.db.get_session_count().await? as usize;
    Ok(Json(SessionActivityResponse {
        activity,
        bucket,
        total,
    }))
}

// ============================================================================
// Archive / Unarchive handlers
// ============================================================================

/// Returns archive dir — respects `CLAUDE_VIEW_DATA_DIR` for sandbox envs.
fn archive_base_dir() -> std::path::PathBuf {
    claude_view_core::paths::archive_dir()
}

#[utoipa::path(post, path = "/api/sessions/{id}/archive", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session archived successfully", body = ArchiveResponse),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn archive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let file_path = state
        .db
        .archive_session(&id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to archive session {id}: {e}");
            ApiError::Internal(format!("archive failed: {e}"))
        })?
        .ok_or(ApiError::NotFound(format!(
            "Session {id} not found or already archived"
        )))?;

    // Move file to ~/.claude-view/archives/
    let src = std::path::PathBuf::from(&file_path);
    let archive_dir = archive_base_dir();
    if let Some(project_dir) = src.parent().and_then(|p| p.file_name()) {
        let dest_dir = archive_dir.join(project_dir);

        // Attempt file move — failure is non-fatal (DB flag is the source of truth)
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            tracing::warn!("Failed to create archive dir: {e}");
        } else if let Some(file_name) = src.file_name() {
            let dest = dest_dir.join(file_name);
            match tokio::fs::rename(&src, &dest).await {
                Ok(()) => {
                    if let Some(dest_str) = dest.to_str() {
                        if let Err(e) = state.db.update_session_file_path(&id, dest_str).await {
                            tracing::error!(session_id = %id, error = %e, "failed to update session file path in DB after move");
                            // Note: file has been moved but DB not updated — this is a data integrity issue
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to move session file to archive: {e}");
                    // DB already marked as archived — indexer guard will skip it
                }
            }
        }
    }

    Ok(Json(ArchiveResponse { archived: true }))
}

#[utoipa::path(post, path = "/api/sessions/{id}/unarchive", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Session unarchived successfully", body = ArchiveResponse),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn unarchive_session_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<ArchiveResponse>> {
    let current_path = state
        .db
        .get_session_file_path(&id)
        .await
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or(ApiError::NotFound(format!("Session {id} not found")))?;

    let archive_base = archive_base_dir();
    let current = std::path::PathBuf::from(&current_path);

    let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
        // Security: validate no path traversal in relative components
        use std::path::Component;
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(ApiError::BadRequest("Invalid archive path".to_string()));
        }

        let Some(home) = dirs::home_dir() else {
            return Err(ApiError::Internal(
                "Cannot determine home directory".to_string(),
            ));
        };
        let original = home.join(".claude").join("projects").join(relative);

        // Move file back — failure is non-fatal
        if current.exists() {
            if let Some(parent) = original.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Err(e) = tokio::fs::rename(&current, &original).await {
                tracing::warn!("Failed to move file back from archive: {e}");
            }
        }

        original.to_string_lossy().to_string()
    } else {
        // File not in archive dir (file move failed during archive) — just clear the flag
        current_path
    };

    state
        .db
        .unarchive_session(&id, &new_path)
        .await
        .map_err(|e| ApiError::Internal(format!("unarchive failed: {e}")))?;

    Ok(Json(ArchiveResponse { archived: false }))
}

#[utoipa::path(post, path = "/api/sessions/archive", tag = "sessions",
    request_body = BulkArchiveRequest,
    responses(
        (status = 200, description = "Sessions archived in bulk", body = BulkArchiveResponse),
    )
)]
pub async fn bulk_archive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let results = state
        .db
        .archive_sessions_bulk(&body.ids)
        .await
        .map_err(|e| ApiError::Internal(format!("bulk archive failed: {e}")))?;

    let archive_dir = archive_base_dir();
    for (id, file_path) in &results {
        let src = std::path::PathBuf::from(file_path);
        let Some(project_dir) = src.parent().and_then(|p| p.file_name()) else {
            continue;
        };
        let dest_dir = archive_dir.join(project_dir);
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            tracing::warn!("Bulk archive: failed to create dir {dest_dir:?}: {e}");
            continue;
        }
        if let Some(file_name) = src.file_name() {
            let dest = dest_dir.join(file_name);
            if let Ok(()) = tokio::fs::rename(&src, &dest).await {
                if let Some(dest_str) = dest.to_str() {
                    if let Err(e) = state.db.update_session_file_path(id, dest_str).await {
                        tracing::error!(session_id = %id, error = %e, "failed to update session file path in DB after move");
                        // Note: file has been moved but DB not updated — this is a data integrity issue
                    }
                }
            }
        }
    }

    Ok(Json(BulkArchiveResponse {
        archived_count: results.len(),
    }))
}

#[utoipa::path(post, path = "/api/sessions/unarchive", tag = "sessions",
    request_body = BulkArchiveRequest,
    responses(
        (status = 200, description = "Sessions unarchived in bulk", body = BulkArchiveResponse),
    )
)]
pub async fn bulk_unarchive_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkArchiveRequest>,
) -> ApiResult<Json<BulkArchiveResponse>> {
    let archive_base = archive_base_dir();
    let mut file_paths: Vec<(String, String)> = Vec::new();

    let Some(home) = dirs::home_dir() else {
        return Err(ApiError::Internal(
            "Cannot determine home directory".to_string(),
        ));
    };

    for id in &body.ids {
        let current_path = match state.db.get_session_file_path(id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                tracing::warn!("Bulk unarchive: session {id} not found, skipping");
                continue;
            }
            Err(e) => {
                tracing::warn!("Bulk unarchive: DB error for session {id}: {e}");
                continue;
            }
        };
        let current = std::path::PathBuf::from(&current_path);
        let new_path = if let Ok(relative) = current.strip_prefix(&archive_base) {
            use std::path::Component;
            if !relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            {
                tracing::warn!("Bulk unarchive: path traversal in {id}, skipping");
                continue;
            }
            let original = home.join(".claude").join("projects").join(relative);
            if current.exists() {
                if let Some(parent) = original.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Err(e) = tokio::fs::rename(&current, &original).await {
                    tracing::warn!(
                        "Bulk unarchive: failed to move {current:?} → {original:?}: {e}"
                    );
                }
            }
            original.to_string_lossy().to_string()
        } else {
            current_path
        };
        file_paths.push((id.clone(), new_path));
    }

    let count = state
        .db
        .unarchive_sessions_bulk(&file_paths)
        .await
        .map_err(|e| ApiError::Internal(format!("bulk unarchive failed: {e}")))?;

    Ok(Json(BulkArchiveResponse {
        archived_count: count,
    }))
}

// ============================================================================
// Cost Estimation (extracted from control.rs for Phase 3)
// ============================================================================

/// Request body for cost estimation.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct EstimateRequest {
    pub session_id: String,
    pub model: Option<String>,
}

/// Cost estimation response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CostEstimate {
    pub session_id: String,
    pub history_tokens: u64,
    pub cache_warm: bool,
    pub first_message_cost: Option<f64>,
    pub per_message_cost: Option<f64>,
    pub has_pricing: bool,
    pub model: String,
    pub explanation: String,
    pub session_title: Option<String>,
    pub project_name: Option<String>,
    pub turn_count: u32,
    pub files_edited: u32,
    pub last_active_secs_ago: i64,
}

/// POST /api/estimate — cost estimation (Rust-only, no sidecar).
#[utoipa::path(post, path = "/api/estimate", tag = "sessions",
    request_body = EstimateRequest,
    responses(
        (status = 200, description = "Cost estimate for resuming a session", body = CostEstimate),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn estimate_cost(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EstimateRequest>,
) -> Result<Json<CostEstimate>, ApiError> {
    let now = chrono::Utc::now().timestamp();

    // Look up session in DB
    let session = state
        .db
        .get_session_by_id(&req.session_id)
        .await
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", req.session_id)))?;

    let model = req.model.unwrap_or_else(|| {
        session
            .primary_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
    });

    let history_tokens = session.total_input_tokens.unwrap_or(0);
    let last_activity = session.modified_at; // epoch seconds
    let cache_warm = last_activity > 0 && (now - last_activity) < 300; // 5 min TTL

    // Look up model pricing
    let pricing = &*state.pricing;
    let model_pricing = claude_view_core::pricing::lookup_pricing(&model, pricing);

    let per_million =
        |tokens: u64, rate_per_m: f64| -> f64 { (tokens as f64 / 1_000_000.0) * rate_per_m };

    let secs_ago = now - last_activity;
    let (first_message_cost, per_message_cost, has_pricing, explanation) = if let Some(p) =
        model_pricing
    {
        let input_base = p.input_cost_per_token * 1_000_000.0;
        let first_message_cost = if cache_warm {
            per_million(history_tokens, input_base * 0.10) // cache read
        } else {
            per_million(history_tokens, input_base * 1.25) // cache write
        };
        let per_message_cost = per_million(history_tokens, input_base * 0.10); // always cache read
        let explanation = if cache_warm {
            format!(
                "Cache is warm (last active {}s ago). First message: ${:.4} (cached). Each follow-up: ~${:.4}.",
                secs_ago, first_message_cost, per_message_cost,
            )
        } else {
            format!(
                "Cache is cold (last active {}m ago). First message: ${:.4} (cache warming). Follow-ups drop to ~${:.4} (cached).",
                secs_ago / 60, first_message_cost, per_message_cost,
            )
        };
        (
            Some(first_message_cost),
            Some(per_message_cost),
            true,
            explanation,
        )
    } else {
        (
            None,
            None,
            false,
            format!(
                "Model pricing not found for {} (last active {}s ago). Cost estimate unavailable without real pricing data.",
                model, secs_ago
            ),
        )
    };

    let project_name = if session.display_name.is_empty() {
        None
    } else {
        Some(session.display_name.clone())
    };

    Ok(Json(CostEstimate {
        session_id: req.session_id,
        history_tokens,
        cache_warm,
        first_message_cost,
        per_message_cost,
        has_pricing,
        model,
        explanation,
        session_title: session.longest_task_preview.clone(),
        project_name,
        turn_count: session.turn_count_api.unwrap_or(0).min(u32::MAX as u64) as u32,
        files_edited: session.files_edited_count,
        last_active_secs_ago: secs_ago,
    }))
}

/// Create the sessions routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/estimate", post(estimate_cost))
        .route("/sessions/activity", get(session_activity))
        .route("/sessions/archive", post(bulk_archive_handler))
        .route("/sessions/unarchive", post(bulk_unarchive_handler))
        .route("/sessions/{id}", get(get_session_detail))
        .route("/sessions/{id}/parsed", get(get_session_parsed))
        .route("/sessions/{id}/messages", get(get_session_messages_by_id))
        .route("/sessions/{id}/rich", get(get_session_rich))
        .route("/sessions/{id}/hook-events", get(get_session_hook_events))
        .route("/sessions/{id}/archive", post(archive_session_handler))
        .route("/sessions/{id}/unarchive", post(unarchive_session_handler))
        .route("/branches", get(list_branches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_core::{Message, SessionMetadata, ToolCounts};
    use claude_view_db::Database;
    use std::path::PathBuf;
    use tower::ServiceExt;

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
            display_name: project.to_string(),
            git_root: None,
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
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
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
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("all, has_commits"));
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
        let (status, body) = do_get(
            app,
            "/api/sessions?time_after=1710000000&time_before=1730000000",
        )
        .await;

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
        let (status, body) = do_get(
            app,
            "/api/sessions?branches=main&has_commits=true&min_duration=1800",
        )
        .await;

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
            messages: vec![Message::user("Hello"), Message::assistant("Hi")],
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
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

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
        )
        .unwrap();

        let mut session = make_session("parsed-ok", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/parsed-ok/parsed").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let messages = json["messages"]
            .as_array()
            .expect("Response should contain messages array");
        assert!(
            !messages.is_empty(),
            "Fixture should produce at least one parsed message"
        );
    }

    // ========================================================================
    // GET /api/sessions/:id/messages tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_session_messages_by_id_not_in_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) =
            do_get(app, "/api/sessions/nonexistent/messages?limit=10&offset=0").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["error"], "Session not found");
    }

    #[tokio::test]
    async fn test_get_session_messages_by_id_file_gone() {
        let db = test_db().await;
        let mut session = make_session("msg-test", "proj", 1700000000);
        session.file_path = "/nonexistent/path.jsonl".to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

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
        )
        .unwrap();

        let mut session = make_session("msg-ok", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/msg-ok/messages?limit=10&offset=0").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let messages = json["messages"]
            .as_array()
            .expect("Response should contain messages array");
        assert!(
            !messages.is_empty(),
            "Fixture should produce at least one parsed message"
        );
        assert!(
            json["total"].as_u64().unwrap() > 0,
            "Total should reflect the fixture message count"
        );
    }

    // ========================================================================
    // GET /api/sessions/activity tests
    // ========================================================================

    #[tokio::test]
    async fn test_session_activity() {
        let db = test_db().await;
        let session = make_session("sess-activity", "project-a", 1700000000);
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/activity").await;

        assert_eq!(status, StatusCode::OK);
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(resp["activity"].is_array());
        assert!(resp["bucket"].is_string());
        assert_eq!(resp["total"].as_u64().unwrap(), 1);
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
                source: "hook".into(),
            },
            claude_view_db::HookEventRow {
                timestamp: 1001,
                event_name: "PreToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Running: git status".into(),
                group_name: "autonomous".into(),
                context: Some(r#"{"command":"git status"}"#.into()),
                source: "hook".into(),
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
        assert!(hook_events[1]["context"]
            .as_str()
            .unwrap()
            .contains("git status"));
    }

    #[tokio::test]
    async fn test_get_hook_events_from_live_session() {
        use crate::live::state::{
            AgentState, AgentStateGroup, HookEvent, HookFields, LiveSession, SessionStatus,
        };
        use claude_view_core::phase::PhaseHistory;
        use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

        let db = test_db().await;

        // Build app state — new_with_indexing returns Arc<AppState>
        let state = crate::state::AppState::new_with_indexing(
            db,
            Arc::new(crate::indexing_state::IndexingState::new()),
        );

        // Insert a live session with hook events into the live_sessions map
        let mut session = LiveSession {
            id: "live-hook-test".to_string(),
            project: "test-project".to_string(),
            project_display_name: "test-project".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: String::new(),
            status: SessionStatus::Working,
            hook: HookFields {
                agent_state: AgentState {
                    state: "working".to_string(),
                    group: AgentStateGroup::Autonomous,
                    label: "Running".to_string(),
                    context: None,
                },
                pid: Some(12345),
                title: String::new(),
                last_user_message: String::new(),
                current_activity: String::new(),
                turn_count: 1,
                last_activity_at: 1001,
                current_turn_started_at: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                compact_count: 0,
                agent_state_set_at: 0,
                hook_events: Vec::new(),
            },
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            started_at: Some(1000),
            model: Some("claude-sonnet-4-5-20250929".to_string()),
            tokens: TokenUsage::default(),
            context_window_tokens: 200000,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            last_turn_task_seconds: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            slug: None,
            user_files: None,
            closed_at: None,
            control: None,
            source: None,
            statusline: crate::live::state::StatuslineFields::default(),
            model_display_name: None,
            model_set_at: 0,
            phase: PhaseHistory::default(),
        };
        session.hook.hook_events.push(HookEvent {
            timestamp: 1000,
            event_name: "SessionStart".to_string(),
            tool_name: None,
            label: "Waiting for prompt".to_string(),
            group: "needs_you".to_string(),
            context: None,
            source: "hook".to_string(),
        });
        session.hook.hook_events.push(HookEvent {
            timestamp: 1001,
            event_name: "PreToolUse".to_string(),
            tool_name: Some("Read".to_string()),
            label: "Reading file".to_string(),
            group: "autonomous".to_string(),
            context: Some(r#"{"file_path":"/foo/bar.rs"}"#.to_string()),
            source: "hook".to_string(),
        });

        state
            .live_sessions
            .write()
            .await
            .insert("live-hook-test".to_string(), session);

        // Build app from the state (already Arc<AppState>)
        let app = crate::api_routes(state);

        // Should return hook events from live session (not SQLite)
        let (status, body) = do_get(app, "/api/sessions/live-hook-test/hook-events").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let hook_events = json["hookEvents"].as_array().unwrap();
        assert_eq!(hook_events.len(), 2);
        assert_eq!(hook_events[0]["eventName"], "SessionStart");
        assert_eq!(hook_events[0]["group"], "needs_you");
        assert_eq!(hook_events[1]["eventName"], "PreToolUse");
        assert_eq!(hook_events[1]["toolName"], "Read");
        assert_eq!(hook_events[1]["label"], "Reading file");
    }

    // ========================================================================
    // GET /api/sessions/:id/messages?format=block tests
    // ========================================================================

    #[tokio::test]
    async fn test_format_block_returns_paginated_blocks() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("block-test.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello"}]},"timestamp":"2026-03-21T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi there!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-21T01:00:01.000Z"}
"#,
        )
        .unwrap();

        let mut session = make_session("block-test", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/block-test/messages?format=block").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            json.get("blocks").is_some(),
            "Response should have 'blocks' key"
        );
        let blocks = json["blocks"].as_array().unwrap();
        assert!(!blocks.is_empty(), "blocks should not be empty");
        assert!(
            blocks[0].get("type").is_some(),
            "Block should have 'type' discriminator"
        );
    }

    #[tokio::test]
    async fn test_format_block_empty_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("block-empty.jsonl");
        std::fs::write(&session_file, "").unwrap();

        let mut session = make_session("block-empty", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/block-empty/messages?format=block").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let blocks = json["blocks"].as_array().unwrap();
        assert!(blocks.is_empty());
        assert_eq!(json["total"], 0);
        assert_eq!(json["hasMore"], false);
    }

    #[tokio::test]
    async fn test_format_block_e2e_block_structure() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("e2e-block.jsonl");
        // Write a multi-line JSONL fixture with user + assistant + tool + boundary
        std::fs::write(&session_file, r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"List files"}]},"timestamp":"2026-03-21T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Sure!"},{"type":"tool_use","id":"tu-1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":500,"output_tokens":100},"stop_reason":"tool_use"},"timestamp":"2026-03-21T01:00:01.000Z"}
{"type":"user","uuid":"u-2","message":{"content":[{"type":"tool_result","tool_use_id":"tu-1","content":"file1\nfile2","is_error":false}]},"timestamp":"2026-03-21T01:00:02.000Z"}
{"type":"assistant","uuid":"a-2","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Here are the files."}],"usage":{"input_tokens":600,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-21T01:00:03.000Z"}
{"type":"system","uuid":"s-1","durationMs":3000,"timestamp":"2026-03-21T01:00:04.000Z"}
{"type":"system","uuid":"s-2","stopReason":"end_turn","hookInfos":[],"hookErrors":[],"hookCount":0,"timestamp":"2026-03-21T01:00:05.000Z"}
"#).unwrap();

        let mut session = make_session("e2e-block", "proj", 1700000000);
        session.file_path = session_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/e2e-block/messages?format=block").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let blocks = json["blocks"].as_array().unwrap();

        // Verify block types present
        let types: Vec<&str> = blocks
            .iter()
            .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(types.contains(&"user"), "Should have user block");
        assert!(types.contains(&"assistant"), "Should have assistant block");
        assert!(
            types.contains(&"turn_boundary"),
            "Should have turn_boundary block"
        );

        // Verify block count matches expected
        assert_eq!(
            json["total"].as_u64().unwrap() as usize,
            blocks.len(),
            "total should match actual block count for small sessions"
        );

        // Verify hasMore is false for small session
        assert_eq!(json["hasMore"], false);
    }

    // ========================================================================
    // Live session file_path fallback tests
    // ========================================================================

    /// Helper: create a LiveSession with a given file_path (no DB insertion).
    fn make_live_session(id: &str, file_path: &str) -> crate::live::state::LiveSession {
        use crate::live::state::{
            AgentState, AgentStateGroup, HookFields, LiveSession, SessionStatus,
        };
        use claude_view_core::phase::PhaseHistory;
        use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

        LiveSession {
            id: id.to_string(),
            project: "test-project".to_string(),
            project_display_name: "test-project".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: file_path.to_string(),
            status: SessionStatus::Working,
            hook: HookFields {
                agent_state: AgentState {
                    state: "working".to_string(),
                    group: AgentStateGroup::Autonomous,
                    label: "Running".to_string(),
                    context: None,
                },
                pid: Some(12345),
                title: String::new(),
                last_user_message: String::new(),
                current_activity: String::new(),
                turn_count: 1,
                last_activity_at: 1001,
                current_turn_started_at: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                compact_count: 0,
                agent_state_set_at: 0,
                hook_events: Vec::new(),
            },
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            started_at: Some(1000),
            model: Some("claude-sonnet-4-5-20250929".to_string()),
            tokens: TokenUsage::default(),
            context_window_tokens: 200000,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            last_turn_task_seconds: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            slug: None,
            user_files: None,
            closed_at: None,
            control: None,
            source: None,
            statusline: crate::live::state::StatuslineFields::default(),
            model_display_name: None,
            model_set_at: 0,
            phase: PhaseHistory::default(),
        }
    }

    /// Regression: GET /api/sessions/:id/messages?format=block must return blocks
    /// for live sessions not yet indexed in the DB (file_path fallback).
    #[tokio::test]
    async fn test_messages_block_format_falls_back_to_live_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("live-only.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello from VS Code"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-23T01:00:01.000Z"}
"#,
        )
        .unwrap();

        // NOT inserted into DB — simulates un-indexed live session
        let state = crate::state::AppState::new_with_indexing(
            db,
            Arc::new(crate::indexing_state::IndexingState::new()),
        );

        let live = make_live_session("live-only", session_file.to_str().unwrap());
        state
            .live_sessions
            .write()
            .await
            .insert("live-only".to_string(), live);

        let app = crate::api_routes(state);
        let (status, body) = do_get(app, "/api/sessions/live-only/messages?format=block").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let blocks = json["blocks"].as_array().unwrap();
        // Must have both user AND assistant blocks
        let types: Vec<&str> = blocks
            .iter()
            .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(
            types.contains(&"user"),
            "Live session fallback must return user blocks, got: {types:?}"
        );
        assert!(
            types.contains(&"assistant"),
            "Live session fallback must return assistant blocks, got: {types:?}"
        );
        assert_eq!(json["total"], 2);
    }

    /// Regression: GET /api/sessions/:id/rich must work for live sessions
    /// not yet indexed in the DB (file_path fallback).
    #[tokio::test]
    async fn test_rich_endpoint_falls_back_to_live_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("live-rich.jsonl");
        std::fs::write(
            &session_file,
            r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
{"type":"assistant","uuid":"a-1","message":{"id":"msg-1","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":100,"output_tokens":50},"stop_reason":"end_turn"},"timestamp":"2026-03-23T01:00:01.000Z"}
"#,
        )
        .unwrap();

        let state = crate::state::AppState::new_with_indexing(
            db,
            Arc::new(crate::indexing_state::IndexingState::new()),
        );

        let live = make_live_session("live-rich", session_file.to_str().unwrap());
        state
            .live_sessions
            .write()
            .await
            .insert("live-rich".to_string(), live);

        let app = crate::api_routes(state);
        let (status, _body) = do_get(app, "/api/sessions/live-rich/rich").await;
        assert_eq!(
            status,
            StatusCode::OK,
            "rich endpoint should succeed via live fallback"
        );
    }

    /// When session is in neither DB nor live store, should return 404.
    #[tokio::test]
    async fn test_messages_returns_404_when_not_in_db_or_live() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/sessions/nonexistent/messages?format=block").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("not found") || body.contains("Session"));
    }

    /// DB path takes precedence over live session (ensures no conflict).
    #[tokio::test]
    async fn test_db_path_takes_precedence_over_live_session() {
        let db = test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        // DB file has one message
        let db_file = tmp.path().join("db-priority.jsonl");
        std::fs::write(
            &db_file,
            r#"{"type":"user","uuid":"u-db","message":{"content":[{"type":"text","text":"from DB"}]},"timestamp":"2026-03-23T01:00:00.000Z"}
"#,
        )
        .unwrap();

        let mut session = make_session("db-priority", "proj", 1700000000);
        session.file_path = db_file.to_str().unwrap().to_string();
        db.insert_session(&session, "proj", "Project")
            .await
            .unwrap();

        // Live session points to a DIFFERENT file with different content
        let live_file = tmp.path().join("live-priority.jsonl");
        std::fs::write(
            &live_file,
            r#"{"type":"user","uuid":"u-live","message":{"content":[{"type":"text","text":"from live"}]},"timestamp":"2026-03-23T02:00:00.000Z"}
"#,
        )
        .unwrap();

        let state = crate::state::AppState::new_with_indexing(
            db,
            Arc::new(crate::indexing_state::IndexingState::new()),
        );
        let live = make_live_session("db-priority", live_file.to_str().unwrap());
        state
            .live_sessions
            .write()
            .await
            .insert("db-priority".to_string(), live);

        let app = crate::api_routes(state);
        let (status, body) = do_get(app, "/api/sessions/db-priority/messages?format=block").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let blocks = json["blocks"].as_array().unwrap();
        // Should use DB file ("from DB"), not live file ("from live")
        if let Some(user_block) = blocks.iter().find(|b| b["type"] == "user") {
            assert_eq!(
                user_block["text"].as_str().unwrap(),
                "from DB",
                "DB path should take precedence over live session"
            );
        } else {
            panic!("Expected a user block from DB file");
        }
    }
}
