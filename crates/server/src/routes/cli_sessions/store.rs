//! In-memory session store for CLI sessions.

use std::collections::HashMap;
use tokio::sync::RwLock;

use super::types::{CliSession, CliSessionStatus};

/// Thread-safe in-memory store for CLI session metadata.
pub struct CliSessionStore {
    inner: RwLock<HashMap<String, CliSession>>,
}

impl CliSessionStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Insert a session into the store.
    pub async fn insert(&self, session: CliSession) {
        let mut map = self.inner.write().await;
        map.insert(session.id.clone(), session);
    }

    /// Remove a session by ID, returning it if it existed.
    pub async fn remove(&self, id: &str) -> Option<CliSession> {
        let mut map = self.inner.write().await;
        map.remove(id)
    }

    /// Get a clone of a session by ID.
    pub async fn get(&self, id: &str) -> Option<CliSession> {
        let map = self.inner.read().await;
        map.get(id).cloned()
    }

    /// List all sessions, sorted by creation time (newest first).
    pub async fn list(&self) -> Vec<CliSession> {
        let map = self.inner.read().await;
        let mut sessions: Vec<CliSession> = map.values().cloned().collect();
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sessions
    }

    /// Update the status of a session by ID.
    /// Returns `true` if the session was found and updated.
    pub async fn update_status(&self, id: &str, status: CliSessionStatus) -> bool {
        let mut map = self.inner.write().await;
        if let Some(session) = map.get_mut(id) {
            session.status = status;
            true
        } else {
            false
        }
    }
}
