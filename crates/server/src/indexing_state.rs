//! Atomic indexing state for lock-free progress tracking.
//!
//! [`IndexingState`] uses atomics so the indexing background task can update
//! progress counters while the HTTP handler reads them without contention.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Which phase the indexer is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IndexingStatus {
    /// No indexing work in progress.
    Idle = 0,
    /// Pass 1 – reading existing indexes / lightweight scan.
    ReadingIndexes = 1,
    /// Pass 2 – deep indexing (JSONL parsing, full-text, etc.).
    DeepIndexing = 2,
    /// Indexing finished successfully.
    Done = 3,
    /// Indexing terminated with an error (see [`IndexingState::error`]).
    Error = 4,
}

impl IndexingStatus {
    /// Convert a raw `u8` into a status variant.
    /// Returns `None` for values outside the valid range.
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::ReadingIndexes),
            2 => Some(Self::DeepIndexing),
            3 => Some(Self::Done),
            4 => Some(Self::Error),
            _ => None,
        }
    }
}

/// Thread-safe, lock-free progress state for the background indexer.
///
/// All numeric counters use [`Ordering::Relaxed`] – we only need
/// monotonically-increasing values visible *eventually* to readers,
/// not cross-field consistency.
pub struct IndexingState {
    status: AtomicU8,
    total: AtomicUsize,
    indexed: AtomicUsize,
    projects_found: AtomicUsize,
    sessions_found: AtomicUsize,
    error: RwLock<Option<String>>,
}

impl IndexingState {
    /// Create a new state initialised to [`IndexingStatus::Idle`] with all
    /// counters at zero.
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(IndexingStatus::Idle as u8),
            total: AtomicUsize::new(0),
            indexed: AtomicUsize::new(0),
            projects_found: AtomicUsize::new(0),
            sessions_found: AtomicUsize::new(0),
            error: RwLock::new(None),
        }
    }

    // -- Status ---------------------------------------------------------------

    /// Current indexing status.
    pub fn status(&self) -> IndexingStatus {
        let raw = self.status.load(Ordering::Relaxed);
        IndexingStatus::from_u8(raw).unwrap_or(IndexingStatus::Error)
    }

    /// Set the indexing status.
    pub fn set_status(&self, status: IndexingStatus) {
        self.status.store(status as u8, Ordering::Relaxed);
    }

    // -- Counters -------------------------------------------------------------

    /// Total items to index.
    pub fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    /// Set the total number of items to index.
    pub fn set_total(&self, val: usize) {
        self.total.store(val, Ordering::Relaxed);
    }

    /// Number of items indexed so far.
    pub fn indexed(&self) -> usize {
        self.indexed.load(Ordering::Relaxed)
    }

    /// Increment the indexed counter by one and return the **new** value.
    pub fn increment_indexed(&self) -> usize {
        self.indexed.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Number of projects discovered.
    pub fn projects_found(&self) -> usize {
        self.projects_found.load(Ordering::Relaxed)
    }

    /// Set the number of projects discovered.
    pub fn set_projects_found(&self, val: usize) {
        self.projects_found.store(val, Ordering::Relaxed);
    }

    /// Number of sessions discovered.
    pub fn sessions_found(&self) -> usize {
        self.sessions_found.load(Ordering::Relaxed)
    }

    /// Set the number of sessions discovered.
    pub fn set_sessions_found(&self, val: usize) {
        self.sessions_found.store(val, Ordering::Relaxed);
    }

    // -- Error ----------------------------------------------------------------

    /// Record an error message (also sets status to [`IndexingStatus::Error`]).
    pub fn set_error(&self, msg: String) {
        self.set_status(IndexingStatus::Error);
        if let Ok(mut guard) = self.error.write() {
            *guard = Some(msg);
        }
    }

    /// Retrieve the current error message, if any.
    pub fn error(&self) -> Option<String> {
        self.error.read().ok().and_then(|g| g.clone())
    }
}

impl Default for IndexingState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn initial_state_is_idle_with_zeroes() {
        let state = IndexingState::new();
        assert_eq!(state.status(), IndexingStatus::Idle);
        assert_eq!(state.total(), 0);
        assert_eq!(state.indexed(), 0);
        assert_eq!(state.projects_found(), 0);
        assert_eq!(state.sessions_found(), 0);
        assert!(state.error().is_none());
    }

    #[test]
    fn status_transitions() {
        let state = IndexingState::new();

        state.set_status(IndexingStatus::ReadingIndexes);
        assert_eq!(state.status(), IndexingStatus::ReadingIndexes);

        state.set_status(IndexingStatus::DeepIndexing);
        assert_eq!(state.status(), IndexingStatus::DeepIndexing);

        state.set_status(IndexingStatus::Done);
        assert_eq!(state.status(), IndexingStatus::Done);

        state.set_status(IndexingStatus::Error);
        assert_eq!(state.status(), IndexingStatus::Error);

        // Back to idle (reset)
        state.set_status(IndexingStatus::Idle);
        assert_eq!(state.status(), IndexingStatus::Idle);
    }

    #[test]
    fn counter_increments() {
        let state = IndexingState::new();

        state.set_total(100);
        assert_eq!(state.total(), 100);

        assert_eq!(state.increment_indexed(), 1);
        assert_eq!(state.increment_indexed(), 2);
        assert_eq!(state.increment_indexed(), 3);
        assert_eq!(state.indexed(), 3);

        state.set_projects_found(5);
        assert_eq!(state.projects_found(), 5);

        state.set_sessions_found(42);
        assert_eq!(state.sessions_found(), 42);
    }

    #[test]
    fn error_state() {
        let state = IndexingState::new();

        state.set_error("disk full".to_string());
        assert_eq!(state.status(), IndexingStatus::Error);
        assert_eq!(state.error(), Some("disk full".to_string()));

        // Overwrite error
        state.set_error("timeout".to_string());
        assert_eq!(state.error(), Some("timeout".to_string()));
    }

    #[test]
    fn thread_safety_concurrent_access() {
        let state = Arc::new(IndexingState::new());
        state.set_total(1000);
        state.set_status(IndexingStatus::DeepIndexing);

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let s = Arc::clone(&state);
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        s.increment_indexed();
                        // Also read values to test concurrent reads
                        let _ = s.status();
                        let _ = s.total();
                        let _ = s.indexed();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        // 8 threads × 100 increments = 800
        assert_eq!(state.indexed(), 800);
        assert_eq!(state.total(), 1000);
        assert_eq!(state.status(), IndexingStatus::DeepIndexing);
    }

    #[test]
    fn from_u8_invalid_returns_none() {
        assert!(IndexingStatus::from_u8(5).is_none());
        assert!(IndexingStatus::from_u8(255).is_none());
    }

    #[test]
    fn default_impl() {
        let state = IndexingState::default();
        assert_eq!(state.status(), IndexingStatus::Idle);
    }
}
