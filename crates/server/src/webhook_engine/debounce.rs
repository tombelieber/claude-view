//! Per-session per-webhook event coalescing.
//!
//! `SessionUpdated` fires 10+/sec during active sessions. The debouncer
//! ensures at most one delivery per interval per (webhook_id, session_id) pair.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct Debouncer {
    /// (webhook_id, session_id) → last sent time
    last_sent: HashMap<(String, String), Instant>,
    /// Minimum interval between sends
    interval: Duration,
}

impl Debouncer {
    pub fn new(interval: Duration) -> Self {
        Self {
            last_sent: HashMap::new(),
            interval,
        }
    }

    /// Returns true if enough time has passed since last send for this pair.
    /// Records the current time if returning true.
    pub fn should_send(&mut self, webhook_id: &str, session_id: &str) -> bool {
        let key = (webhook_id.to_string(), session_id.to_string());
        let now = Instant::now();
        match self.last_sent.get(&key) {
            Some(last) if now.duration_since(*last) < self.interval => false,
            _ => {
                self.last_sent.insert(key, now);
                true
            }
        }
    }

    /// Clean up all entries for a given session (called when session ends).
    pub fn remove_session(&mut self, session_id: &str) {
        self.last_sent.retain(|(_, sid), _| sid != session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_event_always_sends() {
        let mut d = Debouncer::new(Duration::from_secs(10));
        assert!(d.should_send("wh_1", "sess_1"));
    }

    #[test]
    fn immediate_second_event_is_suppressed() {
        let mut d = Debouncer::new(Duration::from_secs(10));
        assert!(d.should_send("wh_1", "sess_1"));
        assert!(!d.should_send("wh_1", "sess_1"));
    }

    #[test]
    fn after_interval_sends_again() {
        let mut d = Debouncer::new(Duration::from_millis(10));
        assert!(d.should_send("wh_1", "sess_1"));
        std::thread::sleep(Duration::from_millis(15));
        assert!(d.should_send("wh_1", "sess_1"));
    }

    #[test]
    fn different_webhook_is_independent() {
        let mut d = Debouncer::new(Duration::from_secs(10));
        assert!(d.should_send("wh_1", "sess_1"));
        assert!(d.should_send("wh_2", "sess_1")); // different webhook → independent
    }

    #[test]
    fn different_session_is_independent() {
        let mut d = Debouncer::new(Duration::from_secs(10));
        assert!(d.should_send("wh_1", "sess_1"));
        assert!(d.should_send("wh_1", "sess_2")); // different session → independent
    }

    #[test]
    fn remove_session_cleans_up() {
        let mut d = Debouncer::new(Duration::from_secs(10));
        assert!(d.should_send("wh_1", "sess_1"));
        assert!(d.should_send("wh_2", "sess_1"));
        d.remove_session("sess_1");
        // After removal, should send again
        assert!(d.should_send("wh_1", "sess_1"));
        assert!(d.should_send("wh_2", "sess_1"));
    }
}
