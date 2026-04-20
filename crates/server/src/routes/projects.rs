//! Projects listing and per-project session endpoints — JSONL-first.

use std::path::Path as FsPath;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use claude_view_core::session_catalog::{Filter as CatFilter, Sort as CatSort};
use claude_view_core::{session_stats, ProjectSummary, SessionInfo, SessionsPage};
use claude_view_db::BranchCount;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ApiResult;
use crate::state::AppState;

use super::sessions::enrichment::fetch_enrichments;
use super::sessions::helpers::build_session_info;

/// GET /api/projects — list all projects backed by the in-memory catalog.
///
/// `is_archived` is derived from filesystem existence of the project directory —
/// if the dir has been deleted, the project is flagged archived. This matches
/// the old DB-backed behavior exactly; the SQL query used `COUNT dir_exists`.
#[utoipa::path(get, path = "/api/projects", tag = "projects",
    responses(
        (status = 200, description = "List of project summaries", body = Vec<claude_view_core::ProjectSummary>),
    )
)]
pub async fn list_projects(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ProjectSummary>>> {
    // Phase 3 PR 3.1: read from `session_stats` via the catalog adapter
    // instead of the in-memory `SessionCatalog`. Phase 7.d retired the
    // legacy-read env var; the adapter still falls back to the in-memory
    // map for rows whose `project_id` is NULL (pre-migration-66).
    //
    // Two parallel reads — one count, one last-activity — are issued to
    // the DB in sequence. The design targets ≤5 ms p99; two indexed
    // GROUP-BY queries on session_stats hit that comfortably.
    let project_counts = state.session_catalog_adapter.projects().await;
    let last_activity_map = state
        .session_catalog_adapter
        .projects_with_last_activity()
        .await;

    let mut summaries: Vec<ProjectSummary> = project_counts
        .into_iter()
        .map(|(project_id, session_count)| {
            let last_activity_at = last_activity_map.get(&project_id).and_then(|v| *v);

            // Project dir existence check — encoded id decodes ambiguously so
            // we walk `~/.claude/projects` looking for a matching subdir.
            let is_archived = !project_dir_exists(&project_id);

            ProjectSummary {
                name: project_id.clone(),
                display_name: project_id.clone(),
                path: project_id,
                session_count,
                active_count: 0, // live-session counter lives on live_sessions map, not here
                last_activity_at,
                is_archived,
            }
        })
        .collect();

    summaries.sort_unstable_by(|a, b| {
        b.last_activity_at
            .unwrap_or(0)
            .cmp(&a.last_activity_at.unwrap_or(0))
    });
    Ok(Json(summaries))
}

/// Check if `~/.claude/projects/<project_id>/` exists as a directory.
fn project_dir_exists(project_id: &str) -> bool {
    let Some(home) = dirs::home_dir() else {
        return true; // best-effort — if HOME can't be resolved, assume not archived
    };
    let path = home.join(".claude").join("projects").join(project_id);
    FsPath::new(&path).is_dir()
}

/// Query parameters for paginated sessions endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SessionsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_sort")]
    pub sort: String,
    /// Accepted for API compatibility but not yet honored — branch filtering
    /// requires per-session JSONL parse + git-branch derivation. Deferred.
    pub branch: Option<String>,
    /// Accepted for API compatibility; the catalog already excludes sidechains.
    #[serde(default, alias = "include_sidechains")]
    pub include_sidechains: bool,
}

fn default_limit() -> i64 {
    50
}
fn default_sort() -> String {
    "recent".to_string()
}

/// Response from GET /api/projects/:id/branches
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct BranchesResponse {
    pub branches: Vec<BranchCount>,
}

/// GET /api/projects/:id/sessions — paginated sessions for one project.
#[utoipa::path(get, path = "/api/projects/{id}/sessions", tag = "projects",
    params(
        ("id" = String, Path, description = "Project ID or git root path (URL-encoded)"),
        SessionsQuery,
    ),
    responses(
        (status = 200, description = "Paginated sessions for a project", body = claude_view_core::SessionsPage),
    )
)]
pub async fn list_project_sessions(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Query(params): Query<SessionsQuery>,
) -> ApiResult<Json<SessionsPage>> {
    let cat_filter = CatFilter::by_project(&project_id);
    let cat_sort = match params.sort.as_str() {
        "oldest" => CatSort::LastTsAsc,
        _ => CatSort::LastTsDesc,
    };

    // Phase 3 PR 3.2: the hot path is a single SELECT over session_stats
    // with all columns. Each row produces a SessionInfo with zero JSONL
    // I/O. Legacy path (env-var flip or DB failure) still parses JSONL
    // per session via the catalog + session_stats::extract_stats pair.
    let pricing = &state.pricing;
    let mut all_sessions: Vec<SessionInfo> = match state
        .session_catalog_adapter
        .list_full(&cat_filter, cat_sort, usize::MAX)
        .await
    {
        Ok(rows) => rows
            .iter()
            .filter_map(|row| {
                let cat_row = crate::session_catalog_adapter::full_row_to_catalog_row(row)?;
                let db_stats: claude_view_core::session_stats::SessionStats = row.into();
                Some(build_session_info(&cat_row, &db_stats, pricing))
            })
            .collect(),
        Err(()) => {
            // Legacy fallback: env-var forced the old code path or
            // the DB query failed. Re-parse each JSONL via the
            // in-memory catalog — matches pre-Phase-3 behaviour.
            let rows = state
                .session_catalog
                .list(&cat_filter, cat_sort, usize::MAX);
            rows.iter()
                .filter_map(|row| {
                    session_stats::extract_stats(&row.file_path, row.is_compressed)
                        .ok()
                        .map(|stats| build_session_info(row, &stats, pricing))
                })
                .collect()
        }
    };

    // Layer DB-only fields (archived, commits, skills, reedit) so the caller
    // can filter/display them. Cheap: one query regardless of result size.
    let ids: Vec<String> = all_sessions.iter().map(|s| s.id.clone()).collect();
    let enrichment_map = fetch_enrichments(&state.db, &ids).await?;
    for info in &mut all_sessions {
        if let Some(enr) = enrichment_map.get(&info.id) {
            info.skills_used = enr.skills_used.clone();
            info.commit_count = enr.commit_count as u32;
        }
    }

    // Apply per-sort tweaks on top of the catalog ordering.
    if let "messages" = params.sort.as_str() {
        all_sessions.sort_by(|a, b| b.message_count.cmp(&a.message_count));
    }

    let total = all_sessions.len();
    let offset = params.offset.max(0) as usize;
    let limit = params.limit.max(1) as usize;
    let sessions: Vec<SessionInfo> = all_sessions.into_iter().skip(offset).take(limit).collect();

    Ok(Json(SessionsPage { sessions, total }))
}

/// GET /api/projects/:id/branches - List distinct branches with session counts.
#[utoipa::path(get, path = "/api/projects/{id}/branches", tag = "projects",
    params(("id" = String, Path, description = "Project ID or git root path (URL-encoded)")),
    responses(
        (status = 200, description = "Distinct branches with session counts", body = BranchesResponse),
    )
)]
pub async fn list_project_branches(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<BranchesResponse>> {
    // Branches require git_branch column data that isn't derivable from the
    // catalog yet. Kept DB-backed until branch extraction lands in session_stats.
    let branches = state.db.list_branches_for_project(&project_id).await?;
    Ok(Json(BranchesResponse { branches }))
}

/// Create the projects routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/projects", get(list_projects))
        .route("/projects/{id}/sessions", get(list_project_sessions))
        .route("/projects/{id}/branches", get(list_project_branches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn list_projects_returns_empty_when_catalog_is_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let app = crate::create_app(db);
        let (status, body) = do_get(app, "/api/projects").await;
        assert_eq!(status, StatusCode::OK);
        let parsed: Vec<ProjectSummary> = serde_json::from_str(&body).unwrap();
        assert!(parsed.is_empty());
    }

    /// Phase 3 PR 3.1 — verifies the cutover reads from `session_stats`.
    /// Seeds two session_stats rows spanning two project_ids and
    /// asserts both show up in the handler's response with correct
    /// counts and last-activity timestamps.
    #[tokio::test]
    async fn list_projects_reads_from_session_stats() {
        let db = Database::new_in_memory().await.unwrap();

        // Seed two rows — one per project — directly via SQL. Bypasses the
        // indexer to keep this unit test tight; the indexer's own round-trip
        // tests cover writer correctness.
        for (idx, (sid, pid, last_ts)) in [
            ("s1", "proj-alpha", 1_800_000_000_i64),
            ("s2", "proj-alpha", 1_800_000_500_i64),
            ("s3", "proj-bravo", 1_700_000_000_i64),
        ]
        .into_iter()
        .enumerate()
        {
            sqlx::query(
                r#"INSERT INTO session_stats (
                       session_id, source_content_hash, source_size,
                       parser_version, stats_version, indexed_at,
                       last_message_at,
                       project_id, file_path, is_compressed, source_mtime
                   ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(sid)
            .bind(vec![idx as u8])
            .bind(1_i64)
            .bind(1_i64)
            .bind(1_i64)
            .bind(1_i64)
            .bind(last_ts)
            .bind(pid)
            .bind(format!("/tmp/{pid}/{sid}.jsonl"))
            .bind(0_i64)
            .bind(last_ts - 10)
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = crate::create_app(db);
        let (status, body) = do_get(app, "/api/projects").await;
        assert_eq!(status, StatusCode::OK);

        let parsed: Vec<ProjectSummary> = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.len(), 2, "must return both seeded projects");

        // Response is sorted by last_activity_at DESC. For proj-alpha,
        // the adapter's query uses MAX(COALESCE(last_message_at, source_mtime)),
        // so the newer session (s2) drives last_activity_at.
        assert_eq!(parsed[0].name, "proj-alpha");
        assert_eq!(parsed[0].session_count, 2);
        assert_eq!(parsed[0].last_activity_at, Some(1_800_000_500));
        assert_eq!(parsed[1].name, "proj-bravo");
        assert_eq!(parsed[1].session_count, 1);
        assert_eq!(parsed[1].last_activity_at, Some(1_700_000_000));
    }
}
