// crates/server/src/state.rs
//! Application state for the Axum server.

use crate::indexing_state::IndexingState;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use vibe_recall_core::Registry;
use vibe_recall_db::Database;

/// Type alias for the shared registry holder.
///
/// The registry is `None` until background indexing builds it, then `Some(Registry)`.
/// Uses `std::sync::RwLock` (not `tokio::sync::RwLock`) because:
/// - The registry is written exactly once from the background task
/// - Read operations are uncontended after the initial write
/// - No need to hold the lock across `.await` points
pub type RegistryHolder = Arc<RwLock<Option<Registry>>>;

/// Shared application state accessible from all route handlers.
pub struct AppState {
    /// Server start time for uptime tracking.
    pub start_time: Instant,
    /// Database handle for session/project queries.
    pub db: Database,
    /// Shared indexing progress state (lock-free atomics).
    pub indexing: Arc<IndexingState>,
    /// Invocable registry (skills, commands, MCP tools, built-in tools).
    /// `None` until background indexing completes registry build.
    pub registry: RegistryHolder,
}

impl AppState {
    /// Create a new application state wrapped in an Arc for sharing.
    ///
    /// Uses a default (idle) `IndexingState` and empty registry holder.
    pub fn new(db: Database) -> Arc<Self> {
        Arc::new(Self {
            start_time: Instant::now(),
            db,
            indexing: Arc::new(IndexingState::new()),
            registry: Arc::new(RwLock::new(None)),
        })
    }

    /// Create with an externally-provided `IndexingState` (for testing and
    /// server-first startup where the caller owns the indexing handle).
    pub fn new_with_indexing(db: Database, indexing: Arc<IndexingState>) -> Arc<Self> {
        Arc::new(Self {
            start_time: Instant::now(),
            db,
            indexing,
            registry: Arc::new(RwLock::new(None)),
        })
    }

    /// Create with both an external `IndexingState` and a shared registry holder.
    pub fn new_with_indexing_and_registry(
        db: Database,
        indexing: Arc<IndexingState>,
        registry: RegistryHolder,
    ) -> Arc<Self> {
        Arc::new(Self {
            start_time: Instant::now(),
            db,
            indexing,
            registry,
        })
    }

    /// Get the server uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    /// Helper to create an AppState with an in-memory database for testing.
    async fn test_state() -> Arc<AppState> {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        AppState::new(db)
    }

    #[tokio::test]
    async fn test_app_state_new() {
        let state = test_state().await;
        assert!(state.uptime_secs() < 1);
    }

    #[tokio::test]
    async fn test_app_state_uptime() {
        let state = test_state().await;
        sleep(Duration::from_millis(100));
        // Should be at least 0 seconds (could be 0 due to timing)
        let uptime = state.uptime_secs();
        assert!(uptime < 5); // Reasonable upper bound
    }

    #[tokio::test]
    async fn test_app_state_clone() {
        let state = test_state().await;
        let cloned = state.clone();
        // Both should report similar uptime
        assert_eq!(state.uptime_secs(), cloned.uptime_secs());
    }
}
