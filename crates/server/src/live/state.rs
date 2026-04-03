//! Live session state types and status derivation for Live Monitor.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.

use crate::live::process::SessionSourceInfo;
use claude_view_core::phase::PhaseHistory;
use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The universal agent state — driven by hooks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group: NeedsYou or Autonomous
    pub group: AgentStateGroup,
    /// Sub-state within group (open string — new states added freely)
    pub state: String,
    /// Human-readable label for the UI
    pub label: String,
    /// Optional context (tool input, error details, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateGroup {
    NeedsYou,
    Autonomous,
}

/// The current status of a live Claude Code session.
///
/// 3-state model: Working (actively streaming/tool use), Paused (waiting for
/// input, task complete, or idle), Done (session over).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Agent is actively streaming or using tools.
    Working,
    /// Agent paused -- reason available in pause_classification.
    Paused,
    /// Session is over (process exited + no new writes for 300s).
    Done,
}

/// A tool integration (MCP server or skill) detected from actual usage in a session.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsed {
    /// Display name: "playwright", "chrome-devtools" for MCP; "commit", "review-pr" for skills.
    pub name: String,
    /// Category: "mcp" or "skill".
    pub kind: String,
}

/// A verified file reference detected from user messages.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedFile {
    /// Absolute path (verified to exist via stat()).
    pub path: String,
    /// How this file was detected.
    pub kind: FileSourceKind,
    /// Project-relative path for UI display.
    pub display_name: String,
}

/// How a file reference was detected in user messages.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum FileSourceKind {
    /// @file mention in user message.
    Mention,
    /// <ide_opened_file> tag from IDE.
    Ide,
    /// Bare absolute path pasted in message.
    Pasted,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

// ---------------------------------------------------------------------------
// StatuslineFields — 32 fields extracted from LiveSession for merge safety
// ---------------------------------------------------------------------------

use crate::live::mutation::merge::{Latest, Monotonic, Transient};

/// Statusline-derived fields, grouped by merge strategy.
///
/// `#[serde(flatten)]` on the parent ensures JSON keys are identical to the old
/// flat layout. Each field uses `Monotonic<T>`, `Latest<T>`, or `Transient<T>`
/// to enforce correct merge semantics at compile time.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct StatuslineFields {
    // -- Monotonic: value only goes up within a session --
    // NOTE: All fields use #[ts(type = "...")] to bypass TS trait bounds on newtypes.
    /// Claude Code's own total cost in USD, from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_cost_usd: Monotonic<f64>,

    /// Wall-clock session duration from statusline cost.total_duration_ms.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_duration_ms: Monotonic<u64>,

    /// API-only duration from statusline cost.total_api_duration_ms.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_api_duration_ms: Monotonic<u64>,

    /// Total lines added from statusline cost.total_lines_added.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_lines_added: Monotonic<u64>,

    /// Total lines removed from statusline cost.total_lines_removed.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_lines_removed: Monotonic<u64>,

    /// Cumulative input tokens across the session from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_input_tokens: Monotonic<u64>,

    /// Cumulative output tokens across the session from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Monotonic::is_none")]
    pub statusline_total_output_tokens: Monotonic<u64>,

    // -- Latest: newest non-null wins --
    /// Authoritative context window size from statusline (200_000 or 1_000_000).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_context_window_size: Latest<u32>,

    /// Authoritative context used percentage from statusline (0.0-100.0).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_used_pct: Latest<f32>,

    /// Remaining context window percentage from statusline (0.0-100.0).
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_remaining_pct: Latest<f32>,

    /// Working directory from statusline workspace.current_dir.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_cwd: Latest<String>,

    /// Project directory from statusline workspace.project_dir.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_project_dir: Latest<String>,

    /// Claude Code version from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_version: Latest<String>,

    /// Transcript path from statusline (used for session dedup).
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_transcript_path: Latest<String>,

    /// 5-hour rate limit used percentage from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_5h_pct: Latest<f64>,

    /// 5-hour rate limit reset timestamp (Unix seconds) from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_5h_resets_at: Latest<i64>,

    /// 7-day rate limit used percentage from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_7d_pct: Latest<f64>,

    /// 7-day rate limit reset timestamp (Unix seconds) from statusline.
    #[ts(type = "number | null")]
    #[serde(default, skip_serializing_if = "Latest::is_none")]
    pub statusline_rate_limit_7d_resets_at: Latest<i64>,

    // -- Transient: absence = cleared --
    /// Current turn input tokens from statusline current_usage.input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_input_tokens: Transient<u64>,

    /// Current turn output tokens from statusline current_usage.output_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_output_tokens: Transient<u64>,

    /// Cache read tokens from statusline current_usage.cache_read_input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_cache_read_tokens: Transient<u64>,

    /// Cache creation tokens from statusline current_usage.cache_creation_input_tokens.
    #[ts(type = "bigint | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_cache_creation_tokens: Transient<u64>,

    /// Whether the session exceeds 200K tokens (from statusline).
    #[ts(type = "boolean | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub exceeds_200k_tokens: Transient<bool>,

    /// Output style name from statusline (e.g. "default", "concise").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_output_style: Transient<String>,

    /// Vim mode from statusline (e.g. "NORMAL", "INSERT").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_vim_mode: Transient<String>,

    /// Subagent name from statusline (e.g. "code-reviewer").
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_agent_name: Transient<String>,

    /// Worktree name from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_name: Transient<String>,

    /// Worktree path from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_path: Transient<String>,

    /// Worktree branch from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_branch: Transient<String>,

    /// Worktree original cwd from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_original_cwd: Transient<String>,

    /// Worktree original branch from statusline.
    #[ts(type = "string | null")]
    #[serde(default, skip_serializing_if = "Transient::is_none")]
    pub statusline_worktree_original_branch: Transient<String>,

    // -- Raw: rolling debug buffer (not serialized) --
    /// Last N raw statusline payloads for debugging. NOT serialized to SSE.
    /// Newest at back. Capped at MAX_STATUSLINE_DEBUG_ENTRIES.
    #[serde(skip)]
    #[ts(skip)]
    pub statusline_debug_log: std::collections::VecDeque<StatuslineDebugEntry>,
}

/// Max entries in the per-session statusline debug ring buffer.
pub const MAX_STATUSLINE_DEBUG_ENTRIES: usize = 20;

/// A timestamped raw statusline payload for debugging.
#[derive(Debug, Clone)]
pub struct StatuslineDebugEntry {
    pub received_at: i64,
    pub payload: serde_json::Value,
    /// Which top-level blocks were present (quick scan without reading payload).
    pub blocks_present: Vec<String>,
}

// ---------------------------------------------------------------------------
// JsonlFields — 22 fields sourced from JSONL watcher, extracted from LiveSession
// ---------------------------------------------------------------------------

/// JSONL-watcher-sourced fields, grouped for decomposition clarity.
///
/// `#[serde(flatten)]` on the parent ensures JSON keys are identical to the old
/// flat layout. Contains project info, branch info, token/cost data, team data,
/// tools, files, phase classification, and session source.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct JsonlFields {
    /// Encoded project directory name (as stored on disk).
    pub project: String,
    /// Human-readable project name (last path component, decoded).
    pub project_display_name: String,
    /// Full decoded project path.
    pub project_path: String,
    /// Absolute path to the JSONL session file.
    pub file_path: String,
    /// Git branch name, if detected.
    pub git_branch: Option<String>,
    /// Resolved branch from worktree HEAD (differs from git_branch when in a worktree).
    pub worktree_branch: Option<String>,
    /// Whether this session is running inside a git worktree.
    pub is_worktree: bool,
    /// Computed: worktree_branch ?? git_branch. Always use this for display.
    pub effective_branch: Option<String>,
    /// Accumulated token usage for this session (cumulative, for cost).
    pub tokens: TokenUsage,
    /// Computed cost breakdown in USD.
    pub cost: CostBreakdown,
    /// Whether the Anthropic prompt cache is likely warm or cold.
    pub cache_status: CacheStatus,
    /// Seconds the agent spent on the last completed turn (frozen on Working->Paused).
    /// Used by frontend to show task time for needs_you sessions.
    pub last_turn_task_seconds: Option<u32>,
    /// Unix timestamp when the last cache hit or creation occurred.
    /// Set only when a turn has cache_read_tokens > 0 OR cache_creation_tokens > 0.
    /// Null if no cache activity has been detected (e.g., new session or below minimum tokens).
    #[ts(type = "number | null")]
    pub last_cache_hit_at: Option<i64>,
    /// Team name if this session is a team lead.
    /// Populated from the top-level `teamName` field in the JSONL (present after TeamCreate).
    /// Frontend uses this to show team badge instead of sub-agent pills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    /// Team members read from ~/.claude/teams/{name}/config.json.
    /// Populated after each JSONL metadata application when team_name is Some.
    /// Empty vec when not a team lead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub team_members: Vec<crate::teams::TeamMember>,
    /// Number of inbox messages for this team (0 when not a team lead).
    /// Used by frontend as a version signal to invalidate inbox queries.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub team_inbox_count: u32,
    /// Number of file-modifying tool uses (Edit + Write) in this session.
    /// Used by frontend as a version signal to invalidate file-history and plan queries.
    #[serde(default)]
    pub edit_count: u32,
    /// Unique tool integrations detected in this session (MCP servers, skills).
    /// Discovered from actual tool_use invocations -- 100% accuracy.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools_used: Vec<ToolUsed>,
    /// Session slug for plan file association.
    pub slug: Option<String>,
    /// Verified file references detected from user messages.
    /// Deduplicated by absolute path across session lifetime (<=10, first-N-wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_files: Option<Vec<VerifiedFile>>,
    /// Where this session was launched from (terminal, IDE, or Agent SDK).
    /// Detected from the parent process at discovery time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SessionSourceInfo>,
    /// SDLC phase classification (current phase, label history, dominant phase).
    pub phase: PhaseHistory,
}

impl Default for JsonlFields {
    fn default() -> Self {
        Self {
            project: String::new(),
            project_display_name: String::new(),
            project_path: String::new(),
            file_path: String::new(),
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            tokens: TokenUsage::default(),
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            last_turn_task_seconds: None,
            last_cache_hit_at: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            slug: None,
            user_files: None,
            source: None,
            phase: PhaseHistory::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// HookFields — 13 fields sourced from hooks, extracted from LiveSession
// ---------------------------------------------------------------------------

/// Hook-sourced fields, grouped for merge clarity.
///
/// `#[serde(flatten)]` on the parent ensures JSON keys are identical to the old
/// flat layout. Contains agent_state, PID, title, turn count, activity tracking,
/// sub-agents, progress items, and hook event log.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct HookFields {
    /// Universal agent state — replaces pause_classification.
    /// Always present (never null), with group/state/label/confidence.
    pub agent_state: AgentState,
    /// PID of the running Claude process, if any.
    pub pid: Option<u32>,
    /// Session title derived from the first non-meta user message.
    pub title: String,
    /// The last user message text (truncated for display).
    pub last_user_message: String,
    /// Human-readable description of the current activity.
    pub current_activity: String,
    /// Number of user/assistant turn pairs.
    pub turn_count: u32,
    /// Unix timestamp of the most recent file modification.
    #[ts(type = "number")]
    pub last_activity_at: i64,
    /// Unix timestamp when the current user turn started (real prompt detected).
    /// Used by frontend to compute live elapsed time for autonomous sessions.
    #[ts(type = "number | null")]
    pub current_turn_started_at: Option<i64>,
    /// Sub-agents spawned via the Task tool in this session.
    /// Empty vec if no sub-agents have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<claude_view_core::subagent::SubAgentInfo>,
    /// Task/todo progress items tracked from TodoWrite and TaskCreate/TaskUpdate.
    /// Empty vec if no progress items have been detected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Number of context compactions in this session (compact_boundary system messages).
    pub compact_count: u32,
    /// Monotonic timestamp when `agent_state` was last set. Same semantics.
    #[serde(skip)]
    #[ts(skip)]
    pub agent_state_set_at: i64,
    /// Hook lifecycle events captured for the event log.
    /// Skipped in SSE serialization (too large); streamed via WS only.
    #[serde(skip_serializing)]
    pub hook_events: Vec<HookEvent>,
    /// Truncated preview of last assistant response (~200 chars). From Stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_assistant_preview: Option<String>,
    /// Last API error type (rate_limit, server_error, etc.). From StopFailure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Last API error details. From StopFailure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_details: Option<String>,
}

impl Default for HookFields {
    fn default() -> Self {
        Self {
            agent_state: AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "unknown".into(),
                label: "Unknown".into(),
                context: None,
            },
            pid: None,
            title: String::new(),
            last_user_message: String::new(),
            current_activity: String::new(),
            turn_count: 0,
            last_activity_at: 0,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            hook_events: Vec::new(),
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
        }
    }
}

/// A live session snapshot broadcast to connected SSE clients.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    /// Session UUID (filename without .jsonl extension).
    pub id: String,
    /// Current derived session status.
    pub status: SessionStatus,
    /// Unix timestamp when the session started, if known.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp when this session's process exited (None = still running).
    /// Set by reconciliation loop or SessionEnd hook. Used by frontend for
    /// "closed Xm ago" display and by recently-closed persistence.
    #[ts(type = "number | null")]
    pub closed_at: Option<i64>,
    /// If Some, this session is being controlled via the sidecar Agent SDK.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlBinding>,
    // -- Cross-source fields (written by both statusline and JSONL watcher) --
    /// The primary model used in this session.
    pub model: Option<String>,
    /// Display name from statusline (e.g. "Opus", "Sonnet"). Source of truth for live sessions.
    /// Cross-source field — NOT moved into any sub-struct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_display_name: Option<String>,
    /// Monotonic timestamp when `model` was last set. Writers only overwrite
    /// when their timestamp > this value, preventing stale statusline updates
    /// from clobbering newer hook values. NOT serialized.
    #[serde(skip)]
    #[ts(skip)]
    pub model_set_at: i64,
    /// Current context window fill: total input tokens from the last assistant turn.
    #[ts(type = "number")]
    pub context_window_tokens: u64,
    // -- Sub-structs (flattened for zero wire format change) --
    /// All statusline-derived fields (32 fields). Flattened into the JSON
    /// output for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub statusline: StatuslineFields,
    /// All hook-sourced fields (agent_state, pid, title, turn_count, etc.).
    /// Flattened into JSON for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub hook: HookFields,
    /// All JSONL-watcher-sourced fields (22 fields). Flattened into JSON
    /// for zero wire format change.
    #[serde(flatten)]
    #[ts(flatten)]
    pub jsonl: JsonlFields,
}

/// A single hook lifecycle event, captured for the event log.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    /// Unix timestamp (seconds).
    #[ts(type = "number")]
    pub timestamp: i64,
    /// Hook event name: "PreToolUse", "PostToolUse", "Stop", etc.
    pub event_name: String,
    /// Tool name, if applicable.
    pub tool_name: Option<String>,
    /// Human-readable label (from resolve_state_from_hook).
    pub label: String,
    /// Agent state group: "autonomous" or "needs_you".
    pub group: String,
    /// Optional context JSON (tool_input, error, prompt snippet, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Origin channel: "hook" (Channel B), "hook_progress" (Channel A), "synthesized".
    pub source: String,
}

impl HookEvent {
    /// Convert to the DB row type. Maps `group` → `group_name`.
    pub fn to_row(&self) -> claude_view_db::HookEventRow {
        claude_view_db::HookEventRow {
            timestamp: self.timestamp,
            event_name: self.event_name.clone(),
            tool_name: self.tool_name.clone(),
            label: self.label.clone(),
            group_name: self.group.clone(),
            context: self.context.clone(),
            source: self.source.clone(),
        }
    }
}

/// Maximum hook events kept in memory per session.
pub(crate) const MAX_HOOK_EVENTS_PER_SESSION: usize = 5000;

/// Append a hook event, draining oldest 100 events if at capacity.
pub(crate) fn append_capped_hook_event(dst: &mut Vec<HookEvent>, event: HookEvent, max: usize) {
    if dst.len() >= max {
        dst.drain(..100.min(dst.len()));
    }
    dst.push(event);
}

/// Append multiple hook events, draining overflow from the front.
#[allow(dead_code)]
pub(crate) fn append_capped_hook_events(
    dst: &mut Vec<HookEvent>,
    mut events: Vec<HookEvent>,
    max: usize,
) {
    if events.is_empty() {
        return;
    }
    dst.append(&mut events);
    if dst.len() > max {
        let overflow = dst.len() - max;
        dst.drain(..overflow);
    }
}

/// Binding from observation (LiveSession) → control (sidecar SDK session).
/// Present when the user has taken interactive control of this session.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ControlBinding {
    /// The sidecar's internal control ID (UUID).
    pub control_id: String,
    /// Unix timestamp when this binding was created.
    #[ts(type = "number")]
    pub bound_at: i64,
    /// Cancellation token to abort the WS relay task on unbind.
    /// Not serialized — runtime-only.
    #[serde(skip)]
    #[ts(skip)]
    pub cancel: tokio_util::sync::CancellationToken,
}

/// A per-session snapshot entry persisted to disk for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEntry {
    /// Bound PID of the Claude process.
    pub pid: u32,
    /// Session status as string: "working", "paused", "done".
    pub status: String,
    /// Last known agent state (from hooks).
    pub agent_state: AgentState,
    /// Unix timestamp of last activity.
    pub last_activity_at: i64,
    /// Persisted control_id so controlled sessions survive Rust server restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub control_id: Option<String>,
}

/// The on-disk snapshot format (v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub version: u8,
    pub sessions: std::collections::HashMap<String, SnapshotEntry>,
}

/// Events broadcast over the SSE channel to connected Live Monitor clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session JSONL file was discovered on disk.
    SessionDiscovered { session: LiveSession },
    /// An existing session was updated (new lines appended to JSONL).
    SessionUpdated { session: LiveSession },
    /// A session's process exited — session moves to "recently closed" on the frontend.
    /// Carries the full session data so the frontend can display it without a REST call.
    SessionClosed { session: LiveSession },
    /// A session has been cleaned up (Complete for >10 min).
    SessionCompleted {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    /// Periodic aggregate summary of all live sessions.
    Summary {
        #[serde(rename = "needsYouCount")]
        needs_you_count: usize,
        #[serde(rename = "autonomousCount")]
        autonomous_count: usize,
        #[serde(rename = "totalCostTodayUsd")]
        total_cost_today_usd: f64,
        #[serde(rename = "totalTokensToday")]
        total_tokens_today: u64,
    },
}

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
pub fn status_from_agent_state(agent_state: &AgentState) -> SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou => SessionStatus::Paused,
        },
    }
}

/// Minimal LiveSession factory for cross-module tests.
#[cfg(test)]
pub(crate) fn test_live_session(id: &str) -> LiveSession {
    LiveSession {
        id: id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(1000),
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 5,
            last_activity_at: 1000,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: JsonlFields {
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            tokens: TokenUsage::default(),
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            last_turn_task_seconds: None,
            last_cache_hit_at: None,
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            tools_used: Vec::new(),
            slug: None,
            user_files: None,
            source: None,
            phase: PhaseHistory::default(),
        },
    }
}

// =============================================================================
// classify_live_session — extracted from control.rs for reuse
// =============================================================================

/// What to do with a session that's in the live_sessions map.
///
/// Extracted from control.rs so the gate logic is reusable across routes.
/// Design rule: **only return `Block` for states the Agent SDK fundamentally
/// cannot handle.** Process liveness is NOT such a state — the SDK creates
/// a new CLI process from session history, so dead-PID sessions resume fine.
#[derive(Debug)]
pub enum LiveSessionAction {
    /// Session not tracked by the live monitor → proceed to SDK resume.
    ResumeNew,
    /// Session tracked but process is dead → proceed to SDK resume (new process).
    ResumeDeadProcess,
    /// Session has an active PID but no control binding → proceed to SDK resume.
    ResumeAlive,
    /// Session is already controlled → reuse the existing binding.
    ReuseExisting {
        control_id: String,
        cancel: tokio_util::sync::CancellationToken,
    },
}

impl PartialEq for LiveSessionAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ResumeNew, Self::ResumeNew)
            | (Self::ResumeDeadProcess, Self::ResumeDeadProcess)
            | (Self::ResumeAlive, Self::ResumeAlive) => true,
            (
                Self::ReuseExisting { control_id: a, .. },
                Self::ReuseExisting { control_id: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

/// Decide what action to take for a live session connect request.
///
/// Pure function (no side effects) so it can be unit-tested directly.
pub fn classify_live_session(session: Option<&LiveSession>) -> LiveSessionAction {
    match session {
        None => LiveSessionAction::ResumeNew,
        Some(s)
            if s.hook.pid.is_none()
                || !crate::live::process::is_pid_alive(s.hook.pid.unwrap_or(0)) =>
        {
            LiveSessionAction::ResumeDeadProcess
        }
        Some(s) if s.control.is_some() => {
            let ctl = s.control.as_ref().unwrap();
            LiveSessionAction::ReuseExisting {
                control_id: ctl.control_id.clone(),
                cancel: ctl.cancel.clone(),
            }
        }
        _ => LiveSessionAction::ResumeAlive,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_file_serializes_to_camel_case() {
        let file = VerifiedFile {
            path: "/Users/dev/project/src/auth.rs".into(),
            kind: FileSourceKind::Mention,
            display_name: "src/auth.rs".into(),
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json["path"], "/Users/dev/project/src/auth.rs");
        assert_eq!(json["kind"], "mention");
        assert_eq!(json["displayName"], "src/auth.rs");
    }

    #[test]
    fn file_source_kind_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(FileSourceKind::Mention).unwrap(),
            "mention"
        );
        assert_eq!(serde_json::to_value(FileSourceKind::Ide).unwrap(), "ide");
        assert_eq!(
            serde_json::to_value(FileSourceKind::Pasted).unwrap(),
            "pasted"
        );
    }

    #[test]
    fn test_status_from_autonomous_acting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_autonomous_thinking() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Thinking".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_autonomous_delegating() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: "Running agent".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_status_from_needs_you_idle() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Idle".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_needs_you_awaiting_input() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: "Asked a question".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_needs_you_needs_permission() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: "Needs permission".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
    }

    #[test]
    fn test_status_from_session_ended() {
        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Done);
    }

    #[test]
    fn test_status_from_session_ended_autonomous_group() {
        // session_ended should always produce Done regardless of group
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "session_ended".into(),
            label: "Ended".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Done);
    }

    #[test]
    fn test_status_from_compacting() {
        let state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "compacting".into(),
            label: "Auto-compacting context...".into(),
            context: None,
        };
        assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
    }

    #[test]
    fn test_control_binding_serializes_to_camel_case() {
        let binding = ControlBinding {
            control_id: "abc-123".to_string(),
            bound_at: 1700000000,
            cancel: tokio_util::sync::CancellationToken::new(),
        };
        let json = serde_json::to_value(&binding).unwrap();
        assert_eq!(json["controlId"], "abc-123");
        assert_eq!(json["boundAt"], 1700000000);
    }

    #[test]
    fn test_snapshot_entry_with_control_id() {
        let entry = SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: Some("ctrl-456".to_string()),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["controlId"], "ctrl-456");
    }

    #[test]
    fn test_snapshot_entry_without_control_id_omits_field() {
        let entry = SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json.get("controlId").is_none());
    }

    #[test]
    fn test_snapshot_entry_backward_compat_no_control_id() {
        let json = r#"{"pid":12345,"status":"working","agentState":{"group":"autonomous","state":"acting","label":"Working"},"lastActivityAt":1700000000}"#;
        let entry: SnapshotEntry = serde_json::from_str(json).unwrap();
        assert!(entry.control_id.is_none());
        assert_eq!(entry.pid, 12345);
    }

    /// Minimal LiveSession for tests.
    fn minimal_live_session(id: &str) -> LiveSession {
        test_live_session(id)
    }

    #[test]
    fn verified_file_in_live_session_serializes_correctly() {
        let mut session = minimal_live_session("test-1");
        session.jsonl.user_files = Some(vec![
            VerifiedFile {
                path: "/Users/dev/src/auth.rs".into(),
                kind: FileSourceKind::Mention,
                display_name: "src/auth.rs".into(),
            },
            VerifiedFile {
                path: "/Users/dev/src/main.rs".into(),
                kind: FileSourceKind::Ide,
                display_name: "src/main.rs".into(),
            },
        ]);
        let json = serde_json::to_value(&session).unwrap();
        let files = json["userFiles"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["kind"], "mention");
        assert_eq!(files[0]["displayName"], "src/auth.rs");
        assert_eq!(files[1]["kind"], "ide");
    }

    #[test]
    fn test_session_closed_event_serializes_with_type_tag() {
        let mut session = minimal_live_session("abc-123");
        session.status = SessionStatus::Done;
        session.closed_at = Some(1_700_000_000);

        let event = SessionEvent::SessionClosed { session };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(
            json["type"], "session_closed",
            "serde tag must produce 'session_closed' (snake_case)"
        );
        assert!(
            json["session"].is_object(),
            "must embed the full session object under 'session' key"
        );
        assert_eq!(json["session"]["id"], "abc-123");
        assert_eq!(json["session"]["closedAt"], 1_700_000_000);
    }

    // -----------------------------------------------------------------------
    // classify_live_session — unit tests for the gate logic.
    //
    // These prevent the dead-PID 410 regression: the function is pure (no IO),
    // so every branch is tested cheaply without spawning sidecar/live_manager.
    // -----------------------------------------------------------------------

    use super::{classify_live_session, LiveSessionAction};

    #[test]
    fn classify_none_returns_resume_new() {
        assert_eq!(classify_live_session(None), LiveSessionAction::ResumeNew);
    }

    /// Regression: dead-PID sessions MUST proceed to resume, never block.
    #[test]
    fn classify_dead_pid_returns_resume_not_block() {
        let mut session = test_live_session("dead-pid-session");
        session.hook.pid = Some(999_999);
        let action = classify_live_session(Some(&session));
        assert_eq!(
            action,
            LiveSessionAction::ResumeDeadProcess,
            "Dead-PID session must resume, not block — SDK creates a new CLI process"
        );
    }

    #[test]
    fn classify_no_pid_returns_resume_dead() {
        let mut session = test_live_session("no-pid-session");
        session.hook.pid = None;
        let action = classify_live_session(Some(&session));
        assert_eq!(action, LiveSessionAction::ResumeDeadProcess);
    }

    #[test]
    fn classify_alive_pid_no_control_returns_resume_alive() {
        let mut session = test_live_session("alive-session");
        session.hook.pid = Some(std::process::id());
        session.control = None;
        let action = classify_live_session(Some(&session));
        assert_eq!(action, LiveSessionAction::ResumeAlive);
    }

    #[test]
    fn classify_already_controlled_returns_reuse() {
        let mut session = test_live_session("controlled-session");
        session.hook.pid = Some(std::process::id());
        session.control = Some(ControlBinding {
            control_id: "ctl-123".to_string(),
            bound_at: 0,
            cancel: tokio_util::sync::CancellationToken::new(),
        });
        let action = classify_live_session(Some(&session));
        match action {
            LiveSessionAction::ReuseExisting { control_id, .. } => {
                assert_eq!(control_id, "ctl-123");
            }
            other => panic!("Expected ReuseExisting, got {other:?}"),
        }
    }
}
