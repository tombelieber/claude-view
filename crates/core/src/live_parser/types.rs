//! Public types for the live parser module.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate, RawTodoItem};

/// Pattern for absolute file paths in plain text. No lookbehinds (regex-lite doesn't support them).
/// URL exclusion is handled by SIMD pre-filter: skip lines containing "://".
/// Matches paths like `/etc/hosts` (no extension) and `/src/auth.rs` (with extension).
/// Rejects bare directories like `/directory/` (trailing slash).
pub(crate) const PASTED_PATH_PATTERN: &str = r"(?:^|\s)(\/(?:[\w.-]+\/)*[\w.-]+)(?:\s|$|[,;:!?)])";

static PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Number of times progress parsing fell back from
/// `data.message.message.content` to `data.message.content`.
pub fn progress_message_content_fallback_count() -> u64 {
    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.load(Ordering::Relaxed)
}

/// Reset fallback counter. Primarily useful for tests.
pub fn reset_progress_message_content_fallback_count() {
    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.store(0, Ordering::Relaxed);
}

/// Increment the fallback counter by 1.
pub(crate) fn increment_progress_message_content_fallback_count() {
    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Byte offset + timestamp for incremental tailing.
pub struct TailState {
    pub offset: u64,
    pub last_modified: std::time::SystemTime,
}

/// Extracted from a Task tool_use block on an assistant line.
#[derive(Debug, Clone)]
pub struct SubAgentSpawn {
    pub tool_use_id: String,
    pub agent_type: String,
    pub description: String,
    /// Present when this spawn is a team member (from `input.team_name`).
    /// Used by accumulator/manager to skip adding to sub_agents[].
    pub team_name: Option<String>,
    /// Model alias or full ID from `input.model` (e.g., "haiku", "sonnet", "claude-haiku-4-5-20251001").
    /// None when omitted (sub-agent inherits parent model).
    pub model: Option<String>,
}

/// Extracted from a `type: "progress"` line with `data.type: "agent_progress"`.
#[derive(Debug, Clone)]
pub struct SubAgentProgress {
    /// Links back to the Task spawn's `tool_use_id`.
    pub parent_tool_use_id: String,
    /// Agent ID (available before completion!).
    pub agent_id: String,
    /// Current tool the sub-agent is using (e.g., "Read", "Grep", "Edit").
    /// Extracted from the latest `tool_use` block in `data.message.content`.
    pub current_tool: Option<String>,
}

/// Extracted from a `<task-notification>` on a user line (background agent completion).
#[derive(Debug, Clone)]
pub struct SubAgentNotification {
    /// The agent ID from `<task-id>`.
    pub agent_id: String,
    /// The completion status: "completed", "failed", or "killed".
    pub status: String,
}

/// Extracted from a toolUseResult on a user line (Task completion).
#[derive(Debug, Clone)]
pub struct SubAgentResult {
    pub tool_use_id: String,
    /// Alphanumeric agent ID from `toolUseResult.agentId`.
    pub agent_id: Option<String>,
    pub status: String, // "completed", "error", etc.
    pub total_duration_ms: Option<u64>,
    /// Number of tool calls from `toolUseResult.totalToolUseCount`.
    pub total_tool_use_count: Option<u32>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
    pub usage_cache_read_tokens: Option<u64>,
    pub usage_cache_creation_tokens: Option<u64>,
    /// Sub-agent 5-minute TTL cache creation tokens (from nested
    /// `cache_creation.ephemeral_5m_input_tokens`).
    pub usage_cache_creation_5m_tokens: Option<u64>,
    /// Sub-agent 1-hour TTL cache creation tokens (from nested
    /// `cache_creation.ephemeral_1h_input_tokens`). This is Claude Code's
    /// default caching TTL, so this field is usually where sub-agent
    /// cache_creation tokens actually live.
    pub usage_cache_creation_1hr_tokens: Option<u64>,
    /// Model used by the sub-agent, from `toolUseResult.model`.
    /// None if not present in the result payload.
    pub model: Option<String>,
}

/// Extracted from a `type: "progress"` line with `data.type: "hook_progress"`.
#[derive(Debug, Clone)]
pub struct HookProgressData {
    pub hook_event: String,
    pub tool_name: Option<String>,
    pub source: Option<String>,
}

/// A single parsed JSONL line from the session log.
#[derive(Debug, Clone)]
pub struct LiveLine {
    pub line_type: LineType,
    pub role: Option<String>,
    pub content_preview: String,
    pub content_extended: String,
    pub tool_names: Vec<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_creation_5m_tokens: Option<u64>,
    pub cache_creation_1hr_tokens: Option<u64>,
    pub timestamp: Option<String>,
    pub stop_reason: Option<String>,
    /// Git branch extracted from user-type JSONL lines.
    pub git_branch: Option<String>,
    /// Current working directory extracted from user-type JSONL lines.
    pub cwd: Option<String>,
    /// Whether this is a meta/system message (not real user content).
    pub is_meta: bool,
    /// Whether this line's content array contains a `tool_result` block,
    /// indicating it's a continuation after tool execution (not a new user turn).
    pub is_tool_result_continuation: bool,
    /// Whether this user-type line starts with a system-injected prefix
    /// (e.g. `<local-command-caveat>`, `<task-notification>`, continuation markers).
    pub has_system_prefix: bool,
    /// If this assistant line contains a Task tool_use, the spawn info.
    /// Vec because a single assistant message can spawn multiple sub-agents.
    pub sub_agent_spawns: Vec<SubAgentSpawn>,
    /// If this user line has a `toolUseResult` (Task completion), the result info.
    pub sub_agent_result: Option<SubAgentResult>,
    /// If this is a progress line with agent_progress data.
    pub sub_agent_progress: Option<SubAgentProgress>,
    /// If this user line contains a `<task-notification>` for a background agent.
    pub sub_agent_notification: Option<SubAgentNotification>,
    /// Full replacement todo list from TodoWrite tool_use on this assistant line.
    pub todo_write: Option<Vec<RawTodoItem>>,
    /// TaskCreate calls on this assistant line (Vec: one message can create multiple tasks).
    pub task_creates: Vec<RawTaskCreate>,
    /// TaskUpdate calls on this assistant line.
    pub task_updates: Vec<RawTaskUpdate>,
    /// Task ID assignments from toolUseResult on this user line.
    pub task_id_assignments: Vec<RawTaskIdAssignment>,
    /// Skill names extracted from Skill tool_use blocks (from `input.skill`).
    pub skill_names: Vec<String>,
    /// Bash commands extracted from Bash tool_use blocks (from `input.command`, truncated to 200 chars).
    pub bash_commands: Vec<String>,
    /// File paths extracted from Edit/Write tool_use blocks (from `input.file_path`).
    pub edited_files: Vec<String>,
    /// Whether this is a system message with subtype "compact_boundary".
    pub is_compact_boundary: bool,
    /// Filename extracted from `<ide_opened_file>` tag, if present.
    pub ide_file: Option<String>,
    /// `message.id` from the API response (for dedup: one API response = multiple JSONL lines).
    pub message_id: Option<String>,
    /// `requestId` from the JSONL entry (for dedup: combined with message_id).
    pub request_id: Option<String>,
    /// If this is a hook_progress line, the extracted event data.
    pub hook_progress: Option<HookProgressData>,
    /// Session slug from top-level JSONL field (e.g. "async-greeting-dewdrop").
    /// Present on every line; extracted once by accumulator.
    pub slug: Option<String>,
    /// Team name from top-level `teamName` JSONL field.
    /// Present on every line after a `TeamCreate` tool_use succeeds.
    /// Set-once by accumulator (first non-None wins).
    pub team_name: Option<String>,
    /// Files referenced with `@filename` syntax in user messages.
    pub at_files: Vec<String>,
    /// Absolute file paths pasted in user messages (detected via regex, URL-filtered).
    pub pasted_paths: Vec<String>,
    /// How the session was launched: "cli", "claude-vscode", "sdk-ts", etc.
    /// Present on the first JSONL line; extracted once by accumulator.
    pub entrypoint: Option<String>,
    /// AI-generated session title from `ai-title` JSONL lines.
    pub ai_title: Option<String>,
    /// Original content byte length before truncation. None if no content extracted.
    pub content_byte_len: Option<usize>,
}

/// Broad classification of a JSONL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    User,
    Assistant,
    System,
    Progress,
    Other,
}
