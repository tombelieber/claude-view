//! Central orchestrator for live session monitoring.
//!
//! The `LiveSessionManager` ties together the file watcher, process detector,
//! JSONL tail parser, and cleanup task to maintain an in-memory map of all
//! active Claude Code sessions.
//!
//! ## Module layout
//!
//! - `accumulator` -- SessionAccumulator, JsonlMetadata, build/apply helpers
//! - `helpers` -- path extraction, timestamp parsing, snapshot I/O, hook event construction
//! - `watcher` -- spawn_file_watcher, process_jsonl_update, startup recovery
//! - `reconciler` -- spawn_reconciliation_loop, cleanup, death consumer

pub(crate) mod accumulator;
mod helpers;
mod line_processor;
mod reconciler;
mod startup;
mod watcher;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::info;

use claude_view_core::live_parser::TailFinders;
use claude_view_core::phase::client::OmlxClient;
use claude_view_core::phase::scheduler::{ClassifyResult, Priority};
use claude_view_core::phase::{
    dominant_phase, PhaseHistory, PhaseLabel, SessionPhase, MAX_PHASE_LABELS,
};
use claude_view_core::pricing::ModelPricing;

use claude_view_db::Database;

use super::coordinator::{MutationContext, SessionCoordinator};
use super::state::{HookEvent, LiveSession, SessionEvent, SessionStatus, SnapshotEntry};

use accumulator::{apply_jsonl_metadata, build_metadata_from_accumulator, SessionAccumulator};
use helpers::{extract_project_info, pid_snapshot_path, save_session_snapshot};

/// Type alias for the shared session map used by both the manager and route handlers.
pub type LiveSessionMap = Arc<RwLock<HashMap<String, LiveSession>>>;

/// Type alias for the transcript path -> session ID dedup map.
/// Shared between `LiveSessionManager` (cleanup on PID death) and `AppState` (statusline handler).
pub type TranscriptMap = Arc<RwLock<HashMap<PathBuf, String>>>;

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
    /// Total number of Claude processes detected.
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
    /// Transcript path -> session ID dedup map, shared with AppState.
    transcript_to_session: TranscriptMap,
    /// Unified process oracle receiver for reading process data.
    oracle_rx: super::process_oracle::OracleReceiver,
    /// Event-driven process death watcher (kqueue on macOS).
    _death_watcher: super::process_death::ProcessDeathWatcher,
    /// Channel to mark sessions dirty for the drain loop classifier.
    dirty_tx: mpsc::Sender<(String, Priority)>,
    /// Per-session broadcast channels for hook events (WebSocket streaming).
    hook_event_channels: Arc<tokio::sync::RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
    /// Shared coordinator for routing mutations through the 4-phase pipeline.
    coordinator: Arc<SessionCoordinator>,
}

impl LiveSessionManager {
    /// Start the live session manager and all background tasks.
    ///
    /// Returns the manager, a shared session map, the transcript dedup map,
    /// and the broadcast sender for SSE event streaming.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        pricing: Arc<HashMap<String, ModelPricing>>,
        db: Database,
        search_index: Arc<StdRwLock<Option<Arc<claude_view_search::SearchIndex>>>>,
        registry: Arc<StdRwLock<Option<claude_view_core::Registry>>>,
        sidecar: Option<Arc<crate::sidecar::SidecarManager>>,
        teams: Arc<crate::teams::TeamsStore>,
        omlx_status: Arc<super::omlx_lifecycle::OmlxStatus>,
        oracle_rx: super::process_oracle::OracleReceiver,
        hook_event_channels: Arc<
            tokio::sync::RwLock<HashMap<String, broadcast::Sender<HookEvent>>>,
        >,
        debug_omlx_tx: Option<mpsc::Sender<String>>,
    ) -> (
        Arc<Self>,
        LiveSessionMap,
        TranscriptMap,
        broadcast::Sender<SessionEvent>,
        Arc<SessionCoordinator>,
    ) {
        let (tx, _rx) = broadcast::channel(256);
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let transcript_to_session: TranscriptMap = Arc::new(RwLock::new(HashMap::new()));

        // Debounced snapshot writer channel
        let (snapshot_tx, snapshot_rx) = mpsc::channel::<()>(1);

        // Start event-driven process death watcher (kqueue on macOS)
        let (death_watcher, death_rx) = super::process_death::ProcessDeathWatcher::start();

        // oMLX phase classifier infrastructure (omlx_status injected from caller)
        let (dirty_tx, dirty_rx) = mpsc::channel::<(String, Priority)>(256);
        let (result_tx, mut result_rx) = mpsc::channel::<ClassifyResult>(64);

        // Create shared coordinator
        let coordinator = Arc::new(SessionCoordinator::new());

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
            dirty_tx,
            hook_event_channels,
            coordinator: coordinator.clone(),
        });

        // Spawn background tasks
        manager.spawn_snapshot_writer(snapshot_rx);
        manager.spawn_file_watcher();
        manager.spawn_reconciliation_loop();
        manager.spawn_cleanup_task();
        manager.spawn_death_consumer(death_rx);

        // Spawn oMLX lifecycle (health check)
        tokio::spawn(super::omlx_lifecycle::run_lifecycle(omlx_status.clone()));

        // Spawn oMLX drain loop (replaces cadence-based scheduler)
        let mut omlx_client = OmlxClient::new(
            format!("http://localhost:{}", omlx_status.port),
            "Qwen3.5-4B-MLX-4bit".into(),
        )
        .with_ready_flag(omlx_status.ready.clone());
        if let Some(tx) = debug_omlx_tx {
            omlx_client = omlx_client.with_debug_tx(tx);
        }
        let client = Arc::new(omlx_client);
        let drain_wake = Arc::new(tokio::sync::Notify::new());
        tokio::spawn(super::drain_loop::run_drain_loop(
            dirty_rx,
            result_tx,
            manager.accumulators.clone(),
            client,
            omlx_status.ready.clone(),
            drain_wake,
        ));

        // Spawn classify result handler
        {
            let accumulators = manager.accumulators.clone();
            let sessions = manager.sessions.clone();
            let tx = manager.tx.clone();
            tokio::spawn(async move {
                while let Some(result) = result_rx.recv().await {
                    let session_id = result.session_id.clone();

                    // Phase 1: Update accumulator (write lock)
                    let phase_history = {
                        let mut accs = accumulators.write().await;
                        let Some(acc) = accs.get_mut(&session_id) else {
                            continue;
                        };

                        // Reject stale results from older in-flight calls
                        if result.generation < acc.last_applied_generation {
                            continue;
                        }
                        acc.last_applied_generation = result.generation;

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

                        // Build phase history from accumulator for session map write
                        PhaseHistory {
                            current: acc.phase_labels.last().cloned(),
                            dominant: dominant_phase(&acc.phase_labels),
                            labels: acc.phase_labels.clone(),
                        }
                    };
                    // accumulators lock dropped here

                    // Phase 2: Write fresh phase into session map, then broadcast
                    let mut sessions = sessions.write().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.jsonl.phase = phase_history;
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

        (manager, sessions, transcript_to_session, tx, coordinator)
    }

    /// Build a `MutationContext` from manager fields for coordinator calls.
    pub(crate) fn mutation_context<'a>(
        &'a self,
        manager_ref: &'a Arc<Self>,
    ) -> MutationContext<'a> {
        MutationContext {
            sessions: &self.sessions,
            live_tx: &self.tx,
            live_manager: Some(manager_ref),
            db: &self.db,
            transcript_to_session: &self.transcript_to_session,
            hook_event_channels: &self.hook_event_channels,
        }
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

    /// Return the JSONL file path from the accumulator, if known.
    /// Used as a fallback when transcript_path is not in the hook payload
    /// but the file watcher has already discovered the JSONL via initial_scan.
    pub async fn accumulator_file_path(&self, session_id: &str) -> Option<std::path::PathBuf> {
        let accumulators = self.accumulators.read().await;
        accumulators
            .get(session_id)
            .and_then(|a| a.file_path.clone())
    }

    /// Apply cached accumulator data to a session object that is NOT yet
    /// in the map. This is the structural guarantee: every session is
    /// enriched from its JSONL before it becomes visible to SSE clients.
    ///
    /// Called by:
    /// - coordinator (Phase 1b → Phase 2: enrich before insert)
    /// - startup recovery (promote_from_snapshot: enrich before broadcast)
    ///
    /// If no accumulator exists or it has no data, this is a no-op.
    pub async fn apply_accumulator_to_session(&self, session_id: &str, session: &mut LiveSession) {
        let accumulators = self.accumulators.read().await;
        let Some(acc) = accumulators.get(session_id) else {
            return;
        };
        if acc.offset == 0 {
            return;
        }
        let Some(ref file_path) = acc.file_path else {
            return;
        };

        let cached_cwd = acc.resolved_cwd.as_deref();
        let (project, project_display_name, project_path, _) =
            extract_project_info(file_path, cached_cwd);

        let last_activity_at = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let metadata = build_metadata_from_accumulator(acc, last_activity_at, None);
        let file_path_str = file_path.to_string_lossy().to_string();
        drop(accumulators);

        apply_jsonl_metadata(
            session,
            &metadata,
            &file_path_str,
            &project,
            &project_display_name,
            &project_path,
        );
        // Populate team data from TeamsStore
        if let Some(ref tn) = session.jsonl.team_name.clone() {
            if let Some(detail) = self.teams.get(tn) {
                session.jsonl.team_members = detail.members;
            }
            session.jsonl.team_inbox_count = self
                .teams
                .inbox(tn)
                .map(|msgs| msgs.len() as u32)
                .unwrap_or(0);
        } else {
            session.jsonl.team_members = Vec::new();
            session.jsonl.team_inbox_count = 0;
        }
    }

    /// Enrich a session that IS already in the map. Used by the file watcher
    /// event loop when a JSONL file changes and the session needs updating.
    pub async fn enrich_session_in_map(&self, session_id: &str) {
        let accumulators = self.accumulators.read().await;
        let Some(acc) = accumulators.get(session_id) else {
            return;
        };
        if acc.offset == 0 {
            return;
        }
        let Some(ref file_path) = acc.file_path else {
            return;
        };

        let cached_cwd = acc.resolved_cwd.as_deref();
        let (project, project_display_name, project_path, _) =
            extract_project_info(file_path, cached_cwd);

        let last_activity_at = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let metadata = build_metadata_from_accumulator(acc, last_activity_at, None);
        let file_path_str = file_path.to_string_lossy().to_string();
        drop(accumulators);

        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if session.closed_at.is_some() {
                return;
            }
            let hook_activity = session.hook.last_activity_at;
            apply_jsonl_metadata(
                session,
                &metadata,
                &file_path_str,
                &project,
                &project_display_name,
                &project_path,
            );
            // Populate team data from TeamsStore
            if let Some(ref tn) = session.jsonl.team_name.clone() {
                if let Some(detail) = self.teams.get(tn) {
                    session.jsonl.team_members = detail.members;
                }
                session.jsonl.team_inbox_count = self
                    .teams
                    .inbox(tn)
                    .map(|msgs| msgs.len() as u32)
                    .unwrap_or(0);
            } else {
                session.jsonl.team_members = Vec::new();
                session.jsonl.team_inbox_count = 0;
            }
            if hook_activity > session.hook.last_activity_at {
                session.hook.last_activity_at = hook_activity;
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
    pub fn request_snapshot_save(&self) {
        let _ = self.snapshot_tx.try_send(());
    }

    /// Spawn the debounced snapshot writer background task.
    fn spawn_snapshot_writer(self: &Arc<Self>, mut rx: mpsc::Receiver<()>) {
        let manager = self.clone();
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                while rx.try_recv().is_ok() {}
                manager.save_session_snapshot_from_state().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
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
            session.jsonl.source = Some(super::process::SessionSourceInfo {
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
                binding.cancel.cancel();
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
                s.hook.pid.map(|pid| {
                    (
                        id.clone(),
                        SnapshotEntry {
                            pid,
                            status: match s.status {
                                SessionStatus::Working => "working".to_string(),
                                SessionStatus::Paused => "paused".to_string(),
                                SessionStatus::Done => "done".to_string(),
                            },
                            agent_state: s.hook.agent_state.clone(),
                            last_activity_at: s.hook.last_activity_at,
                            control_id: s.control.as_ref().map(|c| c.control_id.clone()),
                        },
                    )
                })
            })
            .collect();
        if let Some(snap_path) = pid_snapshot_path() {
            save_session_snapshot(
                &snap_path,
                &super::state::SessionSnapshot {
                    version: 2,
                    sessions: entries,
                },
            );
        } else {
            tracing::error!("could not determine home directory for snapshot save");
        }
    }
}
