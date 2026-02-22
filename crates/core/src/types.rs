// crates/core/src/types.rs
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Tool usage statistics for a session
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ToolCounts {
    pub edit: usize,
    pub read: usize,
    pub bash: usize,
    pub write: usize,
}

impl ToolCounts {
    pub fn total(&self) -> usize {
        self.edit + self.read + self.bash + self.write
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Real user prompt (string content)
    User,
    /// Claude response with text
    Assistant,
    /// Assistant message with only tool_use blocks (no text)
    ToolUse,
    /// User message with tool_result array content
    ToolResult,
    /// System events + queue-ops + file-snapshots
    System,
    /// Progress events (agent, bash, hook, mcp, waiting)
    Progress,
    /// Auto-generated session summaries
    Summary,
}

/// A tool call made by the assistant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ToolCall {
    pub name: String,
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub input: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// A message in a conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl Message {
    /// Private helper to reduce boilerplate across role constructors.
    fn new_with_role(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp: None,
            tool_calls: None,
            thinking: None,
            uuid: None,
            parent_uuid: None,
            metadata: None,
            category: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self { Self::new_with_role(Role::User, content) }
    pub fn assistant(content: impl Into<String>) -> Self { Self::new_with_role(Role::Assistant, content) }
    pub fn system(content: impl Into<String>) -> Self { Self::new_with_role(Role::System, content) }
    pub fn tool_use(content: impl Into<String>) -> Self { Self::new_with_role(Role::ToolUse, content) }
    pub fn tool_result(content: impl Into<String>) -> Self { Self::new_with_role(Role::ToolResult, content) }
    pub fn progress(content: impl Into<String>) -> Self { Self::new_with_role(Role::Progress, content) }
    pub fn summary(content: impl Into<String>) -> Self { Self::new_with_role(Role::Summary, content) }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tools);
        self
    }

    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    pub fn with_uuid(mut self, uuid: impl Into<String>) -> Self {
        self.uuid = Some(uuid.into());
        self
    }

    pub fn with_parent_uuid(mut self, parent_uuid: impl Into<String>) -> Self {
        self.parent_uuid = Some(parent_uuid.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Message content collected during JSONL parsing for full-text search indexing.
/// Only User, Assistant, and ToolUse messages are included (ToolResult, System,
/// Progress, Summary are excluded as noise).
#[derive(Debug, Clone)]
pub struct SearchableMessage {
    /// "user", "assistant", or "tool"
    pub role: String,
    /// The message text content
    pub content: String,
    /// Unix timestamp in seconds, parsed from ISO-8601. None if timestamp missing.
    pub timestamp: Option<i64>,
}

/// Session metadata extracted from parsing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub total_messages: usize,
    pub tool_call_count: usize,
}

/// A parsed session with messages and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct ParsedSession {
    pub messages: Vec<Message>,
    pub metadata: SessionMetadata,
}

impl ParsedSession {
    pub fn new(messages: Vec<Message>, tool_call_count: usize) -> Self {
        Self {
            metadata: SessionMetadata {
                total_messages: messages.len(),
                tool_call_count,
            },
            messages,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn turn_count(&self) -> usize {
        let user_count = self.messages.iter().filter(|m| m.role == Role::User).count();
        let assistant_count = self.messages.iter().filter(|m| m.role == Role::Assistant).count();
        user_count.min(assistant_count)
    }
}

/// A paginated slice of session messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PaginatedMessages {
    pub messages: Vec<Message>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

/// Session info for listing (without full message content)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub file_path: String,
    #[ts(type = "number")]
    pub modified_at: i64,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub preview: String,
    pub last_message: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
    pub tool_counts: ToolCounts,
    pub message_count: usize,
    pub turn_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
    #[serde(default)]
    pub deep_indexed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub total_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub total_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub total_cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub total_cache_creation_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub turn_count_api: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_model: Option<String>,
    // Phase 3: Atomic unit metrics
    #[serde(default)]
    pub user_prompt_count: u32,
    #[serde(default)]
    pub api_call_count: u32,
    #[serde(default)]
    pub tool_call_count: u32,
    #[serde(default)]
    pub files_read: Vec<String>,
    #[serde(default)]
    pub files_edited: Vec<String>,
    #[serde(default)]
    pub files_read_count: u32,
    #[serde(default)]
    pub files_edited_count: u32,
    #[serde(default)]
    pub reedited_files_count: u32,
    #[serde(default)]
    pub duration_seconds: u32,
    #[serde(default)]
    pub commit_count: u32,
    // Phase 3.5: Full parser metrics
    #[serde(default)]
    pub thinking_block_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub turn_duration_avg_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub turn_duration_max_ms: Option<u64>,
    #[serde(default)]
    pub api_error_count: u32,
    #[serde(default)]
    pub compaction_count: u32,
    #[serde(default)]
    pub agent_spawn_count: u32,
    #[serde(default)]
    pub bash_progress_count: u32,
    #[serde(default)]
    pub hook_progress_count: u32,
    #[serde(default)]
    pub mcp_progress_count: u32,
    // Phase C: LOC estimation
    #[serde(default)]
    pub lines_added: u32,
    #[serde(default)]
    pub lines_removed: u32,
    #[serde(default)]
    pub loc_source: u8,  // 0 = not computed, 1 = tool-call estimate, 2 = git diff
    #[serde(default)]
    pub parse_version: u32,
    // Theme 4: Classification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l3: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classified_at: Option<String>,
    // Theme 4: Behavioral metrics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_word_count: Option<u32>,
    #[serde(default)]
    pub correction_count: u32,
    #[serde(default)]
    pub same_file_edit_count: u32,
    // Wall-clock task time metrics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub total_task_time_seconds: Option<u32>,    // sum of all turn wall-clock durations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub longest_task_seconds: Option<u32>,       // single longest turn (wall clock)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longest_task_preview: Option<String>,    // first 60 chars of the prompt that started it
}

impl SessionInfo {
    /// A2.1 Tokens Per Prompt: (total_input + total_output) / user_prompt_count
    ///
    /// Returns `None` if token data is unavailable or user_prompt_count is 0.
    pub fn tokens_per_prompt(&self) -> Option<f64> {
        let total_input = self.total_input_tokens?;
        let total_output = self.total_output_tokens?;
        crate::metrics::tokens_per_prompt(total_input, total_output, self.user_prompt_count)
    }

    /// A2.2 Re-edit Rate: reedited_files_count / files_edited_count
    ///
    /// Returns `None` if files_edited_count is 0.
    pub fn reedit_rate(&self) -> Option<f64> {
        crate::metrics::reedit_rate(self.reedited_files_count, self.files_edited_count)
    }

    /// A2.3 Tool Density: tool_call_count / api_call_count
    ///
    /// Returns `None` if api_call_count is 0.
    pub fn tool_density(&self) -> Option<f64> {
        crate::metrics::tool_density(self.tool_call_count, self.api_call_count)
    }

    /// A2.4 Edit Velocity: files_edited_count / (duration_seconds / 60)
    ///
    /// Returns `None` if duration_seconds is 0.
    pub fn edit_velocity(&self) -> Option<f64> {
        crate::metrics::edit_velocity(self.files_edited_count, self.duration_seconds)
    }

    /// A2.5 Read-to-Edit Ratio: files_read_count / files_edited_count
    ///
    /// Returns `None` if files_edited_count is 0.
    pub fn read_to_edit_ratio(&self) -> Option<f64> {
        crate::metrics::read_to_edit_ratio(self.files_read_count, self.files_edited_count)
    }
}

/// Project info with sessions
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub sessions: Vec<SessionInfo>,
    pub active_count: usize,
}

impl ProjectInfo {
    pub fn total_sessions(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

/// Lightweight project summary for sidebar — no sessions array.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub session_count: usize,
    pub active_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub last_activity_at: Option<i64>,
}

/// Paginated sessions response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionsPage {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Pre-computed dashboard statistics.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_sessions: usize,
    pub total_projects: usize,
    pub heatmap: Vec<DayActivity>,
    pub top_skills: Vec<SkillStat>,
    pub top_commands: Vec<SkillStat>,
    pub top_mcp_tools: Vec<SkillStat>,
    pub top_agents: Vec<SkillStat>,
    pub top_projects: Vec<ProjectStat>,
    pub tool_totals: ToolCounts,
    pub longest_sessions: Vec<SessionDurationStat>,
}

/// A single day in the activity heatmap.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct DayActivity {
    pub date: String,
    pub count: usize,
}

/// A skill with its usage count.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SkillStat {
    pub name: String,
    pub count: usize,
}

/// A project with its session count (for dashboard top projects).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProjectStat {
    pub name: String,
    pub display_name: String,
    pub session_count: usize,
}

/// A session entry for the "Longest Tasks" dashboard card.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionDurationStat {
    pub id: String,
    pub preview: String,
    pub project_name: String,
    pub project_display_name: String,
    pub duration_seconds: u32,
}

// ============================================================================
// Turn-level types (Phase 2B: token & model tracking)
// ============================================================================

/// A single assistant turn extracted from JSONL, capturing model and token usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTurn {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub seq: u32,
    pub model_id: String,
    pub content_type: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub service_tier: Option<String>,
    pub timestamp: Option<i64>,
}

/// A model record for the models table (deduplicated across all sessions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: String,
    pub provider: String,
    pub family: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// Parse a model ID string into (provider, family) slices.
///
/// Known patterns:
/// - `claude-opus-*`   → `("anthropic", "opus")`
/// - `claude-sonnet-*` → `("anthropic", "sonnet")`
/// - `claude-haiku-*`  → `("anthropic", "haiku")`
/// - `gpt-4*` / `gpt-3*` → `("openai", "gpt-4")` / `("openai", "gpt-3")`
/// - `o1*` / `o3*`     → `("openai", "o1")` / `("openai", "o3")`
/// - `gemini-*`        → `("google", "gemini")`
/// - Anything else     → `("unknown", model_str)`
pub fn parse_model_id(model_str: &str) -> (&str, &str) {
    if model_str.starts_with("claude-opus") {
        ("anthropic", "opus")
    } else if model_str.starts_with("claude-sonnet") {
        ("anthropic", "sonnet")
    } else if model_str.starts_with("claude-haiku") {
        ("anthropic", "haiku")
    } else if model_str.starts_with("gpt-4") {
        ("openai", "gpt-4")
    } else if model_str.starts_with("gpt-3") {
        ("openai", "gpt-3")
    } else if model_str.starts_with("o1") {
        ("openai", "o1")
    } else if model_str.starts_with("o3") {
        ("openai", "o3")
    } else if model_str.starts_with("gemini") {
        ("google", "gemini")
    } else {
        ("unknown", model_str)
    }
}

// ============================================================================
// JSONL Parsing Types (internal, for deserializing Claude Code format)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct JsonlMessage {
    pub role: Option<String>,
    pub content: JsonlContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum JsonlContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        name: String,
        #[serde(default)]
        input: Option<serde_json::Value>,
    },
    ToolResult {
        #[serde(default)]
        content: Option<serde_json::Value>,
    },
    #[serde(other)]
    Other,
}

// ============================================================================
// Theme 4: Classification Job Types
// ============================================================================

/// Status of a classification job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ClassificationJobStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl ClassificationJobStatus {
    /// Parse a status string from the database.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => Self::Failed,
        }
    }

    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

/// A classification job record.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationJob {
    #[ts(type = "number")]
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub classified_count: i64,
    #[ts(type = "number")]
    pub skipped_count: i64,
    #[ts(type = "number")]
    pub failed_count: i64,
    pub provider: String,
    pub model: String,
    pub status: ClassificationJobStatus,
    pub error_message: Option<String>,
    #[ts(type = "number | null")]
    pub cost_estimate_cents: Option<i64>,
    #[ts(type = "number | null")]
    pub actual_cost_cents: Option<i64>,
    #[ts(type = "number | null")]
    pub tokens_used: Option<i64>,
}

// ============================================================================
// Theme 4: Index Run Types
// ============================================================================

/// Type of index run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum IndexRunType {
    Full,
    Incremental,
    Deep,
}

impl IndexRunType {
    /// Parse a type string from the database.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "full" => Self::Full,
            "incremental" => Self::Incremental,
            "deep" => Self::Deep,
            _ => Self::Full,
        }
    }

    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
            Self::Deep => "deep",
        }
    }
}

/// Status of an index run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum IndexRunStatus {
    Running,
    Completed,
    Failed,
}

impl IndexRunStatus {
    /// Parse a status string from the database.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Failed,
        }
    }

    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// An index run record.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IndexRun {
    #[ts(type = "number")]
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub run_type: IndexRunType,
    #[ts(type = "number | null")]
    pub sessions_before: Option<i64>,
    #[ts(type = "number | null")]
    pub sessions_after: Option<i64>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
    pub throughput_mb_per_sec: Option<f64>,
    pub status: IndexRunStatus,
    pub error_message: Option<String>,
}

// ============================================================================
// Theme 4 Phase 8: Benchmark Types
// ============================================================================

/// Metrics for a time period (used in Then vs Now comparison).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PeriodMetrics {
    /// Re-edit rate: files re-edited / files edited (0.0-1.0)
    pub reedit_rate: f64,
    /// Average edit operations per file
    pub edits_per_file: f64,
    /// Average user prompts per session
    pub prompts_per_task: f64,
    /// Percentage of sessions with commits (0.0-1.0)
    pub commit_rate: f64,
}

/// Improvement percentages between two periods.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ImprovementMetrics {
    /// Re-edit rate change (negative = improvement)
    pub reedit_rate: f64,
    /// Edits per file change (negative = improvement)
    pub edits_per_file: f64,
    /// Prompts per task change (negative = improvement)
    pub prompts_per_task: f64,
    /// Commit rate change (positive = improvement)
    pub commit_rate: f64,
}

/// Progress comparison between first and last month.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProgressComparison {
    pub first_month: Option<PeriodMetrics>,
    pub last_month: PeriodMetrics,
    pub improvement: Option<ImprovementMetrics>,
    pub insight: String,
}

/// Verdict for category performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CategoryVerdict {
    Excellent,
    Good,
    Average,
    NeedsWork,
}

/// Performance metrics for a single category.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryPerformance {
    pub category: String,
    pub reedit_rate: f64,
    /// Difference from user's overall average (negative = better)
    pub vs_average: f64,
    pub verdict: CategoryVerdict,
    pub insight: String,
}

/// Learning curve data point.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct LearningCurvePoint {
    pub session: u32,
    pub reedit_rate: f64,
}

/// Skill adoption with impact metrics.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SkillAdoption {
    pub skill: String,
    pub adopted_at: String,
    pub session_count: u32,
    /// Percentage improvement in re-edit rate after adoption
    pub impact_on_reedit: f64,
    pub learning_curve: Vec<LearningCurvePoint>,
}

/// Monthly report summary.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub month: String,
    pub session_count: u32,
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    pub commit_count: u32,
    pub estimated_cost: f64,
    pub top_wins: Vec<String>,
    pub focus_areas: Vec<String>,
}

/// Full benchmarks response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BenchmarksResponse {
    pub progress: ProgressComparison,
    pub by_category: Vec<CategoryPerformance>,
    /// User's overall average re-edit rate
    pub user_average_reedit_rate: f64,
    pub skill_adoption: Vec<SkillAdoption>,
    pub report_summary: ReportSummary,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_counts_total() {
        let counts = ToolCounts {
            edit: 5,
            read: 10,
            bash: 3,
            write: 2,
        };
        assert_eq!(counts.total(), 20);
    }

    #[test]
    fn test_tool_counts_empty() {
        let counts = ToolCounts::default();
        assert!(counts.is_empty());

        let counts = ToolCounts { edit: 1, ..Default::default() };
        assert!(!counts.is_empty());
    }

    #[test]
    fn test_message_builders() {
        let msg = Message::user("Hello")
            .with_timestamp("2026-01-27T10:00:00Z");

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.timestamp, Some("2026-01-27T10:00:00Z".to_string()));
    }

    #[test]
    fn test_message_with_tools() {
        let msg = Message::assistant("Let me help")
            .with_tools(vec![ToolCall { name: "Read".to_string(), count: 2, input: None, category: None }]);

        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.unwrap()[0].count, 2);
    }

    #[test]
    fn test_parsed_session_turn_count() {
        let session = ParsedSession::new(
            vec![
                Message::user("Q1"),
                Message::assistant("A1"),
                Message::user("Q2"),
                Message::assistant("A2"),
                Message::user("Q3"), // No response yet
            ],
            0,
        );

        assert_eq!(session.turn_count(), 2); // min(3 users, 2 assistants)
    }

    #[test]
    fn test_parsed_session_empty() {
        let session = ParsedSession::new(vec![], 0);
        assert!(session.is_empty());
        assert_eq!(session.turn_count(), 0);
    }

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(serde_json::to_string(&Role::Assistant).unwrap(), "\"assistant\"");
        assert_eq!(serde_json::to_string(&Role::ToolUse).unwrap(), "\"tool_use\"");
        assert_eq!(serde_json::to_string(&Role::ToolResult).unwrap(), "\"tool_result\"");
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::Progress).unwrap(), "\"progress\"");
        assert_eq!(serde_json::to_string(&Role::Summary).unwrap(), "\"summary\"");
    }

    #[test]
    fn test_role_deserialization() {
        let user: Role = serde_json::from_str("\"user\"").unwrap();
        let assistant: Role = serde_json::from_str("\"assistant\"").unwrap();
        let tool_use: Role = serde_json::from_str("\"tool_use\"").unwrap();
        let tool_result: Role = serde_json::from_str("\"tool_result\"").unwrap();
        let system: Role = serde_json::from_str("\"system\"").unwrap();
        let progress: Role = serde_json::from_str("\"progress\"").unwrap();
        let summary: Role = serde_json::from_str("\"summary\"").unwrap();
        assert_eq!(user, Role::User);
        assert_eq!(assistant, Role::Assistant);
        assert_eq!(tool_use, Role::ToolUse);
        assert_eq!(tool_result, Role::ToolResult);
        assert_eq!(system, Role::System);
        assert_eq!(progress, Role::Progress);
        assert_eq!(summary, Role::Summary);
    }

    #[test]
    fn test_jsonl_content_text() {
        let json = r#""Hello world""#;
        let content: JsonlContent = serde_json::from_str(json).unwrap();

        match content {
            JsonlContent::Text(text) => assert_eq!(text, "Hello world"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_jsonl_content_blocks() {
        let json = r#"[{"type":"text","text":"Hello"},{"type":"tool_use","name":"Read"}]"#;
        let content: JsonlContent = serde_json::from_str(json).unwrap();

        match content {
            JsonlContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                match &blocks[0] {
                    ContentBlock::Text { text } => assert_eq!(text, "Hello"),
                    _ => panic!("Expected Text block"),
                }
                match &blocks[1] {
                    ContentBlock::ToolUse { name, .. } => assert_eq!(name, "Read"),
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn test_content_block_unknown_type() {
        let json = r#"{"type":"future_block_type","data":"something"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(matches!(block, ContentBlock::Other));
    }

    #[test]
    fn test_project_info_methods() {
        let project = ProjectInfo {
            name: "test-project".to_string(),
            display_name: "Test Project".to_string(),
            path: "/path/to/project".to_string(),
            sessions: vec![],
            active_count: 0,
        };

        assert!(project.is_empty());
        assert_eq!(project.total_sessions(), 0);
    }

    #[test]
    fn test_message_serialization_omits_none() {
        let msg = Message::user("Hello");
        let json = serde_json::to_string(&msg).unwrap();

        // Should not contain optional fields when None
        assert!(!json.contains("timestamp"));
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("thinking"));
        assert!(!json.contains("uuid"));
        assert!(!json.contains("parent_uuid"));
        assert!(!json.contains("metadata"));
    }

    #[test]
    fn test_message_with_thinking() {
        let msg = Message::assistant("Here's the answer")
            .with_thinking("Let me reason about this...");

        assert_eq!(msg.thinking, Some("Let me reason about this...".to_string()));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"thinking\":\"Let me reason about this...\""));
    }

    // ============================================================================
    // modifiedAt serialization tests
    // ============================================================================

    #[test]
    fn test_session_info_modified_at_serializes_as_number() {
        let session = SessionInfo {
            id: "test-123".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1769482232, // Unix timestamp
            size_bytes: 1024,
            preview: "Test".to_string(),
            last_message: "Test".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 1,
            turn_count: 1,
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

            parse_version: 0,
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            // Theme 4: Classification
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
        };
        let json = serde_json::to_string(&session).unwrap();

        // Should serialize as number
        assert!(
            json.contains("\"modifiedAt\":1769482232"),
            "modifiedAt should be a number, got: {}",
            json
        );
    }

    // ============================================================================
    // parse_model_id tests
    // ============================================================================

    #[test]
    fn test_parse_model_id_anthropic_opus() {
        assert_eq!(parse_model_id("claude-opus-4-5-20251101"), ("anthropic", "opus"));
        assert_eq!(parse_model_id("claude-opus-4-20250514"), ("anthropic", "opus"));
    }

    #[test]
    fn test_parse_model_id_anthropic_sonnet() {
        assert_eq!(parse_model_id("claude-sonnet-4-20250514"), ("anthropic", "sonnet"));
        assert_eq!(parse_model_id("claude-sonnet-4-5-20260130"), ("anthropic", "sonnet"));
    }

    #[test]
    fn test_parse_model_id_anthropic_haiku() {
        assert_eq!(parse_model_id("claude-haiku-4-20250514"), ("anthropic", "haiku"));
    }

    #[test]
    fn test_parse_model_id_openai_gpt() {
        assert_eq!(parse_model_id("gpt-4o-2024-08-06"), ("openai", "gpt-4"));
        assert_eq!(parse_model_id("gpt-4-turbo"), ("openai", "gpt-4"));
        assert_eq!(parse_model_id("gpt-3.5-turbo"), ("openai", "gpt-3"));
    }

    #[test]
    fn test_parse_model_id_openai_reasoning() {
        assert_eq!(parse_model_id("o1-preview"), ("openai", "o1"));
        assert_eq!(parse_model_id("o1-mini"), ("openai", "o1"));
        assert_eq!(parse_model_id("o3-mini"), ("openai", "o3"));
    }

    #[test]
    fn test_parse_model_id_google_gemini() {
        assert_eq!(parse_model_id("gemini-1.5-pro"), ("google", "gemini"));
        assert_eq!(parse_model_id("gemini-2.0-flash"), ("google", "gemini"));
    }

    #[test]
    fn test_parse_model_id_unknown() {
        assert_eq!(parse_model_id("llama-3-70b"), ("unknown", "llama-3-70b"));
        assert_eq!(parse_model_id("mistral-large"), ("unknown", "mistral-large"));
        assert_eq!(parse_model_id(""), ("unknown", ""));
    }

    // ============================================================================
    // SessionInfo derived metric methods tests
    // ============================================================================

    fn make_test_session() -> SessionInfo {
        SessionInfo {
            id: "test-session".to_string(),
            project: "test-project".to_string(),
            project_path: "/path/to/project".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            modified_at: 1700000000,
            size_bytes: 1024,
            preview: "Test preview".to_string(),
            last_message: "Test message".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: true,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: Some(10),
            primary_model: Some("claude-sonnet-4".to_string()),
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec!["a.rs".to_string(), "b.rs".to_string()],
            files_edited: vec!["c.rs".to_string()],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600, // 10 minutes
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            // Phase C: LOC estimation
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            // Theme 4: Classification
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
        }
    }

    #[test]
    fn test_session_tokens_per_prompt() {
        let session = make_test_session();
        // (10000 + 5000) / 10 = 1500.0
        let result = session.tokens_per_prompt();
        assert_eq!(result, Some(1500.0));
    }

    #[test]
    fn test_session_tokens_per_prompt_missing_tokens() {
        let mut session = make_test_session();
        session.total_input_tokens = None;
        assert_eq!(session.tokens_per_prompt(), None);

        session.total_input_tokens = Some(1000);
        session.total_output_tokens = None;
        assert_eq!(session.tokens_per_prompt(), None);
    }

    #[test]
    fn test_session_tokens_per_prompt_zero_prompts() {
        let mut session = make_test_session();
        session.user_prompt_count = 0;
        assert_eq!(session.tokens_per_prompt(), None);
    }

    #[test]
    fn test_session_reedit_rate() {
        let session = make_test_session();
        // 2 / 5 = 0.4
        let result = session.reedit_rate();
        assert_eq!(result, Some(0.4));
    }

    #[test]
    fn test_session_reedit_rate_zero_edits() {
        let mut session = make_test_session();
        session.files_edited_count = 0;
        assert_eq!(session.reedit_rate(), None);
    }

    #[test]
    fn test_session_tool_density() {
        let session = make_test_session();
        // 50 / 20 = 2.5
        let result = session.tool_density();
        assert_eq!(result, Some(2.5));
    }

    #[test]
    fn test_session_tool_density_zero_api_calls() {
        let mut session = make_test_session();
        session.api_call_count = 0;
        assert_eq!(session.tool_density(), None);
    }

    #[test]
    fn test_session_edit_velocity() {
        let session = make_test_session();
        // 5 files / (600 / 60) = 5 / 10 = 0.5 files/min
        let result = session.edit_velocity();
        assert_eq!(result, Some(0.5));
    }

    #[test]
    fn test_session_edit_velocity_zero_duration() {
        let mut session = make_test_session();
        session.duration_seconds = 0;
        assert_eq!(session.edit_velocity(), None);
    }

    #[test]
    fn test_session_read_to_edit_ratio() {
        let session = make_test_session();
        // 20 / 5 = 4.0
        let result = session.read_to_edit_ratio();
        assert_eq!(result, Some(4.0));
    }

    #[test]
    fn test_session_read_to_edit_ratio_zero_edits() {
        let mut session = make_test_session();
        session.files_edited_count = 0;
        assert_eq!(session.read_to_edit_ratio(), None);
    }
}
