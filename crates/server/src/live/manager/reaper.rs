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
        // Phase 1: Remove from live_sessions, capture for closed_ring.
        let (cleanup_tp, closed_session) = {
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

            // Capture last-known state for ephemeral recently-closed display.
            let mut closed = session.clone();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            closed.status = crate::live::state::SessionStatus::Done;
            closed.closed_at = Some(now);

            // Mark orphaned Running subagents as Error — parent is dead,
            // they can't report back. Without this, the UI shows a green
            // "running" dot on sessions that have long ended.
            crate::live::mutation::apply_lifecycle::finalize_orphaned_subagents(
                &mut closed.hook.sub_agents,
                now,
            );

            sessions.remove(session_id);
            (transcript_path, closed)
        };

        // Phase 1b: Push to bounded ring buffer (FIFO).
        {
            let mut ring = self.closed_ring.write().await;
            if ring.len() >= crate::live::state::CLOSED_RING_CAPACITY {
                ring.pop_front(); // evict oldest
            }
            ring.push_back(closed_session.clone());
        }

        // Phase 2: Clean auxiliary maps (no sessions lock held).
        if let Some(ref tp) = cleanup_tp {
            self.transcript_to_session.write().await.remove(tp);
        }
        self.hook_event_channels.write().await.remove(session_id);
        self.remove_accumulator(session_id).await;

        // Phase 3: Broadcast to frontend — session moved to recently closed.
        let _ = self.tx.send(SessionEvent::SessionRemove {
            session_id: session_id.to_string(),
            session: closed_session,
        });

        // Phase 4: Persist clean snapshot (closed_ring NOT included).
        self.request_snapshot_save();

        info!(session_id = %session_id, "Session reaped → recently closed");

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

    use crate::live::drain_loop::DirtySignal;
    use claude_view_core::live_parser::TailFinders;
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
        let (dirty_tx, _dirty_rx) = mpsc::channel::<DirtySignal>(16);
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
            claude_dir: std::path::PathBuf::from("/tmp/test-claude"),
            claude_view_dir: std::path::PathBuf::from("/tmp/test-claude-view"),
            transcript_to_session: Arc::new(RwLock::new(HashMap::new())),
            oracle_rx: process_oracle::stub(),
            _death_watcher: death_watcher,
            _sessions_watcher: None,
            dirty_tx,
            hook_event_channels: Arc::new(RwLock::new(HashMap::new())),
            coordinator: Arc::new(SessionCoordinator::new()),
            closed_ring: Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(
                crate::live::state::CLOSED_RING_CAPACITY,
            ))),
            cli_sessions: Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new()),
            interaction_data: Arc::new(RwLock::new(HashMap::new())),
            backfill_miss_count: std::sync::atomic::AtomicU64::new(0),
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
            session_kind: None,
            entrypoint: None,
            ownership: None,
            pending_interaction: None,
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
    async fn test_reap_broadcasts_session_closed() {
        let (mgr, mut rx, _snap_rx) = make_test_manager().await;
        let session_id = "sess-broadcast";

        let session = make_dead_session(session_id, DEAD_PID, "/tmp/broadcast.jsonl");
        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        mgr.reap_session(session_id).await;

        // Should have received a SessionRemove event (moved to recently closed).
        let event = rx
            .try_recv()
            .expect("expected a broadcast event after reap");
        match event {
            SessionEvent::SessionRemove {
                session_id: sid,
                session,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(session.id, session_id);
                assert_eq!(session.status, SessionStatus::Done);
                assert!(session.closed_at.is_some());
            }
            other => panic!("expected SessionRemove, got {:?}", other),
        }

        // Should also be in the closed_ring buffer.
        assert!(
            mgr.closed_ring
                .read()
                .await
                .iter()
                .any(|s| s.id == session_id),
            "reaped session should be in closed_ring"
        );
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

    // -----------------------------------------------------------------------
    // Ring buffer eviction
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_ring_buffer_evicts_oldest_at_capacity() {
        let (mgr, mut _rx, _snap_rx) = make_test_manager().await;
        let cap = crate::live::state::CLOSED_RING_CAPACITY;

        // Fill ring to capacity by reaping `cap` sessions.
        for i in 0..cap {
            let id = format!("ring-{i}");
            let session = make_dead_session(&id, DEAD_PID + i as u32, &format!("/tmp/r{i}.jsonl"));
            mgr.sessions.write().await.insert(id.clone(), session);
            mgr.reap_session(&id).await;
        }
        assert_eq!(mgr.closed_ring.read().await.len(), cap);

        // Reap one more — should evict the oldest (ring-0).
        let overflow_id = "ring-overflow";
        let session = make_dead_session(overflow_id, DEAD_PID + cap as u32, "/tmp/overflow.jsonl");
        mgr.sessions
            .write()
            .await
            .insert(overflow_id.to_string(), session);
        mgr.reap_session(overflow_id).await;

        let ring = mgr.closed_ring.read().await;
        assert_eq!(ring.len(), cap, "ring should stay at capacity");
        assert!(
            !ring.iter().any(|s| s.id == "ring-0"),
            "oldest entry (ring-0) should have been evicted"
        );
        assert!(
            ring.iter().any(|s| s.id == overflow_id),
            "newest entry should be present"
        );
        // Second entry should still be there.
        assert!(
            ring.iter().any(|s| s.id == "ring-1"),
            "ring-1 should survive (only oldest evicted)"
        );
    }

    // -----------------------------------------------------------------------
    // Subagent status marking on reap
    // -----------------------------------------------------------------------

    /// When a session is reaped, any sub-agents with status Running must be
    /// marked as Error in the closed_ring copy. The parent is dead — they
    /// can never report back, so showing "running" is wrong.
    #[tokio::test]
    async fn test_reap_marks_running_subagents_as_error() {
        use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};

        let (mgr, mut rx, _snap_rx) = make_test_manager().await;
        let session_id = "sess-with-subagents";

        let mut session = make_dead_session(session_id, DEAD_PID, "/tmp/subagents.jsonl");

        // Add 3 subagents: 1 Running, 1 Complete, 1 Running with activity.
        session.hook.sub_agents = vec![
            SubAgentInfo {
                tool_use_id: "toolu_run1".to_string(),
                agent_id: Some("agent1".to_string()),
                agent_type: "Explore".to_string(),
                description: "Still running agent".to_string(),
                status: SubAgentStatus::Running,
                started_at: 1000,
                completed_at: None,
                duration_ms: None,
                tool_use_count: None,
                model: None,
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                cost_usd: None,
                current_activity: Some("Read".to_string()),
                error_reason: None,
            },
            SubAgentInfo {
                tool_use_id: "toolu_done".to_string(),
                agent_id: Some("agent2".to_string()),
                agent_type: "Edit".to_string(),
                description: "Completed agent".to_string(),
                status: SubAgentStatus::Complete,
                started_at: 1000,
                completed_at: Some(1050),
                duration_ms: Some(50000),
                tool_use_count: Some(10),
                model: Some("haiku".to_string()),
                input_tokens: Some(500),
                output_tokens: Some(200),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                cost_usd: Some(0.001),
                current_activity: None,
                error_reason: None,
            },
            SubAgentInfo {
                tool_use_id: "toolu_run2".to_string(),
                agent_id: None, // Running agents may not have agent_id yet
                agent_type: "Search".to_string(),
                description: "Another running agent".to_string(),
                status: SubAgentStatus::Running,
                started_at: 1010,
                completed_at: None,
                duration_ms: None,
                tool_use_count: None,
                model: None,
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                cost_usd: None,
                current_activity: Some("Grep".to_string()),
                error_reason: None,
            },
        ];

        mgr.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);

        // Reap the session.
        let result = mgr.reap_session(session_id).await;
        assert_eq!(result, ReapResult::Reaped);

        // Check closed_ring: Running agents should now be Error.
        let ring = mgr.closed_ring.read().await;
        let closed = ring
            .iter()
            .find(|s| s.id == session_id)
            .expect("reaped session should be in closed_ring");

        assert_eq!(closed.hook.sub_agents.len(), 3);

        // Agent 0: was Running → should be Error, current_activity cleared, completed_at set.
        assert_eq!(
            closed.hook.sub_agents[0].status,
            SubAgentStatus::Error,
            "Running subagent should be marked Error after reap"
        );
        assert_eq!(
            closed.hook.sub_agents[0].current_activity, None,
            "current_activity should be cleared for errored subagent"
        );
        assert!(
            closed.hook.sub_agents[0].completed_at.is_some(),
            "completed_at should be set for orphaned subagent"
        );

        // Agent 1: was Complete → should remain Complete (untouched).
        assert_eq!(
            closed.hook.sub_agents[1].status,
            SubAgentStatus::Complete,
            "Complete subagent should remain Complete after reap"
        );
        assert_eq!(
            closed.hook.sub_agents[1].cost_usd,
            Some(0.001),
            "Complete subagent data should be preserved"
        );
        assert_eq!(
            closed.hook.sub_agents[1].completed_at,
            Some(1050),
            "Complete subagent completed_at should be preserved"
        );

        // Agent 2: was Running → should be Error, current_activity cleared, completed_at set.
        assert_eq!(
            closed.hook.sub_agents[2].status,
            SubAgentStatus::Error,
            "Second running subagent should be marked Error after reap"
        );
        assert_eq!(
            closed.hook.sub_agents[2].current_activity, None,
            "current_activity should be cleared for second errored subagent"
        );
        assert!(
            closed.hook.sub_agents[2].completed_at.is_some(),
            "completed_at should be set for second orphaned subagent"
        );

        // Also verify the broadcast event carries the corrected statuses.
        let event = rx
            .try_recv()
            .expect("expected a broadcast event after reap");
        match event {
            SessionEvent::SessionRemove { session, .. } => {
                assert_eq!(session.hook.sub_agents[0].status, SubAgentStatus::Error);
                assert_eq!(session.hook.sub_agents[1].status, SubAgentStatus::Complete);
                assert_eq!(session.hook.sub_agents[2].status, SubAgentStatus::Error);
            }
            other => panic!("expected SessionRemove, got {:?}", other),
        }
    }
}
