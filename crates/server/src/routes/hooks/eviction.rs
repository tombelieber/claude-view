//! PID-based ghost session eviction.
//!
//! One PID = one active session. When a new session starts on the same PID
//! (e.g. session resume), stale entries are immediately evicted.

use crate::live::state::{AgentState, AgentStateGroup, SessionStatus};
use crate::state::AppState;

/// Evict stale sessions that share the same PID as the new session.
///
/// PID uniqueness: one PID = one active session. When a new session starts
/// on the same PID (e.g. session resume), the old entry is immediately
/// evicted — no 10s reconciliation delay.
///
/// Ghost sessions (no JSONL, zero turns) are removed entirely.
/// Real sessions move to "recently closed".
/// Sidecar sessions are never evicted.
pub(super) async fn evict_stale_sessions_for_pid(
    state: &AppState,
    pid: u32,
    new_session_id: &str,
    now: i64,
) {
    use crate::live::state::SessionEvent;

    let mut sessions = state.live_sessions.write().await;

    // Collect eviction targets (session_id, transcript_path, is_ghost)
    let mut pid_evicted: Vec<(String, Option<std::path::PathBuf>, bool)> = Vec::new();
    for (id, session) in sessions.iter_mut() {
        if *id == new_session_id {
            continue;
        }
        if session.hook.pid != Some(pid) {
            continue;
        }
        if session.status == SessionStatus::Done {
            continue;
        }
        // Sidecar sessions: lifecycle managed by SDK, never evict
        if session.control.is_some() {
            continue;
        }
        // Same PID, different session_id -> stale. Close it.
        session.status = SessionStatus::Done;
        session.closed_at = Some(now);
        session.hook.agent_state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session ended".into(),
            context: None,
        };
        session.hook.hook_events.clear();
        let is_ghost = session.jsonl.file_path.is_empty() && session.hook.turn_count == 0;
        let tp = session
            .statusline
            .statusline_transcript_path
            .get()
            .map(std::path::PathBuf::from);
        pid_evicted.push((id.clone(), tp, is_ghost));
        tracing::info!(
            evicted_id = %id,
            new_id = %new_session_id,
            pid = pid,
            "PID uniqueness: closed stale session (same PID, new session_id)"
        );
    }

    // Ghost sessions removed entirely; real sessions stay as "recently closed"
    let mut evicted_real: Vec<crate::live::state::LiveSession> = Vec::new();
    let mut evicted_ghost_ids: Vec<String> = Vec::new();
    for (id, _, is_ghost) in &pid_evicted {
        if *is_ghost {
            sessions.remove(id);
            evicted_ghost_ids.push(id.clone());
        } else if let Some(s) = sessions.get(id) {
            evicted_real.push(s.clone());
        }
    }
    let evicted_transcript_paths: Vec<std::path::PathBuf> = pid_evicted
        .into_iter()
        .filter_map(|(_, tp, _)| tp)
        .collect();

    // Drop sessions lock before any other async work
    drop(sessions);

    // Clean transcript map for evicted sessions
    if !evicted_transcript_paths.is_empty() {
        let mut tmap = state.transcript_to_session.write().await;
        for tp in &evicted_transcript_paths {
            tmap.remove(tp);
        }
    }

    // Clean accumulators for evicted sessions
    if let Some(mgr) = &state.live_manager {
        for s in &evicted_real {
            mgr.remove_accumulator(&s.id).await;
        }
        for id in &evicted_ghost_ids {
            mgr.remove_accumulator(id).await;
        }
    }

    // Broadcast evictions
    let total_evicted = evicted_real.len() + evicted_ghost_ids.len();
    if total_evicted > 1 {
        tracing::warn!(
            count = total_evicted,
            pid = pid,
            "Multiple sessions evicted for same PID — unexpected (possible rapid PID reuse)"
        );
    }
    for evicted in &evicted_real {
        let _ = state.live_tx.send(SessionEvent::SessionClosed {
            session: evicted.clone(),
        });
    }
    for id in &evicted_ghost_ids {
        let _ = state.live_tx.send(SessionEvent::SessionCompleted {
            session_id: id.clone(),
        });
    }
}
