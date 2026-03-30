//! PID-based session eviction via the unified reaper.
//!
//! One PID = one active session. When a new session starts on the same PID
//! (e.g. session resume), stale entries are immediately reaped.

use crate::live::state::SessionStatus;
use crate::state::AppState;

/// Evict all stale sessions sharing the same PID as a new SessionStart.
///
/// Called on every SessionStart to enforce the invariant: one PID = one active session.
/// Sidecar-controlled sessions and already-Done sessions are exempt.
pub(super) async fn evict_stale_sessions_for_pid(
    state: &AppState,
    pid: u32,
    new_session_id: &str,
    _now: i64,
) {
    // Phase 1: Identify sessions to evict (read lock only).
    let evict_ids: Vec<String> = {
        let sessions = state.live_sessions.read().await;
        sessions
            .iter()
            .filter(|(id, session)| {
                *id != new_session_id
                    && session.hook.pid == Some(pid)
                    && session.status != SessionStatus::Done
                    && session.control.is_none()
            })
            .map(|(id, _)| id.clone())
            .collect()
    };

    if evict_ids.is_empty() {
        return;
    }

    // Phase 2: Reap via the unified reaper.
    if let Some(ref mgr) = state.live_manager {
        let count = mgr.reap_sessions(&evict_ids).await;
        if count > 0 {
            tracing::info!(
                pid = pid,
                new_session = %new_session_id,
                evicted = count,
                "PID eviction: reaped stale sessions"
            );
        }
    }
}
