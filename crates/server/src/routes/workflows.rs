//! Workflow management endpoints.
//!
//! GET  /api/workflows          — list all workflows (official + user)
//! GET  /api/workflows/:id      — get single workflow YAML + metadata
//! POST /api/workflows          — create user workflow (save YAML to disk)
//! DELETE /api/workflows/:id    — delete user workflow (official: returns 404 — only user dir checked)

#[allow(unused_imports)]
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
#[allow(unused_imports)]
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// Note: `Infallible`, `Sse`, `Event` are used in Task 10 (chat_workflow handler).
// All imports are declared upfront per Rust convention — unused warnings only
// appear if Task 10 is not yet implemented.

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageGate {
    pub condition: String,
    pub retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStage {
    pub name: String,
    pub skills: Vec<String>,
    pub gate: Option<WorkflowStageGate>,
    #[serde(default)]
    pub parallel: bool,
    pub model: Option<String>,
    #[serde(default)]
    pub agents: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInput {
    pub name: String,
    pub r#type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    pub name: String,
    pub description: String,
    pub author: String,
    pub category: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub stages: Vec<WorkflowStage>,
    #[serde(default)]
    pub inputs: Vec<WorkflowInput>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[cfg_attr(test, derive(Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub author: String,
    pub version: String,
    pub stage_count: usize,
    /// "official" | "user"
    pub source: String,
    pub last_run_at: Option<i64>,
    pub run_count: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDetail {
    pub id: String,
    pub source: String,
    pub definition: WorkflowDefinition,
    /// Raw YAML string (for Preview tab)
    pub yaml: String,
}

#[derive(Debug, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowRequest {
    pub yaml: String,
}

// ---------------------------------------------------------------------------
// File storage
// ---------------------------------------------------------------------------

fn home() -> PathBuf {
    dirs::home_dir().expect("home directory must be available")
}

fn official_dir() -> PathBuf {
    home()
        .join(".claude-view")
        .join("workflows")
        .join("official")
}

fn user_dir() -> PathBuf {
    home().join(".claude-view").join("workflows").join("user")
}

/// Validate a workflow ID: non-empty, max 64 chars, starts with ASCII alpha,
/// only alphanumeric, hyphens, and underscores. Prevents path traversal.
fn is_valid_workflow_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.starts_with(|c: char| c.is_ascii_alphabetic())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !id.contains("..")
}

fn to_summary(path: &std::path::Path, source: &str) -> Option<WorkflowSummary> {
    let id = path.file_stem()?.to_string_lossy().to_string();
    let raw = std::fs::read_to_string(path).ok()?;
    let def: WorkflowDefinition = serde_yaml::from_str(&raw).ok()?;
    Some(WorkflowSummary {
        id,
        name: def.name,
        description: def.description,
        category: def.category,
        author: def.author,
        version: def.version,
        stage_count: def.stages.len(),
        source: source.to_string(),
        last_run_at: None,
        run_count: 0,
    })
}

// ---------------------------------------------------------------------------
// Startup seeding
// ---------------------------------------------------------------------------

/// Called at server startup — seeds official workflow YAMLs if missing.
pub fn seed_official_workflows() {
    let dir = official_dir();
    let _ = std::fs::create_dir_all(&dir);

    let samples: &[(&str, &str)] = &[
        (
            "plan-polisher",
            include_str!("workflow_samples/plan-polisher.yaml"),
        ),
        (
            "plan-executor",
            include_str!("workflow_samples/plan-executor.yaml"),
        ),
    ];

    for (name, yaml) in samples {
        let path = dir.join(format!("{name}.yaml"));
        if !path.exists() {
            let _ = std::fs::write(&path, yaml);
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_workflows(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<WorkflowSummary>>> {
    let mut summaries: Vec<WorkflowSummary> = Vec::new();
    for (dir, source) in &[(official_dir(), "official"), (user_dir(), "user")] {
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(dir)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                if let Some(s) = to_summary(&path, source) {
                    summaries.push(s);
                }
            }
        }
    }
    // Official first, then alphabetical within group
    summaries.sort_by(|a, b| a.source.cmp(&b.source).reverse().then(a.name.cmp(&b.name)));
    Ok(Json(summaries))
}

async fn get_workflow(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<WorkflowDetail>> {
    if !is_valid_workflow_id(&id) {
        return Err(ApiError::BadRequest("Invalid workflow ID".to_string()));
    }
    for (dir, source) in &[(official_dir(), "official"), (user_dir(), "user")] {
        let path = dir.join(format!("{id}.yaml"));
        if path.exists() {
            let yaml =
                std::fs::read_to_string(&path).map_err(|e| ApiError::Internal(e.to_string()))?;
            let definition: WorkflowDefinition = serde_yaml::from_str(&yaml)
                .map_err(|e| ApiError::BadRequest(format!("Invalid YAML: {e}")))?;
            return Ok(Json(WorkflowDetail {
                id,
                source: source.to_string(),
                definition,
                yaml,
            }));
        }
    }
    Err(ApiError::NotFound(format!("Workflow '{id}' not found")))
}

async fn create_workflow(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateWorkflowRequest>,
) -> ApiResult<Json<WorkflowDetail>> {
    let definition: WorkflowDefinition = serde_yaml::from_str(&req.yaml)
        .map_err(|e| ApiError::BadRequest(format!("Invalid YAML: {e}")))?;
    let id = definition
        .name
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if !is_valid_workflow_id(&id) {
        return Err(ApiError::BadRequest(
            "Workflow name produces an invalid ID. Use letters, numbers, and hyphens only."
                .to_string(),
        ));
    }
    let dir = user_dir();
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(e.to_string()))?;
    std::fs::write(dir.join(format!("{id}.yaml")), &req.yaml)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(WorkflowDetail {
        id,
        source: "user".to_string(),
        definition,
        yaml: req.yaml,
    }))
}

async fn delete_workflow(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    if !is_valid_workflow_id(&id) {
        return Err(ApiError::BadRequest("Invalid workflow ID".to_string()));
    }
    let path = user_dir().join(format!("{id}.yaml"));
    if !path.exists() {
        return Err(ApiError::NotFound(format!(
            "User workflow '{id}' not found"
        )));
    }
    std::fs::remove_file(&path).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Workflow chat (POST → SSE streaming)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    workflow_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    role: String,
    content: String,
}

/// POST /api/workflows/chat — direct POST-to-SSE streaming (same pattern as generate_report())
async fn chat_workflow(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> ApiResult<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>> {
    let messages = req.messages;
    let stream = async_stream::stream! {
        // TODO Phase 8: call Claude API with messages + system prompt, stream deltas.
        let _ = messages;
        yield Ok(Event::default().event("chunk").data(r#"{"delta":""}"#));
        yield Ok(Event::default().event("done").data("{}"));
    };
    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

// ---------------------------------------------------------------------------
// Run control
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunControlRequest {
    command: String, // "pause" | "skip" | "abort"
}

async fn control_run(
    State(_state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    Json(body): Json<RunControlRequest>,
) -> ApiResult<impl IntoResponse> {
    // TODO Phase 8: forward to sidecar runner via tokio channel
    tracing::info!("Workflow run {run_id} control command: {}", body.command);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/workflows", get(list_workflows).post(create_workflow))
        // Literal routes MUST be registered before parameterised routes —
        // otherwise `/workflows/chat` is captured by `{id}` as id="chat".
        .route("/workflows/chat", post(chat_workflow))
        .route("/workflows/run/{run_id}/control", post(control_run))
        .route("/workflows/{id}", get(get_workflow).delete(delete_workflow))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        Router,
    };
    use claude_view_db::Database;
    use tempfile::TempDir;
    use tower::ServiceExt;

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

    #[test]
    fn test_valid_workflow_ids() {
        assert!(is_valid_workflow_id("plan-polisher"));
        assert!(is_valid_workflow_id("MyWorkflow"));
        assert!(is_valid_workflow_id("a1b2-c3"));
    }

    #[test]
    fn test_invalid_workflow_ids() {
        assert!(!is_valid_workflow_id(""));
        assert!(!is_valid_workflow_id("../etc/passwd"));
        assert!(!is_valid_workflow_id("1-starts-with-digit"));
        assert!(!is_valid_workflow_id("has spaces"));
        assert!(!is_valid_workflow_id(&"a".repeat(65)));
    }

    #[tokio::test]
    async fn test_list_workflows_returns_ok() {
        let app = Router::new()
            .nest("/api", router())
            .with_state(AppState::new(test_db().await));
        let (status, body) = do_request(app, Method::GET, "/api/workflows", None).await;
        assert_eq!(status, StatusCode::OK);
        let summaries: Vec<WorkflowSummary> = serde_json::from_str(&body).unwrap();
        assert!(
            summaries.len() <= 100,
            "Unexpected workflow count: {}",
            summaries.len()
        );
    }

    #[tokio::test]
    async fn test_get_workflow_not_found() {
        let app = Router::new()
            .nest("/api", router())
            .with_state(AppState::new(test_db().await));
        let (status, _) = do_request(app, Method::GET, "/api/workflows/nonexistent", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_and_get_workflow() {
        let _tmp = TempDir::new().unwrap();
        let yaml = "name: \"Test Wf\"\ndescription: \"Test\"\nauthor: \"user\"\ncategory: \"Dev\"\nstages: []";
        let app = Router::new()
            .nest("/api", router())
            .with_state(AppState::new(test_db().await));
        let payload = serde_json::json!({ "yaml": yaml });
        let (status, body) = do_request(
            app,
            Method::POST,
            "/api/workflows",
            Some(&payload.to_string()),
        )
        .await;
        assert!(status != StatusCode::INTERNAL_SERVER_ERROR, "body: {body}");
    }
}
