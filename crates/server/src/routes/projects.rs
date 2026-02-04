//! Projects listing and per-project session endpoints.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use vibe_recall_core::{ProjectSummary, SessionsPage};
use vibe_recall_db::BranchCount;

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/projects - List all projects as lightweight summaries.
///
/// Returns ProjectSummary[] (no sessions array). ~2 KB for 10 projects.
pub async fn list_projects(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ProjectSummary>>> {
    let summaries = state.db.list_project_summaries().await?;
    Ok(Json(summaries))
}

/// Query parameters for paginated sessions endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_sort")]
    pub sort: String,
    pub branch: Option<String>,
    #[serde(default, alias = "include_sidechains")]
    pub include_sidechains: bool,
}

fn default_limit() -> i64 { 50 }
fn default_sort() -> String { "recent".to_string() }

/// Response from GET /api/projects/:id/branches
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct BranchesResponse {
    pub branches: Vec<BranchCount>,
}

/// GET /api/projects/:id/sessions - Paginated sessions for a project.
pub async fn list_project_sessions(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
    Query(params): Query<SessionsQuery>,
) -> ApiResult<Json<SessionsPage>> {
    let page = state
        .db
        .list_sessions_for_project(
            &project_id,
            params.limit,
            params.offset,
            &params.sort,
            params.branch.as_deref(),
            params.include_sidechains,
        )
        .await?;
    Ok(Json(page))
}

/// GET /api/projects/:id/branches - List distinct branches with session counts.
///
/// Returns all unique git_branch values for sessions in this project,
/// sorted by session count descending.
pub async fn list_project_branches(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<BranchesResponse>> {
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
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_core::{SessionInfo, ToolCounts};
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn make_session(id: &str, project: &str, modified_at: i64) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: project.to_string(),
            project_path: format!("/home/user/{}", project),
            file_path: format!(
                "/home/user/.claude/projects/{}/{}.jsonl",
                project, id
            ),
            modified_at,
            size_bytes: 2048,
            preview: format!("Preview for {}", id),
            last_message: format!("Last message for {}", id),
            files_touched: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            // Phase 3: Atomic unit metrics
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
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
            summary_text: None,
            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
        }
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

    #[tokio::test]
    async fn test_projects_returns_summaries() {
        let db = test_db().await;

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 2000);
        let s3 = make_session("sess-3", "project-b", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-b", "Project B").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/projects").await;

        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let projects = json.as_array().expect("should be array");
        assert_eq!(projects.len(), 2);

        // No sessions key
        assert!(projects[0].get("sessions").is_none(), "Should NOT have sessions array");
        // Has sessionCount
        assert!(projects[0].get("sessionCount").is_some(), "Should have sessionCount");
        assert!(projects[0].get("activeCount").is_some());
        assert!(projects[0].get("lastActivityAt").is_some());
    }

    #[tokio::test]
    async fn test_projects_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/projects").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_project_sessions_pagination() {
        let db = test_db().await;

        for i in 1..=5 {
            let s = make_session(&format!("sess-{}", i), "project-a", i as i64 * 1000);
            db.insert_session(&s, "project-a", "Project A").await.unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/projects/project-a/sessions?limit=2&offset=0").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 5);
        assert_eq!(json["sessions"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_project_sessions_sort() {
        let db = test_db().await;

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = make_session("sess-2", "project-a", 3000);

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();

        let app = build_app(db);

        // Sort oldest first
        let (_, body) = do_get(app, "/api/projects/project-a/sessions?sort=oldest").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["sessions"][0]["id"], "sess-1");
    }

    #[tokio::test]
    async fn test_project_sessions_excludes_sidechains() {
        let db = test_db().await;

        let s1 = make_session("sess-1", "project-a", 1000);
        let s2 = SessionInfo { is_sidechain: true, ..make_session("sess-2", "project-a", 2000) };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();

        let app = build_app(db);

        // Default: exclude sidechains
        let (_, body) = do_get(app.clone(), "/api/projects/project-a/sessions").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 1);

        // Include sidechains
        let (_, body) = do_get(app, "/api/projects/project-a/sessions?includeSidechains=true").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total"], 2);
    }

    #[tokio::test]
    async fn test_project_branches_returns_counts() {
        let db = test_db().await;

        // Create sessions with different branches
        let s1 = SessionInfo {
            git_branch: Some("main".to_string()),
            ..make_session("sess-1", "project-a", 1000)
        };
        let s2 = SessionInfo {
            git_branch: Some("main".to_string()),
            ..make_session("sess-2", "project-a", 2000)
        };
        let s3 = SessionInfo {
            git_branch: Some("feature/auth".to_string()),
            ..make_session("sess-3", "project-a", 3000)
        };
        let s4 = SessionInfo {
            git_branch: None,
            ..make_session("sess-4", "project-a", 4000)
        };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();
        db.insert_session(&s3, "project-a", "Project A").await.unwrap();
        db.insert_session(&s4, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/projects/project-a/branches").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let branches = json["branches"].as_array().expect("should have branches array");

        assert_eq!(branches.len(), 3, "should have 3 distinct branches");

        // First should be main with count 2 (sorted by count DESC)
        assert_eq!(branches[0]["branch"], "main");
        assert_eq!(branches[0]["count"], 2);

        // Second and third are both count 1, so order may vary
        // Just verify they exist
        let has_feature_auth = branches.iter().any(|b| b["branch"] == "feature/auth" && b["count"] == 1);
        let has_null = branches.iter().any(|b| b["branch"].is_null() && b["count"] == 1);

        assert!(has_feature_auth, "should have feature/auth branch with count 1");
        assert!(has_null, "should have null branch with count 1");
    }

    #[tokio::test]
    async fn test_project_branches_empty_project() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/projects/nonexistent/branches").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let branches = json["branches"].as_array().expect("should have branches array");
        assert_eq!(branches.len(), 0, "should return empty array for nonexistent project");
    }

    #[tokio::test]
    async fn test_project_branches_excludes_sidechains() {
        let db = test_db().await;

        let s1 = SessionInfo {
            git_branch: Some("main".to_string()),
            ..make_session("sess-1", "project-a", 1000)
        };
        let s2 = SessionInfo {
            git_branch: Some("feature/sidechain".to_string()),
            is_sidechain: true,
            ..make_session("sess-2", "project-a", 2000)
        };

        db.insert_session(&s1, "project-a", "Project A").await.unwrap();
        db.insert_session(&s2, "project-a", "Project A").await.unwrap();

        let app = build_app(db);
        let (_, body) = do_get(app, "/api/projects/project-a/branches").await;

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let branches = json["branches"].as_array().expect("should have branches array");

        // Should only see main, not the sidechain branch
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0]["branch"], "main");
        assert_eq!(branches[0]["count"], 1);
    }
}
