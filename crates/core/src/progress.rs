// crates/core/src/progress.rs
//! Task/todo progress types for live session monitoring.
//!
//! Supports two independent task tracking systems from Claude Code:
//! - TodoWrite: flat checklist with full-replacement semantics
//! - TaskCreate/TaskUpdate: structured tasks with incremental updates

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// =============================================================================
// Raw extraction structs (parsed from JSONL, not serialized to frontend)
// =============================================================================

/// Raw todo item extracted from TodoWrite tool_use input.
#[derive(Debug, Clone)]
pub struct RawTodoItem {
    pub content: String,
    pub status: String, // "pending" | "in_progress" | "completed"
    pub active_form: String,
}

/// Raw task create extracted from TaskCreate tool_use input.
#[derive(Debug, Clone)]
pub struct RawTaskCreate {
    pub tool_use_id: String, // links to tool_result for ID assignment
    pub subject: String,
    pub description: String,
    pub active_form: String,
}

/// Raw task update extracted from TaskUpdate tool_use input.
#[derive(Debug, Clone)]
pub struct RawTaskUpdate {
    pub task_id: String, // system-assigned ID ("1", "2", ...)
    pub status: Option<String>,
    pub subject: Option<String>,
    pub active_form: Option<String>,
}

/// Task ID assignment extracted from toolUseResult on a user line.
#[derive(Debug, Clone)]
pub struct RawTaskIdAssignment {
    pub tool_use_id: String, // from message.content[].tool_use_id
    pub task_id: String,     // from toolUseResult.task.id
}

// =============================================================================
// Wire types (serialized to frontend via SSE)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ProgressStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ProgressSource {
    Todo,
    Task,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProgressItem {
    /// For tasks: system-assigned ID ("1", "2", "3").
    /// For todos: None (no stable ID in TodoWrite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Internal dedup key for replay resilience. Matches the `tool_use_id`
    /// from the TaskCreate tool_use block. Not serialized to frontend.
    /// For todos: None (TodoWrite uses full-replacement semantics).
    #[serde(skip)]
    pub tool_use_id: Option<String>,

    /// Display text. From `content` (TodoWrite) or `subject` (TaskCreate).
    pub title: String,

    /// Current status.
    pub status: ProgressStatus,

    /// Present-continuous verb for spinner display (e.g., "Running tests").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,

    /// Which system produced this item.
    pub source: ProgressSource,
}
