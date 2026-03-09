//! Workflow management endpoints.
//!
//! GET  /api/workflows          — list all workflows (official + user)
//! GET  /api/workflows/:id      — get single workflow YAML + metadata
//! POST /api/workflows          — create user workflow (save YAML to disk)
//! DELETE /api/workflows/:id    — delete user workflow (official: returns 404 — only user dir checked)

#[allow(unused_imports)]
use std::convert::Infallible;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use std::sync::Arc;

#[allow(unused_imports)]
use axum::extract::{Path, State};
#[allow(unused_imports)]
use axum::response::sse::{Event, Sse};
#[allow(unused_imports)]
use axum::routing::get;
#[allow(unused_imports)]
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[allow(unused_imports)]
use crate::error::{ApiError, ApiResult};
#[allow(unused_imports)]
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
