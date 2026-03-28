//! Per-session accumulator state and metadata types.
//!
//! The `SessionAccumulator` tracks byte offsets, token counts, and other
//! per-session state that persists across JSONL tail polls. `JsonlMetadata`
//! is the snapshot of accumulator state applied to a `LiveSession`.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use claude_view_core::phase::client::ConversationTurn;
use claude_view_core::phase::stabilizer::ClassificationStabilizer;
use claude_view_core::phase::{PhaseHistory, PhaseLabel};
use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};
use claude_view_core::subagent::SubAgentInfo;

use crate::live::file_resolver::resolve_file_path;
use crate::live::state::{
    AgentState, AgentStateGroup, FileSourceKind, HookFields, JsonlFields, LiveSession,
    SessionStatus, SnapshotEntry, StatuslineFields, VerifiedFile,
};

use super::helpers::{extract_project_info, seconds_since_modified_from_timestamp};

/// Resolve accumulated raw file references (at_files + pasted_paths) into VerifiedFiles.
/// Deduplicates by absolute path, caps at 10 total entries.
pub(super) fn resolve_accumulated_files(
    at_files: &HashSet<String>,
    pasted_paths: &HashSet<String>,
    cwd: Option<&str>,
    project_dir: Option<&str>,
) -> Option<Vec<VerifiedFile>> {
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    // Resolve @file mentions first
    let mut sorted_at: Vec<_> = at_files.iter().cloned().collect();
    sorted_at.sort();
    for raw in &sorted_at {
        if resolved.len() >= 10 {
            break;
        }
        if let Some(vf) = resolve_file_path(raw, FileSourceKind::Mention, cwd, project_dir) {
            if seen.insert(vf.path.clone()) {
                resolved.push(vf);
            }
        }
    }

    // Then pasted absolute paths
    let mut sorted_pasted: Vec<_> = pasted_paths.iter().cloned().collect();
    sorted_pasted.sort();
    for raw in &sorted_pasted {
        if resolved.len() >= 10 {
            break;
        }
        if let Some(vf) = resolve_file_path(raw, FileSourceKind::Pasted, cwd, project_dir) {
            if seen.insert(vf.path.clone()) {
                resolved.push(vf);
            }
        }
    }

    if resolved.is_empty() {
        None
    } else {
        Some(resolved)
    }
}

/// Accumulated per-session state that persists across tail polls.
pub(super) struct SessionAccumulator {
    /// Byte offset for the next `parse_tail` call.
    pub offset: u64,
    /// Accumulated token counts (for cost calculation).
    pub tokens: TokenUsage,
    /// Last assistant turn's total input tokens (= current context window fill).
    pub context_window_tokens: u64,
    /// Last parsed model ID.
    pub model: Option<String>,
    /// Number of user turns seen.
    pub user_turn_count: u32,
    /// The first non-meta user message (used as session title).
    pub first_user_message: String,
    /// The last user message content (truncated).
    pub last_user_message: String,
    /// Git branch name extracted from user messages.
    pub git_branch: Option<String>,
    /// Latest cwd from user messages (for worktree branch resolution).
    pub latest_cwd: Option<String>,
    /// The timestamp of the first line (session start).
    pub started_at: Option<i64>,
    /// Unix timestamp when the current user turn started.
    pub current_turn_started_at: Option<i64>,
    /// Seconds the agent spent on the last completed turn.
    pub last_turn_task_seconds: Option<u32>,
    /// Sub-agents spawned in this session.
    pub sub_agents: Vec<SubAgentInfo>,
    /// Team name if this session is a team lead.
    pub team_name: Option<String>,
    /// Current todo items from the latest TodoWrite call (full replacement).
    pub todo_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Structured tasks from TaskCreate/TaskUpdate (incremental).
    pub task_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Unix timestamp of the most recent cache hit or creation.
    pub last_cache_hit_at: Option<i64>,
    /// Unique MCP server names seen (deduplicated).
    pub mcp_servers: HashSet<String>,
    /// Unique skill names seen (deduplicated).
    pub skills: HashSet<String>,
    /// Accumulated @file mentions from user messages.
    pub at_files: HashSet<String>,
    /// Accumulated pasted absolute paths from user messages.
    pub pasted_paths: HashSet<String>,
    /// Path to the JSONL file on disk.
    pub file_path: Option<PathBuf>,
    /// Decoded project path.
    pub project_path: Option<String>,
    /// Cached cwd resolved from JSONL.
    pub resolved_cwd: Option<String>,
    /// Accumulated tool counts (cumulative across tail polls).
    pub tool_counts_edit: u32,
    pub tool_counts_read: u32,
    pub tool_counts_bash: u32,
    pub tool_counts_write: u32,
    /// Number of compact_boundary system messages seen.
    pub compact_count: u32,
    /// Session slug for plan file association.
    pub slug: Option<String>,
    /// Per-turn accumulated cost breakdown.
    pub accumulated_cost: CostBreakdown,
    /// Dedup guard for split assistant content blocks.
    pub seen_api_calls: HashSet<String>,
    /// Sliding window of recent conversation turns for phase classification.
    pub message_buf: VecDeque<ConversationTurn>,
    /// Whether message_buf has new content since last classify request.
    pub message_buf_dirty: bool,
    /// Total messages accumulated (monotonic counter for skip logic).
    pub message_buf_total: u32,
    /// Stabilizer for smoothing noisy LLM classifications.
    pub stabilizer: ClassificationStabilizer,
    /// Monotonic generation counter for classify request dedup.
    pub classify_generation: u64,
    /// Phase labels emitted so far (one per classification).
    pub phase_labels: Vec<PhaseLabel>,
}

impl SessionAccumulator {
    pub fn new() -> Self {
        Self {
            offset: 0,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            model: None,
            user_turn_count: 0,
            first_user_message: String::new(),
            last_user_message: String::new(),
            git_branch: None,
            latest_cwd: None,
            started_at: None,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            team_name: None,
            todo_items: Vec::new(),
            task_items: Vec::new(),
            last_cache_hit_at: None,
            mcp_servers: HashSet::new(),
            skills: HashSet::new(),
            at_files: HashSet::new(),
            pasted_paths: HashSet::new(),
            file_path: None,
            project_path: None,
            resolved_cwd: None,
            tool_counts_edit: 0,
            tool_counts_read: 0,
            tool_counts_bash: 0,
            tool_counts_write: 0,
            compact_count: 0,
            slug: None,
            accumulated_cost: CostBreakdown::default(),
            seen_api_calls: HashSet::new(),
            message_buf: VecDeque::new(),
            message_buf_dirty: false,
            message_buf_total: 0,
            stabilizer: ClassificationStabilizer::new(),
            classify_generation: 0,
            phase_labels: Vec::new(),
        }
    }
}

/// Metadata extracted from JSONL processing -- never touches agent_state or status.
pub(super) struct JsonlMetadata {
    pub git_branch: Option<String>,
    pub worktree_branch: Option<String>,
    pub is_worktree: bool,
    pub pid: Option<u32>,
    pub title: String,
    pub last_user_message: String,
    pub turn_count: u32,
    pub started_at: Option<i64>,
    pub last_activity_at: i64,
    pub model: Option<String>,
    pub tokens: TokenUsage,
    pub context_window_tokens: u64,
    pub cost: CostBreakdown,
    pub cache_status: CacheStatus,
    pub current_turn_started_at: Option<i64>,
    pub last_turn_task_seconds: Option<u32>,
    pub sub_agents: Vec<SubAgentInfo>,
    pub team_name: Option<String>,
    pub progress_items: Vec<claude_view_core::progress::ProgressItem>,
    pub last_cache_hit_at: Option<i64>,
    pub tools_used: Vec<crate::live::state::ToolUsed>,
    pub compact_count: u32,
    pub slug: Option<String>,
    pub user_files: Option<Vec<VerifiedFile>>,
    pub edit_count: u32,
    pub phase: PhaseHistory,
}

/// Build a skeleton LiveSession from a crash-recovery snapshot entry.
/// The session will be enriched by `apply_jsonl_metadata` on the next JSONL poll.
pub(super) fn build_recovered_session(
    session_id: &str,
    entry: &SnapshotEntry,
    file_path: &str,
) -> LiveSession {
    let path = Path::new(file_path);
    let (project, project_display_name, project_path, _) = extract_project_info(path, None);

    let status = match entry.status.as_str() {
        "working" => SessionStatus::Working,
        "paused" => SessionStatus::Paused,
        _ => crate::live::state::status_from_agent_state(&entry.agent_state),
    };

    LiveSession {
        id: session_id.to_string(),
        status,
        started_at: None,
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            agent_state: entry.agent_state.clone(),
            pid: Some(entry.pid),
            title: String::new(),
            last_user_message: String::new(),
            current_activity: entry.agent_state.label.clone(),
            turn_count: 0,
            last_activity_at: entry.last_activity_at,
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
            project,
            project_display_name,
            project_path,
            file_path: file_path.to_string(),
            ..JsonlFields::default()
        },
    }
}

/// Derive agent state from the JSONL file's tail -- ground truth for startup recovery.
pub(super) async fn derive_agent_state_from_jsonl(path: &Path) -> Option<AgentState> {
    use claude_view_core::live_parser::{LineType, TailFinders};

    let lines = claude_view_core::tail::tail_lines(path, 10).await.ok()?;
    if lines.is_empty() {
        return None;
    }

    let finders = TailFinders::new();

    // Scan from the end to find the last meaningful line
    for raw in lines.iter().rev() {
        let parsed = claude_view_core::live_parser::parse_single_line(raw.as_bytes(), &finders);

        match parsed.line_type {
            LineType::Progress | LineType::System | LineType::Other => {
                continue; // Skip non-meaningful lines
            }
            LineType::Assistant => {
                let has_tool_use = !parsed.tool_names.is_empty();
                let stop = parsed.stop_reason.as_deref();
                let is_tool_active = stop == Some("tool_use") || has_tool_use;

                return Some(if stop == Some("end_turn") {
                    AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for your next prompt".into(),
                        context: None,
                    }
                } else if is_tool_active {
                    AgentState {
                        group: AgentStateGroup::Autonomous,
                        state: "acting".into(),
                        label: format!(
                            "Using {}",
                            parsed
                                .tool_names
                                .first()
                                .map(|s| s.as_str())
                                .unwrap_or("tool")
                        ),
                        context: None,
                    }
                } else {
                    AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for your next prompt".into(),
                        context: None,
                    }
                });
            }
            LineType::User => {
                return Some(AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: if parsed.is_tool_result_continuation {
                        "Processing tool result...".into()
                    } else {
                        "Processing prompt...".into()
                    },
                    context: None,
                });
            }
        }
    }

    None
}

/// Apply JSONL metadata to an existing session without touching hook-owned fields
/// (agent_state, status, current_activity).
pub(super) fn apply_jsonl_metadata(
    session: &mut LiveSession,
    m: &JsonlMetadata,
    file_path: &str,
    project: &str,
    project_display_name: &str,
    project_path: &str,
) {
    session.jsonl.file_path = file_path.to_string();
    session.jsonl.project = project.to_string();
    session.jsonl.project_display_name = project_display_name.to_string();
    session.jsonl.project_path = project_path.to_string();
    // Only overwrite branch fields when the JSONL accumulator has a definitive value.
    // Hooks resolve branch eagerly from CWD (filesystem HEAD); the accumulator learns
    // gitBranch later when user-type JSONL lines are parsed. Without this guard,
    // the first process_jsonl_update (which may only contain metadata lines) overwrites
    // the hook-resolved branch with None, causing a "(no branch)" flash in the UI.
    if m.git_branch.is_some() {
        session.jsonl.git_branch = m.git_branch.clone();
    }
    if m.worktree_branch.is_some() {
        session.jsonl.worktree_branch = m.worktree_branch.clone();
    }
    if m.is_worktree {
        session.jsonl.is_worktree = true;
    }
    // Recompute effective_branch from current session state
    let new_effective = session
        .jsonl
        .worktree_branch
        .clone()
        .or(session.jsonl.git_branch.clone());
    if new_effective.is_some() {
        session.jsonl.effective_branch = new_effective;
    }
    // PID binding: only assign PID on first discovery.
    if session.hook.pid.is_none() {
        session.hook.pid = m.pid;
    }
    if !m.title.is_empty() {
        session.hook.title = m.title.clone();
    }
    if !m.last_user_message.is_empty() {
        session.hook.last_user_message = m.last_user_message.clone();
    }
    session.hook.turn_count = m.turn_count;
    if m.started_at.is_some() {
        session.started_at = m.started_at;
    }
    session.hook.last_activity_at = m.last_activity_at;
    // Model -- timestamp-guarded. JSONL parser has lower authority than statusline.
    if m.model.is_some() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if now > session.model_set_at {
            session.model = m.model.clone();
            session.model_set_at = now;
        }
    }
    session.jsonl.tokens = m.tokens.clone();
    session.context_window_tokens = m.context_window_tokens;
    session.jsonl.cost = m.cost.clone();
    session.jsonl.cache_status = m.cache_status.clone();
    session.hook.current_turn_started_at = m.current_turn_started_at;
    session.jsonl.last_turn_task_seconds = m.last_turn_task_seconds;
    session.hook.sub_agents = m.sub_agents.clone();
    session.jsonl.team_name = m.team_name.clone();
    session.hook.progress_items = m.progress_items.clone();
    session.jsonl.tools_used = m.tools_used.clone();
    session.jsonl.last_cache_hit_at = m.last_cache_hit_at;
    session.hook.compact_count = m.compact_count;
    session.jsonl.slug = m.slug.clone();
    if m.user_files.is_some() {
        session.jsonl.user_files = m.user_files.clone();
    }
    session.jsonl.edit_count = m.edit_count;
    session.jsonl.phase = m.phase.clone();
}

/// Build a `JsonlMetadata` snapshot from an accumulator's current state.
/// Used by both `process_jsonl_update` and `enrich_session_from_accumulator`.
pub(super) fn build_metadata_from_accumulator(
    acc: &SessionAccumulator,
    last_activity_at: i64,
    pid: Option<u32>,
) -> JsonlMetadata {
    use claude_view_core::discovery::resolve_worktree_branch;
    use claude_view_core::phase::dominant_phase;
    use claude_view_core::pricing::finalize_cost_breakdown;

    let mut cost = acc.accumulated_cost.clone();
    finalize_cost_breakdown(&mut cost, &acc.tokens);

    let cache_status = match acc.last_cache_hit_at {
        Some(ts) => {
            let secs = seconds_since_modified_from_timestamp(ts);
            if secs < 300 {
                CacheStatus::Warm
            } else {
                CacheStatus::Cold
            }
        }
        None => CacheStatus::Unknown,
    };

    let wt_branch = acc.latest_cwd.as_deref().and_then(resolve_worktree_branch);

    JsonlMetadata {
        git_branch: acc.git_branch.clone(),
        is_worktree: wt_branch.is_some(),
        worktree_branch: wt_branch,
        pid,
        title: acc.first_user_message.clone(),
        last_user_message: acc.last_user_message.clone(),
        turn_count: acc.user_turn_count,
        started_at: acc.started_at,
        last_activity_at,
        model: acc.model.clone(),
        tokens: acc.tokens.clone(),
        context_window_tokens: acc.context_window_tokens,
        cost,
        cache_status,
        current_turn_started_at: acc.current_turn_started_at,
        last_turn_task_seconds: acc.last_turn_task_seconds,
        sub_agents: acc.sub_agents.clone(),
        team_name: acc.team_name.clone(),
        progress_items: {
            let mut items = acc.todo_items.clone();
            items.extend(acc.task_items.clone());
            items
        },
        tools_used: {
            let mut mcp: Vec<_> = acc.mcp_servers.iter().cloned().collect();
            mcp.sort();
            let mut skill: Vec<_> = acc.skills.iter().cloned().collect();
            skill.sort();
            let mut tools = Vec::with_capacity(mcp.len() + skill.len());
            for name in mcp {
                tools.push(crate::live::state::ToolUsed {
                    name,
                    kind: "mcp".to_string(),
                });
            }
            for name in skill {
                tools.push(crate::live::state::ToolUsed {
                    name,
                    kind: "skill".to_string(),
                });
            }
            tools
        },
        last_cache_hit_at: acc.last_cache_hit_at,
        compact_count: acc.compact_count,
        slug: acc.slug.clone(),
        user_files: resolve_accumulated_files(
            &acc.at_files,
            &acc.pasted_paths,
            acc.resolved_cwd.as_deref(),
            acc.project_path.as_deref(),
        ),
        edit_count: acc.tool_counts_edit + acc.tool_counts_write,
        phase: PhaseHistory {
            current: acc.phase_labels.last().cloned(),
            dominant: dominant_phase(&acc.phase_labels),
            labels: acc.phase_labels.clone(),
        },
    }
}
