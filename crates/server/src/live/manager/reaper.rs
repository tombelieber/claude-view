//! Unified session reaper — single function that cleans ALL maps on session death.
//!
//! Every death path (kqueue, polling, eviction) converges here.
//! No partial cleanup. No "recently closed" zombie state.

use std::sync::Arc;

use tracing::info;

use crate::live::state::SessionEvent;

use super::LiveSessionManager;

/// Result of a reap attempt.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReapResult {
    /// Session was found and reaped from all maps.
    Reaped,
    /// Session not found in the live map (already reaped or never existed).
    NotFound,
    /// Session is still alive — refused to reap.
    StillAlive,
}

impl LiveSessionManager {
    /// Reap a dead session from ALL shared maps, broadcast removal, and save snapshot.
    ///
    /// This is the ONLY function that removes sessions from the live map during
    /// normal operation. Every death detection path (kqueue, polling, eviction)
    /// MUST call this instead of doing inline cleanup.
    ///
    /// Cleans: live_sessions, transcript_to_session, hook_event_channels,
    ///         accumulators, and signals drain_loop to drop its dirty entry.
    pub(crate) async fn reap_session(self: &Arc<Self>, session_id: &str) -> ReapResult {
        // Phase 1: Remove from live_sessions map, capture cleanup data.
        let cleanup_data = {
            let mut sessions = self.sessions.write().await;
            let Some(session) = sessions.get(session_id) else {
                return ReapResult::NotFound;
            };

            // Safety: refuse to reap a session whose PID is still alive.
            if let Some(pid) = session.hook.pid {
                if crate::live::process::is_pid_alive(pid) {
                    return ReapResult::StillAlive;
                }
            }

            let transcript_path = if !session.jsonl.file_path.is_empty() {
                Some(std::path::PathBuf::from(&session.jsonl.file_path))
            } else {
                session
                    .statusline
                    .statusline_transcript_path
                    .get()
                    .map(std::path::PathBuf::from)
            };

            sessions.remove(session_id);
            transcript_path
        };

        // Phase 2: Clean auxiliary maps (no sessions lock held).

        // 2a: transcript_to_session dedup map
        if let Some(ref tp) = cleanup_data {
            let mut tmap = self.transcript_to_session.write().await;
            tmap.remove(tp);
        }

        // 2b: hook_event_channels
        {
            let mut channels = self.hook_event_channels.write().await;
            channels.remove(session_id);
        }

        // 2c: accumulators
        self.remove_accumulator(session_id).await;

        // Phase 3: Broadcast removal to frontend via SSE.
        let _ = self.tx.send(SessionEvent::SessionCompleted {
            session_id: session_id.to_string(),
        });

        // Phase 4: Persist clean snapshot.
        self.request_snapshot_save();

        info!(session_id = %session_id, "Session reaped from all maps");

        ReapResult::Reaped
    }

    /// Reap multiple sessions. Returns count of successfully reaped sessions.
    pub(crate) async fn reap_sessions(self: &Arc<Self>, session_ids: &[String]) -> u32 {
        let mut count = 0u32;
        for id in session_ids {
            if matches!(self.reap_session(id).await, ReapResult::Reaped) {
                count += 1;
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::{Arc, RwLock as StdRwLock};

    use tokio::sync::{broadcast, mpsc, RwLock};

    use claude_view_core::live_parser::TailFinders;
    use claude_view_core::phase::scheduler::Priority;
    use claude_view_core::pricing::ModelPricing;

    use crate::live::coordinator::SessionCoordinator;
    use crate::live::manager::accumulator::SessionAccumulator;
    use crate::live::manager::LiveSessionManager;
    use crate::live::process_oracle;
    use crate::live::state::{
        AgentState, AgentStateGroup, HookEvent, HookFields, JsonlFields, LiveSession, SessionEvent,
        SessionStatus, StatuslineFields,
    };

    use super::ReapResult;

    /// Create a minimal LiveSessionManager for unit tests.
    ///
    /// Only initialises the fields that `reap_session` touches:
    /// sessions, transcript_to_session, hook_event_channels, accumulators,
    /// tx (broadcast), snapshot_tx.
    async fn make_test_manager() -> (
        Arc<LiveSessionManager>,
        broadcast::Receiver<SessionEvent>,
        mpsc::Receiver<()>,
    ) {
        let (tx, rx) = broadcast::channel(16);
        let (snapshot_tx, snapshot_rx) = mpsc::channel::<()>(4);
        let (dirty_tx, _dirty_rx) = mpsc::channel::<(String, Priority)>(16);
        let (death_watcher, _death_rx) = crate::live::process_death::ProcessDeathWatcher::start();
        let db = claude_view_db::Database::new_in_memory().await.unwrap();

        let manager = Arc::new(LiveSessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            tx,
            finders: Arc::new(TailFinders::new()),
            accumulators: Arc::new(RwLock::new(HashMap::new())),
            process_count: Arc::new(AtomicU32::new(0)),
            pricing: Arc::new(HashMap::<String, ModelPricing>::new()),
            db,
            search_index: Arc::new(StdRwLock::new(None)),
            registry: Arc::new(StdRwLock::new(None)),
            snapshot_tx,
            sidecar: None,
            teams: Arc::new(crate::teams::TeamsStore::empty()),
            transcript_to_session: Arc::new(RwLock::new(HashMap::new())),
            oracle_rx: process_oracle::stub(),
            _death_watcher: death_watcher,
            dirty_tx,
            hook_event_channels: Arc::new(RwLock::new(HashMap::new())),
            coordinator: Arc::new(SessionCoordinator::new()),
        });

        (manager, rx, snapshot_rx)
    }

    /// Create a minimal LiveSession with a dead PID for reaper tests.
    fn make_dead_session(id: &str, pid: u32, transcript_path: &str) -> LiveSession {
        LiveSession {
            id: id.to_string(),
            status: SessionStatus::Working,
            started_at: None,
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
                pid: Some(pid),
                title: "Test session".into(),
                last_user_message: String::new(),
                current_activity: "Working".into(),
                turn_count: 1,
                last_activity_at: 100,
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
                file_path: transcript_path.to_string(),
                project: "test".to_string(),
                project_display_name: "test".to_string(),
                project_path: "/tmp/test".to_string(),
                ..JsonlFields::default()
            },
        }
    }

    // A PID that is almost certainly dead (used as stand-in for a reaped session).
    const DEAD_PID: u32 = 99998;

    // -----------------------------------------------------------------------
    // Core reap behaviour
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_reap_removes_from_all_maps() {
        let (mgr, _rx, _snap_rx) = make_test_manager().await;
        let session_id = "sess-dead-1";
        let transcript = "/tmp/test.jsonl";

        // Populate all 5 maps.
        let session = make_dead_session(session_id, DEAD_PID, transcript);
        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);
        mgr.transcript_to_session
            .write()
            .await
            .insert(PathBuf::from(transcript), session_id.to_string());
        mgr.hook_event_channels
            .write()
            .await
            .insert(session_id.to_string(), broadcast::channel::<HookEvent>(4).0);
        mgr.accumulators
            .write()
            .await
            .insert(session_id.to_string(), SessionAccumulator::new());

        // Reap.
        let result = mgr.reap_session(session_id).await;
        assert_eq!(result, ReapResult::Reaped, "should reap a dead session");

        // Verify ALL maps are clean.
        assert!(
            !mgr.sessions.read().await.contains_key(session_id),
            "sessions map should not contain reaped session"
        );
        assert!(
            !mgr.transcript_to_session
                .read()
                .await
                .contains_key(&PathBuf::from(transcript)),
            "transcript_to_session should not contain reaped path"
        );
        assert!(
            !mgr.hook_event_channels
                .read()
                .await
                .contains_key(session_id),
            "hook_event_channels should not contain reaped session"
        );
        assert!(
            !mgr.accumulators.read().await.contains_key(session_id),
            "accumulators should not contain reaped session"
        );
    }

    #[tokio::test]
    async fn test_reap_returns_not_found_for_missing_session() {
        let (mgr, _rx, _snap_rx) = make_test_manager().await;
        let result = mgr.reap_session("no-such-session").await;
        assert_eq!(result, ReapResult::NotFound);
    }

    #[tokio::test]
    async fn test_reap_refuses_alive_pid() {
        let (mgr, _rx, _snap_rx) = make_test_manager().await;
        let session_id = "sess-alive";

        // Use the current process PID — guaranteed alive.
        let alive_pid = std::process::id();
        let session = make_dead_session(session_id, alive_pid, "/tmp/alive.jsonl");
        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        let result = mgr.reap_session(session_id).await;
        assert_eq!(
            result,
            ReapResult::StillAlive,
            "must refuse to reap a session with live PID"
        );

        // Session should still be in the map.
        assert!(mgr.sessions.read().await.contains_key(session_id));
    }

    #[tokio::test]
    async fn test_reap_broadcasts_session_completed() {
        let (mgr, mut rx, _snap_rx) = make_test_manager().await;
        let session_id = "sess-broadcast";

        let session = make_dead_session(session_id, DEAD_PID, "/tmp/broadcast.jsonl");
        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        mgr.reap_session(session_id).await;

        // Should have received a SessionCompleted event.
        let event = rx
            .try_recv()
            .expect("expected a broadcast event after reap");
        match event {
            SessionEvent::SessionCompleted { session_id: id } => {
                assert_eq!(id, session_id);
            }
            other => panic!("expected SessionCompleted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reap_requests_snapshot_save() {
        let (mgr, _rx, mut snap_rx) = make_test_manager().await;
        let session_id = "sess-snap";

        let session = make_dead_session(session_id, DEAD_PID, "/tmp/snap.jsonl");
        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        mgr.reap_session(session_id).await;

        // Snapshot channel should have a message.
        let msg = snap_rx.try_recv();
        assert!(msg.is_ok(), "expected snapshot save request after reap");
    }

    #[tokio::test]
    async fn test_reap_sessions_batch_returns_count() {
        let (mgr, _rx, _snap_rx) = make_test_manager().await;

        // Insert 3 dead sessions.
        for i in 0..3 {
            let id = format!("batch-{}", i);
            let session =
                make_dead_session(&id, DEAD_PID + i as u32, &format!("/tmp/b{}.jsonl", i));
            mgr.sessions.write().await.insert(id, session);
        }

        let ids: Vec<String> = (0..3).map(|i| format!("batch-{}", i)).collect();
        let count = mgr.reap_sessions(&ids).await;
        assert_eq!(count, 3, "should have reaped all 3 sessions");
        assert!(
            mgr.sessions.read().await.is_empty(),
            "all sessions should be gone"
        );
    }
}
