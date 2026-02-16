//! WebSocket connection manager for Mission Control terminal monitoring.
//!
//! Tracks active WebSocket connections per session and enforces global
//! and per-session connection limits to prevent resource exhaustion.

use std::collections::HashMap;
use std::sync::RwLock;

/// Maximum total WebSocket connections across all sessions.
/// 64 accommodates a full 4x4 grid (16 sessions × 1 WS each) plus expanded
/// overlays, multiple browser tabs, and brief reconnect transients.
/// Orphaned connections are prevented by the ConnectionGuard RAII pattern
/// and 10s WebSocket Ping heartbeat in routes/terminal.rs.
const MAX_WS_CONNECTIONS: usize = 64;

/// Maximum concurrent viewers for a single session.
const MAX_VIEWERS_PER_SESSION: usize = 4;

/// Error returned when a connection limit is exceeded.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionLimitError {
    #[error("global connection limit exceeded ({MAX_WS_CONNECTIONS} max)")]
    GlobalLimitExceeded,
    #[error("per-session viewer limit exceeded ({MAX_VIEWERS_PER_SESSION} max for session '{0}')")]
    SessionLimitExceeded(String),
}

/// Manages active WebSocket connections for live terminal monitoring.
///
/// Each session can have multiple viewers (up to `MAX_VIEWERS_PER_SESSION`),
/// and the total connections across all sessions is capped at `MAX_WS_CONNECTIONS`.
/// When a session's viewer count drops to 0, its entry is removed so the
/// corresponding file watcher can be cleaned up.
pub struct TerminalConnectionManager {
    /// Map of session_id -> count of active WebSocket connections.
    active: RwLock<HashMap<String, usize>>,
}

impl TerminalConnectionManager {
    /// Create a new connection manager with no active connections.
    pub fn new() -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new WebSocket connection for the given session.
    ///
    /// Returns `Ok(())` if the connection is accepted, or an error if either
    /// the global or per-session limit would be exceeded.
    pub fn connect(&self, session_id: &str) -> Result<(), ConnectionLimitError> {
        let mut active = self.active.write().expect("RwLock poisoned");

        // Check global limit
        let total: usize = active.values().sum();
        if total >= MAX_WS_CONNECTIONS {
            return Err(ConnectionLimitError::GlobalLimitExceeded);
        }

        // Check per-session limit
        let current = active.get(session_id).copied().unwrap_or(0);
        if current >= MAX_VIEWERS_PER_SESSION {
            return Err(ConnectionLimitError::SessionLimitExceeded(
                session_id.to_string(),
            ));
        }

        *active.entry(session_id.to_string()).or_insert(0) += 1;
        Ok(())
    }

    /// Unregister a WebSocket connection for the given session.
    ///
    /// Decrements the viewer count. If the count reaches 0, the session
    /// entry is removed entirely (signaling that the file watcher can be dropped).
    pub fn disconnect(&self, session_id: &str) {
        let mut active = self.active.write().expect("RwLock poisoned");

        if let Some(count) = active.get_mut(session_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                active.remove(session_id);
            }
        }
    }

    /// Get the current number of viewers for a specific session.
    pub fn viewer_count(&self, session_id: &str) -> usize {
        let active = self.active.read().expect("RwLock poisoned");
        active.get(session_id).copied().unwrap_or(0)
    }

    /// Get the total number of active WebSocket connections across all sessions.
    pub fn total_connections(&self) -> usize {
        let active = self.active.read().expect("RwLock poisoned");
        active.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_increments_count() {
        let mgr = TerminalConnectionManager::new();
        mgr.connect("session-1").unwrap();
        assert_eq!(mgr.viewer_count("session-1"), 1);

        mgr.connect("session-1").unwrap();
        assert_eq!(mgr.viewer_count("session-1"), 2);
    }

    #[test]
    fn disconnect_decrements_count() {
        let mgr = TerminalConnectionManager::new();
        mgr.connect("session-1").unwrap();
        mgr.connect("session-1").unwrap();
        assert_eq!(mgr.viewer_count("session-1"), 2);

        mgr.disconnect("session-1");
        assert_eq!(mgr.viewer_count("session-1"), 1);
    }

    #[test]
    fn disconnect_removes_at_zero() {
        let mgr = TerminalConnectionManager::new();
        mgr.connect("session-1").unwrap();
        assert_eq!(mgr.total_connections(), 1);

        mgr.disconnect("session-1");
        assert_eq!(mgr.viewer_count("session-1"), 0);
        assert_eq!(mgr.total_connections(), 0);

        // Disconnecting again should be a no-op (not panic)
        mgr.disconnect("session-1");
        assert_eq!(mgr.total_connections(), 0);
    }

    #[test]
    fn per_session_limit_enforced() {
        let mgr = TerminalConnectionManager::new();

        // Connect up to the per-session limit
        for _ in 0..MAX_VIEWERS_PER_SESSION {
            mgr.connect("session-1").unwrap();
        }
        assert_eq!(mgr.viewer_count("session-1"), MAX_VIEWERS_PER_SESSION);

        // The next connection should be rejected
        let result = mgr.connect("session-1");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConnectionLimitError::SessionLimitExceeded(_)
        ));

        // Count should not have changed
        assert_eq!(mgr.viewer_count("session-1"), MAX_VIEWERS_PER_SESSION);

        // But a different session should still work
        mgr.connect("session-2").unwrap();
        assert_eq!(mgr.viewer_count("session-2"), 1);
    }

    #[test]
    fn global_limit_enforced() {
        let mgr = TerminalConnectionManager::new();

        // Fill up to global limit across multiple sessions
        // Use MAX_VIEWERS_PER_SESSION connections per session
        let sessions_needed = MAX_WS_CONNECTIONS / MAX_VIEWERS_PER_SESSION;
        for i in 0..sessions_needed {
            for _ in 0..MAX_VIEWERS_PER_SESSION {
                mgr.connect(&format!("session-{i}")).unwrap();
            }
        }
        assert_eq!(mgr.total_connections(), MAX_WS_CONNECTIONS);

        // The next connection (to a new session) should be rejected
        let result = mgr.connect("session-overflow");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConnectionLimitError::GlobalLimitExceeded
        ));

        // Total should not have changed
        assert_eq!(mgr.total_connections(), MAX_WS_CONNECTIONS);
    }

    #[test]
    fn disconnect_nonexistent_session_is_noop() {
        let mgr = TerminalConnectionManager::new();
        mgr.disconnect("does-not-exist");
        assert_eq!(mgr.total_connections(), 0);
    }

    #[test]
    fn multiple_sessions_tracked_independently() {
        let mgr = TerminalConnectionManager::new();
        mgr.connect("a").unwrap();
        mgr.connect("a").unwrap();
        mgr.connect("b").unwrap();

        assert_eq!(mgr.viewer_count("a"), 2);
        assert_eq!(mgr.viewer_count("b"), 1);
        assert_eq!(mgr.total_connections(), 3);

        mgr.disconnect("a");
        assert_eq!(mgr.viewer_count("a"), 1);
        assert_eq!(mgr.viewer_count("b"), 1);
        assert_eq!(mgr.total_connections(), 2);
    }
}
