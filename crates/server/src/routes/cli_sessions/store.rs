//! In-memory session store for CLI sessions.

use std::collections::HashMap;
use tokio::sync::RwLock;

use super::types::{CliSession, CliSessionStatus};

/// Thread-safe in-memory store for CLI session metadata.
pub struct CliSessionStore {
    inner: RwLock<HashMap<String, CliSession>>,
}

impl Default for CliSessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CliSessionStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Create a store pre-populated with existing sessions.
    pub fn from_sessions(sessions: Vec<CliSession>) -> Self {
        let map: HashMap<String, CliSession> =
            sessions.into_iter().map(|s| (s.id.clone(), s)).collect();
        Self {
            inner: RwLock::new(map),
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

    /// Set the `claude_session_id` for a CLI session. Returns `true` if found and updated.
    pub async fn set_claude_session_id(&self, id: &str, claude_session_id: String) -> bool {
        let mut map = self.inner.write().await;
        if let Some(session) = map.get_mut(id) {
            session.claude_session_id = Some(claude_session_id);
            true
        } else {
            false
        }
    }

    /// Find a running CLI session whose `claude_session_id` matches the given UUID.
    pub async fn find_by_claude_session_id(&self, session_id: &str) -> Option<CliSession> {
        let map = self.inner.read().await;
        map.values()
            .find(|s| {
                s.status == CliSessionStatus::Running
                    && s.claude_session_id.as_deref() == Some(session_id)
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a CliSession with specified fields.
    fn make_cli_session(
        id: &str,
        status: CliSessionStatus,
        claude_session_id: Option<&str>,
    ) -> CliSession {
        CliSession {
            id: id.to_string(),
            created_at: 1000,
            status,
            project_dir: None,
            args: vec![],
            claude_session_id: claude_session_id.map(String::from),
        }
    }

    #[tokio::test]
    async fn find_by_claude_session_id_returns_matching_running_session() {
        let store = CliSessionStore::new();
        store
            .insert(make_cli_session(
                "cli-1",
                CliSessionStatus::Running,
                Some("uuid-abc"),
            ))
            .await;

        let result = store.find_by_claude_session_id("uuid-abc").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "cli-1");
    }

    #[tokio::test]
    async fn find_by_claude_session_id_returns_none_when_no_match() {
        let store = CliSessionStore::new();
        store
            .insert(make_cli_session(
                "cli-1",
                CliSessionStatus::Running,
                Some("uuid-abc"),
            ))
            .await;

        let result = store.find_by_claude_session_id("uuid-does-not-exist").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_by_claude_session_id_returns_none_when_exited() {
        let store = CliSessionStore::new();
        store
            .insert(make_cli_session(
                "cli-1",
                CliSessionStatus::Exited,
                Some("uuid-abc"),
            ))
            .await;

        let result = store.find_by_claude_session_id("uuid-abc").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn set_claude_session_id_updates_store() {
        let store = CliSessionStore::new();
        store
            .insert(make_cli_session("cli-1", CliSessionStatus::Running, None))
            .await;

        // Before: no claude_session_id
        assert!(store.find_by_claude_session_id("uuid-abc").await.is_none());

        // Write back to store
        assert!(
            store
                .set_claude_session_id("cli-1", "uuid-abc".into())
                .await
        );

        // After: ownership resolves
        let found = store.find_by_claude_session_id("uuid-abc").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "cli-1");
    }

    #[tokio::test]
    async fn set_claude_session_id_returns_false_for_missing() {
        let store = CliSessionStore::new();
        assert!(!store.set_claude_session_id("nope", "uuid".into()).await);
    }

    #[tokio::test]
    async fn find_by_claude_session_id_returns_none_when_claude_session_id_is_none() {
        let store = CliSessionStore::new();
        store
            .insert(make_cli_session("cli-1", CliSessionStatus::Running, None))
            .await;

        let result = store.find_by_claude_session_id("uuid-abc").await;
        assert!(result.is_none());
    }
}
