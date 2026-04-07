//! Type definitions for the session accumulator.
//!
//! Contains the core accumulator state struct and its output type.

use std::collections::{HashMap, HashSet};

use crate::phase::{PhaseHistory, PhaseLabel};
use crate::pricing::{CacheStatus, CostBreakdown, TokenUsage};
use crate::progress::ProgressItem;
use crate::subagent::SubAgentInfo;

/// Accumulated per-session state -- shared between live monitoring and history batch parsing.
///
/// Feed lines via [`process_line`](SessionAccumulator::process_line), then call
/// [`finish`](SessionAccumulator::finish) to produce the final [`RichSessionData`].
pub struct SessionAccumulator {
    pub tokens: TokenUsage,
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub user_turn_count: u32,
    pub first_user_message: String,
    pub last_user_message: String,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub sub_agents: Vec<SubAgentInfo>,
    /// Team name if this session is a team lead (captured from first team spawn).
    pub team_name: Option<String>,
    pub todo_items: Vec<ProgressItem>,
    pub task_items: Vec<ProgressItem>,
    pub last_cache_hit_at: Option<i64>,
    pub slug: Option<String>,
    /// Per-turn accumulated cost breakdown. Each assistant turn's tokens are
    /// priced individually (correct: 200k tiering is per-API-request, not
    /// per-session). This avoids the inflation bug from applying tiered
    /// pricing to cumulative session totals.
    pub accumulated_cost: CostBreakdown,
    /// Dedup: track seen `message.id:requestId` pairs to avoid counting
    /// tokens/cost multiple times when Claude Code splits one API response into
    /// multiple JSONL lines (one per content block: thinking, text, tool_use).
    pub(crate) seen_api_calls: HashSet<String>,
    // Phase fields will be added in Task 8 (oMLX classification wiring)
    /// Phase labels emitted so far (one per classification).
    pub(crate) phase_labels: Vec<PhaseLabel>,
}

/// Rich session data -- output of accumulation. Same shape for live and history.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RichSessionData {
    pub tokens: TokenUsage,
    pub cost: CostBreakdown,
    pub cache_status: CacheStatus,
    pub sub_agents: Vec<SubAgentInfo>,
    pub team_name: Option<String>,
    pub progress_items: Vec<ProgressItem>,
    #[ts(type = "number")]
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub turn_count: u32,
    pub first_user_message: Option<String>,
    pub last_user_message: Option<String>,
    #[ts(type = "number | null")]
    pub last_cache_hit_at: Option<i64>,
    pub slug: Option<String>,
    /// SDLC phase classification: current phase, label history, and dominant phase.
    pub phase: PhaseHistory,
}

impl SessionAccumulator {
    /// Create a zero-initialized accumulator.
    pub fn new() -> Self {
        Self {
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            model: None,
            user_turn_count: 0,
            first_user_message: String::new(),
            last_user_message: String::new(),
            git_branch: None,
            started_at: None,
            sub_agents: Vec::new(),
            team_name: None,
            todo_items: Vec::new(),
            task_items: Vec::new(),
            last_cache_hit_at: None,
            slug: None,
            accumulated_cost: CostBreakdown::default(),
            seen_api_calls: HashSet::new(),
            phase_labels: Vec::new(),
        }
    }
}

impl Default for SessionAccumulator {
    fn default() -> Self {
        Self::new()
    }
}
