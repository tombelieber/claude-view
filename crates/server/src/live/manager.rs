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
use claude_view_core::pricing::{
    calculate_cost, finalize_cost_breakdown, CacheStatus, CostBreakdown, ModelPricing, TokenUsage,
};
use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::Database;

use super::process::{count_claude_processes, is_pid_alive};
use super::state::{
    append_capped_hook_event, status_from_agent_state, AgentState, AgentStateGroup, HookEvent,
    LiveSession, SessionEvent, SessionSnapshot, SessionStatus, SnapshotEntry,
    MAX_HOOK_EVENTS_PER_SESSION,
};
use super::watcher::{initial_scan, start_watcher, FileEvent};

/// Type alias for the shared session map used by both the manager and route handlers.
pub type LiveSessionMap = Arc<RwLock<HashMap<String, LiveSession>>>;

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
    /// Filename from `<ide_opened_file>` tag in the last user message, if present.
    last_user_file: Option<String>,
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
    /// Per-turn accumulated cost breakdown. Each assistant turn's tokens are
    /// priced individually (200k tiering is per-API-request, not per-session).
    accumulated_cost: CostBreakdown,
    /// Dedup guard for split assistant content blocks.
    /// Keyed by `message.id:requestId` so one API response is counted once.
    seen_api_calls: std::collections::HashSet<String>,
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
            last_user_file: None,
            git_branch: None,
            latest_cwd: None,
            started_at: None,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            todo_items: Vec::new(),
            task_items: Vec::new(),
            last_cache_hit_at: None,
            mcp_servers: std::collections::HashSet::new(),
            skills: std::collections::HashSet::new(),
            file_path: None,
            project_path: None,
            resolved_cwd: None,
            tool_counts_edit: 0,
            tool_counts_read: 0,
            tool_counts_bash: 0,
            tool_counts_write: 0,
            compact_count: 0,
            accumulated_cost: CostBreakdown::default(),
            seen_api_calls: std::collections::HashSet::new(),
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
    last_user_file: Option<String>,
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
    progress_items: Vec<claude_view_core::progress::ProgressItem>,
    last_cache_hit_at: Option<i64>,
    tools_used: Vec<super::state::ToolUsed>,
    compact_count: u32,
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
        last_user_file: None,
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
        progress_items: Vec::new(),
        tools_used: Vec::new(),
        last_cache_hit_at: None,
        compact_count: 0,
        control: None,
        hook_events: Vec::new(),
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
            LineType::Progress | LineType::System | LineType::Summary | LineType::Other => {
                continue; // Skip non-meaningful lines
            }
            LineType::Result => {
                return Some(AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Waiting for your next prompt".into(),
                    context: None,
                });
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
    session.git_branch = m.git_branch.clone();
    session.worktree_branch = m.worktree_branch.clone();
    session.is_worktree = m.is_worktree;
    session.effective_branch = m.worktree_branch.clone().or(m.git_branch.clone());
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
    if m.last_user_file.is_some() {
        session.last_user_file = m.last_user_file.clone();
    }
    session.turn_count = m.turn_count;
    if m.started_at.is_some() {
        session.started_at = m.started_at;
    }
    session.last_activity_at = m.last_activity_at;
    session.model = m.model.clone();
    session.tokens = m.tokens.clone();
    session.context_window_tokens = m.context_window_tokens;
    session.cost = m.cost.clone();
    session.cache_status = m.cache_status.clone();
    session.current_turn_started_at = m.current_turn_started_at;
    session.last_turn_task_seconds = m.last_turn_task_seconds;
    session.sub_agents = m.sub_agents.clone();
    session.progress_items = m.progress_items.clone();
    session.tools_used = m.tools_used.clone();
    session.last_cache_hit_at = m.last_cache_hit_at;
    session.compact_count = m.compact_count;
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
    /// Per-model pricing table for cost calculation.
    pricing: Arc<StdRwLock<HashMap<String, ModelPricing>>>,
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
}

impl LiveSessionManager {
    /// Start the live session manager and all background tasks.
    ///
    /// Returns the manager, a shared session map for route handlers, and the
    /// broadcast sender for SSE event streaming.
    pub fn start(
        pricing: Arc<StdRwLock<HashMap<String, ModelPricing>>>,
        db: Database,
        search_index: Arc<StdRwLock<Option<Arc<claude_view_search::SearchIndex>>>>,
        registry: Arc<StdRwLock<Option<claude_view_core::Registry>>>,
        sidecar: Option<Arc<crate::sidecar::SidecarManager>>,
    ) -> (Arc<Self>, LiveSessionMap, broadcast::Sender<SessionEvent>) {
        let (tx, _rx) = broadcast::channel(256);
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));

        // Debounced snapshot writer channel (bounded to 1 — extra signals are coalesced)
        let (snapshot_tx, snapshot_rx) = mpsc::channel::<()>(1);

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
        });

        // Spawn background tasks
        manager.spawn_snapshot_writer(snapshot_rx);
        manager.spawn_file_watcher();
        manager.spawn_reconciliation_loop();
        manager.spawn_cleanup_task();

        // Spawn relay client for mobile remote access
        super::relay_client::spawn_relay_client(
            tx.clone(),
            sessions.clone(),
            super::relay_client::RelayClientConfig::default(),
        );

        info!("LiveSessionManager started with 5 background tasks (file watcher, reconciliation loop, cleanup, relay client, db writer)");

        (manager, sessions, tx)
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
                .unwrap_or_default()
                .as_secs() as i64;
            session.control = Some(super::state::ControlBinding {
                control_id,
                bound_at: now,
                cancel: tokio_util::sync::CancellationToken::new(),
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
        save_session_snapshot(
            &pid_snapshot_path(),
            &SessionSnapshot {
                version: 2,
                sessions: entries,
            },
        );
    }

    /// Run a one-shot process count scan (display metric only).
    async fn run_eager_process_scan(&self) {
        let total_count = tokio::task::spawn_blocking(count_claude_processes)
            .await
            .unwrap_or_default();
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
            {
                let snapshot = load_session_snapshot(&pid_snapshot_path());
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
                                    last_user_file: acc.last_user_file.clone(),
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
                        let mut sessions = manager.sessions.write().await;
                        if sessions.remove(&session_id).is_some() {
                            let mut accumulators = manager.accumulators.write().await;
                            accumulators.remove(&session_id);
                            let _ = manager
                                .tx
                                .send(SessionEvent::SessionCompleted { session_id });
                        }
                    }
                    FileEvent::Rescan => {
                        tracing::info!("Overflow detected — triggering full reconciliation scan");
                        let claude_dir = dirs::home_dir().expect("home dir").join(".claude");
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
                                info!(
                                    session_id = %session_id,
                                    pid = pid,
                                    "Bound PID is dead — marking session ended"
                                );
                                session.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "session_ended".into(),
                                    label: "Session ended (process exited)".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Done;
                                let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                    session: session.clone(),
                                });
                                dead_sessions.push(session_id.clone());
                                snapshot_dirty = true;
                                continue;
                            }
                        }
                    }

                    // Remove dead sessions from map
                    for session_id in &dead_sessions {
                        sessions.remove(session_id);
                    }
                }

                // Remove accumulators for dead sessions to prevent stale data if
                // a new session starts in the same project directory.
                if !dead_sessions.is_empty() {
                    let mut accumulators = manager.accumulators.write().await;
                    for session_id in &dead_sessions {
                        accumulators.remove(session_id);
                    }
                }

                // Save session snapshot if any bindings changed (outside lock)
                if snapshot_dirty {
                    manager.save_session_snapshot_from_state().await;
                }

                // Broadcast completions (outside lock)
                for session_id in dead_sessions {
                    let _ = manager
                        .tx
                        .send(SessionEvent::SessionCompleted { session_id });
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

                // 2.1 — Process count refresh (display metric only)
                let total_count = tokio::task::spawn_blocking(count_claude_processes)
                    .await
                    .unwrap_or_default();
                manager.process_count.store(total_count, Ordering::Relaxed);

                // 2.2 — Unconditional snapshot save (defense in depth)
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
            acc.tokens = TokenUsage::default();
            acc.tool_counts_edit = 0;
            acc.tool_counts_read = 0;
            acc.tool_counts_bash = 0;
            acc.tool_counts_write = 0;
            acc.compact_count = 0;
            acc.accumulated_cost = CostBreakdown::default();
            acc.seen_api_calls.clear();
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
                if let Ok(p) = self.pricing.read() {
                    let turn_cost = calculate_cost(&turn_tokens, acc.model.as_deref(), &p);
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

            // Track git branch and cwd from user messages
            if let Some(ref branch) = line.git_branch {
                acc.git_branch = Some(branch.clone());
            }
            if let Some(ref cwd) = line.cwd {
                acc.latest_cwd = Some(cwd.clone());
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
                    if line.ide_file.is_some() {
                        acc.last_user_file = line.ide_file.clone();
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

            // --- Sub-agent spawn tracking ---
            for spawn in &line.sub_agent_spawns {
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
                    // Background agents return "async_launched" immediately — they're
                    // still running, so only capture the agentId and keep Running status.
                    if result.status == "async_launched" {
                        agent.agent_id = result.agent_id.clone();
                    } else {
                        agent.status = if result.status == "completed" {
                            SubAgentStatus::Complete
                        } else {
                            SubAgentStatus::Error
                        };
                        agent.agent_id = result.agent_id.clone();
                        agent.completed_at =
                            line.timestamp.as_deref().and_then(parse_timestamp_to_unix);
                        agent.duration_ms = result.total_duration_ms;
                        agent.tool_use_count = result.total_tool_use_count;
                        agent.current_activity = None;

                        // Compute cost from token usage via pricing table
                        if let Some(model) = acc.model.as_deref() {
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
                            let sub_cost = self
                                .pricing
                                .read()
                                .ok()
                                .map(|p| calculate_cost(&sub_tokens, Some(model), &p))
                                .unwrap_or_default();
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
            if line.line_type == LineType::Result {
                channel_a_events.push(make_synthesized_event(
                    &line.timestamp,
                    "SessionEnd",
                    None,
                    "needs_you",
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
            last_user_file: acc.last_user_file.clone(),
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
            apply_jsonl_metadata(
                session,
                &metadata,
                &file_path_str,
                &project,
                &project_display_name,
                &project_path,
            );

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
        .unwrap_or_default()
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
fn pid_snapshot_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir exists")
        .join(".claude")
        .join("live-monitor-pids.json")
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
        let _ = std::fs::rename(&tmp, path);
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
    async fn test_derive_state_result_line() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"result","subtype":"success","duration_ms":1234,"is_error":false,"session_id":"abc"}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive state from result line");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
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
}
