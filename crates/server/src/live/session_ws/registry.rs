//! Connection registry for multiplexed session WebSockets.
//!
//! Tracks active connections per session and globally, enforcing limits
//! to prevent resource exhaustion (matching TerminalConnectionManager pattern).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Per-session and global connection limits.
const MAX_PER_SESSION: usize = 4;
const MAX_GLOBAL: usize = 64;

/// Tracks active multiplexed WS connections.
///
/// Thread-safe: internal `Mutex` for the per-session map, atomics for global count.
pub struct SessionChannelRegistry {
    /// Per-session connection counts.
    per_session: Mutex<HashMap<String, usize>>,
    /// Global connection count (atomic for fast reads).
    global: AtomicUsize,
}

impl SessionChannelRegistry {
    pub fn new() -> Self {
        Self {
            per_session: Mutex::new(HashMap::new()),
            global: AtomicUsize::new(0),
        }
    }

    /// Try to register a new connection for the given session.
    /// Returns `Ok(())` if under limits, `Err(reason)` if at capacity.
    pub fn try_connect(&self, session_id: &str) -> Result<(), &'static str> {
        let global = self.global.load(Ordering::Relaxed);
        if global >= MAX_GLOBAL {
            return Err("global connection limit reached");
        }

        let mut map = self.per_session.lock().unwrap();
        let count = map.entry(session_id.to_string()).or_insert(0);
        if *count >= MAX_PER_SESSION {
            return Err("per-session connection limit reached");
        }

        *count += 1;
        self.global.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Unregister a connection for the given session.
    pub fn disconnect(&self, session_id: &str) {
        let mut map = self.per_session.lock().unwrap();
        if let Some(count) = map.get_mut(session_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(session_id);
            }
        }
        self.global.fetch_sub(1, Ordering::Relaxed);
    }

    /// Current global connection count.
    pub fn global_count(&self) -> usize {
        self.global.load(Ordering::Relaxed)
    }

    /// Current connection count for a session.
    pub fn session_count(&self, session_id: &str) -> usize {
        let map = self.per_session.lock().unwrap();
        map.get(session_id).copied().unwrap_or(0)
    }
}
