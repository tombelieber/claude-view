//! Central orchestrator for live session monitoring.
//!
//! The `LiveSessionManager` ties together the file watcher, process detector,
//! JSONL tail parser, and cleanup task to maintain an in-memory map of all
//! active Claude Code sessions.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn};

use vibe_recall_core::cost::{
    self, calculate_live_cost, derive_cache_status, TokenUsage,
};
use vibe_recall_core::live_parser::{LiveLine, LineType, TailFinders, parse_tail};
use vibe_recall_core::subagent::{SubAgentInfo, SubAgentStatus};
use vibe_recall_db::ModelPricing;

use super::classifier::{
    MessageSummary, PauseClassification, PauseReason,
    SessionStateClassifier, SessionStateContext,
};
use super::process::{ClaudeProcess, detect_claude_processes, has_running_process};
use super::state::{
    AgentState, AgentStateGroup, SignalSource,
    LiveSession, SessionEvent, SessionStatus, derive_activity, derive_status,
};
use super::state_resolver::StateResolver;
use super::watcher::{FileEvent, initial_scan, start_watcher};

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
    /// Git branch name extracted from user messages.
    git_branch: Option<String>,
    /// The timestamp of the first line (session start).
    started_at: Option<i64>,
    /// The last LiveLine parsed (for status derivation).
    last_line: Option<LiveLine>,
    /// Unix timestamp when this session was marked Done (for cleanup).
    completed_at: Option<u64>,
    /// Current agent state (replaces pause_classification).
    agent_state: AgentState,
    /// Recent messages for classification context (ring buffer, last 5).
    recent_messages: VecDeque<MessageSummary>,
    /// Previous status for transition detection.
    last_status: Option<SessionStatus>,
    /// Unix timestamp when the current user turn started (real prompt, not meta/tool-result/system).
    current_turn_started_at: Option<i64>,
    /// Seconds the agent spent on the last completed turn (Working->Paused).
    last_turn_task_seconds: Option<u32>,
    /// Sub-agents spawned in this session (accumulated across tail polls).
    sub_agents: Vec<SubAgentInfo>,
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
            started_at: None,
            last_line: None,
            completed_at: None,
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "thinking".into(),
                label: "Discovered...".into(),
                confidence: 0.3,
                source: SignalSource::Fallback,
                context: None,
            },
            recent_messages: VecDeque::new(),
            last_status: None,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
        }
    }
}

/// Convert a PauseClassification (from the structural classifier) to AgentState.
fn pause_classification_to_agent_state(c: &PauseClassification) -> AgentState {
    let (group, state) = match c.reason {
        PauseReason::NeedsInput => (AgentStateGroup::NeedsYou, "awaiting_input"),
        PauseReason::TaskComplete => (AgentStateGroup::NeedsYou, "task_complete"),
        PauseReason::WorkDelivered => (AgentStateGroup::NeedsYou, "work_delivered"),
        PauseReason::MidWork => (AgentStateGroup::NeedsYou, "idle"),
        PauseReason::Error => (AgentStateGroup::NeedsYou, "error"),
    };
    AgentState {
        group,
        state: state.into(),
        label: c.label.clone(),
        confidence: c.confidence,
        source: SignalSource::Jsonl,
        context: None,
    }
}

/// Pure function: derive agent state from current evidence.
///
/// Called on EVERY update — not gated by status transitions. This eliminates
/// the race condition where a tool_result line steals the Working→Paused
/// transition from the real end_turn line.
///
/// The 120s MidWork timeout (previously Phase 4 in spawn_process_detector)
/// is incorporated here for instant reactivity.
fn derive_agent_state(
    status: &SessionStatus,
    last_line: Option<&LiveLine>,
    recent_messages: &VecDeque<MessageSummary>,
    classifier: &SessionStateClassifier,
    has_running_process: bool,
    seconds_since_modified: u64,
    turn_count: u32,
    is_first_poll: bool,
) -> AgentState {
    match status {
        SessionStatus::Working => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working...".into(),
            confidence: 0.7,
            source: SignalSource::Jsonl,
            context: None,
        },
        SessionStatus::Done => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session ended".into(),
            confidence: 0.9,
            source: SignalSource::Jsonl,
            context: None,
        },
        SessionStatus::Paused => {
            let ctx = SessionStateContext {
                recent_messages: recent_messages.iter().cloned().collect(),
                last_stop_reason: last_line.and_then(|l| l.stop_reason.clone()),
                last_tool: last_line.and_then(|l| l.tool_names.last().cloned()),
                has_running_process,
                seconds_since_modified,
                turn_count,
            };

            // Tier 1: structural classification (instant)
            if let Some(c) = classifier.structural_classify(&ctx) {
                return pause_classification_to_agent_state(&c);
            }

            // Fallback classification
            let c = classifier.fallback_classify(&ctx);

            // MidWork = ambiguous pause. Keep Autonomous if ALL of:
            //   (a) fallback says MidWork (no end_turn detected), AND
            //   (b) process detected OR file active within 60s, AND
            //   (c) not stale (≤120s) — absorbs former Phase 4, AND
            //   (d) not first poll without process evidence
            let keep_autonomous = c.reason == PauseReason::MidWork && if is_first_poll {
                has_running_process
            } else {
                (has_running_process || seconds_since_modified <= 60)
                    && seconds_since_modified <= 120
            };

            if keep_autonomous {
                AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Between steps...".into(),
                    confidence: if has_running_process { 0.5 } else { 0.4 },
                    source: SignalSource::Jsonl,
                    context: None,
                }
            } else {
                pause_classification_to_agent_state(&c)
            }
        }
    }
}

/// Handle side effects of status transitions.
///
/// This is NOT classification — classification is done by `derive_agent_state()`.
/// This function only handles:
/// - Task time computation on Working→Paused
/// - Task time clearing on →Working
/// - Completion tracking + sub-agent cleanup on →Done
fn handle_transitions(
    new_status: &SessionStatus,
    acc: &mut SessionAccumulator,
    last_activity_at: i64,
) {
    let old_status = acc.last_status.clone();

    // Working→Paused (or first-discovery-as-Paused): compute task time
    let is_working_to_paused = *new_status == SessionStatus::Paused
        && (old_status == Some(SessionStatus::Working) || old_status.is_none());
    if is_working_to_paused {
        if let Some(turn_start) = acc.current_turn_started_at {
            let elapsed = (last_activity_at - turn_start).max(0) as u32;
            acc.last_turn_task_seconds = Some(elapsed);
        }
    }

    // →Working: clear frozen task time
    if *new_status == SessionStatus::Working {
        acc.last_turn_task_seconds = None;
    }

    // →Done: track completion time + orphaned sub-agent cleanup
    if *new_status == SessionStatus::Done && acc.completed_at.is_none() {
        let completed_at_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        acc.completed_at = Some(completed_at_secs);
        for agent in &mut acc.sub_agents {
            if agent.status == SubAgentStatus::Running {
                agent.status = SubAgentStatus::Error;
                agent.completed_at = Some(completed_at_secs as i64);
                agent.current_activity = None;
            }
        }
    } else if *new_status != SessionStatus::Done {
        acc.completed_at = None;
    }

    acc.last_status = Some(new_status.clone());
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
    /// Detected Claude processes, keyed by working directory.
    processes: Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>,
    /// Per-model pricing table for cost calculation (core-level types).
    pricing: Arc<HashMap<String, cost::ModelPricing>>,
    /// Session state classifier for intelligent pause classification.
    classifier: Arc<SessionStateClassifier>,
    /// Resolves agent state by merging hook and JSONL signals (hook wins if fresh).
    state_resolver: StateResolver,
}

impl LiveSessionManager {
    /// Start the live session manager and all background tasks.
    ///
    /// Returns the manager, a shared session map for route handlers, and the
    /// broadcast sender for SSE event streaming.
    pub fn start(
        pricing: HashMap<String, ModelPricing>,
        state_resolver: StateResolver,
    ) -> (Arc<Self>, LiveSessionMap, broadcast::Sender<SessionEvent>) {
        let classifier = Arc::new(SessionStateClassifier::new());
        let (tx, _rx) = broadcast::channel(256);
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));

        // Convert vibe_recall_db::ModelPricing -> vibe_recall_core::cost::ModelPricing
        let core_pricing: HashMap<String, cost::ModelPricing> = pricing
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    cost::ModelPricing {
                        input_cost_per_token: v.input_cost_per_token,
                        output_cost_per_token: v.output_cost_per_token,
                        cache_creation_cost_per_token: v.cache_creation_cost_per_token,
                        cache_read_cost_per_token: v.cache_read_cost_per_token,
                    },
                )
            })
            .collect();

        let manager = Arc::new(Self {
            sessions: sessions.clone(),
            tx: tx.clone(),
            finders: Arc::new(TailFinders::new()),
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(HashMap::new())),
            pricing: Arc::new(core_pricing),
            classifier,
            state_resolver,
        });

        // Spawn background tasks
        manager.spawn_file_watcher();
        manager.spawn_process_detector();
        manager.spawn_cleanup_task();

        info!("LiveSessionManager started with 3 background tasks");

        (manager, sessions, tx)
    }

    /// Subscribe to session events for SSE streaming.
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.tx.subscribe()
    }

    /// Spawn the file watcher background task.
    ///
    /// 1. Performs an initial scan of `~/.claude/projects/` for recent JSONL files.
    /// 2. Starts a notify watcher for ongoing file changes.
    /// 3. Processes each Modified/Removed event by parsing new JSONL lines.
    fn spawn_file_watcher(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            // Initial scan
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

            info!("Initial scan found {} recent JSONL files", initial_paths.len());

            // Process each discovered file
            for path in &initial_paths {
                manager.process_jsonl_update(path).await;
                // Mark initial discoveries
                let session_id = extract_session_id(path);
                let sessions = manager.sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let _ = manager.tx.send(SessionEvent::SessionDiscovered {
                        session: session.clone(),
                    });
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
                        let sid = extract_session_id(path);
                        let is_new = {
                            let sessions = manager.sessions.read().await;
                            !sessions.contains_key(&sid)
                        };
                        manager.process_jsonl_update(path).await;
                        if is_new {
                            let sessions = manager.sessions.read().await;
                            if let Some(session) = sessions.get(&sid) {
                                let _ = manager.tx.send(SessionEvent::SessionDiscovered {
                                    session: session.clone(),
                                });
                            }
                        }
                    }
                }
                match event {
                    FileEvent::Modified(path) => {
                        let session_id = extract_session_id(&path);
                        let is_new = {
                            let sessions = manager.sessions.read().await;
                            !sessions.contains_key(&session_id)
                        };

                        manager.process_jsonl_update(&path).await;

                        let sessions = manager.sessions.read().await;
                        if let Some(session) = sessions.get(&session_id) {
                            let event = if is_new {
                                SessionEvent::SessionDiscovered {
                                    session: session.clone(),
                                }
                            } else {
                                SessionEvent::SessionUpdated {
                                    session: session.clone(),
                                }
                            };
                            let _ = manager.tx.send(event);
                        }
                    }
                    FileEvent::Removed(path) => {
                        let session_id = extract_session_id(&path);
                        let mut sessions = manager.sessions.write().await;
                        if sessions.remove(&session_id).is_some() {
                            let mut accumulators = manager.accumulators.write().await;
                            accumulators.remove(&session_id);
                            let _ = manager.tx.send(SessionEvent::SessionCompleted {
                                session_id,
                            });
                        }
                    }
                }
            }
        });
    }

    /// Spawn the process detector background task.
    ///
    /// Every 2 seconds, scans the process table for running Claude instances
    /// and updates the shared process map. Re-derives status AND agent state
    /// for all sessions (agent state derivation is not transition-gated).
    ///
    /// Uses a 3-phase pattern to avoid deadlocks and TOCTOU races:
    /// - Phase 1 (under sessions+accumulators locks): Derive status + agent state.
    /// - Phase 2 (no locks): Feed JSONL states into the resolver.
    /// - Phase 3 (under sessions lock only): Call resolve() and apply final state.
    fn spawn_process_detector(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;

                let new_processes = tokio::task::spawn_blocking(detect_claude_processes)
                    .await
                    .unwrap_or_default();

                {
                    let mut processes = manager.processes.write().await;
                    *processes = new_processes;
                }

                // Phase 1: Collect changes under locks, then drop locks.
                // Sync work ONLY — no .await while holding both guards.
                let mut pending_updates: Vec<(String, SessionStatus, Option<u32>, AgentState)> =
                    Vec::new();

                {
                    let processes = manager.processes.read().await;
                    let mut sessions = manager.sessions.write().await;
                    let mut accumulators = manager.accumulators.write().await;

                    for (session_id, session) in sessions.iter_mut() {
                        if let Some(acc) = accumulators.get_mut(session_id) {
                            let seconds_since = seconds_since_modified_from_timestamp(
                                session.last_activity_at,
                            );
                            let (running, pid) =
                                has_running_process(&processes, &session.project_path);
                            let new_status =
                                derive_status(acc.last_line.as_ref(), seconds_since, running);

                            let status_or_pid_changed =
                                session.status != new_status || session.pid != pid;

                            if status_or_pid_changed {
                                // Status/pid changed — must re-derive and handle transitions
                                let is_first_poll = acc.last_status.is_none();
                                let new_agent_state = derive_agent_state(
                                    &new_status,
                                    acc.last_line.as_ref(),
                                    &acc.recent_messages,
                                    &manager.classifier,
                                    running,
                                    seconds_since,
                                    acc.user_turn_count,
                                    is_first_poll,
                                );
                                handle_transitions(
                                    &new_status, acc, session.last_activity_at,
                                );
                                acc.agent_state = new_agent_state;

                                pending_updates.push((
                                    session_id.clone(),
                                    new_status,
                                    pid,
                                    acc.agent_state.clone(),
                                ));
                            } else {
                                // Status unchanged — check if agent state group changed
                                // (time-based transitions like MidWork→NeedsYou at 120s).
                                // Skip hook-sourced states: long subagent runs are
                                // legitimately autonomous, don't override with JSONL derivation.
                                if matches!(session.agent_state.source, SignalSource::Hook) {
                                    continue;
                                }
                                let is_first_poll = acc.last_status.is_none();
                                let new_agent_state = derive_agent_state(
                                    &new_status,
                                    acc.last_line.as_ref(),
                                    &acc.recent_messages,
                                    &manager.classifier,
                                    running,
                                    seconds_since,
                                    acc.user_turn_count,
                                    is_first_poll,
                                );
                                if session.agent_state.group != new_agent_state.group {
                                    acc.agent_state = new_agent_state;
                                    pending_updates.push((
                                        session_id.clone(),
                                        new_status,
                                        pid,
                                        acc.agent_state.clone(),
                                    ));
                                }
                            }
                        }
                    }
                }
                // All locks dropped here.

                // Phase 2: Feed JSONL states into resolver (async, no external locks held).
                // update_from_jsonl() only needs StateResolver's internal lock.
                for (session_id, ref new_status, _, ref jsonl_state) in &pending_updates {
                    // Clear stale hook states when JSONL evidence shows Working.
                    if *new_status == SessionStatus::Working {
                        manager.state_resolver.clear_hook_state(session_id).await;
                    }
                    manager.state_resolver.update_from_jsonl(session_id, jsonl_state.clone()).await;
                }

                // Phase 3: Resolve and apply under sessions lock.
                // CRITICAL: resolve() is called HERE (not Phase 2) to prevent TOCTOU race.
                if !pending_updates.is_empty() {
                    let mut sessions = manager.sessions.write().await;
                    for (session_id, new_status, pid, _) in pending_updates {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            let resolved = manager.state_resolver.resolve(&session_id).await;
                            session.status = new_status.clone();
                            session.pid = pid;
                            session.agent_state = resolved;
                            // Clear stale activity when session is no longer Working.
                            if new_status != SessionStatus::Working {
                                session.current_activity = String::new();
                            }
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                        }
                    }
                }
            }
        });
    }

    /// Spawn the cleanup background task.
    ///
    /// Every 30 seconds, removes sessions that have been `Done` for more
    /// than 10 minutes and broadcasts `SessionCompleted` events.
    fn spawn_cleanup_task(self: &Arc<Self>) {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let mut to_remove = Vec::new();

                {
                    let sessions = manager.sessions.read().await;
                    let accumulators = manager.accumulators.read().await;

                    for (session_id, session) in sessions.iter() {
                        if session.status == SessionStatus::Done {
                            if let Some(acc) = accumulators.get(session_id) {
                                if let Some(completed_at) = acc.completed_at {
                                    if now.saturating_sub(completed_at) > 600 {
                                        to_remove.push(session_id.clone());
                                    }
                                }
                            }
                        }
                    }
                }

                if !to_remove.is_empty() {
                    let mut sessions = manager.sessions.write().await;
                    let mut accumulators = manager.accumulators.write().await;
                    for session_id in &to_remove {
                        sessions.remove(session_id);
                        accumulators.remove(session_id);
                        let _ = manager.tx.send(SessionEvent::SessionCompleted {
                            session_id: session_id.clone(),
                        });
                    }
                    info!("Cleaned up {} done sessions", to_remove.len());
                }

                // Clean up stale hook states (entries older than 10 minutes)
                manager.state_resolver.cleanup_stale(Duration::from_secs(600)).await;
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
        let (project, project_display_name, project_path) = extract_project_info(path);

        // Get the current offset for this session
        let current_offset = {
            let accumulators = self.accumulators.read().await;
            accumulators
                .get(&session_id)
                .map(|a| a.offset)
                .unwrap_or(0)
        };

        // Parse new lines from the JSONL file (blocking I/O)
        let finders = self.finders.clone();
        let path_owned = path.to_path_buf();
        let parse_result = tokio::task::spawn_blocking(move || {
            parse_tail(&path_owned, current_offset, &finders)
        })
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

        let seconds_since = seconds_since_modified_from_timestamp(last_activity_at);

        // Update accumulator with new lines
        let mut accumulators = self.accumulators.write().await;
        let acc = accumulators
            .entry(session_id.clone())
            .or_insert_with(SessionAccumulator::new);

        acc.offset = new_offset;

        for line in &new_lines {
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

            // Track model
            if let Some(ref model) = line.model {
                acc.model = Some(model.clone());
            }

            // Track git branch from user messages
            if let Some(ref branch) = line.git_branch {
                acc.git_branch = Some(branch.clone());
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
                if acc.sub_agents.iter().any(|a| a.tool_use_id == spawn.tool_use_id) {
                    continue;
                }

                // Parse timestamp from the JSONL line to get started_at
                let started_at = line.timestamp.as_deref()
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
                if let Some(agent) = acc.sub_agents.iter_mut()
                    .find(|a| a.tool_use_id == result.tool_use_id)
                {
                    agent.status = if result.status == "completed" {
                        SubAgentStatus::Complete
                    } else {
                        SubAgentStatus::Error
                    };
                    agent.agent_id = result.agent_id.clone();
                    agent.completed_at = line.timestamp.as_deref()
                        .and_then(parse_timestamp_to_unix);
                    agent.duration_ms = result.total_duration_ms;
                    agent.tool_use_count = result.total_tool_use_count;
                    agent.current_activity = None; // No longer running, clear activity
                    // Compute cost from token usage via pricing table
                    if let Some(model) = acc.model.as_deref() {
                        let sub_tokens = TokenUsage {
                            input_tokens: result.usage_input_tokens.unwrap_or(0),
                            output_tokens: result.usage_output_tokens.unwrap_or(0),
                            cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                            cache_creation_tokens: result.usage_cache_creation_tokens.unwrap_or(0),
                            total_tokens: 0, // not used by calculate_live_cost
                        };
                        let sub_cost = calculate_live_cost(&sub_tokens, Some(model), &self.pricing);
                        if sub_cost.total_usd > 0.0 {
                            agent.cost_usd = Some(sub_cost.total_usd);
                        }
                    }
                }
                // If no matching spawn found, ignore gracefully (orphaned tool_result)
            }

            // --- Sub-agent progress tracking (early agentId + current activity) ---
            if let Some(ref progress) = line.sub_agent_progress {
                if let Some(agent) = acc.sub_agents.iter_mut()
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
        }

        // Track recent messages for pause classification
        for line in &new_lines {
            if line.line_type == LineType::User || line.line_type == LineType::Assistant {
                acc.recent_messages.push_back(MessageSummary {
                    role: match line.line_type {
                        LineType::User => "user".to_string(),
                        LineType::Assistant => "assistant".to_string(),
                        _ => continue,
                    },
                    content_preview: line.content_preview.clone(),
                    tool_names: line.tool_names.clone(),
                });

                // Keep only last 5 messages
                const MAX_RECENT_MESSAGES: usize = 5;
                while acc.recent_messages.len() > MAX_RECENT_MESSAGES {
                    acc.recent_messages.pop_front();
                }
            }
        }

        // Keep the last line for status derivation
        if let Some(last) = new_lines.last() {
            acc.last_line = Some(last.clone());
        }

        // Derive status
        let processes = self.processes.read().await;
        let (running, pid) = has_running_process(&processes, &project_path);
        let status = derive_status(acc.last_line.as_ref(), seconds_since, running);

        // Capture before handle_transitions mutates it
        let is_first_poll = acc.last_status.is_none();

        // Side effects only (task time, completion, sub-agent cleanup)
        handle_transitions(&status, acc, last_activity_at);

        // Derive agent state from current evidence (always runs, no transition gating)
        acc.agent_state = derive_agent_state(
            &status,
            acc.last_line.as_ref(),
            &acc.recent_messages,
            &self.classifier,
            running,
            seconds_since,
            acc.user_turn_count,
            is_first_poll,
        );

        // Derive activity
        let tool_names = acc
            .last_line
            .as_ref()
            .map(|l| l.tool_names.as_slice())
            .unwrap_or(&[]);
        let is_streaming = status == SessionStatus::Working
            && acc.last_line.as_ref().map_or(false, |l| {
                l.tool_names.is_empty() && l.stop_reason.as_deref() != Some("end_turn")
            });
        let current_activity = derive_activity(tool_names, is_streaming);

        // Calculate cost
        let cost = calculate_live_cost(
            &acc.tokens,
            acc.model.as_deref(),
            &self.pricing,
        );

        // Derive cache status from time since last activity
        let cache_status = if seconds_since < 300 {
            derive_cache_status(Some(seconds_since))
        } else {
            derive_cache_status(Some(seconds_since))
        };

        let file_path_str = path
            .to_str()
            .unwrap_or("")
            .to_string();

        // Build LiveSession while accumulators lock is still held (reads ~15 fields from acc).
        // Made `mut` so we can overwrite agent_state with the resolved value after dropping locks.
        let mut live_session = LiveSession {
            id: session_id.clone(),
            project: project.clone(),
            project_display_name,
            project_path,
            file_path: file_path_str,
            status,
            agent_state: acc.agent_state.clone(),  // Temporarily uses JSONL-derived state
            git_branch: acc.git_branch.clone(),
            pid,
            title: acc.first_user_message.clone(),
            last_user_message: acc.last_user_message.clone(),
            current_activity,
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
        };

        // Drop the accumulators lock before acquiring sessions lock
        drop(processes);
        drop(accumulators);

        // When JSONL shows Working, clear stale hook states (e.g. awaiting_input
        // from a prior turn). The user has responded and Claude is active again.
        if live_session.status == SessionStatus::Working {
            self.state_resolver.clear_hook_state(&session_id).await;
        }

        // Feed JSONL state to resolver, then resolve (hook wins if fresh).
        // These calls only acquire StateResolver's internal locks, safe without external locks.
        self.state_resolver.update_from_jsonl(&session_id, live_session.agent_state.clone()).await;
        let resolved_state = self.state_resolver.resolve(&session_id).await;
        live_session.agent_state = resolved_state;  // Overwrite with resolved state

        // Update the shared session map
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, live_session);
    }

    // NOTE: Tier 2 AI classification (spawn_ai_classification) was removed.
    // It spawned unbounded `claude -p` processes on startup (40+ sessions discovered
    // simultaneously). Re-add with a Semaphore(1) rate limiter when needed.
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
/// Returns `(encoded_project_name, display_name, decoded_project_path)`.
///
/// The encoded project directory name uses URL-encoding where path separators
/// are percent-encoded. The display name is the last component of the decoded
/// path.
fn extract_project_info(path: &Path) -> (String, String, String) {
    let project_encoded = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Decode the URL-encoded project directory name to get the real path
    let project_path = urlencoding::decode(&project_encoded)
        .unwrap_or_else(|_| project_encoded.clone().into())
        .to_string();

    // The display name is the last path component of the decoded path
    let project_display_name = project_path
        .rsplit('/')
        .next()
        .unwrap_or(&project_path)
        .to_string();

    (project_encoded, project_display_name, project_path)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_extract_project_info_simple() {
        let path = PathBuf::from(
            "/home/user/.claude/projects/my-project/session.jsonl",
        );
        let (encoded, display, decoded) = extract_project_info(&path);
        assert_eq!(encoded, "my-project");
        assert_eq!(display, "my-project");
        assert_eq!(decoded, "my-project");
    }

    #[test]
    fn test_extract_project_info_url_encoded() {
        let path = PathBuf::from(
            "/home/user/.claude/projects/%2FUsers%2Ftest%2Fmy-project/session.jsonl",
        );
        let (encoded, display, decoded) = extract_project_info(&path);
        assert_eq!(encoded, "%2FUsers%2Ftest%2Fmy-project");
        assert_eq!(display, "my-project");
        assert_eq!(decoded, "/Users/test/my-project");
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

    // =========================================================================
    // derive_agent_state tests
    // =========================================================================

    /// Helper to create a LiveLine for agent state derivation tests.
    fn make_test_line(
        line_type: LineType,
        tool_names: Vec<String>,
        stop_reason: Option<&str>,
        is_tool_result: bool,
    ) -> LiveLine {
        LiveLine {
            line_type,
            role: None,
            content_preview: String::new(),
            tool_names,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            timestamp: None,
            stop_reason: stop_reason.map(String::from),
            git_branch: None,
            is_meta: false,
            is_tool_result_continuation: is_tool_result,
            has_system_prefix: false,
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
            sub_agent_progress: None,
        }
    }

    #[test]
    fn test_derive_agent_state_working_is_autonomous() {
        let classifier = SessionStateClassifier::new();
        let state = derive_agent_state(
            &SessionStatus::Working,
            None,
            &VecDeque::new(),
            &classifier,
            true, 5, 3, false,
        );
        assert_eq!(state.group, AgentStateGroup::Autonomous);
        assert_eq!(state.state, "acting");
    }

    #[test]
    fn test_derive_agent_state_done_is_needs_you() {
        let classifier = SessionStateClassifier::new();
        let state = derive_agent_state(
            &SessionStatus::Done,
            None,
            &VecDeque::new(),
            &classifier,
            false, 400, 3, false,
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "session_ended");
    }

    #[test]
    fn test_derive_agent_state_paused_end_turn_is_needs_you() {
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &VecDeque::new(),
            &classifier,
            true, 5, 5, false,
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou,
            "end_turn assistant line should always produce NeedsYou");
    }

    #[test]
    fn test_derive_agent_state_tool_result_midwork_autonomous() {
        // tool_result with process running = between steps = Autonomous (correct for intermediate state)
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(LineType::User, vec![], None, true);
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &VecDeque::new(),
            &classifier,
            true, 5, 5, false,
        );
        assert_eq!(state.group, AgentStateGroup::Autonomous,
            "tool_result with process running should be Autonomous (between steps)");
    }

    #[test]
    fn test_derive_agent_state_midwork_stale_120s_is_needs_you() {
        // MidWork + process running but >120s idle = stale, should be NeedsYou
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(LineType::User, vec![], None, true);
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &VecDeque::new(),
            &classifier,
            true, 130, 5, false,
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou,
            "MidWork >120s should force NeedsYou even with process running");
    }

    /// THE critical regression test: simulates the race condition.
    /// tool_result arrives first (Autonomous), then end_turn (should flip to NeedsYou).
    #[test]
    fn test_derive_agent_state_race_condition_tool_result_then_end_turn() {
        let classifier = SessionStateClassifier::new();

        // Step 1: tool_result line → should be Autonomous (between steps)
        let tool_result_line = make_test_line(LineType::User, vec![], None, true);
        let state1 = derive_agent_state(
            &SessionStatus::Paused,
            Some(&tool_result_line),
            &VecDeque::new(),
            &classifier,
            true, 5, 5, false,
        );
        assert_eq!(state1.group, AgentStateGroup::Autonomous,
            "Intermediate: tool_result with process should be Autonomous");

        // Step 2: end_turn line → MUST be NeedsYou (the fix!)
        let end_turn_line = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
        let state2 = derive_agent_state(
            &SessionStatus::Paused,
            Some(&end_turn_line),
            &VecDeque::new(),
            &classifier,
            true, 5, 5, false,
        );
        assert_eq!(state2.group, AgentStateGroup::NeedsYou,
            "REGRESSION: end_turn must produce NeedsYou regardless of previous state");
    }

    #[test]
    fn test_derive_agent_state_first_poll_no_process_is_needs_you() {
        // On first discovery, MidWork without confirmed process → NeedsYou
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(LineType::User, vec![], None, true);
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &VecDeque::new(),
            &classifier,
            false, 5, 5, true, // is_first_poll = true, no process
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou,
            "First poll without process should not keep Autonomous");
    }

    #[test]
    fn test_derive_agent_state_ask_user_question_is_needs_you() {
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(
            LineType::Assistant,
            vec!["AskUserQuestion".to_string()],
            Some("end_turn"),
            false,
        );
        let mut recent = VecDeque::new();
        recent.push_back(MessageSummary {
            role: "assistant".to_string(),
            content_preview: "Which option?".to_string(),
            tool_names: vec!["AskUserQuestion".to_string()],
        });
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &recent,
            &classifier,
            true, 5, 5, false,
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "awaiting_input");
    }

    #[test]
    fn test_derive_agent_state_single_turn_end_turn_is_task_complete() {
        // Structural classifier single-turn Q&A path: turn_count ≤ 2 + end_turn + assistant message
        let classifier = SessionStateClassifier::new();
        let last = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
        let mut recent = VecDeque::new();
        recent.push_back(MessageSummary {
            role: "assistant".to_string(),
            content_preview: "The answer is 42.".to_string(),
            tool_names: vec![],
        });
        let state = derive_agent_state(
            &SessionStatus::Paused,
            Some(&last),
            &recent,
            &classifier,
            true, 5, 1, false, // turn_count = 1 → triggers single-turn Q&A structural match
        );
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "task_complete",
            "Single-turn Q&A with end_turn should hit structural classifier → task_complete");
    }

    // =========================================================================
    // handle_transitions tests
    // =========================================================================

    #[test]
    fn test_handle_transitions_working_to_paused_computes_task_time() {
        let mut acc = SessionAccumulator::new();
        acc.last_status = Some(SessionStatus::Working);
        acc.current_turn_started_at = Some(1000);

        handle_transitions(&SessionStatus::Paused, &mut acc, 1033);

        assert_eq!(acc.last_turn_task_seconds, Some(33),
            "Working→Paused should compute task time as last_activity_at - turn_start");
        assert_eq!(acc.last_status, Some(SessionStatus::Paused));
    }

    #[test]
    fn test_handle_transitions_to_working_clears_task_time() {
        let mut acc = SessionAccumulator::new();
        acc.last_turn_task_seconds = Some(42);

        handle_transitions(&SessionStatus::Working, &mut acc, 0);

        assert_eq!(acc.last_turn_task_seconds, None,
            "Entering Working should clear task time");
    }

    #[test]
    fn test_handle_transitions_to_done_sets_completed_at() {
        let mut acc = SessionAccumulator::new();
        acc.last_status = Some(SessionStatus::Paused);

        handle_transitions(&SessionStatus::Done, &mut acc, 0);

        assert!(acc.completed_at.is_some());
        assert_eq!(acc.last_status, Some(SessionStatus::Done));
    }

    #[test]
    fn test_handle_transitions_done_cleans_up_running_subagents() {
        let mut acc = SessionAccumulator::new();
        acc.sub_agents.push(SubAgentInfo {
            tool_use_id: "toolu_1".into(),
            agent_id: None,
            agent_type: "Explore".into(),
            description: "test".into(),
            status: SubAgentStatus::Running,
            started_at: 1000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            cost_usd: None,
            current_activity: None,
        });

        handle_transitions(&SessionStatus::Done, &mut acc, 0);

        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Error,
            "Running sub-agents should be marked Error on session Done");
    }

    #[test]
    fn test_handle_transitions_paused_to_paused_no_task_time_change() {
        let mut acc = SessionAccumulator::new();
        acc.last_status = Some(SessionStatus::Paused);
        acc.last_turn_task_seconds = Some(33);

        handle_transitions(&SessionStatus::Paused, &mut acc, 2000);

        assert_eq!(acc.last_turn_task_seconds, Some(33),
            "Paused→Paused should NOT recompute task time");
    }

    #[test]
    fn test_handle_transitions_first_discovery_as_paused_computes_task_time() {
        // First discovery (last_status = None) as Paused should still compute task time
        let mut acc = SessionAccumulator::new();
        // last_status is None (first discovery)
        acc.current_turn_started_at = Some(500);

        handle_transitions(&SessionStatus::Paused, &mut acc, 555);

        assert_eq!(acc.last_turn_task_seconds, Some(55),
            "First discovery as Paused (old_status.is_none()) should compute task time");
        assert_eq!(acc.last_status, Some(SessionStatus::Paused));
    }
}
