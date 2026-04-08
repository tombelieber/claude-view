//! GET /api/sessions/:id — Extended session detail with commits and derived metrics.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use claude_view_core::task_files;
use claude_view_core::todo_files;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::types::{CommitWithTier, DerivedMetrics, SessionDetail};

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
    let mut warnings: Vec<String> = Vec::new();

    let tasks = match task_files::claude_tasks_dir() {
        Some(dir) => task_files::parse_session_tasks(&dir, &session_id),
        None => {
            warnings.push(
                "Failed to read task files: could not resolve ~/.claude/tasks directory".into(),
            );
            Vec::new()
        }
    };

    // Read agent-level todo checklists (if any, 96% are empty)
    let todos = match todo_files::claude_todos_dir() {
        Some(dir) => todo_files::parse_session_todos(&dir, &session_id),
        None => Vec::new(),
    };

    // Check if plan files exist for this session's slug
    let has_plans = session.slug.as_ref().is_some_and(|slug| {
        match claude_view_core::plan_files::claude_plans_dir() {
            Some(dir) => claude_view_core::plan_files::has_plan_files(&dir, slug),
            None => {
                warnings.push(
                    "Failed to check plan files: could not resolve ~/.claude/plans directory"
                        .into(),
                );
                false
            }
        }
    });

    Ok(Json(SessionDetail {
        info: session,
        commits,
        derived_metrics,
        tasks,
        todos,
        has_plans,
        warnings,
    }))
}
