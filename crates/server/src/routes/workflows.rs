//! Workflow management endpoints.
//!
//! GET  /api/workflows          — list all workflows (official + user)
//! GET  /api/workflows/:id      — get single workflow definition (JSON)
//! POST /api/workflows          — create user workflow (save JSON to disk)
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

// ---------------------------------------------------------------------------
// Structs — JSON-native schema (nodes/edges, not stages)
// ---------------------------------------------------------------------------

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
pub struct WorkflowDefaults {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting_sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_budget_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
}

impl Default for WorkflowDefaults {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            permission_mode: None,
            setting_sources: None,
            effort: None,
            max_budget_usd: None,
            max_turns: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub category: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub inputs: Vec<WorkflowInput>,
    #[serde(default)]
    pub defaults: WorkflowDefaults,
    /// React Flow node objects — stored as opaque JSON.
    #[serde(default)]
    pub nodes: Vec<serde_json::Value>,
    /// React Flow edge objects — stored as opaque JSON.
    #[serde(default)]
    pub edges: Vec<serde_json::Value>,
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
    pub node_count: usize,
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
}

/// JSON body for creating a workflow — accepts the full definition inline.
#[derive(Debug, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowRequest {
    pub definition: WorkflowDefinition,
}

// ---------------------------------------------------------------------------
// File storage
// ---------------------------------------------------------------------------

fn official_dir() -> PathBuf {
    claude_view_core::paths::workflows_official_dir()
}

fn user_dir() -> PathBuf {
    claude_view_core::paths::workflows_user_dir()
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

/// Read a workflow.json from a `{id}/workflow.json` directory layout.
fn read_workflow_json(dir: &std::path::Path, id: &str) -> Option<WorkflowDefinition> {
    let path = dir.join(id).join("workflow.json");
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Build a summary from a WorkflowDefinition.
fn def_to_summary(def: &WorkflowDefinition, source: &str) -> WorkflowSummary {
    WorkflowSummary {
        id: def.id.clone(),
        name: def.name.clone(),
        description: def.description.clone(),
        category: def.category.clone(),
        author: def.author.clone(),
        version: def.version.clone(),
        node_count: def.nodes.len(),
        source: source.to_string(),
        last_run_at: None,
        run_count: 0,
    }
}

/// Scan a directory for `*/workflow.json` entries and collect summaries.
fn collect_json_summaries(dir: &std::path::Path, source: &str) -> Vec<WorkflowSummary> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        let wf_path = sub.join("workflow.json");
        if !wf_path.exists() {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&wf_path) {
            if let Ok(def) = serde_json::from_str::<WorkflowDefinition>(&raw) {
                out.push(def_to_summary(&def, source));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Legacy YAML → JSON migration
// ---------------------------------------------------------------------------

/// Intermediate struct for deserializing the old YAML stage format.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyStageGate {
    condition: String,
    #[serde(default)]
    retry: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyStage {
    name: String,
    #[serde(default)]
    skills: Vec<String>,
    gate: Option<LegacyStageGate>,
    #[serde(default)]
    parallel: bool,
    model: Option<String>,
    #[serde(default)]
    agents: u32,
}

#[derive(Debug, Deserialize)]
struct LegacyWorkflow {
    name: String,
    description: String,
    author: String,
    category: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    inputs: Vec<WorkflowInput>,
    #[serde(default)]
    stages: Vec<LegacyStage>,
}

/// Convert a legacy YAML workflow into the new JSON WorkflowDefinition.
/// Each stage becomes a node; edges connect them sequentially.
fn convert_legacy_to_definition(id: &str, legacy: &LegacyWorkflow) -> WorkflowDefinition {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut edges: Vec<serde_json::Value> = Vec::new();

    let model = legacy
        .stages
        .first()
        .and_then(|s| s.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    for (i, stage) in legacy.stages.iter().enumerate() {
        let node_id = format!("stage-{i}");
        let node = serde_json::json!({
            "id": node_id,
            "type": "agent",
            "position": { "x": 250, "y": (i as u32) * 200 },
            "data": {
                "label": stage.name,
                "skills": stage.skills,
                "model": stage.model.as_deref().unwrap_or(&model),
                "parallel": stage.parallel,
                "agents": stage.agents,
                "gate": stage.gate.as_ref().map(|g| serde_json::json!({
                    "condition": g.condition,
                    "retry": g.retry,
                })),
            }
        });
        nodes.push(node);

        if i > 0 {
            let edge = serde_json::json!({
                "id": format!("e-stage-{}-stage-{}", i - 1, i),
                "source": format!("stage-{}", i - 1),
                "target": node_id,
            });
            edges.push(edge);
        }
    }

    WorkflowDefinition {
        id: id.to_string(),
        name: legacy.name.clone(),
        description: legacy.description.clone(),
        author: legacy.author.clone(),
        category: legacy.category.clone(),
        version: legacy.version.clone(),
        inputs: legacy.inputs.clone(),
        defaults: WorkflowDefaults {
            model,
            ..Default::default()
        },
        nodes,
        edges,
    }
}

/// Try to migrate a legacy `{id}.yaml` file into `{id}/workflow.json`.
/// Returns the converted definition on success, or None if no legacy file.
fn migrate_legacy_yaml(dir: &std::path::Path, id: &str) -> Option<WorkflowDefinition> {
    let yaml_path = dir.join(format!("{id}.yaml"));
    if !yaml_path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(&yaml_path).ok()?;
    let legacy: LegacyWorkflow = serde_yaml::from_str(&raw).ok()?;
    let def = convert_legacy_to_definition(id, &legacy);

    // Write the new JSON
    let out_dir = dir.join(id);
    let _ = std::fs::create_dir_all(&out_dir);
    let json = serde_json::to_string_pretty(&def).ok()?;
    std::fs::write(out_dir.join("workflow.json"), json).ok()?;

    // Remove old YAML
    let _ = std::fs::remove_file(&yaml_path);

    tracing::info!("Migrated legacy YAML workflow '{id}' to JSON");
    Some(def)
}

/// Scan a directory for legacy `*.yaml` files and migrate them.
fn migrate_all_legacy_yaml(dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
            if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                let _ = migrate_legacy_yaml(dir, id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Startup seeding
// ---------------------------------------------------------------------------

/// Called at server startup — seeds official workflow JSONs if missing,
/// and migrates any legacy YAML files to the new format.
pub fn seed_official_workflows() {
    let dir = official_dir();
    let _ = std::fs::create_dir_all(&dir);

    let samples: &[(&str, &str)] = &[
        (
            "plan-polisher",
            include_str!("workflow_samples/plan-polisher.json"),
        ),
        (
            "plan-executor",
            include_str!("workflow_samples/plan-executor.json"),
        ),
    ];

    for (name, json) in samples {
        let wf_dir = dir.join(name);
        let wf_path = wf_dir.join("workflow.json");
        if !wf_path.exists() {
            let _ = std::fs::create_dir_all(&wf_dir);
            let _ = std::fs::write(&wf_path, json);
        }
    }

    // Migrate any leftover legacy YAML files in official + user dirs
    migrate_all_legacy_yaml(&dir);
    let udir = user_dir();
    if udir.exists() {
        migrate_all_legacy_yaml(&udir);
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
        summaries.extend(collect_json_summaries(dir, source));
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
        // Try JSON first
        if let Some(def) = read_workflow_json(dir, &id) {
            return Ok(Json(WorkflowDetail {
                id,
                source: source.to_string(),
                definition: def,
            }));
        }
        // Fallback: auto-migrate legacy YAML
        if let Some(def) = migrate_legacy_yaml(dir, &id) {
            return Ok(Json(WorkflowDetail {
                id,
                source: source.to_string(),
                definition: def,
            }));
        }
    }
    Err(ApiError::NotFound(format!("Workflow '{id}' not found")))
}

async fn create_workflow(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateWorkflowRequest>,
) -> ApiResult<Json<WorkflowDetail>> {
    let mut definition = req.definition;

    // Derive ID from name if not set
    if definition.id.is_empty() {
        definition.id = definition
            .name
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
    }
    let id = &definition.id;

    if !is_valid_workflow_id(id) {
        return Err(ApiError::BadRequest(
            "Workflow name produces an invalid ID. Use letters, numbers, and hyphens only."
                .to_string(),
        ));
    }

    let wf_dir = user_dir().join(id);
    std::fs::create_dir_all(&wf_dir).map_err(|e| ApiError::Internal(e.to_string()))?;
    let json =
        serde_json::to_string_pretty(&definition).map_err(|e| ApiError::Internal(e.to_string()))?;
    std::fs::write(wf_dir.join("workflow.json"), json)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(WorkflowDetail {
        id: definition.id.clone(),
        source: "user".to_string(),
        definition,
    }))
}

async fn delete_workflow(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    if !is_valid_workflow_id(&id) {
        return Err(ApiError::BadRequest("Invalid workflow ID".to_string()));
    }
    // New format: directory-based
    let dir_path = user_dir().join(&id);
    if dir_path.is_dir() {
        std::fs::remove_dir_all(&dir_path).map_err(|e| ApiError::Internal(e.to_string()))?;
        return Ok(axum::http::StatusCode::NO_CONTENT);
    }
    // Legacy fallback
    let yaml_path = user_dir().join(format!("{id}.yaml"));
    if yaml_path.exists() {
        std::fs::remove_file(&yaml_path).map_err(|e| ApiError::Internal(e.to_string()))?;
        return Ok(axum::http::StatusCode::NO_CONTENT);
    }
    Err(ApiError::NotFound(format!(
        "User workflow '{id}' not found"
    )))
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
    async fn test_create_workflow_json() {
        let _tmp = TempDir::new().unwrap();
        let payload = serde_json::json!({
            "definition": {
                "id": "test-wf",
                "name": "Test Wf",
                "description": "Test",
                "author": "user",
                "category": "Dev",
                "version": "1.0.0",
                "inputs": [],
                "defaults": { "model": "claude-sonnet-4-20250514" },
                "nodes": [],
                "edges": []
            }
        });
        let app = Router::new()
            .nest("/api", router())
            .with_state(AppState::new(test_db().await));
        let (status, body) = do_request(
            app,
            Method::POST,
            "/api/workflows",
            Some(&payload.to_string()),
        )
        .await;
        assert!(status != StatusCode::INTERNAL_SERVER_ERROR, "body: {body}");
    }

    #[test]
    fn test_legacy_yaml_conversion() {
        let yaml = r#"
name: "Test Plan"
description: "A test"
author: "tester"
category: "Dev"
version: "1.0.0"
inputs:
  - name: "plan_file"
    type: "file_path"
    description: "Path to the plan"
stages:
  - name: "Audit"
    skills: ["/auditing-plans"]
    gate:
      condition: "verdict = Pass"
      retry: true
    model: "claude-opus-4-6"
    agents: 1
  - name: "Ship"
    skills: ["/shippable"]
    gate:
      condition: "verdict = SHIP IT"
      retry: false
    model: "claude-opus-4-6"
    agents: 1
"#;
        let legacy: LegacyWorkflow = serde_yaml::from_str(yaml).unwrap();
        let def = convert_legacy_to_definition("test-plan", &legacy);

        assert_eq!(def.id, "test-plan");
        assert_eq!(def.name, "Test Plan");
        assert_eq!(def.nodes.len(), 2);
        assert_eq!(def.edges.len(), 1);
        assert_eq!(def.inputs.len(), 1);
        assert_eq!(def.defaults.model, "claude-opus-4-6");

        // Verify node structure
        let node0 = &def.nodes[0];
        assert_eq!(node0["id"], "stage-0");
        assert_eq!(node0["type"], "agent");
        assert_eq!(node0["data"]["label"], "Audit");

        // Verify edge connects stage-0 → stage-1
        let edge0 = &def.edges[0];
        assert_eq!(edge0["source"], "stage-0");
        assert_eq!(edge0["target"], "stage-1");
    }
}
