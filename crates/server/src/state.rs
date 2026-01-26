// crates/server/src/state.rs
//! Application state for the Axum server.

use std::sync::Arc;
use std::time::Instant;

/// Shared application state accessible from all route handlers.
#[derive(Clone)]
pub struct AppState {
    /// Server start time for uptime tracking.
    pub start_time: Instant,
}

impl AppState {
    /// Create a new application state wrapped in an Arc for sharing.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            start_time: Instant::now(),
        })
    }

    /// Get the server uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert!(state.uptime_secs() < 1);
    }

    #[test]
    fn test_app_state_uptime() {
        let state = AppState::new();
        sleep(Duration::from_millis(100));
        // Should be at least 0 seconds (could be 0 due to timing)
        let uptime = state.uptime_secs();
        assert!(uptime < 5); // Reasonable upper bound
    }

    #[test]
    fn test_app_state_clone() {
        let state = AppState::new();
        let cloned = state.clone();
        // Both should report similar uptime
        assert_eq!(state.uptime_secs(), cloned.uptime_secs());
    }

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert!(state.uptime_secs() < 1);
    }
}
