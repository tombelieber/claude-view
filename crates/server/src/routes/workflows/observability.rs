//! Read-only Claude Code workflow observability endpoints.
//!
//! GET /api/workflows/runs                                   — list dynamic runs
//! GET /api/workflows/runs/{session}/{run}                   — run detail
//! GET /api/workflows/runs/{session}/{run}/agents/{agent}    — agent detail
//! GET /api/claude-home                                      — safe ~/.claude browser
//!
//! All data is scanned on demand from `~/.claude`; nothing is persisted and no
//! workflow script is ever executed. See `claude_view_core::workflow_files`.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use claude_view_core::workflow_files::{
    claude_home_dir, get_workflow_agent, get_workflow_run, scan_claude_home_entries,
    scan_workflow_runs, ClaudeHomeEntry, WorkflowAgentDetail, WorkflowArtifactError,
    WorkflowRunDetail, WorkflowRunSummary,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunsResponse {
    pub runs: Vec<WorkflowRunSummary>,
}

fn workflow_claude_home() -> ApiResult<PathBuf> {
    claude_home_dir().ok_or_else(|| ApiError::Internal("Unable to resolve Claude home".to_string()))
}

fn workflow_artifact_error(error: WorkflowArtifactError) -> ApiError {
    ApiError::BadRequest(error.to_string())
}

#[utoipa::path(get, path = "/api/workflows/runs", tag = "workflows",
    responses((status = 200, description = "Claude Code dynamic workflow runs", body = WorkflowRunsResponse))
)]
pub async fn list_workflow_runs(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<WorkflowRunsResponse>> {
    let home = workflow_claude_home()?;
    let scan = scan_workflow_runs(&home);
    for warning in &scan.warnings {
        tracing::warn!(warning = %warning, "Workflow artifact scan warning");
    }
    Ok(Json(WorkflowRunsResponse { runs: scan.runs }))
}

#[utoipa::path(get, path = "/api/workflows/runs/{session_id}/{run_id}", tag = "workflows",
    params(
        ("session_id" = String, Path, description = "Claude Code session ID"),
        ("run_id" = String, Path, description = "Workflow run ID"),
    ),
    responses(
        (status = 200, description = "Claude Code workflow run detail", body = WorkflowRunDetail),
        (status = 404, description = "Workflow run not found"),
    )
)]
pub async fn get_workflow_run_detail(
    State(_state): State<Arc<AppState>>,
    Path((session_id, run_id)): Path<(String, String)>,
) -> ApiResult<Json<WorkflowRunDetail>> {
    let home = workflow_claude_home()?;
    let detail = get_workflow_run(&home, &session_id, &run_id).map_err(workflow_artifact_error)?;
    detail
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Workflow run '{run_id}' not found")))
}

#[utoipa::path(get, path = "/api/workflows/runs/{session_id}/{run_id}/agents/{agent_id}", tag = "workflows",
    params(
        ("session_id" = String, Path, description = "Claude Code session ID"),
        ("run_id" = String, Path, description = "Workflow run ID"),
        ("agent_id" = String, Path, description = "Workflow-scoped agent ID"),
    ),
    responses(
        (status = 200, description = "Claude Code workflow agent detail", body = WorkflowAgentDetail),
        (status = 404, description = "Workflow agent not found"),
    )
)]
pub async fn get_workflow_agent_detail(
    State(_state): State<Arc<AppState>>,
    Path((session_id, run_id, agent_id)): Path<(String, String, String)>,
) -> ApiResult<Json<WorkflowAgentDetail>> {
    let home = workflow_claude_home()?;
    let detail = get_workflow_agent(&home, &session_id, &run_id, &agent_id)
        .map_err(workflow_artifact_error)?;
    detail.map(Json).ok_or_else(|| {
        ApiError::NotFound(format!(
            "Workflow agent '{agent_id}' for run '{run_id}' not found"
        ))
    })
}

#[utoipa::path(get, path = "/api/claude-home", tag = "claude-home",
    responses((status = 200, description = "Safe Claude home metadata and previews", body = Vec<ClaudeHomeEntry>))
)]
pub async fn list_claude_home(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ClaudeHomeEntry>>> {
    let home = workflow_claude_home()?;
    Ok(Json(scan_claude_home_entries(&home)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::workflows::router;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        Router,
    };
    use claude_view_db::Database;
    use serial_test::serial;
    use std::ffi::OsString;
    use std::fs;
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// Scopes a `CLAUDE_HOME` override to a fixture dir, restoring the prior
    /// value on drop. Tests touching the process env must be `#[serial]`.
    struct EnvGuard {
        key: &'static str,
        old: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old.as_ref() {
                std::env::set_var(self.key, old);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    async fn do_request(
        app: Router,
        method: Method,
        uri: &str,
        body: Option<&str>,
    ) -> (StatusCode, String) {
        let mut builder = Request::builder().method(method).uri(uri);
        let body = if let Some(json) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(json.to_string())
        } else {
            Body::empty()
        };
        let resp = app.oneshot(builder.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn build_app() -> Router {
        Router::new()
            .nest("/api", router())
            .with_state(AppState::new(test_db().await))
    }

    #[tokio::test]
    #[serial]
    async fn test_list_workflow_runs_uses_claude_home_fixture() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        let workflows = tmp
            .path()
            .join("projects")
            .join("proj-route")
            .join("sess-route")
            .join("workflows");
        fs::create_dir_all(&workflows).unwrap();
        fs::write(
            workflows.join("wf_route.json"),
            r#"{"runId":"wf_route","workflowName":"Route Run","status":"completed","totalTokens":99}"#,
        )
        .unwrap();

        let app = build_app().await;
        let (status, body) = do_request(app, Method::GET, "/api/workflows/runs", None).await;

        assert_eq!(status, StatusCode::OK);
        let response: WorkflowRunsResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(response.runs.len(), 1);
        assert_eq!(response.runs[0].workflow_name, "Route Run");
        assert_eq!(response.runs[0].total_tokens, 99);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_workflow_run_and_agent_detail_routes() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        let session_dir = tmp
            .path()
            .join("projects")
            .join("proj-route")
            .join("sess-route");
        let workflows = session_dir.join("workflows");
        let run_dir = session_dir
            .join("subagents")
            .join("workflows")
            .join("wf_route");
        fs::create_dir_all(&workflows).unwrap();
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(
            workflows.join("wf_route.json"),
            serde_json::json!({
                "runId": "wf_route",
                "workflowName": "Route Run",
                "status": "completed",
                "workflowProgress": [{
                    "type": "workflow_agent",
                    "agentId": "abc",
                    "state": "completed",
                    "promptPreview": "Prompt",
                    "resultPreview": "Result"
                }]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            run_dir.join("agent-abc.jsonl"),
            r#"{"message":{"role":"assistant","content":"agent result"}}"#,
        )
        .unwrap();

        let app = build_app().await;
        let (status, body) = do_request(
            app.clone(),
            Method::GET,
            "/api/workflows/runs/sess-route/wf_route",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let detail: WorkflowRunDetail = serde_json::from_str(&body).unwrap();
        assert_eq!(detail.summary.workflow_name, "Route Run");
        assert_eq!(detail.agents.len(), 1);

        let (status, body) = do_request(
            app,
            Method::GET,
            "/api/workflows/runs/sess-route/wf_route/agents/abc",
            None,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let agent: WorkflowAgentDetail = serde_json::from_str(&body).unwrap();
        assert_eq!(agent.summary.agent_id, "abc");
        assert_eq!(agent.events.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_run_detail_missing_returns_404() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        let app = build_app().await;
        let (status, _) = do_request(
            app,
            Method::GET,
            "/api/workflows/runs/sess-x/wf_missing",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[serial]
    async fn test_run_detail_invalid_run_id_returns_400() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        let app = build_app().await;
        // `badid` lacks the required `wf_` prefix -> identifier rejected.
        let (status, _) =
            do_request(app, Method::GET, "/api/workflows/runs/sess-x/badid", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    #[serial]
    async fn test_agent_detail_missing_returns_404() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        let app = build_app().await;
        let (status, _) = do_request(
            app,
            Method::GET,
            "/api/workflows/runs/sess-x/wf_missing/agents/ghost",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[serial]
    async fn test_claude_home_route_hides_sensitive_previews() {
        let tmp = TempDir::new().unwrap();
        let _env = EnvGuard::set("CLAUDE_HOME", tmp.path());
        fs::create_dir_all(tmp.path().join("session-env").join("sess")).unwrap();
        fs::write(
            tmp.path().join("session-env").join("sess").join("env.json"),
            r#"{"TOKEN":"secret"}"#,
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join("hooks")).unwrap();
        fs::write(tmp.path().join("hooks").join("stop.sh"), "echo stop").unwrap();

        let app = build_app().await;
        let (status, body) = do_request(app, Method::GET, "/api/claude-home", None).await;

        assert_eq!(status, StatusCode::OK);
        let entries: Vec<ClaudeHomeEntry> = serde_json::from_str(&body).unwrap();
        let session_env = entries
            .iter()
            .find(|entry| entry.kind == "session-env")
            .unwrap();
        assert!(session_env.metadata_only);
        assert_eq!(session_env.preview, None);
        assert!(entries
            .iter()
            .any(|entry| entry.name == "stop.sh" && entry.preview.as_deref() == Some("echo stop")));
    }
}
