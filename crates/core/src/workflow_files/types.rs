//! Serializable types for the workflow observability + Claude home surfaces.
//!
//! These cross the HTTP boundary (utoipa/ts-rs) so field shapes are part of the
//! API contract. Internal-only helper structs (`WorkflowArtifact`, `ParsedRun`)
//! live here too but are not exported.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use utoipa::ToSchema;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowArtifactError {
    #[error("invalid {0}")]
    InvalidIdentifier(&'static str),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPhaseSummary {
    pub index: u32,
    pub title: String,
    pub detail: Option<String>,
    pub agent_count: u32,
    pub completed_agent_count: u32,
    #[ts(type = "number")]
    pub token_count: u64,
    #[ts(type = "number")]
    pub tool_call_count: u64,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgentSummary {
    pub agent_id: String,
    pub label: Option<String>,
    pub phase_index: Option<u32>,
    pub phase_title: Option<String>,
    pub model: Option<String>,
    pub state: String,
    pub started_at: Option<String>,
    pub queued_at: Option<String>,
    pub last_progress_at: Option<String>,
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub tool_calls: u64,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub prompt_preview: Option<String>,
    pub result_preview: Option<String>,
    pub events_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunSummary {
    pub session_id: String,
    pub run_id: String,
    pub project_dir: String,
    pub workflow_name: String,
    pub status: String,
    pub summary: Option<String>,
    pub default_model: Option<String>,
    #[ts(type = "number | null")]
    pub start_time: Option<i64>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    #[ts(type = "number")]
    pub total_tokens: u64,
    #[ts(type = "number")]
    pub total_tool_calls: u64,
    pub agent_count: u32,
    pub phase_count: u32,
    #[ts(type = "number | null")]
    pub updated_at: Option<i64>,
    pub script_preview: Option<String>,
    pub result_preview: Option<String>,
    pub has_summary_json: bool,
    pub has_journal: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowJournalEvent {
    pub kind: String,
    pub agent_id: Option<String>,
    pub preview: Option<String>,
    #[ts(type = "number | null")]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunDetail {
    pub summary: WorkflowRunSummary,
    pub phases: Vec<WorkflowPhaseSummary>,
    pub agents: Vec<WorkflowAgentSummary>,
    pub script: Option<String>,
    pub result: Option<String>,
    pub journal: Vec<WorkflowJournalEvent>,
    pub artifact_relative_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgentEvent {
    pub kind: String,
    pub role: Option<String>,
    pub preview: String,
    #[ts(type = "number | null")]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgentDetail {
    pub summary: WorkflowAgentSummary,
    pub prompt_preview: Option<String>,
    pub result_preview: Option<String>,
    pub events: Vec<WorkflowAgentEvent>,
    pub meta_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHomeEntry {
    pub kind: String,
    pub name: String,
    pub relative_path: String,
    pub path: String,
    pub is_directory: bool,
    #[ts(type = "number")]
    pub item_count: u64,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number | null")]
    pub modified_at: Option<i64>,
    pub preview: Option<String>,
    pub preview_truncated: bool,
    pub metadata_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkflowScanResult {
    pub runs: Vec<WorkflowRunSummary>,
    pub warnings: Vec<String>,
}

/// One discovered run, before any artifact files are parsed.
#[derive(Debug, Clone)]
pub(crate) struct WorkflowArtifact {
    pub project_dir: String,
    pub session_id: String,
    pub run_id: String,
    pub summary_path: Option<PathBuf>,
    pub run_dir: Option<PathBuf>,
}

/// A fully parsed run (summary + optional detail bodies).
#[derive(Debug, Clone)]
pub(crate) struct ParsedRun {
    pub summary: WorkflowRunSummary,
    pub phases: Vec<WorkflowPhaseSummary>,
    pub agents: Vec<WorkflowAgentSummary>,
    pub script: Option<String>,
    pub result: Option<String>,
}
