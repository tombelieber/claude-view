use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Buffered statusline payload for sessions not yet discovered.
/// Replaces the silent drop in handle_statusline() and the
/// OnceLock<Mutex<HashMap>> global static in hooks.rs.
pub struct PendingMutations<T> {
    entries: HashMap<String, Vec<TimestampedEntry<T>>>,
    ttl: Duration,
}

struct TimestampedEntry<T> {
    payload: T,
    buffered_at: Instant,
}

impl<T> PendingMutations<T> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    /// Buffer a payload for a session that doesn't exist yet.
    pub fn push(&mut self, session_id: &str, payload: T) {
        self.entries
            .entry(session_id.to_string())
            .or_default()
            .push(TimestampedEntry {
                payload,
                buffered_at: Instant::now(),
            });
    }

    /// Drain all buffered payloads for a session (FIFO order).
    /// Called when session is created/discovered.
    pub fn drain(&mut self, session_id: &str) -> Vec<T> {
        self.entries
            .remove(session_id)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.payload)
            .collect()
    }

    /// Remove expired entries to prevent memory leak.
    pub fn sweep_expired(&mut self) {
        let now = Instant::now();
        let ttl = self.ttl;
        self.entries.retain(|_, entries| {
            entries.retain(|e| now.duration_since(e.buffered_at) < ttl);
            !entries.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_drain_fifo() {
        let mut buf = PendingMutations::<String>::new(Duration::from_secs(60));
        buf.push("abc", "first".into());
        buf.push("abc", "second".into());
        let drained = buf.drain("abc");
        assert_eq!(drained, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn drain_empty_returns_empty() {
        let mut buf = PendingMutations::<String>::new(Duration::from_secs(60));
        let drained = buf.drain("nonexistent");
        assert!(drained.is_empty());
    }

    #[test]
    fn drain_removes_entries() {
        let mut buf = PendingMutations::<String>::new(Duration::from_secs(60));
        buf.push("abc", "data".into());
        buf.drain("abc");
        let drained = buf.drain("abc");
        assert!(drained.is_empty());
    }

    #[test]
    fn sweep_expired_removes_old_entries() {
        let mut buf = PendingMutations::<String>::new(Duration::from_millis(1));
        buf.push("abc", "old".into());
        std::thread::sleep(Duration::from_millis(10));
        buf.sweep_expired();
        let drained = buf.drain("abc");
        assert!(drained.is_empty());
    }

    #[test]
    fn sweep_keeps_fresh_entries() {
        let mut buf = PendingMutations::<String>::new(Duration::from_secs(60));
        buf.push("abc", "fresh".into());
        buf.sweep_expired();
        let drained = buf.drain("abc");
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn multiple_sessions_independent() {
        let mut buf = PendingMutations::<String>::new(Duration::from_secs(60));
        buf.push("abc", "a-data".into());
        buf.push("def", "d-data".into());
        let a = buf.drain("abc");
        let d = buf.drain("def");
        assert_eq!(a, vec!["a-data".to_string()]);
        assert_eq!(d, vec!["d-data".to_string()]);
    }
}
