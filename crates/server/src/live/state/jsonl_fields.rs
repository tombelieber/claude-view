//! JSONL-watcher-sourced fields extracted from LiveSession.

use crate::live::process::SessionSourceInfo;
use claude_view_core::phase::PhaseHistory;
use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};
use serde::Serialize;
use ts_rs::TS;

use super::field_types::{is_zero_u32, ToolUsed, VerifiedFile};

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
    /// AI-generated session title (from `ai-title` JSONL lines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_title: Option<String>,
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
            ai_title: None,
        }
    }
}
