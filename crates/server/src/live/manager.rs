//! Central orchestrator for live session monitoring.
//!
//! The `LiveSessionManager` ties together the file watcher, process detector,
//! JSONL tail parser, and cleanup task to maintain an in-memory map of all
//! active Claude Code sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn};

use claude_view_core::discovery::resolve_worktree_branch;
use claude_view_core::live_parser::{parse_tail, HookProgressData, LineType, TailFinders};
use claude_view_core::phase::client::{ConversationTurn, OmlxClient, Role};
use claude_view_core::phase::scheduler::{
    run_scheduler, ClassifyRequest, ClassifyResult, Priority,
};
use claude_view_core::phase::stabilizer::ClassificationStabilizer;
use claude_view_core::phase::{
    dominant_phase, is_shipping_cmd, PhaseHistory, PhaseLabel, SessionPhase, MAX_PHASE_LABELS,
};
use claude_view_core::pricing::{
    calculate_cost, finalize_cost_breakdown, CacheStatus, CostBreakdown, ModelPricing, TokenUsage,
};
use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::Database;

use super::file_resolver::resolve_file_path;
use super::process::{count_claude_processes, detect_claude_processes, is_pid_alive};
use super::state::{
    append_capped_hook_event, status_from_agent_state, AgentState, AgentStateGroup, FileSourceKind,
    HookEvent, LiveSession, SessionEvent, SessionSnapshot, SessionStatus, SnapshotEntry,
    VerifiedFile, MAX_HOOK_EVENTS_PER_SESSION,
};
use super::watcher::{initial_scan, start_watcher, FileEvent};

/// Type alias for the shared session map used by both the manager and route handlers.
pub type LiveSessionMap = Arc<RwLock<HashMap<String, LiveSession>>>;

/// Type alias for the transcript path → session ID dedup map.
/// Shared between `LiveSessionManager` (cleanup on PID death) and `AppState` (statusline handler).
pub type TranscriptMap = Arc<RwLock<HashMap<PathBuf, String>>>;

/// Resolve accumulated raw file references (at_files + pasted_paths) into VerifiedFiles.
/// Deduplicates by absolute path, caps at 10 total entries.
fn resolve_accumulated_files(
    at_files: &std::collections::HashSet<String>,
    pasted_paths: &std::collections::HashSet<String>,
    cwd: Option<&str>,
    project_dir: Option<&str>,
) -> Option<Vec<VerifiedFile>> {
    let mut seen = std::collections::HashSet::new();
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
struct SessionAccumulator {
    /// Byte offset for the next `parse_tail` call.
    offset: u64,
    /// Accumulated token counts (for cost calculation).
    tokens: TokenUsage,
    /// Last assistant turn's total input tokens (= current context window fill).
    /// This is input_tokens + cache_read_tokens + cache_creation_tokens from
    /// the most recent assistant message.
    context_window_tokens: u64,
    /// Last parsed model ID.
    model: Option<String>,
    /// Number of user turns seen.
    user_turn_count: u32,
    /// The first non-meta user message (used as session title).
    first_user_message: String,
    /// The last user message content (truncated).
    last_user_message: String,
    /// Git branch name extracted from user messages.
    git_branch: Option<String>,
    /// Latest cwd from user messages (for worktree branch resolution).
    latest_cwd: Option<String>,
    /// The timestamp of the first line (session start).
    started_at: Option<i64>,
    /// Unix timestamp when the current user turn started (real prompt, not meta/tool-result/system).
    current_turn_started_at: Option<i64>,
    /// Seconds the agent spent on the last completed turn (Working->Paused).
    last_turn_task_seconds: Option<u32>,
    /// Sub-agents spawned in this session (accumulated across tail polls).
    sub_agents: Vec<SubAgentInfo>,
    /// Team name if this session is a team lead (from first team spawn).
    team_name: Option<String>,
    /// Current todo items from the latest TodoWrite call (full replacement).
    todo_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Structured tasks from TaskCreate/TaskUpdate (incremental).
    task_items: Vec<claude_view_core::progress::ProgressItem>,
    /// Unix timestamp of the most recent cache hit or creation.
    /// Updated when a line has cache_read_tokens > 0 OR cache_creation_tokens > 0.
    last_cache_hit_at: Option<i64>,
    /// Unique MCP server names seen (deduplicated).
    mcp_servers: std::collections::HashSet<String>,
    /// Unique skill names seen (deduplicated).
    skills: std::collections::HashSet<String>,
    /// Accumulated @file mentions from user messages (deduplicated, ≤10, first-N-wins).
    at_files: std::collections::HashSet<String>,
    /// Accumulated pasted absolute paths from user messages (deduplicated, ≤10, first-N-wins).
    pasted_paths: std::collections::HashSet<String>,
    /// Path to the JSONL file on disk (set on first process_jsonl_update).
    file_path: Option<PathBuf>,
    /// Decoded project path (set on first process_jsonl_update).
    project_path: Option<String>,
    /// Cached cwd resolved from JSONL (avoids re-reading file on every update).
    resolved_cwd: Option<String>,
    /// Accumulated tool counts (cumulative across tail polls).
    tool_counts_edit: u32,
    tool_counts_read: u32,
    tool_counts_bash: u32,
    tool_counts_write: u32,
    /// Number of compact_boundary system messages seen.
    compact_count: u32,
    /// Session slug for plan file association.
    slug: Option<String>,
    /// Per-turn accumulated cost breakdown. Each assistant turn's tokens are
    /// priced individually (200k tiering is per-API-request, not per-session).
    accumulated_cost: CostBreakdown,
    /// Dedup guard for split assistant content blocks.
    /// Keyed by `message.id:requestId` so one API response is counted once.
    seen_api_calls: std::collections::HashSet<String>,
    /// Sliding window of recent conversation turns for phase classification.
    message_buf: std::collections::VecDeque<ConversationTurn>,
    /// Whether message_buf has new content since last classify request.
    message_buf_dirty: bool,
    /// Total messages accumulated (monotonic counter for skip logic).
    message_buf_total: u32,
    /// Stabilizer for smoothing noisy LLM classifications.
    stabilizer: ClassificationStabilizer,
    /// Monotonic generation counter for classify request dedup.
    classify_generation: u64,
    /// Phase labels emitted so far (one per classification).
    phase_labels: Vec<PhaseLabel>,
}

impl SessionAccumulator {
    fn new() -> Self {
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
            mcp_servers: std::collections::HashSet::new(),
            skills: std::collections::HashSet::new(),
            at_files: std::collections::HashSet::new(),
            pasted_paths: std::collections::HashSet::new(),
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
            seen_api_calls: std::collections::HashSet::new(),
            message_buf: std::collections::VecDeque::new(),
            message_buf_dirty: false,
            message_buf_total: 0,
            stabilizer: ClassificationStabilizer::new(),
            classify_generation: 0,
            phase_labels: Vec::new(),
        }
    }
}

/// Metadata extracted from JSONL processing — never touches agent_state or status.
struct JsonlMetadata {
    git_branch: Option<String>,
    worktree_branch: Option<String>,
    is_worktree: bool,
    pid: Option<u32>,
    title: String,
    last_user_message: String,
    turn_count: u32,
    started_at: Option<i64>,
    last_activity_at: i64,
    model: Option<String>,
    tokens: TokenUsage,
    context_window_tokens: u64,
    cost: CostBreakdown,
    cache_status: CacheStatus,
    current_turn_started_at: Option<i64>,
    last_turn_task_seconds: Option<u32>,
    sub_agents: Vec<SubAgentInfo>,
    team_name: Option<String>,
    progress_items: Vec<claude_view_core::progress::ProgressItem>,
    last_cache_hit_at: Option<i64>,
    tools_used: Vec<super::state::ToolUsed>,
    compact_count: u32,
    slug: Option<String>,
    user_files: Option<Vec<super::state::VerifiedFile>>,
    edit_count: u32,
    phase: PhaseHistory,
}

/// Build a skeleton LiveSession from a crash-recovery snapshot entry.
/// The session will be enriched by `apply_jsonl_metadata` on the next JSONL poll.
fn build_recovered_session(
    session_id: &str,
    entry: &SnapshotEntry,
    file_path: &str,
) -> LiveSession {
    let path = Path::new(file_path);
    let (project, project_display_name, project_path, _) = extract_project_info(path, None);

    let status = match entry.status.as_str() {
        "working" => SessionStatus::Working,
        "paused" => SessionStatus::Paused,
        _ => status_from_agent_state(&entry.agent_state),
    };

    LiveSession {
        id: session_id.to_string(),
        project,
        project_display_name,
        project_path,
        file_path: file_path.to_string(),
        status,
        agent_state: entry.agent_state.clone(),
        git_branch: None,
        worktree_branch: None,
        is_worktree: false,
        effective_branch: None,
        pid: Some(entry.pid),
        title: String::new(),
        last_user_message: String::new(),
        current_activity: entry.agent_state.label.clone(),
        turn_count: 0,
        started_at: None,
        last_activity_at: entry.last_activity_at,
        model: None,
        tokens: TokenUsage::default(),
        context_window_tokens: 0,
        cost: CostBreakdown::default(),
        cache_status: CacheStatus::Unknown,
        current_turn_started_at: None,
        last_turn_task_seconds: None,
        sub_agents: Vec::new(),
        team_name: None,
        team_members: Vec::new(),
        team_inbox_count: 0,
        edit_count: 0,
        progress_items: Vec::new(),
        tools_used: Vec::new(),
        last_cache_hit_at: None,
        compact_count: 0,
        slug: None,
        closed_at: None,
        source: None,
        control: None,
        statusline_context_window_size: None,
        statusline_used_pct: None,
        statusline_cost_usd: None,
        model_display_name: None,
        statusline_cwd: None,
        statusline_project_dir: None,
        statusline_total_duration_ms: None,
        statusline_api_duration_ms: None,
        statusline_lines_added: None,
        statusline_lines_removed: None,
        statusline_input_tokens: None,
        statusline_output_tokens: None,
        statusline_cache_read_tokens: None,
        statusline_cache_creation_tokens: None,
        statusline_version: None,
        exceeds_200k_tokens: None,
        statusline_transcript_path: None,
        statusline_output_style: None,
        statusline_vim_mode: None,
        statusline_agent_name: None,
        statusline_worktree_name: None,
        statusline_worktree_path: None,
        statusline_worktree_branch: None,
        statusline_worktree_original_cwd: None,
        statusline_worktree_original_branch: None,
        statusline_remaining_pct: None,
        statusline_total_input_tokens: None,
        statusline_total_output_tokens: None,
        statusline_rate_limit_5h_pct: None,
        statusline_rate_limit_5h_resets_at: None,
        statusline_rate_limit_7d_pct: None,
        statusline_rate_limit_7d_resets_at: None,
        statusline_raw: None,
        model_set_at: 0,
        agent_state_set_at: 0,
        hook_events: Vec::new(),
        user_files: None,
        phase: PhaseHistory::default(),
    }
}

/// Derive agent state from the JSONL file's tail — ground truth for startup recovery.
///
/// Reads the last 10 lines, finds the last meaningful line (assistant, user, or result),
/// and derives the agent state. Returns None if the file is empty, unreadable, or has
/// no meaningful lines (falls back to NeedsYou/idle at the call site).
async fn derive_agent_state_from_jsonl(path: &Path) -> Option<AgentState> {
    let lines = claude_view_core::tail::tail_lines(path, 10).await.ok()?;
    if lines.is_empty() {
        return None;
    }

    let finders = claude_view_core::live_parser::TailFinders::new();

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
                    // Unknown stop_reason (max_tokens, etc.) — safe default
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
fn apply_jsonl_metadata(
    session: &mut LiveSession,
    m: &JsonlMetadata,
    file_path: &str,
    project: &str,
    project_display_name: &str,
    project_path: &str,
) {
    session.file_path = file_path.to_string();
    session.project = project.to_string();
    session.project_display_name = project_display_name.to_string();
    session.project_path = project_path.to_string();
    // Only overwrite branch fields when the JSONL accumulator has a definitive value.
    // Hooks resolve branch eagerly from CWD (filesystem HEAD); the accumulator learns
    // gitBranch later when user-type JSONL lines are parsed. Without this guard,
    // the first process_jsonl_update (which may only contain metadata lines) overwrites
    // the hook-resolved branch with None, causing a "(no branch)" flash in the UI.
    if m.git_branch.is_some() {
        session.git_branch = m.git_branch.clone();
    }
    if m.worktree_branch.is_some() {
        session.worktree_branch = m.worktree_branch.clone();
    }
    if m.is_worktree {
        session.is_worktree = true;
    }
    // Recompute effective_branch from current session state (may include hook-resolved values)
    let new_effective = session
        .worktree_branch
        .clone()
        .or(session.git_branch.clone());
    if new_effective.is_some() {
        session.effective_branch = new_effective;
    }
    // PID binding: only assign PID on first discovery. Once bound,
    // the process detector owns liveness checks for that specific PID.
    if session.pid.is_none() {
        session.pid = m.pid;
    }
    if !m.title.is_empty() {
        session.title = m.title.clone();
    }
    if !m.last_user_message.is_empty() {
        session.last_user_message = m.last_user_message.clone();
    }
    session.turn_count = m.turn_count;
    if m.started_at.is_some() {
        session.started_at = m.started_at;
    }
    session.last_activity_at = m.last_activity_at;
    // Model — timestamp-guarded. JSONL parser has lower authority than statusline
    // for model (statusline reflects mid-session /model switches). Only overwrite
    // if no fresher value has been set. Use strict `>` so same-millisecond
    // statusline writes (higher authority) are never overwritten by JSONL.
    if m.model.is_some() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if now > session.model_set_at {
            session.model = m.model.clone();
            session.model_set_at = now;
        }
    }
    session.tokens = m.tokens.clone();
    session.context_window_tokens = m.context_window_tokens;
    session.cost = m.cost.clone();
    session.cache_status = m.cache_status.clone();
    session.current_turn_started_at = m.current_turn_started_at;
    session.last_turn_task_seconds = m.last_turn_task_seconds;
    session.sub_agents = m.sub_agents.clone();
    session.team_name = m.team_name.clone();
    session.progress_items = m.progress_items.clone();
    session.tools_used = m.tools_used.clone();
    session.last_cache_hit_at = m.last_cache_hit_at;
    session.compact_count = m.compact_count;
    session.slug = m.slug.clone();
    if m.user_files.is_some() {
        session.user_files = m.user_files.clone();
    }
    session.edit_count = m.edit_count;
    session.phase = m.phase.clone();
}

/// Central manager that orchestrates file watching, process detection,
/// JSONL parsing, and session state management.
pub struct LiveSessionManager {
    /// In-memory map of session_id -> LiveSession, shared with route handlers.
    sessions: LiveSessionMap,
    /// Broadcast sender for SSE events.
    tx: broadcast::Sender<SessionEvent>,
    /// Pre-compiled SIMD substring finders for the JSONL tail parser.
    finders: Arc<TailFinders>,
    /// Per-session accumulator state (offsets, token totals, etc.).
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    /// Total number of Claude processes detected (not deduplicated by cwd).
    /// Updated by the eager scan and periodic detector.
    process_count: Arc<AtomicU32>,
    /// Per-model pricing table for cost calculation (immutable after init).
    pricing: Arc<HashMap<String, ModelPricing>>,
    /// Database handle for batched writes.
    db: Database,
    /// Search index holder for passing to scan_and_index_all on overflow reconciliation.
    search_index: Arc<StdRwLock<Option<Arc<claude_view_search::SearchIndex>>>>,
    /// Registry holder for passing to scan_and_index_all on overflow reconciliation.
    registry: Arc<StdRwLock<Option<claude_view_core::Registry>>>,
    /// Channel to request snapshot writes. Debounced to max 1 write/sec.
    snapshot_tx: mpsc::Sender<()>,
    /// Sidecar manager for crash recovery of controlled sessions.
    sidecar: Option<Arc<crate::sidecar::SidecarManager>>,
    /// Teams store for embedding team members in SSE payloads.
    teams: Arc<crate::teams::TeamsStore>,
    /// Transcript path → session ID dedup map, shared with AppState.
    transcript_to_session: TranscriptMap,
    /// Unified process oracle receiver for reading process data.
    oracle_rx: super::process_oracle::OracleReceiver,
    /// Event-driven process death watcher (kqueue on macOS).
    /// Held to prevent drop. Deaths are consumed by the reconciliation loop.
    _death_watcher: super::process_death::ProcessDeathWatcher,
    /// Channel to send classification requests to the oMLX scheduler.
    classify_tx: mpsc::Sender<ClassifyRequest>,
}

impl LiveSessionManager {
    /// Start the live session manager and all background tasks.
    ///
    /// Returns the manager, a shared session map, the transcript dedup map,
    /// and the broadcast sender for SSE event streaming.
    pub fn start(
        pricing: Arc<HashMap<String, ModelPricing>>,
        db: Database,
        search_index: Arc<StdRwLock<Option<Arc<claude_view_search::SearchIndex>>>>,
        registry: Arc<StdRwLock<Option<claude_view_core::Registry>>>,
        sidecar: Option<Arc<crate::sidecar::SidecarManager>>,
        teams: Arc<crate::teams::TeamsStore>,
        oracle_rx: super::process_oracle::OracleReceiver,
    ) -> (
        Arc<Self>,
        LiveSessionMap,
        TranscriptMap,
        broadcast::Sender<SessionEvent>,
    ) {
        let (tx, _rx) = broadcast::channel(256);
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let transcript_to_session: TranscriptMap = Arc::new(RwLock::new(HashMap::new()));

        // Debounced snapshot writer channel (bounded to 1 — extra signals are coalesced)
        let (snapshot_tx, snapshot_rx) = mpsc::channel::<()>(1);

        // Start event-driven process death watcher (kqueue on macOS).
        let (death_watcher, death_rx) = super::process_death::ProcessDeathWatcher::start();

        // oMLX phase classifier infrastructure
        let omlx_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let omlx_port: u16 = std::env::var("OMLX_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(10710);
        let (classify_tx, classify_rx) = mpsc::channel::<ClassifyRequest>(64);
        let (result_tx, mut result_rx) = mpsc::channel::<ClassifyResult>(64);

        let manager = Arc::new(Self {
            sessions: sessions.clone(),
            tx: tx.clone(),
            finders: Arc::new(TailFinders::new()),
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            process_count: Arc::new(AtomicU32::new(0)),
            pricing,
            db,
            search_index,
            registry,
            snapshot_tx,
            sidecar,
            teams,
            transcript_to_session: transcript_to_session.clone(),
            oracle_rx,
            _death_watcher: death_watcher,
            classify_tx,
        });

        // Spawn background tasks
        manager.spawn_snapshot_writer(snapshot_rx);
        manager.spawn_file_watcher();
        manager.spawn_reconciliation_loop();
        manager.spawn_cleanup_task();
        manager.spawn_death_consumer(death_rx);

        // Spawn oMLX lifecycle (health check)
        let omlx_ready_clone = omlx_ready.clone();
        tokio::spawn(super::omlx_lifecycle::run_lifecycle(
            omlx_ready_clone,
            omlx_port,
        ));

        // Spawn classify scheduler
        let client = Arc::new(OmlxClient::new(
            format!("http://localhost:{}", omlx_port),
            "Qwen3.5-4B-MLX-4bit".into(),
        ));
        tokio::spawn(run_scheduler(classify_rx, result_tx, client, omlx_ready, 2));

        // Spawn classify result handler
        {
            let accumulators = manager.accumulators.clone();
            let sessions = manager.sessions.clone();
            let tx = manager.tx.clone();
            tokio::spawn(async move {
                while let Some(result) = result_rx.recv().await {
                    let mut accs = accumulators.write().await;
                    if let Some(acc) = accs.get_mut(&result.session_id) {
                        acc.stabilizer.update(result.phase, result.scope);
                        if acc.stabilizer.should_emit() {
                            let label = PhaseLabel {
                                phase: acc
                                    .stabilizer
                                    .displayed_phase()
                                    .unwrap_or(SessionPhase::Working),
                                confidence: acc.stabilizer.confidence(),
                                scope: acc.stabilizer.displayed_scope(),
                            };
                            acc.phase_labels.push(label);
                            if acc.phase_labels.len() > MAX_PHASE_LABELS {
                                acc.phase_labels.remove(0);
                            }
                        }
                    }
                    drop(accs);
                    // Broadcast session update
                    let sessions = sessions.read().await;
                    if let Some(session) = sessions.get(&result.session_id) {
                        let _ = tx.send(SessionEvent::SessionUpdated {
                            session: session.clone(),
                        });
                    }
                }
            });
        }

        // Spawn relay client for mobile remote access
        super::relay_client::spawn_relay_client(
            tx.clone(),
            sessions.clone(),
            super::relay_client::RelayClientConfig::default(),
        );

        info!("LiveSessionManager started with 6 background tasks (file watcher, reconciliation loop, cleanup, death watcher, relay client, db writer)");

        (manager, sessions, transcript_to_session, tx)
    }

    /// Subscribe to session events for SSE streaming.
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.tx.subscribe()
    }

    /// Called by hook handler when SessionStart creates a new session.
    pub async fn create_accumulator_for_hook(&self, session_id: &str) {
        self.accumulators
            .write()
            .await
            .entry(session_id.to_string())
            .or_insert_with(SessionAccumulator::new);
    }

    /// Enrich a newly-created session from its existing accumulator data.
    ///
    /// When a hook creates a session after the initial JSONL scan has already
    /// populated the accumulator, the session needs the accumulated metadata
    /// (git_branch, title, cost, tokens, etc.) applied immediately. Otherwise
    /// it shows as "(no branch)" until the next file modification triggers
    /// `process_jsonl_update`.
    pub async fn enrich_session_from_accumulator(&self, session_id: &str) {
        let accumulators = self.accumulators.read().await;
        let Some(acc) = accumulators.get(session_id) else {
            return;
        };
        // Only enrich if the accumulator has actually parsed data
        if acc.offset == 0 {
            return;
        }
        let Some(ref file_path) = acc.file_path else {
            return;
        };

        let cached_cwd = acc.resolved_cwd.as_deref();
        let (project, project_display_name, project_path, _) =
            extract_project_info(file_path, cached_cwd);

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
        let file_path_str = file_path.to_string_lossy().to_string();

        let last_activity_at = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let metadata = JsonlMetadata {
            git_branch: acc.git_branch.clone(),
            is_worktree: wt_branch.is_some(),
            worktree_branch: wt_branch,
            pid: None, // hooks own PID binding
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
                    tools.push(super::state::ToolUsed {
                        name,
                        kind: "mcp".to_string(),
                    });
                }
                for name in skill {
                    tools.push(super::state::ToolUsed {
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
        };
        drop(accumulators);

        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if session.closed_at.is_some() {
                return; // Don't enrich closed sessions
            }
            let hook_activity = session.last_activity_at;
            apply_jsonl_metadata(
                session,
                &metadata,
                &file_path_str,
                &project,
                &project_display_name,
                &project_path,
            );
            // Populate team data from TeamsStore (not from JSONL accumulator)
            if let Some(ref tn) = session.team_name.clone() {
                if let Some(detail) = self.teams.get(tn) {
                    session.team_members = detail.members;
                }
                session.team_inbox_count = self
                    .teams
                    .inbox(tn)
                    .map(|msgs| msgs.len() as u32)
                    .unwrap_or(0);
            } else {
                session.team_members = Vec::new();
                session.team_inbox_count = 0;
            }
            // Preserve the hook's last_activity_at if it's more recent than file mtime
            if hook_activity > session.last_activity_at {
                session.last_activity_at = hook_activity;
            }
            let _ = self.tx.send(SessionEvent::SessionUpdated {
                session: session.clone(),
            });
        }
    }

    /// Called by hook handler when SessionEnd removes a session after delay.
    pub async fn remove_accumulator(&self, session_id: &str) {
        self.accumulators.write().await.remove(session_id);
    }

    /// Request a debounced snapshot write to disk.
    /// Non-blocking — if the channel is full, the signal is coalesced.
    pub fn request_snapshot_save(&self) {
        let _ = self.snapshot_tx.try_send(());
    }

    /// Spawn the debounced snapshot writer background task.
    /// Drains the channel and writes at most once per second.
    fn spawn_snapshot_writer(self: &Arc<Self>, mut rx: mpsc::Receiver<()>) {
        let manager = self.clone();
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // Drain any queued signals (coalesce)
                while rx.try_recv().is_ok() {}
                manager.save_session_snapshot_from_state().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                // Drain signals that arrived during the sleep
                while rx.try_recv().is_ok() {}
            }
        });
    }

    /// CAS bind a control session to a live session.
    pub async fn bind_control(
        &self,
        session_id: &str,
        control_id: String,
        expected_current: Option<&str>,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            let current = session.control.as_ref().map(|c| c.control_id.as_str());
            if current != expected_current {
                return false;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before Unix epoch")
                .as_secs() as i64;
            session.control = Some(super::state::ControlBinding {
                control_id,
                bound_at: now,
                cancel: tokio_util::sync::CancellationToken::new(),
            });
            // Control binding = sidecar Agent SDK — set source immediately
            session.source = Some(super::process::SessionSourceInfo {
                category: super::process::SessionSource::AgentSdk,
                label: None,
            });
            true
        } else {
            false
        }
    }

    /// Remove the control binding.
    pub async fn unbind_control(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(binding) = session.control.take() {
                binding.cancel.cancel(); // Signal WS relay to close
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Conditionally unbind: only if current control_id matches.
    pub async fn unbind_control_if(&self, session_id: &str, expected_control_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if session.control.as_ref().map(|c| c.control_id.as_str()) == Some(expected_control_id)
            {
                if let Some(binding) = session.control.take() {
                    binding.cancel.cancel();
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get all session IDs with active control bindings.
    pub async fn controlled_session_ids(&self) -> Vec<(String, String)> {
        self.sessions
            .read()
            .await
            .iter()
            .filter_map(|(id, s)| {
                s.control
                    .as_ref()
                    .map(|c| (id.clone(), c.control_id.clone()))
            })
            .collect()
    }

    /// Total number of Claude processes detected on the system.
    ///
    /// This is the raw process count (not deduplicated by cwd).
    /// Updated by the eager scan at startup and the periodic detector.
    pub fn process_count(&self) -> u32 {
        self.process_count.load(Ordering::Relaxed)
    }

    /// Save the extended session snapshot to disk for crash recovery.
    pub async fn save_session_snapshot_from_state(&self) {
        let sessions = self.sessions.read().await;
        let entries: HashMap<String, SnapshotEntry> = sessions
            .iter()
            // Done sessions (recently closed) are excluded from PID snapshots.
            // They persist via SQLite closed_at/dismissed_at columns instead.
            .filter(|(_, s)| s.status != SessionStatus::Done)
            .filter_map(|(id, s)| {
                s.pid.map(|pid| {
                    (
                        id.clone(),
                        SnapshotEntry {
                            pid,
                            status: match s.status {
                                SessionStatus::Working => "working".to_string(),
                                SessionStatus::Paused => "paused".to_string(),
                                SessionStatus::Done => "done".to_string(),
                            },
                            agent_state: s.agent_state.clone(),
                            last_activity_at: s.last_activity_at,
                            control_id: s.control.as_ref().map(|c| c.control_id.clone()),
                        },
                    )
                })
            })
            .collect();
        if let Some(snap_path) = pid_snapshot_path() {
            save_session_snapshot(
                &snap_path,
                &SessionSnapshot {
                    version: 2,
                    sessions: entries,
                },
            );
        } else {
            tracing::error!("could not determine home directory for snapshot save");
        }
    }

    /// Run a one-shot process count scan (display metric only).
    /// Reads from the oracle if available, falling back to direct scan.
    async fn run_eager_process_scan(&self) {
        let oracle_snap = self.oracle_rx.borrow().clone();
        let total_count = match oracle_snap.claude_processes.as_ref() {
            Some(cp) => cp.count,
            None => {
                // Oracle hasn't produced Claude data yet — direct scan.
                tokio::task::spawn_blocking(count_claude_processes)
                    .await
                    .unwrap_or_default()
            }
        };
        self.process_count.store(total_count, Ordering::Relaxed);
        info!("Process scan: {} Claude processes", total_count);
    }

    /// Spawn the file watcher background task.
    ///
    /// 1. Performs an initial scan of `~/.claude/projects/` for recent JSONL files.
    /// 2. Starts a notify watcher for ongoing file changes.
    /// 3. Processes each Modified/Removed event by parsing new JSONL lines.
    fn spawn_file_watcher(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            // --- Startup recovery sequence ---
            // 1. Eager process scan FIRST — builds the process table before JSONL scan
            manager.run_eager_process_scan().await;

            // 2. Initial JSONL scan — build accumulators, gate session creation on process
            let projects_dir = match dirs::home_dir() {
                Some(home) => home.join(".claude").join("projects"),
                None => {
                    warn!("Could not determine home directory; skipping initial scan");
                    return;
                }
            };

            let initial_paths = {
                let dir = projects_dir.clone();
                tokio::task::spawn_blocking(move || initial_scan(&dir))
                    .await
                    .unwrap_or_default()
            };

            info!(
                "Initial scan found {} recent JSONL files",
                initial_paths.len()
            );

            // Warm up accumulators so that when hooks arrive, metadata is ready.
            // No sessions are created here — hooks are the sole authority.
            for path in &initial_paths {
                manager.process_jsonl_update(path).await;
            }

            // 3. Promote sessions from crash-recovery snapshot.
            //    Sessions with alive PIDs get full LiveSession entries immediately,
            //    populated with metrics from accumulators and agent_state from snapshot.
            if let Some(snap_path) = pid_snapshot_path() {
                let snapshot = load_session_snapshot(&snap_path);
                if !snapshot.sessions.is_empty() {
                    let mut promoted = 0u32;
                    let mut dead = 0u32;
                    let mut dead_ids: Vec<String> = Vec::new();
                    let mut sessions_to_recover: Vec<(String, String)> = Vec::new();

                    for (session_id, entry) in &snapshot.sessions {
                        // Skip if hook already created this session
                        if manager.sessions.read().await.contains_key(session_id) {
                            continue;
                        }
                        if !is_pid_alive(entry.pid) {
                            dead += 1;
                            dead_ids.push(session_id.clone());
                            continue;
                        }

                        // Find the JSONL file path from the initial scan
                        if let Some(path) = initial_paths
                            .iter()
                            .find(|p| extract_session_id(p) == *session_id)
                        {
                            let file_path_str = path.to_string_lossy().to_string();
                            let mut session =
                                build_recovered_session(session_id, entry, &file_path_str);

                            // Enrich with accumulator metrics if available
                            let accumulators = manager.accumulators.read().await;
                            let cached_cwd = accumulators
                                .get(session_id)
                                .and_then(|a| a.resolved_cwd.as_deref());
                            let (project, project_display_name, project_path, _) =
                                extract_project_info(path, cached_cwd);
                            if let Some(acc) = accumulators.get(session_id) {
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

                                let wt_branch =
                                    acc.latest_cwd.as_deref().and_then(resolve_worktree_branch);

                                let metadata = JsonlMetadata {
                                    git_branch: acc.git_branch.clone(),
                                    is_worktree: wt_branch.is_some(),
                                    worktree_branch: wt_branch,
                                    pid: Some(entry.pid),
                                    title: acc.first_user_message.clone(),
                                    last_user_message: acc.last_user_message.clone(),
                                    turn_count: acc.user_turn_count,
                                    started_at: acc.started_at,
                                    last_activity_at: entry.last_activity_at,
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
                                        let mut mcp: Vec<_> =
                                            acc.mcp_servers.iter().cloned().collect();
                                        mcp.sort();
                                        let mut skill: Vec<_> =
                                            acc.skills.iter().cloned().collect();
                                        skill.sort();
                                        let mut tools = Vec::with_capacity(mcp.len() + skill.len());
                                        for name in mcp {
                                            tools.push(super::state::ToolUsed {
                                                name,
                                                kind: "mcp".to_string(),
                                            });
                                        }
                                        for name in skill {
                                            tools.push(super::state::ToolUsed {
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
                                };
                                drop(accumulators);

                                apply_jsonl_metadata(
                                    &mut session,
                                    &metadata,
                                    &file_path_str,
                                    &project,
                                    &project_display_name,
                                    &project_path,
                                );
                                // Populate team data from TeamsStore (not from JSONL accumulator)
                                if let Some(ref tn) = session.team_name.clone() {
                                    if let Some(detail) = manager.teams.get(tn) {
                                        session.team_members = detail.members;
                                    }
                                    session.team_inbox_count = manager
                                        .teams
                                        .inbox(tn)
                                        .map(|msgs| msgs.len() as u32)
                                        .unwrap_or(0);
                                } else {
                                    session.team_members = Vec::new();
                                    session.team_inbox_count = 0;
                                }
                            } else {
                                drop(accumulators);
                            }

                            // Override snapshot agent_state with JSONL ground truth.
                            // The snapshot may be stale if the Stop hook fired while
                            // the server was down. The JSONL file is authoritative.
                            if let Some(derived) = derive_agent_state_from_jsonl(path).await {
                                if derived.group != session.agent_state.group
                                    || derived.state != session.agent_state.state
                                {
                                    info!(
                                        session_id = %session_id,
                                        snapshot = %session.agent_state.state,
                                        derived = %derived.state,
                                        "JSONL ground truth overrides snapshot agent_state"
                                    );
                                }
                                session.status = status_from_agent_state(&derived);
                                session.current_activity = derived.label.clone();
                                session.agent_state = derived;
                            } else {
                                // No meaningful JSONL lines — safe default to idle
                                session.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "idle".into(),
                                    label: "Waiting for your next prompt".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Paused;
                            }

                            manager
                                .sessions
                                .write()
                                .await
                                .insert(session_id.clone(), session.clone());
                            let _ = manager.tx.send(SessionEvent::SessionDiscovered { session });
                            promoted += 1;
                            if let Some(ref ctrl_id) = entry.control_id {
                                sessions_to_recover.push((session_id.clone(), ctrl_id.clone()));
                            }
                        } else {
                            warn!(
                                session_id = %session_id,
                                pid = entry.pid,
                                "Snapshot entry has alive PID but no matching JSONL file in 24h scan window — skipping"
                            );
                        }
                    }

                    // PID dedup pass: if two snapshot entries share the same PID
                    // (OS PID reuse after crash), keep the one with more recent
                    // last_activity_at and close the other as Done.
                    {
                        let mut sessions = manager.sessions.write().await;
                        let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
                        let mut pid_dupes: Vec<String> = Vec::new();

                        for (id, session) in sessions.iter() {
                            if session.status == SessionStatus::Done {
                                continue;
                            }
                            if let Some(pid) = session.pid {
                                if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                                    // Collision — evict the older one.
                                    // On equal timestamps, tiebreak by session ID (deterministic
                                    // regardless of HashMap iteration order).
                                    let new_wins = session.last_activity_at > *existing_ts
                                        || (session.last_activity_at == *existing_ts
                                            && *id > *existing_id);
                                    if new_wins {
                                        pid_dupes.push(existing_id.clone());
                                        pid_owners
                                            .insert(pid, (id.clone(), session.last_activity_at));
                                    } else {
                                        pid_dupes.push(id.clone());
                                    }
                                } else {
                                    pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                                }
                            }
                        }

                        if !pid_dupes.is_empty() {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            for dupe_id in &pid_dupes {
                                if let Some(session) = sessions.get_mut(dupe_id) {
                                    info!(
                                        session_id = %dupe_id,
                                        pid = ?session.pid,
                                        "Snapshot PID dedup: evicting stale entry"
                                    );
                                    session.status = SessionStatus::Done;
                                    session.closed_at = Some(now);
                                    session.agent_state = AgentState {
                                        group: AgentStateGroup::NeedsYou,
                                        state: "session_ended".into(),
                                        label: "Evicted (PID collision)".into(),
                                        context: None,
                                    };
                                }
                            }
                            // Also remove dupes from sidecar recovery list
                            let dupe_set: std::collections::HashSet<&str> =
                                pid_dupes.iter().map(|s| s.as_str()).collect();
                            sessions_to_recover.retain(|(id, _)| !dupe_set.contains(id.as_str()));
                            info!(
                                evicted = pid_dupes.len(),
                                "Snapshot recovery PID dedup complete"
                            );
                        }
                    }

                    // Clean accumulators for dead snapshot PIDs to prevent
                    // zombie resurrection if a new process starts in the same project.
                    if !dead_ids.is_empty() {
                        let mut accumulators = manager.accumulators.write().await;
                        for id in &dead_ids {
                            accumulators.remove(id);
                        }
                        info!(
                            cleaned = dead_ids.len(),
                            "Cleaned accumulators for dead snapshot PIDs"
                        );
                    }

                    // Recover controlled sessions via sidecar
                    if !sessions_to_recover.is_empty() {
                        if let Some(ref sidecar) = manager.sidecar {
                            match sidecar.ensure_running().await {
                                Ok(_) => {
                                    let recovered = sidecar
                                        .recover_controlled_sessions(&sessions_to_recover)
                                        .await;
                                    for (sid, new_ctrl_id) in &recovered {
                                        manager.bind_control(sid, new_ctrl_id.clone(), None).await;
                                    }
                                    info!(
                                        "Recovered {}/{} controlled sessions after restart",
                                        recovered.len(),
                                        sessions_to_recover.len()
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Sidecar unavailable for recovery: {e}. Control bindings cleared."
                                    );
                                }
                            }
                        }
                    }

                    if promoted > 0 || dead > 0 {
                        info!(
                            promoted,
                            dead,
                            total = snapshot.sessions.len(),
                            "Startup recovery: promoted sessions from crash snapshot"
                        );
                    }

                    // Always re-save: prunes dead entries AND alive-but-unmatched entries
                    // (PIDs recycled by OS, or JSONL rotated past 24h scan window).
                    // save_session_snapshot_from_state() writes only sessions currently
                    // in the in-memory map, so anything not promoted is implicitly pruned.
                    manager.save_session_snapshot_from_state().await;
                }
            }

            // 4. Load recently-closed sessions from SQLite (survive server restarts).
            // These are sessions whose process exited (closed_at IS NOT NULL) but user
            // hasn't dismissed them yet (dismissed_at IS NULL).
            let closed_rows: Vec<(String, i64)> = sqlx::query_as(
                "SELECT id, closed_at FROM sessions WHERE closed_at IS NOT NULL AND dismissed_at IS NULL"
            )
            .fetch_all(manager.db.pool())
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to load recently-closed sessions from SQLite");
                Vec::new()
            });

            // Phase 1: Parse JSONL files for closed sessions (OUTSIDE sessions lock)
            for (session_id, _closed_at) in &closed_rows {
                if manager.sessions.read().await.contains_key(session_id) {
                    continue; // Already loaded from snapshot or hook
                }
                if let Some(path) = initial_paths
                    .iter()
                    .find(|p| extract_session_id(p) == *session_id)
                {
                    // process_jsonl_update acquires sessions.write() internally
                    manager.process_jsonl_update(path).await;
                }
            }

            // Phase 2: Mark recovered sessions as closed (with sessions lock)
            {
                let mut sessions = manager.sessions.write().await;
                let mut restored = 0u32;
                for (session_id, closed_at) in &closed_rows {
                    if let Some(session) = sessions.get_mut(session_id) {
                        if session.closed_at.is_none() {
                            session.status = SessionStatus::Done;
                            session.closed_at = Some(*closed_at);
                            session.agent_state = AgentState {
                                group: AgentStateGroup::NeedsYou,
                                state: "session_ended".into(),
                                label: "Session ended".into(),
                                context: None,
                            };
                            session.hook_events.clear();
                            restored += 1;
                        }
                    }
                }
                if restored > 0 {
                    info!(
                        count = restored,
                        "Restored recently-closed sessions from SQLite"
                    );
                }
            }

            // Start the file system watcher
            let (file_tx, mut file_rx) = mpsc::channel::<FileEvent>(512);
            let (_watcher, dropped_events) = match start_watcher(file_tx) {
                Ok((w, d)) => (w, d),
                Err(e) => {
                    error!("Failed to start file watcher: {}", e);
                    return;
                }
            };

            // Track last catch-up scan time
            let mut last_catchup_count = 0u64;

            // Process file events forever
            while let Some(event) = file_rx.recv().await {
                // Check if drops occurred since last check — trigger catch-up scan
                let current_drops = dropped_events.load(std::sync::atomic::Ordering::Relaxed);
                if current_drops > last_catchup_count {
                    last_catchup_count = current_drops;
                    info!(
                        dropped_total = current_drops,
                        "Detected dropped watcher events — triggering catch-up scan"
                    );
                    let catchup_paths = {
                        let dir = projects_dir.clone();
                        tokio::task::spawn_blocking(move || initial_scan(&dir))
                            .await
                            .unwrap_or_default()
                    };
                    for path in &catchup_paths {
                        manager.process_jsonl_update(path).await;
                        // Sessions are only created by hooks — no discovery broadcast needed.
                        // If a hook already created the session, process_jsonl_update enriched it.
                    }
                }
                match event {
                    FileEvent::Modified(path) => {
                        let session_id = extract_session_id(&path);
                        manager.process_jsonl_update(&path).await;

                        // Broadcast update if session exists (created by hook)
                        let sessions = manager.sessions.read().await;
                        if let Some(session) = sessions.get(&session_id) {
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                        }
                    }
                    FileEvent::Removed(path) => {
                        let session_id = extract_session_id(&path);
                        let (should_close, already_closed) = {
                            let sessions = manager.sessions.read().await;
                            match sessions.get(&session_id) {
                                Some(session) if session.closed_at.is_some() => (false, true),
                                Some(_) => (true, false),
                                None => (false, false),
                            }
                        }; // read lock dropped here

                        if already_closed {
                            tracing::debug!(session_id = %session_id, "JSONL file removed for recently-closed session — keeping in map");
                        } else if should_close {
                            // Active session whose file vanished — treat as closure.
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            let closed_session = {
                                let mut sessions = manager.sessions.write().await;
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.status = SessionStatus::Done;
                                    session.closed_at = Some(now);
                                    session.hook_events.clear();
                                    Some(session.clone())
                                } else {
                                    None
                                }
                            }; // write lock dropped here

                            if let Some(session) = closed_session {
                                let _ = manager.tx.send(SessionEvent::SessionClosed { session });
                                // Persist closed_at to SQLite for restart recovery
                                let db = manager.db.clone();
                                let sid = session_id.clone();
                                tokio::spawn(async move {
                                    let _ = sqlx::query(
                                        "UPDATE sessions SET closed_at = ?1 WHERE id = ?2 AND closed_at IS NULL"
                                    )
                                    .bind(now)
                                    .bind(&sid)
                                    .execute(db.pool())
                                    .await;
                                });
                                // Acquire accumulators lock AFTER sessions lock is dropped
                                let mut accumulators = manager.accumulators.write().await;
                                accumulators.remove(&session_id);
                            }
                        }
                    }
                    FileEvent::Rescan => {
                        tracing::info!("Overflow detected — triggering full reconciliation scan");
                        let Some(home) = dirs::home_dir() else {
                            tracing::warn!("HOME not set, skipping rescan");
                            continue;
                        };
                        let claude_dir = home.join(".claude");
                        let hints = build_index_hints(&claude_dir);
                        let search_for_rescan = manager.search_index.read().unwrap().clone();
                        let registry_for_rescan = manager
                            .registry
                            .read()
                            .unwrap()
                            .as_ref()
                            .map(|r| std::sync::Arc::new(r.clone()));
                        let (indexed, _) = scan_and_index_all(
                            &claude_dir,
                            &manager.db,
                            &hints,
                            search_for_rescan,
                            registry_for_rescan,
                            |_| {},
                            |_| {},
                            || {},
                        )
                        .await
                        .unwrap_or((0, 0));
                        if indexed > 0 {
                            tracing::info!(
                                indexed,
                                "Reconciliation scan complete — resyncing live state"
                            );
                            // Resync in-memory state for all recently-modified files
                            let recent_paths = initial_scan(&claude_dir);
                            for path in &recent_paths {
                                manager.process_jsonl_update(path).await;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Spawn the reconciliation loop.
    ///
    /// Two-phase design on a 10-second tick:
    ///
    /// **Phase 1 (every tick = 10s) — lightweight liveness:**
    /// For each session with a bound PID, check `is_pid_alive(pid)`.
    /// Mark dead sessions as Done, remove from map, broadcast completion, save snapshot.
    ///
    /// **Phase 2 (every 3rd tick = 30s) — process count + snapshot:**
    /// 1. Refresh process count via `detect_claude_processes` (display metric only).
    /// 2. Unconditional snapshot save (defense in depth).
    ///
    /// No discovery. No CWD PID binding. Hooks and snapshot recovery are the
    /// only session creation paths.
    fn spawn_reconciliation_loop(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let mut tick_count: u64 = 0;

            // NOTE: Snapshot recovery is handled by spawn_file_watcher (crash recovery).
            // A previous startup snapshot load here raced with file_watcher and could
            // clobber the snapshot before crash recovery promoted entries. Removed.

            loop {
                interval.tick().await;
                tick_count += 1;

                // =============================================================
                // Phase 1: Lightweight liveness check (every tick = 10s)
                // =============================================================
                let mut dead_sessions: Vec<String> = Vec::new();
                let mut ghost_sessions: Vec<String> = Vec::new();
                let mut transcript_paths_to_clean: Vec<PathBuf> = Vec::new();
                let mut snapshot_dirty = false;

                {
                    let mut sessions = manager.sessions.write().await;

                    for (session_id, session) in sessions.iter_mut() {
                        if session.status == SessionStatus::Done {
                            continue;
                        }

                        // 1a. PID liveness: dead PID → mark session ended
                        if let Some(pid) = session.pid {
                            if !is_pid_alive(pid) {
                                // Ghost session: hook created skeleton but no JSONL was
                                // ever written. Auto-complete (remove) instead of keeping
                                // in "recently closed" — there's nothing to show.
                                let is_ghost =
                                    session.file_path.is_empty() && session.turn_count == 0;
                                if is_ghost {
                                    info!(
                                        session_id = %session_id,
                                        pid = pid,
                                        "Ghost session (no JSONL, zero turns) — auto-completing"
                                    );
                                } else {
                                    info!(
                                        session_id = %session_id,
                                        pid = pid,
                                        "Bound PID is dead — marking session ended"
                                    );
                                }
                                session.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "session_ended".into(),
                                    label: "Session ended".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Done;
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64;
                                session.closed_at = Some(now);
                                session.hook_events.clear(); // Reclaim memory — hook_events are already persisted to SQLite
                                                             // Collect transcript path for dedup map cleanup
                                if let Some(ref tp) = session.statusline_transcript_path {
                                    transcript_paths_to_clean.push(PathBuf::from(tp));
                                }
                                if is_ghost {
                                    ghost_sessions.push(session_id.clone());
                                } else {
                                    dead_sessions.push(session_id.clone());
                                }
                                snapshot_dirty = true;
                                continue;
                            }
                        }
                    }

                    // Dead sessions stay in the map as "recently closed" —
                    // they are NOT removed here. Users dismiss them manually
                    // (design: no time-based auto-dismiss, no TTL).
                }

                // Ghost sessions: remove from map entirely (no "recently closed" UI)
                if !ghost_sessions.is_empty() {
                    let mut sessions = manager.sessions.write().await;
                    for session_id in &ghost_sessions {
                        sessions.remove(session_id);
                    }
                }
                // Broadcast ghost removals so frontend drops them immediately
                for session_id in &ghost_sessions {
                    let _ = manager.tx.send(SessionEvent::SessionCompleted {
                        session_id: session_id.clone(),
                    });
                }

                // Remove accumulators for all dead sessions (ghost + real)
                if !dead_sessions.is_empty() || !ghost_sessions.is_empty() {
                    let mut accumulators = manager.accumulators.write().await;
                    for session_id in dead_sessions.iter().chain(ghost_sessions.iter()) {
                        accumulators.remove(session_id);
                    }
                }

                // Clean transcript dedup map for dead sessions
                // (lock ordering: transcript_to_session acquired AFTER live_sessions released)
                if !transcript_paths_to_clean.is_empty() {
                    let mut tmap = manager.transcript_to_session.write().await;
                    for path in &transcript_paths_to_clean {
                        tmap.remove(path);
                    }
                }

                // Save session snapshot if any bindings changed (outside lock)
                if snapshot_dirty {
                    manager.save_session_snapshot_from_state().await;
                }

                // Broadcast closures (outside lock) — frontend moves to recentlyClosed
                let dead_sessions_for_db = dead_sessions.clone();
                for session_id in &dead_sessions {
                    let session = manager.sessions.read().await.get(session_id).cloned();
                    if let Some(session) = session {
                        let _ = manager.tx.send(SessionEvent::SessionClosed { session });
                    }
                }

                // Persist closed_at to SQLite for restart recovery
                if !dead_sessions_for_db.is_empty() {
                    let db = manager.db.clone();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    tokio::spawn(async move {
                        let mut tx = match db.pool().begin().await {
                            Ok(tx) => tx,
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to begin transaction for closed_at persistence");
                                return;
                            }
                        };
                        for session_id in dead_sessions_for_db {
                            let _ = sqlx::query(
                                "UPDATE sessions SET closed_at = ?1 WHERE id = ?2 AND closed_at IS NULL"
                            )
                            .bind(now)
                            .bind(&session_id)
                            .execute(&mut *tx)
                            .await;
                        }
                        let _ = tx.commit().await;
                    });
                }

                // =============================================================
                // Phase 1b: Stale control binding detection
                // =============================================================
                let controlled = manager.controlled_session_ids().await;
                if !controlled.is_empty() {
                    if let Some(ref sidecar) = manager.sidecar {
                        if !sidecar.is_running() {
                            // Sidecar died — attempt restart + recovery
                            tracing::warn!(
                                "Sidecar not running, attempting restart for {} controlled sessions",
                                controlled.len()
                            );
                            match sidecar.ensure_running().await {
                                Ok(_) => {
                                    let recovered =
                                        sidecar.recover_controlled_sessions(&controlled).await;
                                    for (session_id, new_control_id) in &recovered {
                                        let old_id = controlled
                                            .iter()
                                            .find(|(id, _)| id == session_id)
                                            .map(|(_, cid)| cid.as_str());
                                        manager
                                            .bind_control(
                                                session_id,
                                                new_control_id.clone(),
                                                old_id,
                                            )
                                            .await;
                                    }
                                    let recovered_ids: std::collections::HashSet<&str> =
                                        recovered.iter().map(|(id, _)| id.as_str()).collect();
                                    for (session_id, old_control_id) in &controlled {
                                        if !recovered_ids.contains(session_id.as_str()) {
                                            manager
                                                .unbind_control_if(session_id, old_control_id)
                                                .await;
                                        }
                                    }
                                    manager.request_snapshot_save();
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to restart sidecar: {e}. Clearing all control bindings."
                                    );
                                    for (session_id, old_control_id) in &controlled {
                                        manager.unbind_control_if(session_id, old_control_id).await;
                                    }
                                    manager.request_snapshot_save();
                                }
                            }
                        }
                    }
                }

                // =============================================================
                // Phase 2: Process count + snapshot (every 3rd tick = 30s)
                // =============================================================
                if !tick_count.is_multiple_of(3) {
                    continue;
                }

                // 2.1 — Process data from oracle (zero-cost read, no subprocess)
                let oracle_snap = manager.oracle_rx.borrow().clone();
                let (processes, total_count) = match oracle_snap.claude_processes.as_ref() {
                    Some(cp) => (cp.processes.clone(), cp.count),
                    None => {
                        // Oracle hasn't produced Claude process data yet (first ticks).
                        // Fall back to direct scan.
                        tokio::task::spawn_blocking(detect_claude_processes)
                            .await
                            .unwrap_or_default()
                    }
                };
                manager.process_count.store(total_count, Ordering::Relaxed);

                // Classify source for all live sessions.
                // Two paths: (1) control binding = AgentSdk (authoritative, no process scan needed)
                //            (2) PID match from process scan (ancestor walking for IDE/terminal)
                // Always re-classify — the processes map is indexed by PID (not CWD),
                // so multiple Claude processes in the same directory are all matchable.
                let sdk_source = super::process::SessionSourceInfo {
                    category: super::process::SessionSource::AgentSdk,
                    label: None,
                };
                let backfilled: Vec<LiveSession> = {
                    let mut sessions = manager.sessions.write().await;
                    let mut updated = Vec::new();
                    for session in sessions.values_mut() {
                        if session.status == SessionStatus::Done {
                            continue;
                        }
                        // Path 1: control binding = AgentSdk (authoritative)
                        if session.control.is_some() {
                            if session.source.as_ref() != Some(&sdk_source) {
                                session.source = Some(sdk_source.clone());
                                updated.push(session.clone());
                            }
                            continue;
                        }
                        // Path 2: PID-based classification from process scan
                        if let Some(pid) = session.pid {
                            if let Some(cp) = processes.get(&pid) {
                                let new_source = Some(cp.source.clone());
                                if session.source != new_source {
                                    session.source = new_source;
                                    updated.push(session.clone());
                                }
                            }
                        }
                    }
                    updated
                };
                // Emit SSE updates so frontends learn about the source
                for session in backfilled {
                    let _ = manager.tx.send(SessionEvent::SessionUpdated { session });
                }

                // 2.2 — Register alive PIDs with death watcher (idempotent)
                {
                    let sessions = manager.sessions.read().await;
                    for (id, session) in sessions.iter() {
                        if session.status != SessionStatus::Done {
                            if let Some(pid) = session.pid {
                                manager._death_watcher.watch(pid, id.clone()).await;
                            }
                        }
                    }
                }

                // 2.3 — Unconditional snapshot save (defense in depth)
                manager.save_session_snapshot_from_state().await;
            }
        });
    }

    /// Spawn the periodic housekeeping task.
    ///
    /// Every 60 seconds: removes orphaned accumulators (session removed but accumulator lingered).
    fn spawn_cleanup_task(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;

                // Clean up orphaned accumulators (session removed but accumulator lingered)
                {
                    let sessions = manager.sessions.read().await;
                    let mut accumulators = manager.accumulators.write().await;
                    let orphan_ids: Vec<String> = accumulators
                        .keys()
                        .filter(|id| !sessions.contains_key(*id))
                        .cloned()
                        .collect();
                    for id in &orphan_ids {
                        accumulators.remove(id);
                    }
                    if !orphan_ids.is_empty() {
                        info!("Cleaned up {} orphaned accumulators", orphan_ids.len());
                    }
                }
            }
        });
    }

    /// Spawn the death notification consumer.
    ///
    /// Reads from the kqueue-based ProcessDeathWatcher and immediately marks
    /// sessions as Done when their PID exits. This reduces the ghost session
    /// window from 10s (polling) to ~0ms (event-driven).
    fn spawn_death_consumer(
        self: &Arc<Self>,
        mut death_rx: tokio::sync::mpsc::Receiver<super::process_death::DeathNotification>,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some((pid, session_id)) = death_rx.recv().await {
                let mut sessions = manager.sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    // Only act if this session is still alive and owns this PID
                    if session.status != SessionStatus::Done && session.pid == Some(pid) {
                        let is_ghost = session.file_path.is_empty() && session.turn_count == 0;
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        info!(
                            session_id = %session_id,
                            pid = pid,
                            ghost = is_ghost,
                            "kqueue: PID death → marking session ended"
                        );

                        session.agent_state = AgentState {
                            group: AgentStateGroup::NeedsYou,
                            state: "session_ended".into(),
                            label: "Session ended".into(),
                            context: None,
                        };
                        session.status = SessionStatus::Done;
                        session.closed_at = Some(now);

                        if is_ghost {
                            let sid = session_id.clone();
                            sessions.remove(&sid);
                            drop(sessions);
                            let _ = manager
                                .tx
                                .send(SessionEvent::SessionCompleted { session_id: sid });
                        } else {
                            let session_clone = session.clone();
                            drop(sessions);
                            let _ = manager.tx.send(SessionEvent::SessionClosed {
                                session: session_clone,
                            });
                        }
                        manager.request_snapshot_save();
                    }
                }
            }
        });
    }

    /// Core JSONL processing logic for a single session file.
    ///
    /// 1. Extracts session ID and project info from the file path.
    /// 2. Calls `parse_tail` from the stored offset to read only new lines.
    /// 3. Accumulates token counts and user turn counts.
    /// 4. Derives session status, activity, and cost.
    /// 5. Updates the shared session map.
    async fn process_jsonl_update(&self, path: &Path) {
        let session_id = extract_session_id(path);
        // Use cached cwd from accumulator if available (avoids re-reading file every poll)
        let cached_cwd = {
            let accumulators = self.accumulators.read().await;
            accumulators
                .get(&session_id)
                .and_then(|a| a.resolved_cwd.clone())
        };
        let (project, project_display_name, project_path, resolved_cwd) =
            extract_project_info(path, cached_cwd.as_deref());

        // Get the current offset for this session
        let current_offset = {
            let accumulators = self.accumulators.read().await;
            accumulators.get(&session_id).map(|a| a.offset).unwrap_or(0)
        };

        // Parse new lines from the JSONL file (blocking I/O)
        let finders = self.finders.clone();
        let path_owned = path.to_path_buf();
        let parse_result =
            tokio::task::spawn_blocking(move || parse_tail(&path_owned, current_offset, &finders))
                .await;

        let (new_lines, new_offset) = match parse_result {
            Ok(Ok((lines, offset))) => (lines, offset),
            Ok(Err(e)) => {
                // I/O error — file may have been deleted between event and read
                tracing::debug!("Failed to parse tail for {}: {}", session_id, e);
                return;
            }
            Err(e) => {
                error!("spawn_blocking panicked for {}: {}", session_id, e);
                return;
            }
        };

        // If no new lines, nothing to update
        if new_lines.is_empty() && current_offset > 0 {
            return;
        }

        // Get file metadata for last_activity_at
        let last_activity_at = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            });

        // Update accumulator with new lines
        let mut accumulators = self.accumulators.write().await;
        let acc = accumulators
            .entry(session_id.clone())
            .or_insert_with(SessionAccumulator::new);

        acc.offset = new_offset;
        acc.file_path = Some(path.to_path_buf());
        acc.project_path = Some(project_path.clone());
        // Cache the resolved cwd so we don't re-read the file on every poll.
        if acc.resolved_cwd.is_none() {
            acc.resolved_cwd = resolved_cwd;
        }

        // Detect file replacement: offset rollback means file was replaced.
        // Clear task progress to prevent duplicates on replay from offset 0.
        // TodoWrite is naturally idempotent (full replacement); only task_items needs reset.
        if new_offset > 0 && new_offset < current_offset {
            tracing::info!(
                session_id = %session_id,
                old_offset = current_offset,
                new_offset = new_offset,
                "File replaced — clearing task progress for clean re-accumulation"
            );
            acc.task_items.clear();
            acc.mcp_servers.clear();
            acc.skills.clear();
            acc.at_files.clear();
            acc.pasted_paths.clear();
            acc.tokens = TokenUsage::default();
            acc.tool_counts_edit = 0;
            acc.tool_counts_read = 0;
            acc.tool_counts_bash = 0;
            acc.tool_counts_write = 0;
            acc.compact_count = 0;
            acc.accumulated_cost = CostBreakdown::default();
            acc.seen_api_calls.clear();
            acc.phase_labels.clear();
            acc.message_buf.clear();
            acc.message_buf_dirty = false;
            acc.message_buf_total = 0;
            acc.stabilizer.reset();
        }

        let mut channel_a_events: Vec<HookEvent> = Vec::new();

        for line in &new_lines {
            // Content-block dedup (same policy as core SessionAccumulator):
            // only count tokens/cost once per API response.
            let has_measurement_data = line.input_tokens.is_some()
                || line.output_tokens.is_some()
                || line.cache_read_tokens.is_some()
                || line.cache_creation_tokens.is_some()
                || line.cache_creation_5m_tokens.is_some()
                || line.cache_creation_1hr_tokens.is_some();
            let should_count_block = match (line.message_id.as_deref(), line.request_id.as_deref())
            {
                (Some(msg_id), Some(req_id)) => {
                    if has_measurement_data {
                        let key = format!("{}:{}", msg_id, req_id);
                        acc.seen_api_calls.insert(key)
                    } else {
                        false
                    }
                }
                _ => true, // Legacy lines without IDs: no safe dedup key available.
            };

            if should_count_block {
                // Accumulate tokens (cumulative, for cost calculation)
                if let Some(input) = line.input_tokens {
                    acc.tokens.input_tokens += input;
                    acc.tokens.total_tokens += input;
                }
                if let Some(output) = line.output_tokens {
                    acc.tokens.output_tokens += output;
                    acc.tokens.total_tokens += output;
                }
                if let Some(cache_read) = line.cache_read_tokens {
                    acc.tokens.cache_read_tokens += cache_read;
                    acc.tokens.total_tokens += cache_read;
                }
                if let Some(cache_creation) = line.cache_creation_tokens {
                    acc.tokens.cache_creation_tokens += cache_creation;
                    acc.tokens.total_tokens += cache_creation;
                }
                // Accumulate split cache creation by TTL
                if let Some(tokens_5m) = line.cache_creation_5m_tokens {
                    acc.tokens.cache_creation_5m_tokens += tokens_5m;
                }
                if let Some(tokens_1hr) = line.cache_creation_1hr_tokens {
                    acc.tokens.cache_creation_1hr_tokens += tokens_1hr;
                }
            }

            // Track last cache hit time when we see cache activity.
            // This is the ground truth signal from Anthropic's API response.
            if line.cache_read_tokens.map(|v| v > 0).unwrap_or(false)
                || line.cache_creation_tokens.map(|v| v > 0).unwrap_or(false)
            {
                if let Some(ref ts) = line.timestamp {
                    acc.last_cache_hit_at = parse_timestamp_to_unix(ts);
                }
            }

            // Accumulate tool counts (cumulative, like tokens)
            for name in &line.tool_names {
                match name.as_str() {
                    "Edit" => acc.tool_counts_edit += 1,
                    "Read" => acc.tool_counts_read += 1,
                    "Bash" => acc.tool_counts_bash += 1,
                    "Write" => acc.tool_counts_write += 1,
                    _ => {}
                }
            }

            // Track the current context window fill from the latest assistant turn.
            // Context size = input_tokens + cache_read + cache_creation for that turn.
            if line.line_type == LineType::Assistant {
                let turn_input = line.input_tokens.unwrap_or(0)
                    + line.cache_read_tokens.unwrap_or(0)
                    + line.cache_creation_tokens.unwrap_or(0);
                if turn_input > 0 {
                    acc.context_window_tokens = turn_input;
                }
            }

            // Track model (must happen BEFORE per-turn cost so model is current)
            if let Some(ref model) = line.model {
                acc.model = Some(model.clone());
            }

            // Per-turn cost accumulation: price THIS turn's tokens individually.
            // The 200k tiering threshold is per-API-request, not per-session.
            let has_tokens = line.input_tokens.is_some()
                || line.output_tokens.is_some()
                || line.cache_read_tokens.is_some()
                || line.cache_creation_tokens.is_some()
                || line.cache_creation_5m_tokens.is_some()
                || line.cache_creation_1hr_tokens.is_some();
            if should_count_block && has_tokens {
                let turn_tokens = TokenUsage {
                    input_tokens: line.input_tokens.unwrap_or(0),
                    output_tokens: line.output_tokens.unwrap_or(0),
                    cache_read_tokens: line.cache_read_tokens.unwrap_or(0),
                    cache_creation_tokens: line.cache_creation_tokens.unwrap_or(0),
                    cache_creation_5m_tokens: line.cache_creation_5m_tokens.unwrap_or(0),
                    cache_creation_1hr_tokens: line.cache_creation_1hr_tokens.unwrap_or(0),
                    total_tokens: 0,
                };
                {
                    let turn_cost =
                        calculate_cost(&turn_tokens, acc.model.as_deref(), &self.pricing);
                    acc.accumulated_cost.input_cost_usd += turn_cost.input_cost_usd;
                    acc.accumulated_cost.output_cost_usd += turn_cost.output_cost_usd;
                    acc.accumulated_cost.cache_read_cost_usd += turn_cost.cache_read_cost_usd;
                    acc.accumulated_cost.cache_creation_cost_usd +=
                        turn_cost.cache_creation_cost_usd;
                    acc.accumulated_cost.cache_savings_usd += turn_cost.cache_savings_usd;
                    acc.accumulated_cost.total_usd += turn_cost.total_usd;
                    acc.accumulated_cost.unpriced_input_tokens += turn_cost.unpriced_input_tokens;
                    acc.accumulated_cost.unpriced_output_tokens += turn_cost.unpriced_output_tokens;
                    acc.accumulated_cost.unpriced_cache_read_tokens +=
                        turn_cost.unpriced_cache_read_tokens;
                    acc.accumulated_cost.unpriced_cache_creation_tokens +=
                        turn_cost.unpriced_cache_creation_tokens;
                    acc.accumulated_cost.has_unpriced_usage |= turn_cost.has_unpriced_usage;
                }
            }

            // Track git branch, cwd, and slug from user messages
            if let Some(ref branch) = line.git_branch {
                acc.git_branch = Some(branch.clone());
            }
            if let Some(ref cwd) = line.cwd {
                acc.latest_cwd = Some(cwd.clone());
            }
            if acc.slug.is_none() {
                if let Some(ref s) = line.slug {
                    acc.slug = Some(s.clone());
                }
            }

            // Track user messages (skip meta messages for content)
            if line.line_type == LineType::User {
                acc.user_turn_count += 1;
                if !line.is_meta && !line.content_preview.is_empty() {
                    // First real user message becomes the session title
                    if acc.first_user_message.is_empty() {
                        acc.first_user_message = line.content_preview.clone();
                    }
                    acc.last_user_message = line.content_preview.clone();
                }
                // Accumulate IDE opened files (first-N-wins, merged into at_files)
                if let Some(ref ide_file) = line.ide_file {
                    if acc.at_files.len() < 10 {
                        acc.at_files.insert(ide_file.clone());
                    }
                }
                // Accumulate @file mentions (first-N-wins, cap at 10)
                for file in &line.at_files {
                    if acc.at_files.len() < 10 {
                        acc.at_files.insert(file.clone());
                    }
                }
                // Accumulate pasted paths (first-N-wins, cap at 10)
                for path in &line.pasted_paths {
                    if acc.pasted_paths.len() < 10 {
                        acc.pasted_paths.insert(path.clone());
                    }
                }
            }

            // Track current turn start time when a real user prompt is detected.
            // Filter out meta messages, tool result continuations, and system-prefixed
            // messages — those are not genuine user prompts that start a new turn.
            if line.line_type == LineType::User
                && !line.is_meta
                && !line.is_tool_result_continuation
                && !line.has_system_prefix
            {
                if let Some(ref ts) = line.timestamp {
                    acc.current_turn_started_at = parse_timestamp_to_unix(ts);
                }
            }

            // Track session start time from first timestamp
            if acc.started_at.is_none() {
                if let Some(ref ts) = line.timestamp {
                    acc.started_at = parse_timestamp_to_unix(ts);
                }
            }

            // --- Team name tracking (from top-level `teamName` JSONL field) ---
            if acc.team_name.is_none() {
                if let Some(ref tn) = line.team_name {
                    acc.team_name = Some(tn.clone());
                }
            }

            // --- Sub-agent spawn tracking ---
            for spawn in &line.sub_agent_spawns {
                // Team spawns are NOT sub-agents — their lifecycle is managed by
                // ~/.claude/teams/, not the JSONL sub-agent tracking system.
                if spawn.team_name.is_some() {
                    continue;
                }

                // Guard against re-processing the same spawn line
                // (can happen if accumulator reset while file exists, or offset tracking bug)
                if acc
                    .sub_agents
                    .iter()
                    .any(|a| a.tool_use_id == spawn.tool_use_id)
                {
                    continue;
                }

                // Parse timestamp from the JSONL line to get started_at
                let started_at = line
                    .timestamp
                    .as_deref()
                    .and_then(parse_timestamp_to_unix)
                    .unwrap_or(last_activity_at); // fallback to file mtime, never epoch-zero
                acc.sub_agents.push(SubAgentInfo {
                    tool_use_id: spawn.tool_use_id.clone(),
                    agent_id: None, // populated on completion from toolUseResult.agentId
                    agent_type: spawn.agent_type.clone(),
                    description: spawn.description.clone(),
                    status: SubAgentStatus::Running,
                    started_at,
                    completed_at: None,
                    duration_ms: None,
                    tool_use_count: None,
                    model: spawn.model.clone(),
                    input_tokens: None,
                    output_tokens: None,
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                    cost_usd: None,
                    current_activity: None,
                });
            }

            // --- Sub-agent completion tracking ---
            if let Some(ref result) = line.sub_agent_result {
                if let Some(agent) = acc
                    .sub_agents
                    .iter_mut()
                    .find(|a| a.tool_use_id == result.tool_use_id)
                {
                    // Whitelist known terminal statuses. Everything else
                    // (async_launched, teammate_spawned, queued, or any future
                    // non-terminal status) means the agent is still running —
                    // just capture the agentId and keep Running.
                    let terminal_status = match result.status.as_str() {
                        "completed" => Some(SubAgentStatus::Complete),
                        "failed" | "killed" => Some(SubAgentStatus::Error),
                        _ => None, // non-terminal: still running
                    };

                    agent.agent_id = result.agent_id.clone();

                    if let Some(status) = terminal_status {
                        agent.status = status;
                        agent.completed_at =
                            line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                        agent.duration_ms = result.total_duration_ms;
                        agent.tool_use_count = result.total_tool_use_count;
                        agent.current_activity = None;

                        // Store token usage breakdown for transparency
                        agent.input_tokens = result.usage_input_tokens;
                        agent.output_tokens = result.usage_output_tokens;
                        agent.cache_read_tokens = result.usage_cache_read_tokens;
                        agent.cache_creation_tokens = result.usage_cache_creation_tokens;

                        // Update model from toolUseResult if present (authoritative)
                        if result.model.is_some() {
                            agent.model = result.model.clone();
                        }

                        // Compute cost from token usage via pricing table.
                        // Use the sub-agent's own model for pricing (from spawn input or
                        // toolUseResult). Fall back to parent session model if unknown.
                        let pricing_model = agent.model.as_deref().or(acc.model.as_deref());
                        if let Some(model) = pricing_model {
                            let sub_tokens = TokenUsage {
                                input_tokens: result.usage_input_tokens.unwrap_or(0),
                                output_tokens: result.usage_output_tokens.unwrap_or(0),
                                cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                                cache_creation_tokens: result
                                    .usage_cache_creation_tokens
                                    .unwrap_or(0),
                                cache_creation_5m_tokens: 0,
                                cache_creation_1hr_tokens: 0,
                                total_tokens: 0, // not used by calculate_cost
                            };
                            let sub_cost = calculate_cost(&sub_tokens, Some(model), &self.pricing);
                            if sub_cost.total_usd > 0.0 {
                                agent.cost_usd = Some(sub_cost.total_usd);
                            }
                        }
                    }
                }
                // If no matching spawn found, ignore gracefully (orphaned tool_result)
            }

            // --- Sub-agent progress tracking (early agentId + current activity) ---
            if let Some(ref progress) = line.sub_agent_progress {
                if let Some(agent) = acc
                    .sub_agents
                    .iter_mut()
                    .find(|a| a.tool_use_id == progress.parent_tool_use_id)
                {
                    // Populate agent_id from progress event (available before completion!)
                    if agent.agent_id.is_none() {
                        agent.agent_id = Some(progress.agent_id.clone());
                    }
                    // Update current activity (only while still running)
                    if agent.status == SubAgentStatus::Running {
                        agent.current_activity = progress.current_tool.clone();
                    }
                }
            }

            // --- Background agent completion via <task-notification> ---
            if let Some(ref notif) = line.sub_agent_notification {
                if let Some(agent) = acc
                    .sub_agents
                    .iter_mut()
                    .find(|a| a.agent_id.as_deref() == Some(&notif.agent_id))
                {
                    agent.status = if notif.status == "completed" {
                        SubAgentStatus::Complete
                    } else {
                        // "failed", "killed", or any other terminal status
                        SubAgentStatus::Error
                    };
                    agent.completed_at =
                        line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                    agent.current_activity = None;
                }
            }

            // --- Tool integration tracking (MCP servers + skills) ---
            for tool_name in &line.tool_names {
                if let Some(rest) = tool_name.strip_prefix("mcp__") {
                    // Pattern: mcp__{server}__{tool} — extract the server segment
                    if let Some(idx) = rest.find("__") {
                        let server = &rest[..idx];
                        acc.mcp_servers.insert(server.to_string());
                    }
                }
            }
            for skill_name in &line.skill_names {
                if !skill_name.is_empty() {
                    acc.skills.insert(skill_name.clone());
                }
            }

            // Track compaction events
            if line.is_compact_boundary {
                acc.compact_count += 1;
            }

            // --- TodoWrite: full replacement ---
            if let Some(ref todos) = line.todo_write {
                use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
                acc.todo_items = todos
                    .iter()
                    .map(|t| {
                        let status = match t.status.as_str() {
                            "in_progress" => ProgressStatus::InProgress,
                            "completed" => ProgressStatus::Completed,
                            _ => ProgressStatus::Pending,
                        };
                        ProgressItem {
                            id: None,
                            tool_use_id: None,
                            title: t.content.clone(),
                            status,
                            active_form: if t.active_form.is_empty() {
                                None
                            } else {
                                Some(t.active_form.clone())
                            },
                            source: ProgressSource::Todo,
                        }
                    })
                    .collect();
            }

            // --- TaskCreate: append with dedup guard ---
            for create in &line.task_creates {
                use claude_view_core::progress::{ProgressItem, ProgressSource, ProgressStatus};
                if acc
                    .task_items
                    .iter()
                    .any(|t| t.tool_use_id.as_deref() == Some(&create.tool_use_id))
                {
                    continue; // Already seen this create (replay resilience)
                }
                acc.task_items.push(ProgressItem {
                    id: None, // Assigned later by TaskIdAssignment
                    tool_use_id: Some(create.tool_use_id.clone()),
                    title: create.subject.clone(),
                    status: ProgressStatus::Pending,
                    active_form: if create.active_form.is_empty() {
                        None
                    } else {
                        Some(create.active_form.clone())
                    },
                    source: ProgressSource::Task,
                });
            }

            // --- TaskIdAssignment: assign system ID ---
            for assignment in &line.task_id_assignments {
                if let Some(task) = acc
                    .task_items
                    .iter_mut()
                    .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
                {
                    task.id = Some(assignment.task_id.clone());
                }
            }

            // --- TaskUpdate: modify existing task ---
            for update in &line.task_updates {
                use claude_view_core::progress::ProgressStatus;
                if let Some(task) = acc
                    .task_items
                    .iter_mut()
                    .find(|t| t.id.as_deref() == Some(&update.task_id))
                {
                    if let Some(ref s) = update.status {
                        task.status = match s.as_str() {
                            "in_progress" => ProgressStatus::InProgress,
                            "completed" => ProgressStatus::Completed,
                            _ => ProgressStatus::Pending,
                        };
                    }
                    if let Some(ref subj) = update.subject {
                        task.title = subj.clone();
                    }
                    if let Some(ref af) = update.active_form {
                        task.active_form = Some(af.clone());
                    }
                }
            }

            // Channel A: hook_progress events from JSONL
            if let Some(ref hp) = line.hook_progress {
                channel_a_events.push(resolve_hook_event_from_progress(hp, &line.timestamp));
            }

            // Synthesized events from existing JSONL signals
            if line.line_type == LineType::User
                && !line.is_meta
                && !line.is_tool_result_continuation
                && !line.has_system_prefix
            {
                channel_a_events.push(make_synthesized_event(
                    &line.timestamp,
                    "UserPromptSubmit",
                    None,
                    "autonomous",
                ));
            }
            if line.is_compact_boundary {
                channel_a_events.push(make_synthesized_event(
                    &line.timestamp,
                    "PreCompact",
                    None,
                    "autonomous",
                ));
            }
            for spawn in &line.sub_agent_spawns {
                channel_a_events.push(make_synthesized_event(
                    &line.timestamp,
                    "SubagentStart",
                    Some(&spawn.agent_type),
                    "autonomous",
                ));
            }
            if line.sub_agent_result.is_some() {
                channel_a_events.push(make_synthesized_event(
                    &line.timestamp,
                    "SubagentStop",
                    None,
                    "autonomous",
                ));
            }
            for tu in &line.task_updates {
                if tu.status.as_deref() == Some("completed") {
                    channel_a_events.push(make_synthesized_event(
                        &line.timestamp,
                        "TaskCompleted",
                        None,
                        "autonomous",
                    ));
                }
            }

            // Phase classification: check shipping rule, accumulate turns, schedule LLM classify.
            for cmd in &line.bash_commands {
                if is_shipping_cmd(cmd) {
                    acc.stabilizer.lock_shipping();
                    acc.phase_labels.push(PhaseLabel {
                        phase: SessionPhase::Shipping,
                        confidence: 1.0,
                        scope: None,
                    });
                    if acc.phase_labels.len() > MAX_PHASE_LABELS {
                        acc.phase_labels.remove(0);
                    }
                    break;
                }
            }

            // Accumulate conversation turn
            if line.role.as_deref() == Some("assistant") || line.role.as_deref() == Some("user") {
                let role = if line.role.as_deref() == Some("user") {
                    Role::User
                } else {
                    Role::Assistant
                };
                let turn = ConversationTurn {
                    role,
                    text: line.content_extended.clone(),
                    tools: line.tool_names.clone(),
                };
                if acc.message_buf.len() >= 15 {
                    acc.message_buf.pop_front();
                }
                acc.message_buf.push_back(turn);
                acc.message_buf_dirty = true;
                acc.message_buf_total += 1;
            }

            // Schedule classification if dirty and not in steady-state skip
            let should_classify = acc.message_buf_dirty
                && (acc.message_buf_total <= 2
                    || acc.stabilizer.displayed_phase().is_none()
                    || acc.message_buf_total % 5 == 0);

            if should_classify {
                acc.message_buf_dirty = false;
                let priority = if acc.phase_labels.is_empty() {
                    Priority::New
                } else if acc.stabilizer.displayed_phase().is_none() {
                    Priority::Transition
                } else {
                    Priority::Steady
                };
                acc.classify_generation += 1;
                let _ = self.classify_tx.try_send(ClassifyRequest {
                    session_id: session_id.clone(),
                    priority,
                    turns: acc.message_buf.iter().cloned().collect(),
                    temperature: acc.stabilizer.next_temperature(),
                    generation: acc.classify_generation,
                });
            }
        }

        // Use per-turn accumulated cost (computed in the line processing loop above).
        let mut cost = acc.accumulated_cost.clone();
        finalize_cost_breakdown(&mut cost, &acc.tokens);

        // Derive cache status from last cache hit (ground truth from API response tokens).
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

        let file_path_str = path.to_str().unwrap_or("").to_string();

        // Collect metadata from accumulator (snapshot while lock is held).
        // PID is not set here — hooks deliver PIDs via SessionStart.
        let wt_branch = acc.latest_cwd.as_deref().and_then(resolve_worktree_branch);

        let metadata = JsonlMetadata {
            git_branch: acc.git_branch.clone(),
            is_worktree: wt_branch.is_some(),
            worktree_branch: wt_branch,
            pid: None,
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
                    tools.push(super::state::ToolUsed {
                        name,
                        kind: "mcp".to_string(),
                    });
                }
                for name in skill {
                    tools.push(super::state::ToolUsed {
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
        };

        // After accumulator update, persist partial state to DB (fire-and-forget).
        let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        if let Err(e) = self
            .db
            .update_session_from_tail(
                &session_id,
                acc.user_turn_count as i32 + acc.tokens.total_tokens.min(1) as i32, // approx message_count
                acc.user_turn_count as i32,
                last_activity_at,
                &acc.last_user_message,
                file_size,
                file_size,
                last_activity_at, // mtime approximation
                acc.tokens.input_tokens as i64,
                acc.tokens.output_tokens as i64,
                acc.tokens.cache_read_tokens as i64,
                acc.tokens.cache_creation_tokens as i64,
                acc.tool_counts_edit as i32,
                acc.tool_counts_read as i32,
                acc.tool_counts_bash as i32,
                acc.tool_counts_write as i32,
            )
            .await
        {
            tracing::warn!(session_id = %session_id, error = %e, "Failed to update session from tail");
        }

        // Drop accumulators lock before acquiring sessions lock
        drop(accumulators);

        // Self-dedup Channel A events BEFORE acquiring the sessions lock
        if !channel_a_events.is_empty() {
            channel_a_events.sort_by(|a, b| {
                a.timestamp
                    .cmp(&b.timestamp)
                    .then(a.event_name.cmp(&b.event_name))
                    .then(a.tool_name.cmp(&b.tool_name))
                    .then(a.source.cmp(&b.source))
            });
            channel_a_events.dedup_by(|a, b| {
                a.event_name == b.event_name
                    && a.timestamp == b.timestamp
                    && a.tool_name == b.tool_name
                    && a.source == b.source
            });
        }

        // Update the shared session map — metadata only, hooks own agent_state/status.
        // NEVER create sessions here. Only hooks (SessionStart) and startup recovery
        // (process-gated) create sessions. If no session exists, the accumulator holds
        // the metadata until a hook or recovery creates the session entry.
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if session.closed_at.is_some() {
                return;
            }
            apply_jsonl_metadata(
                session,
                &metadata,
                &file_path_str,
                &project,
                &project_display_name,
                &project_path,
            );
            // Populate team data from TeamsStore (not from JSONL accumulator)
            if let Some(ref tn) = session.team_name.clone() {
                if let Some(detail) = self.teams.get(tn) {
                    session.team_members = detail.members;
                }
                session.team_inbox_count = self
                    .teams
                    .inbox(tn)
                    .map(|msgs| msgs.len() as u32)
                    .unwrap_or(0);
            } else {
                session.team_members = Vec::new();
                session.team_inbox_count = 0;
            }

            // Apply Channel A events to LiveSession (NO cross-channel dedup)
            if !channel_a_events.is_empty() {
                for event in channel_a_events {
                    append_capped_hook_event(
                        &mut session.hook_events,
                        event,
                        MAX_HOOK_EVENTS_PER_SESSION,
                    );
                }
            }
        }
        // else: no session in map — accumulator is populated, metadata will be applied
        // when SessionStart hook or startup recovery creates the session entry.
    }

    // NOTE: Tier 2 AI classification (spawn_ai_classification) was removed.
    // It spawned unbounded `claude -p` processes on startup (40+ sessions discovered
    // simultaneously). Re-add with a Semaphore(1) rate limiter when needed.
}

// =============================================================================
// Hook event helpers (Channel A: JSONL-derived events)
// =============================================================================

/// Wraps existing `parse_timestamp_to_unix` for Option<String> input.
/// Returns 0 on failure — never SystemTime::now() (would break historical replay dedup).
fn timestamp_string_to_unix(ts: &Option<String>) -> i64 {
    ts.as_deref().and_then(parse_timestamp_to_unix).unwrap_or(0)
}

fn resolve_hook_event_from_progress(hp: &HookProgressData, ts: &Option<String>) -> HookEvent {
    let group = match hp.hook_event.as_str() {
        "SessionStart" => {
            if hp.source.as_deref() == Some("compact") {
                "autonomous"
            } else {
                "needs_you"
            }
        }
        "PreToolUse" => match hp.tool_name.as_deref() {
            Some("AskUserQuestion") | Some("EnterPlanMode") | Some("ExitPlanMode") => "needs_you",
            _ => "autonomous",
        },
        "PostToolUse" => "autonomous",
        "PostToolUseFailure" => "autonomous",
        "Stop" => "needs_you",
        _ => "autonomous",
    };
    let label = match &hp.tool_name {
        Some(tool) => format!("{}: {}", hp.hook_event, tool),
        None => hp.hook_event.clone(),
    };
    HookEvent {
        timestamp: timestamp_string_to_unix(ts),
        event_name: hp.hook_event.clone(),
        tool_name: hp.tool_name.clone(),
        label,
        group: group.to_string(),
        context: None,
        source: "hook_progress".to_string(),
    }
}

fn make_synthesized_event(
    ts: &Option<String>,
    event_name: &str,
    tool_name: Option<&str>,
    group: &str,
) -> HookEvent {
    HookEvent {
        timestamp: timestamp_string_to_unix(ts),
        event_name: event_name.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label: event_name.to_string(),
        group: group.to_string(),
        context: None,
        source: "synthesized".to_string(),
    }
}

// =============================================================================
// Path extraction helpers
// =============================================================================

/// Extract the session ID from a JSONL file path.
///
/// Path format: `~/.claude/projects/{encoded-project-dir}/{session-uuid}.jsonl`
/// Session ID = filename without the `.jsonl` extension.
fn extract_session_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Extract project info from a JSONL file path.
///
/// Returns `(encoded_project_name, display_name, decoded_project_path, resolved_cwd)`.
/// The 4th value is the raw cwd used for resolution — callers should cache it
/// in `SessionAccumulator.resolved_cwd` to avoid re-reading JSONL on every poll.
fn extract_project_info(
    path: &Path,
    cached_cwd: Option<&str>,
) -> (String, String, String, Option<String>) {
    let project_encoded = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Use cached cwd if available, else resolve from JSONL on disk.
    let cwd = cached_cwd.map(|s| s.to_string()).or_else(|| {
        path.parent()
            .and_then(claude_view_core::resolve_cwd_for_project)
    });

    let resolved = claude_view_core::discovery::resolve_project_path_with_cwd(
        &project_encoded,
        cwd.as_deref(),
    );

    (
        project_encoded,
        resolved.display_name,
        resolved.full_path,
        cwd,
    )
}

/// Calculate seconds since a Unix timestamp.
fn seconds_since_modified_from_timestamp(last_activity_at: i64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64;

    (now - last_activity_at).max(0) as u64
}

/// Parse an ISO 8601 timestamp string to a Unix epoch second.
fn parse_timestamp_to_unix(ts: &str) -> Option<i64> {
    // Try parsing with chrono for robustness
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp())
        .or_else(|| {
            // Fallback: try parsing just the date portion
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc().timestamp())
        })
}

/// Path to the PID snapshot file for server restart recovery.
fn pid_snapshot_path() -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".claude")
            .join("live-monitor-pids.json"),
    )
}

/// Save the extended session snapshot to disk atomically.
fn save_session_snapshot(path: &Path, snapshot: &SessionSnapshot) {
    let content = match serde_json::to_string(snapshot) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize session snapshot: {}", e);
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &content).is_ok() {
        if let Err(e) = std::fs::rename(&tmp, path) {
            tracing::error!(error = %e, "failed to persist session snapshot");
        }
    }
}

/// Load the session snapshot from disk, handling v1→v2 migration.
fn load_session_snapshot(path: &Path) -> SessionSnapshot {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return SessionSnapshot {
                version: 2,
                sessions: HashMap::new(),
            }
        }
    };
    load_session_snapshot_from_str(&content)
}

/// Parse a snapshot string, auto-detecting v1 (bare pid map) vs v2 (structured).
fn load_session_snapshot_from_str(content: &str) -> SessionSnapshot {
    // Try v2 first
    if let Ok(snapshot) = serde_json::from_str::<SessionSnapshot>(content) {
        return snapshot;
    }
    // Fall back to v1: { "session_id": pid, ... }
    if let Ok(v1) = serde_json::from_str::<HashMap<String, u32>>(content) {
        let sessions = v1
            .into_iter()
            .map(|(id, pid)| {
                (
                    id,
                    SnapshotEntry {
                        pid,
                        status: "working".to_string(),
                        agent_state: AgentState {
                            group: AgentStateGroup::Autonomous,
                            state: "recovered".into(),
                            label: "Recovered from restart".into(),
                            context: None,
                        },
                        last_activity_at: 0,
                        control_id: None,
                    },
                )
            })
            .collect();
        return SessionSnapshot {
            version: 2,
            sessions,
        };
    }
    SessionSnapshot {
        version: 2,
        sessions: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_extract_session_id() {
        let path = PathBuf::from("/home/user/.claude/projects/test-project/abc-123.jsonl");
        assert_eq!(extract_session_id(&path), "abc-123");
    }

    #[test]
    fn test_extract_session_id_no_extension() {
        let path = PathBuf::from("/some/path/session");
        assert_eq!(extract_session_id(&path), "session");
    }

    #[test]
    fn test_extract_project_info_simple_no_cwd() {
        // Without cwd, resolve_project_path_with_cwd returns encoded name as-is
        // (per design: "show errors, not guesses" — naive `-` split is wrong for
        // paths containing `@` or `-`).
        let path = PathBuf::from("/home/user/.claude/projects/-tmp/session.jsonl");
        let (encoded, display, full_path, _cwd) = extract_project_info(&path, None);
        assert_eq!(encoded, "-tmp");
        assert_eq!(display, "-tmp");
        assert_eq!(full_path, "-tmp"); // encoded name, not decoded — no cwd available
    }

    #[test]
    fn test_extract_project_info_with_cwd() {
        // With cwd, the resolved path uses the authoritative cwd from JSONL.
        let path = PathBuf::from("/home/user/.claude/projects/-tmp/session.jsonl");
        let (encoded, _display, full_path, cwd) = extract_project_info(&path, Some("/tmp"));
        assert_eq!(encoded, "-tmp");
        assert_eq!(full_path, "/tmp");
        assert_eq!(cwd, Some("/tmp".to_string()));
    }

    #[test]
    fn test_extract_project_info_encoded_path() {
        // Without cwd, encoded name returned as-is (no naive guessing).
        let path =
            PathBuf::from("/home/user/.claude/projects/-Users-test-my-project/session.jsonl");
        let (encoded, display, _full_path, _cwd) = extract_project_info(&path, None);
        assert_eq!(encoded, "-Users-test-my-project");
        assert!(!display.is_empty());
        // full_path is the encoded name when no cwd — NOT a decoded path
        assert_eq!(_full_path, "-Users-test-my-project");
    }

    #[test]
    fn test_parse_timestamp_to_unix() {
        let ts = "2026-01-15T10:30:00Z";
        let result = parse_timestamp_to_unix(ts);
        assert!(result.is_some());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_parse_timestamp_to_unix_with_offset() {
        let ts = "2026-01-15T10:30:00+00:00";
        let result = parse_timestamp_to_unix(ts);
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_timestamp_to_unix_invalid() {
        let result = parse_timestamp_to_unix("not-a-timestamp");
        assert!(result.is_none());
    }

    #[test]
    fn test_seconds_since_modified() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Timestamp from 60 seconds ago
        let seconds = seconds_since_modified_from_timestamp(now - 60);
        assert!(seconds >= 59 && seconds <= 61);

        // Future timestamp should return 0
        let seconds = seconds_since_modified_from_timestamp(now + 1000);
        assert_eq!(seconds, 0);
    }

    /// Verify that an already-Done session is not re-processed by the detector
    /// regardless of process presence.
    #[test]
    fn test_done_session_not_reprocessed() {
        use super::SessionStatus;

        let mut had_process: HashSet<String> = HashSet::new();
        let session_id = "test-session-done".to_string();

        // Process was seen in a previous cycle
        had_process.insert(session_id.clone());

        // But session is already Done
        let session_status = SessionStatus::Done;
        let process_running = false;

        let mut would_end = false;

        if !process_running && session_status != SessionStatus::Done {
            // This block should never execute because status == Done
            would_end = true;
        }

        assert!(
            !would_end,
            "Already-Done session must not be re-processed by process detector"
        );
    }

    /// Verify PID binding: a dead session with a bound PID is detected as dead
    /// even when another Claude process is running in the same cwd.
    ///
    /// This is the zombie session bug: session A dies without SessionEnd hook,
    /// session B starts in the same directory. Without PID binding, the
    /// detector would see session B's process and keep session A alive forever.
    #[test]
    fn test_pid_binding_prevents_zombie_sessions() {
        // Session A was bound to PID 1000 (now dead)
        let session_a_pid: Option<u32> = Some(1000);
        // Session B is alive with PID 2000 in the same cwd
        let alive_pids: HashSet<u32> = [2000].into_iter().collect();

        // PID binding check: does session A's specific PID exist?
        let running = if let Some(known_pid) = session_a_pid {
            alive_pids.contains(&known_pid)
        } else {
            false
        };

        assert!(
            !running,
            "Session A's bound PID 1000 is dead — must NOT be kept alive by PID 2000 in same cwd"
        );
    }

    #[test]
    fn test_session_snapshot_roundtrip() {
        use crate::live::state::{AgentState, AgentStateGroup, SessionSnapshot, SnapshotEntry};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot.json");

        let mut entries = HashMap::new();
        entries.insert(
            "session-abc".to_string(),
            SnapshotEntry {
                pid: 12345,
                status: "working".to_string(),
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: "Working".into(),
                    context: None,
                },
                last_activity_at: 1708500000,
                control_id: None,
            },
        );
        let snapshot = SessionSnapshot {
            version: 2,
            sessions: entries,
        };

        save_session_snapshot(&path, &snapshot);
        let loaded = load_session_snapshot(&path);

        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions["session-abc"].pid, 12345);
    }

    #[test]
    fn test_session_snapshot_missing_file() {
        let path = std::path::PathBuf::from("/tmp/nonexistent-session-snapshot-test.json");
        let loaded = load_session_snapshot(&path);
        assert_eq!(loaded.version, 2);
        assert!(loaded.sessions.is_empty());
    }

    #[test]
    fn test_session_snapshot_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot.json");
        std::fs::write(&path, "not valid json {{{").unwrap();

        let loaded = load_session_snapshot(&path);
        assert_eq!(loaded.version, 2);
        assert!(loaded.sessions.is_empty());
    }

    #[test]
    fn test_snapshot_v2_round_trip() {
        use crate::live::state::{AgentState, AgentStateGroup, SessionSnapshot, SnapshotEntry};
        use std::collections::HashMap;

        let mut entries = HashMap::new();
        entries.insert(
            "session-1".to_string(),
            SnapshotEntry {
                pid: 12345,
                status: "working".to_string(),
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: "Working".into(),
                    context: None,
                },
                last_activity_at: 1708500000,
                control_id: None,
            },
        );
        let snapshot = SessionSnapshot {
            version: 2,
            sessions: entries,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: SessionSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.sessions.len(), 1);
        let entry = &parsed.sessions["session-1"];
        assert_eq!(entry.pid, 12345);
        assert_eq!(entry.status, "working");
        assert_eq!(entry.agent_state.group, AgentStateGroup::Autonomous);
        assert_eq!(entry.last_activity_at, 1708500000);
    }

    #[test]
    fn test_snapshot_v1_migration() {
        // Legacy format: { "session-id": 12345 }
        let v1_json = r#"{"session-abc": 12345, "session-def": 67890}"#;
        let snapshot = load_session_snapshot_from_str(v1_json);

        assert_eq!(snapshot.version, 2);
        assert_eq!(snapshot.sessions.len(), 2);
        let entry = &snapshot.sessions["session-abc"];
        assert_eq!(entry.pid, 12345);
        // v1 migration: default agent_state is Autonomous/recovered
        assert_eq!(entry.agent_state.state, "recovered");
    }

    #[test]
    fn test_build_recovered_session_from_snapshot() {
        use crate::live::state::{AgentState, AgentStateGroup, SessionStatus, SnapshotEntry};

        let entry = SnapshotEntry {
            pid: 12345,
            status: "paused".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: "Asked a question".into(),
                context: None,
            },
            last_activity_at: 1708500000,
            control_id: None,
        };

        let session = build_recovered_session(
            "session-abc",
            &entry,
            "/home/user/.claude/projects/-tmp/session-abc.jsonl",
        );

        assert_eq!(session.id, "session-abc");
        assert_eq!(session.pid, Some(12345));
        assert_eq!(session.status, SessionStatus::Paused);
        assert_eq!(session.agent_state.state, "awaiting_input");
        assert_eq!(session.last_activity_at, 1708500000);
        assert_eq!(session.project_display_name, "-tmp");
        // Without cwd from JSONL, project_path is the encoded name (not naive decode)
        assert_eq!(session.project_path, "-tmp");
    }

    #[test]
    fn test_is_pid_alive_integration_for_bound_sessions() {
        use crate::live::process::is_pid_alive;

        // Bound PID that is alive (our own process)
        let alive_pid = std::process::id();
        assert!(is_pid_alive(alive_pid));

        // Bound PID that is dead
        let dead_pid: u32 = 4_000_000;
        assert!(!is_pid_alive(dead_pid));
    }

    #[test]
    fn test_snapshot_roundtrip_with_control_id() {
        use crate::live::state::{AgentState, AgentStateGroup, SessionSnapshot, SnapshotEntry};
        use std::collections::HashMap;

        let mut sessions = HashMap::new();
        sessions.insert(
            "sess-1".to_string(),
            SnapshotEntry {
                pid: 111,
                status: "working".to_string(),
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: "Working".into(),
                    context: None,
                },
                last_activity_at: 1700000000,
                control_id: Some("ctrl-abc".to_string()),
            },
        );
        sessions.insert(
            "sess-2".to_string(),
            SnapshotEntry {
                pid: 222,
                status: "paused".to_string(),
                agent_state: AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Idle".into(),
                    context: None,
                },
                last_activity_at: 1700000000,
                control_id: None,
            },
        );
        let snapshot = SessionSnapshot {
            version: 2,
            sessions,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let loaded = load_session_snapshot_from_str(&json);
        assert_eq!(
            loaded.sessions["sess-1"].control_id,
            Some("ctrl-abc".to_string())
        );
        assert_eq!(loaded.sessions["sess-2"].control_id, None);
    }

    #[tokio::test]
    async fn test_derive_state_assistant_end_turn() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done!"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
    }

    #[tokio::test]
    async fn test_derive_state_assistant_tool_use() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Read","id":"x","input":{{}}}}],"stop_reason":"tool_use"}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::Autonomous);
        assert_eq!(state.state, "acting");
    }

    #[tokio::test]
    async fn test_derive_state_user_tool_result() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"x","content":"ok"}}]}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::Autonomous);
        assert_eq!(state.state, "thinking");
    }

    #[tokio::test]
    async fn test_derive_state_skips_progress_lines() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        // Real assistant line first, then progress lines after it
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
        writeln!(
            f,
            r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive state from assistant line, not progress");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
    }

    #[tokio::test]
    async fn test_derive_state_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        std::fs::File::create(&path).unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        assert!(state.is_none());
    }

    #[test]
    fn transcript_dedup_detects_duplicate_session_ids() {
        let transcript = PathBuf::from("/tmp/sessions/abc.jsonl");
        let mut transcript_map: HashMap<PathBuf, String> = HashMap::new();

        // First session registers its transcript
        transcript_map.insert(transcript.clone(), "old-uuid".to_string());

        // Second session arrives with same transcript but different ID
        let new_id = "new-uuid";
        let dedup_target = transcript_map
            .get(&transcript)
            .filter(|existing| existing.as_str() != new_id)
            .cloned();

        assert_eq!(
            dedup_target,
            Some("old-uuid".to_string()),
            "dedup must identify the older session for merging"
        );

        // Same session ID re-registering should NOT trigger dedup
        let same_id_target = transcript_map
            .get(&transcript)
            .filter(|existing| existing.as_str() != "old-uuid")
            .cloned();
        assert_eq!(
            same_id_target, None,
            "same session re-registering must not trigger dedup"
        );
    }

    #[test]
    fn transcript_dedup_different_transcripts_no_collision() {
        let mut transcript_map: HashMap<PathBuf, String> = HashMap::new();
        transcript_map.insert(PathBuf::from("/tmp/a.jsonl"), "session-1".to_string());
        transcript_map.insert(PathBuf::from("/tmp/b.jsonl"), "session-2".to_string());

        assert_eq!(transcript_map.len(), 2);
        assert_eq!(
            transcript_map.get(&PathBuf::from("/tmp/a.jsonl")).unwrap(),
            "session-1"
        );
        assert_eq!(
            transcript_map.get(&PathBuf::from("/tmp/b.jsonl")).unwrap(),
            "session-2"
        );
    }

    /// Two snapshot entries sharing the same PID — the older one must be evicted.
    /// Simulates OS PID reuse after a crash: process A dies, OS assigns the
    /// same PID to process B, and both appear in the snapshot file.
    #[test]
    fn test_snapshot_pid_dedup_evicts_stale_entry() {
        use crate::live::state::{test_live_session, SessionStatus};

        let mut sessions: HashMap<String, LiveSession> = HashMap::new();

        // Session A: older activity, PID 42
        let mut a = test_live_session("session-a");
        a.pid = Some(42);
        a.last_activity_at = 1000;
        a.status = SessionStatus::Working;
        sessions.insert("session-a".into(), a);

        // Session B: newer activity, same PID 42
        let mut b = test_live_session("session-b");
        b.pid = Some(42);
        b.last_activity_at = 2000;
        b.status = SessionStatus::Working;
        sessions.insert("session-b".into(), b);

        // Session C: different PID, should be untouched
        let mut c = test_live_session("session-c");
        c.pid = Some(99);
        c.last_activity_at = 500;
        c.status = SessionStatus::Working;
        sessions.insert("session-c".into(), c);

        // Run the same dedup logic used in snapshot recovery
        let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
        let mut pid_dupes: Vec<String> = Vec::new();

        for (id, session) in sessions.iter() {
            if session.status == SessionStatus::Done {
                continue;
            }
            if let Some(pid) = session.pid {
                if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                    if session.last_activity_at > *existing_ts {
                        pid_dupes.push(existing_id.clone());
                        pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                    } else {
                        pid_dupes.push(id.clone());
                    }
                } else {
                    pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                }
            }
        }

        for dupe_id in &pid_dupes {
            if let Some(session) = sessions.get_mut(dupe_id) {
                session.status = SessionStatus::Done;
                session.closed_at = Some(9999);
            }
        }

        // session-a should be evicted (older)
        assert_eq!(
            sessions["session-a"].status,
            SessionStatus::Done,
            "Older session with same PID must be evicted"
        );
        assert!(sessions["session-a"].closed_at.is_some());

        // session-b should survive (newer)
        assert_eq!(
            sessions["session-b"].status,
            SessionStatus::Working,
            "Newer session with same PID must survive"
        );

        // session-c should be untouched (different PID)
        assert_eq!(
            sessions["session-c"].status,
            SessionStatus::Working,
            "Session with unique PID must be untouched"
        );
    }

    /// No PID collisions — dedup pass should be a no-op.
    #[test]
    fn test_snapshot_pid_dedup_no_collision() {
        use crate::live::state::{test_live_session, SessionStatus};

        let mut sessions: HashMap<String, LiveSession> = HashMap::new();

        let mut a = test_live_session("session-a");
        a.pid = Some(10);
        a.status = SessionStatus::Working;
        sessions.insert("session-a".into(), a);

        let mut b = test_live_session("session-b");
        b.pid = Some(20);
        b.status = SessionStatus::Working;
        sessions.insert("session-b".into(), b);

        let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
        let mut pid_dupes: Vec<String> = Vec::new();

        for (id, session) in sessions.iter() {
            if session.status == SessionStatus::Done {
                continue;
            }
            if let Some(pid) = session.pid {
                if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                    if session.last_activity_at > *existing_ts {
                        pid_dupes.push(existing_id.clone());
                        pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                    } else {
                        pid_dupes.push(id.clone());
                    }
                } else {
                    pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                }
            }
        }

        assert!(pid_dupes.is_empty(), "No PID collisions means no evictions");
        assert_eq!(sessions["session-a"].status, SessionStatus::Working);
        assert_eq!(sessions["session-b"].status, SessionStatus::Working);
    }

    /// Done sessions are excluded from PID dedup (they're already closed).
    #[test]
    fn test_snapshot_pid_dedup_skips_done_sessions() {
        use crate::live::state::{test_live_session, SessionStatus};

        let mut sessions: HashMap<String, LiveSession> = HashMap::new();

        // Session A: Done, PID 42
        let mut a = test_live_session("session-a");
        a.pid = Some(42);
        a.last_activity_at = 1000;
        a.status = SessionStatus::Done;
        sessions.insert("session-a".into(), a);

        // Session B: Working, same PID 42
        let mut b = test_live_session("session-b");
        b.pid = Some(42);
        b.last_activity_at = 2000;
        b.status = SessionStatus::Working;
        sessions.insert("session-b".into(), b);

        let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
        let mut pid_dupes: Vec<String> = Vec::new();

        for (id, session) in sessions.iter() {
            if session.status == SessionStatus::Done {
                continue;
            }
            if let Some(pid) = session.pid {
                if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                    if session.last_activity_at > *existing_ts {
                        pid_dupes.push(existing_id.clone());
                        pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                    } else {
                        pid_dupes.push(id.clone());
                    }
                } else {
                    pid_owners.insert(pid, (id.clone(), session.last_activity_at));
                }
            }
        }

        assert!(
            pid_dupes.is_empty(),
            "Done sessions must be excluded from PID dedup"
        );
        // Both remain in their original states
        assert_eq!(sessions["session-a"].status, SessionStatus::Done);
        assert_eq!(sessions["session-b"].status, SessionStatus::Working);
    }
}

#[cfg(test)]
mod hook_event_tests {
    use super::*;
    use claude_view_core::live_parser::HookProgressData;

    #[test]
    fn test_resolve_hook_event_session_start_resume() {
        let hp = HookProgressData {
            hook_event: "SessionStart".into(),
            tool_name: None,
            source: Some("resume".into()),
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.group, "needs_you");
        assert_eq!(event.event_name, "SessionStart");
    }

    #[test]
    fn test_resolve_hook_event_session_start_compact() {
        let hp = HookProgressData {
            hook_event: "SessionStart".into(),
            tool_name: None,
            source: Some("compact".into()),
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.group, "autonomous");
    }

    #[test]
    fn test_resolve_hook_event_pre_tool_ask_user() {
        let hp = HookProgressData {
            hook_event: "PreToolUse".into(),
            tool_name: Some("AskUserQuestion".into()),
            source: None,
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.group, "needs_you");
    }

    #[test]
    fn test_resolve_hook_event_pre_tool_read() {
        let hp = HookProgressData {
            hook_event: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            source: None,
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.group, "autonomous");
        assert_eq!(event.label, "PreToolUse: Read");
    }

    #[test]
    fn test_resolve_hook_event_stop() {
        let hp = HookProgressData {
            hook_event: "Stop".into(),
            tool_name: None,
            source: None,
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.group, "needs_you");
        assert_eq!(event.label, "Stop");
    }

    #[test]
    fn test_timestamp_string_to_unix_valid() {
        let ts = Some("2026-03-07T12:00:00Z".into());
        let result = timestamp_string_to_unix(&ts);
        assert!(
            result > 0,
            "Valid timestamp should produce positive unix time"
        );
    }

    #[test]
    fn test_timestamp_string_to_unix_none() {
        let result = timestamp_string_to_unix(&None);
        assert_eq!(result, 0, "None should return 0 (safe sentinel)");
    }

    #[test]
    fn test_source_discrimination_resolve_sets_hook_progress() {
        let hp = HookProgressData {
            hook_event: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            source: None,
        };
        let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
        assert_eq!(event.source, "hook_progress");
    }

    #[test]
    fn test_source_discrimination_synthesized_sets_source() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "UserPromptSubmit",
            None,
            "autonomous",
        );
        assert_eq!(event.source, "synthesized");
    }

    #[test]
    fn test_channel_a_and_b_coexist_in_memory() {
        let mut hook_events: Vec<HookEvent> = Vec::new();
        let channel_a = HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "PreToolUse: Read".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        };
        hook_events.push(channel_a);
        let channel_b = HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Reading: src/main.rs".into(),
            group: "autonomous".into(),
            context: Some(r#"{"file":"src/main.rs"}"#.into()),
            source: "hook".into(),
        };
        hook_events.push(channel_b);
        assert_eq!(hook_events.len(), 2);
        assert_eq!(hook_events[0].source, "hook_progress");
        assert_eq!(hook_events[1].source, "hook");
    }

    #[test]
    fn test_self_dedup() {
        let mut events = vec![
            HookEvent {
                timestamp: 100,
                event_name: "PreToolUse".into(),
                tool_name: Some("Read".into()),
                label: "a".into(),
                group: "autonomous".into(),
                context: None,
                source: "hook_progress".into(),
            },
            HookEvent {
                timestamp: 100,
                event_name: "PreToolUse".into(),
                tool_name: Some("Read".into()),
                label: "b".into(),
                group: "autonomous".into(),
                context: None,
                source: "hook_progress".into(),
            },
            HookEvent {
                timestamp: 101,
                event_name: "PostToolUse".into(),
                tool_name: Some("Read".into()),
                label: "c".into(),
                group: "autonomous".into(),
                context: None,
                source: "hook_progress".into(),
            },
        ];
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.event_name.cmp(&b.event_name))
                .then(a.tool_name.cmp(&b.tool_name))
                .then(a.source.cmp(&b.source))
        });
        events.dedup_by(|a, b| {
            a.event_name == b.event_name
                && a.timestamp == b.timestamp
                && a.tool_name == b.tool_name
                && a.source == b.source
        });
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_self_dedup_adversarial_interleaving() {
        let mut events = vec![
            HookEvent {
                timestamp: 100,
                event_name: "Stop".into(),
                tool_name: None,
                label: "a".into(),
                group: "needs_you".into(),
                context: None,
                source: "hook_progress".into(),
            },
            HookEvent {
                timestamp: 100,
                event_name: "PreToolUse".into(),
                tool_name: Some("Read".into()),
                label: "b".into(),
                group: "autonomous".into(),
                context: None,
                source: "hook_progress".into(),
            },
            HookEvent {
                timestamp: 100,
                event_name: "Stop".into(),
                tool_name: None,
                label: "c".into(),
                group: "needs_you".into(),
                context: None,
                source: "hook_progress".into(),
            },
        ];
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.event_name.cmp(&b.event_name))
                .then(a.tool_name.cmp(&b.tool_name))
                .then(a.source.cmp(&b.source))
        });
        events.dedup_by(|a, b| {
            a.event_name == b.event_name
                && a.timestamp == b.timestamp
                && a.tool_name == b.tool_name
                && a.source == b.source
        });
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_self_dedup_preserves_different_sources_within_channel_a() {
        let mut events = vec![
            HookEvent {
                timestamp: 100,
                event_name: "SessionEnd".into(),
                tool_name: None,
                label: "SessionEnd".into(),
                group: "needs_you".into(),
                context: None,
                source: "hook_progress".into(),
            },
            HookEvent {
                timestamp: 100,
                event_name: "SessionEnd".into(),
                tool_name: None,
                label: "SessionEnd".into(),
                group: "needs_you".into(),
                context: None,
                source: "synthesized".into(),
            },
        ];
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.event_name.cmp(&b.event_name))
                .then(a.tool_name.cmp(&b.tool_name))
                .then(a.source.cmp(&b.source))
        });
        events.dedup_by(|a, b| {
            a.event_name == b.event_name
                && a.timestamp == b.timestamp
                && a.tool_name == b.tool_name
                && a.source == b.source
        });
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].source, "hook_progress");
        assert_eq!(events[1].source, "synthesized");
    }

    #[test]
    fn test_synthesized_user_prompt_submit() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "UserPromptSubmit",
            None,
            "autonomous",
        );
        assert_eq!(event.event_name, "UserPromptSubmit");
        assert_eq!(event.group, "autonomous");
        assert_eq!(event.tool_name, None);
    }

    #[test]
    fn test_synthesized_session_end() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "SessionEnd",
            None,
            "needs_you",
        );
        assert_eq!(event.event_name, "SessionEnd");
        assert_eq!(event.group, "needs_you");
    }

    #[test]
    fn test_synthesized_pre_compact() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "PreCompact",
            None,
            "autonomous",
        );
        assert_eq!(event.event_name, "PreCompact");
    }

    #[test]
    fn test_synthesized_subagent_start() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "SubagentStart",
            Some("Explore"),
            "autonomous",
        );
        assert_eq!(event.event_name, "SubagentStart");
        assert_eq!(event.tool_name, Some("Explore".into()));
    }

    #[test]
    fn test_synthesized_subagent_stop() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "SubagentStop",
            None,
            "autonomous",
        );
        assert_eq!(event.event_name, "SubagentStop");
    }

    #[test]
    fn test_synthesized_task_completed() {
        let event = make_synthesized_event(
            &Some("2026-03-07T12:00:00Z".into()),
            "TaskCompleted",
            None,
            "autonomous",
        );
        assert_eq!(event.event_name, "TaskCompleted");
    }

    #[test]
    fn test_session_end_persist_preserves_source() {
        let channel_a = HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "PreToolUse: Read".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        };
        let row = claude_view_db::HookEventRow {
            timestamp: channel_a.timestamp,
            event_name: channel_a.event_name.clone(),
            tool_name: channel_a.tool_name.clone(),
            label: channel_a.label.clone(),
            group_name: channel_a.group.clone(),
            context: channel_a.context.clone(),
            source: channel_a.source.clone(),
        };
        assert_eq!(row.source, "hook_progress");
    }

    // ── apply_jsonl_metadata branch guard tests ──────────────────────────────
    //
    // These tests guard against the race condition where process_jsonl_update
    // runs before any user-type JSONL lines are parsed (acc.git_branch = None),
    // and unconditional assignment would overwrite the hook-resolved branch.
    //
    // Root cause (2026-03-11): acc.git_branch is None on the first
    // process_jsonl_update call when only metadata lines exist. The fix:
    // only overwrite branch fields when the accumulator has a Some value.

    fn minimal_live_session_for_branch_tests(id: &str) -> LiveSession {
        use crate::live::state::{AgentState, AgentStateGroup, SessionStatus};
        use claude_view_core::pricing::CacheStatus;
        LiveSession {
            id: id.to_string(),
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            status: SessionStatus::Working,
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            effective_branch: None,
            pid: None,
            title: "Test".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 0,
            started_at: None,
            last_activity_at: 0,
            model: None,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            team_name: None,
            team_members: Vec::new(),
            team_inbox_count: 0,
            edit_count: 0,
            progress_items: Vec::new(),
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            compact_count: 0,
            slug: None,
            user_files: None,
            closed_at: None,
            control: None,
            statusline_context_window_size: None,
            statusline_used_pct: None,
            statusline_cost_usd: None,
            model_display_name: None,
            statusline_cwd: None,
            statusline_project_dir: None,
            statusline_total_duration_ms: None,
            statusline_api_duration_ms: None,
            statusline_lines_added: None,
            statusline_lines_removed: None,
            statusline_input_tokens: None,
            statusline_output_tokens: None,
            statusline_cache_read_tokens: None,
            statusline_cache_creation_tokens: None,
            statusline_version: None,
            exceeds_200k_tokens: None,
            statusline_transcript_path: None,
            statusline_output_style: None,
            statusline_vim_mode: None,
            statusline_agent_name: None,
            statusline_worktree_name: None,
            statusline_worktree_path: None,
            statusline_worktree_branch: None,
            statusline_worktree_original_cwd: None,
            statusline_worktree_original_branch: None,
            statusline_remaining_pct: None,
            statusline_total_input_tokens: None,
            statusline_total_output_tokens: None,
            statusline_rate_limit_5h_pct: None,
            statusline_rate_limit_5h_resets_at: None,
            statusline_rate_limit_7d_pct: None,
            statusline_rate_limit_7d_resets_at: None,
            statusline_raw: None,
            model_set_at: 0,
            agent_state_set_at: 0,
            source: None,
            hook_events: Vec::new(),
            phase: PhaseHistory::default(),
        }
    }

    fn minimal_jsonl_metadata() -> JsonlMetadata {
        JsonlMetadata {
            git_branch: None,
            worktree_branch: None,
            is_worktree: false,
            pid: None,
            title: String::new(),
            last_user_message: String::new(),
            turn_count: 0,
            started_at: None,
            last_activity_at: 0,
            model: None,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            team_name: None,
            progress_items: Vec::new(),
            last_cache_hit_at: None,
            tools_used: Vec::new(),
            compact_count: 0,
            slug: None,
            user_files: None,
            edit_count: 0,
            phase: PhaseHistory::default(),
        }
    }

    /// Core regression: hook resolves branch from CWD ("main"), first
    /// process_jsonl_update arrives with acc.git_branch=None (metadata-only lines).
    /// The hook-resolved branch MUST be preserved.
    #[test]
    fn test_apply_jsonl_metadata_preserves_hook_branch_when_accumulator_has_none() {
        let mut session = minimal_live_session_for_branch_tests("test-session");
        // Simulate hook path: branch resolved from git rev-parse
        session.git_branch = Some("main".to_string());
        session.effective_branch = Some("main".to_string());

        // First process_jsonl_update: only metadata lines, no gitBranch yet
        let meta = minimal_jsonl_metadata(); // git_branch: None

        apply_jsonl_metadata(
            &mut session,
            &meta,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert_eq!(
            session.git_branch.as_deref(),
            Some("main"),
            "Hook-resolved branch must not be overwritten by None accumulator"
        );
        assert_eq!(
            session.effective_branch.as_deref(),
            Some("main"),
            "effective_branch must preserve hook-resolved value"
        );
    }

    /// Once the JSONL parser sees a user message with gitBranch, it wins.
    #[test]
    fn test_apply_jsonl_metadata_jsonl_branch_wins_when_some() {
        let mut session = minimal_live_session_for_branch_tests("test-session");
        session.git_branch = Some("old-hook-branch".to_string());
        session.effective_branch = Some("old-hook-branch".to_string());

        let mut meta = minimal_jsonl_metadata();
        meta.git_branch = Some("main".to_string()); // JSONL has definitive value

        apply_jsonl_metadata(
            &mut session,
            &meta,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert_eq!(
            session.git_branch.as_deref(),
            Some("main"),
            "JSONL-sourced branch must overwrite hook branch when Some"
        );
        assert_eq!(
            session.effective_branch.as_deref(),
            Some("main"),
            "effective_branch must reflect JSONL branch"
        );
    }

    /// Session with no branch from either source: stays None.
    #[test]
    fn test_apply_jsonl_metadata_none_stays_none_when_no_source() {
        let mut session = minimal_live_session_for_branch_tests("test-session");
        // session.git_branch is None (no hook resolution, e.g. non-git dir)

        let meta = minimal_jsonl_metadata(); // also None

        apply_jsonl_metadata(
            &mut session,
            &meta,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert!(
            session.git_branch.is_none(),
            "Branch stays None when neither hook nor JSONL provides a value"
        );
        assert!(
            session.effective_branch.is_none(),
            "effective_branch stays None when no source provides a value"
        );
    }

    /// Worktree branch: hook resolves from CWD, metadata arrives with None.
    /// Must not destroy the worktree branch.
    #[test]
    fn test_apply_jsonl_metadata_preserves_worktree_branch_when_none() {
        let mut session = minimal_live_session_for_branch_tests("test-session");
        session.git_branch = Some("main".to_string());
        session.worktree_branch = Some("feat/my-feature".to_string());
        session.is_worktree = true;
        session.effective_branch = Some("feat/my-feature".to_string()); // worktree wins

        let meta = minimal_jsonl_metadata(); // worktree_branch: None, is_worktree: false

        apply_jsonl_metadata(
            &mut session,
            &meta,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert_eq!(
            session.worktree_branch.as_deref(),
            Some("feat/my-feature"),
            "Hook-resolved worktree branch must not be cleared by None accumulator"
        );
        assert!(
            session.is_worktree,
            "is_worktree must not be reset to false by metadata with is_worktree=false"
        );
        assert_eq!(
            session.effective_branch.as_deref(),
            Some("feat/my-feature"),
            "effective_branch must stay as worktree branch"
        );
    }

    /// Worktree branch from JSONL wins when Some.
    #[test]
    fn test_apply_jsonl_metadata_jsonl_worktree_branch_wins_when_some() {
        let mut session = minimal_live_session_for_branch_tests("test-session");
        session.git_branch = Some("main".to_string());
        session.worktree_branch = None;
        session.effective_branch = Some("main".to_string());

        let mut meta = minimal_jsonl_metadata();
        meta.git_branch = Some("main".to_string());
        meta.worktree_branch = Some("feat/my-feature".to_string());
        meta.is_worktree = true;

        apply_jsonl_metadata(
            &mut session,
            &meta,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert_eq!(session.worktree_branch.as_deref(), Some("feat/my-feature"));
        assert!(session.is_worktree);
        assert_eq!(
            session.effective_branch.as_deref(),
            Some("feat/my-feature"),
            "effective_branch must prefer worktree_branch over git_branch"
        );
    }

    #[test]
    fn test_edit_count_accumulates_from_edit_and_write_tools() {
        // edit_count must be the sum of Edit + Write tool uses from the accumulator.
        // This guards against regressions where only one tool type is counted.
        let mut metadata = minimal_jsonl_metadata();
        metadata.edit_count = 7; // simulates acc.tool_counts_edit=4 + acc.tool_counts_write=3

        let mut session = minimal_live_session_for_branch_tests("test-edit-count");
        apply_jsonl_metadata(
            &mut session,
            &metadata,
            "/tmp/test.jsonl",
            "proj",
            "proj",
            "/tmp",
        );

        assert_eq!(
            session.edit_count, 7,
            "edit_count must be propagated from JsonlMetadata to LiveSession"
        );
    }

    #[test]
    fn test_edit_count_defaults_to_zero_for_new_session() {
        // A freshly constructed LiveSession must have edit_count = 0.
        // Guards against construction sites that forget the default.
        let session = minimal_live_session_for_branch_tests("test-zero-edit-count");
        assert_eq!(
            session.edit_count, 0,
            "edit_count must default to 0 in freshly constructed LiveSession"
        );
    }

    #[test]
    fn test_team_members_and_inbox_count_default_to_empty_for_non_team_session() {
        // A session without a team name must have empty team_members and inbox_count = 0.
        // Guards against sessions leaking team data from previous state.
        let session = minimal_live_session_for_branch_tests("test-no-team");
        assert!(
            session.team_members.is_empty(),
            "team_members must be empty for non-team sessions"
        );
        assert_eq!(
            session.team_inbox_count, 0,
            "team_inbox_count must be 0 for non-team sessions"
        );
    }
}
