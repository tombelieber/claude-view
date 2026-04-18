//! GET /api/sessions/:id — Extended session detail (JSONL-first + DB enrichment).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use claude_view_core::{session_stats, task_files, todo_files};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::enrichment::fetch_enrichments;
use super::helpers::build_session_info;
use super::types::{CommitWithTier, DerivedMetrics, SessionDetail};

/// GET /api/sessions/:id — extended session detail.
///
/// Pipeline (same shape as the list endpoint, scoped to one id):
///   1. `SessionCatalog::get` — id → file metadata.
///   2. `session_stats::extract_stats` — JSONL on-demand compute.
///   3. `enrichment::fetch_enrichments` — bulk DB layer (archived, commits_count,
///      skills, reedit_rate). Single-id call here; same layer as list path.
///   4. `get_commits_for_session` — per-session linked commit detail (full tier +
///      evidence), not derivable from JSONL so still DB-backed.
///   5. Read task/todo/plan files from `~/.claude/{tasks,todos,plans}`.
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
    // Phase 3 PR 3.3: try session_stats first (single indexed SELECT);
    // fall back to SessionCatalog + JSONL parse if the adapter returns
    // None (not-yet-indexed, env-var override, DB error).
    let (row, stats) = match state.session_catalog_adapter.get_full(&session_id).await {
        Ok(Some(full)) if full.project_id.is_some() && full.file_path.is_some() => {
            // Happy path — DB row has everything the detail view needs.
            let cat_row = crate::session_catalog_adapter::full_row_to_catalog_row(&full)
                .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
            let stats: claude_view_core::session_stats::SessionStats = (&full).into();
            (cat_row, stats)
        }
        _ => {
            // Fallback: catalog + JSONL parse. Same behaviour as pre-Phase-3.
            // Hit when env var is set, the session hasn't been indexed yet,
            // or the DB query failed.
            let row = state
                .session_catalog
                .get(&session_id)
                .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
            let stats = session_stats::extract_stats(&row.file_path, row.is_compressed)
                .map_err(|e| ApiError::Internal(format!("JSONL parse failed: {e}")))?;
            (row, stats)
        }
    };

    // DB enrichment — archived, commit_count, skills_used, reedit_rate.
    //    `linked_commits` isn't populated here; we fetch tier/evidence via the
    //    dedicated commits query below so the response preserves CommitWithTier.
    let enrichment_map = fetch_enrichments(&state.db, std::slice::from_ref(&session_id))
        .await
        .map_err(ApiError::from)?;
    let enrichment = enrichment_map.get(&session_id).cloned().unwrap_or_default();

    // Build the base SessionInfo, then layer in the DB-sourced fields the
    // list path also applies.
    let mut info = build_session_info(&row, &stats, &state.pricing);
    info.skills_used = enrichment.skills_used.clone();
    info.commit_count = enrichment.commit_count as u32;
    info.reedited_files_count =
        (enrichment.reedit_rate * stats.files_edited_count as f32).round() as u32;

    // 4. Linked commits with tier — DB-only, not derivable from JSONL.
    let commits_raw = state.db.get_commits_for_session(&session_id).await?;
    let commits: Vec<CommitWithTier> = commits_raw.into_iter().map(Into::into).collect();

    let derived_metrics = DerivedMetrics::from(&info);

    // 5. Task / todo / plan sidecar files.
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

    let todos = match todo_files::claude_todos_dir() {
        Some(dir) => todo_files::parse_session_todos(&dir, &session_id),
        None => Vec::new(),
    };

    let has_plans =
        info.slug.as_ref().is_some_and(
            |slug| match claude_view_core::plan_files::claude_plans_dir() {
                Some(dir) => claude_view_core::plan_files::has_plan_files(&dir, slug),
                None => {
                    warnings.push(
                        "Failed to check plan files: could not resolve ~/.claude/plans directory"
                            .into(),
                    );
                    false
                }
            },
        );

    Ok(Json(SessionDetail {
        info,
        commits,
        derived_metrics,
        tasks,
        todos,
        has_plans,
        warnings,
    }))
}
