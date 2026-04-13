//! Thin index tracking which tmux session names are alive.
//! NOT an entity store — LiveSessionMap owns session state.

use std::collections::HashSet;
use tokio::sync::RwLock;

/// Tracks alive tmux session names (e.g. "cv-abc123").
/// Used by: ownership resolver, DELETE handler, reconciliation.
pub struct TmuxSessionIndex {
    active: RwLock<HashSet<String>>,
}

impl Default for TmuxSessionIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxSessionIndex {
    pub fn new() -> Self {
        Self {
            active: RwLock::new(HashSet::new()),
        }
    }

    /// Create from a list of existing tmux session names (startup reconciliation).
    pub fn from_names(names: Vec<String>) -> Self {
        Self {
            active: RwLock::new(names.into_iter().collect()),
        }
    }

    /// Register a tmux session as active.
    pub async fn insert(&self, name: String) {
        self.active.write().await.insert(name);
    }

    /// Remove a tmux session (killed or exited).
    pub async fn remove(&self, name: &str) -> bool {
        self.active.write().await.remove(name)
    }

    /// Check if a tmux session name is tracked.
    pub async fn contains(&self, name: &str) -> bool {
        self.active.read().await.contains(name)
    }

    /// List all active tmux session names.
    pub async fn list(&self) -> Vec<String> {
        self.active.read().await.iter().cloned().collect()
    }

    /// Number of active tmux sessions.
    pub async fn len(&self) -> usize {
        self.active.read().await.len()
    }

    /// Returns `true` if there are no active tmux sessions.
    pub async fn is_empty(&self) -> bool {
        self.active.read().await.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_contains() {
        let idx = TmuxSessionIndex::new();
        idx.insert("cv-abc".into()).await;
        assert!(idx.contains("cv-abc").await);
        assert!(!idx.contains("cv-xyz").await);
    }

    #[tokio::test]
    async fn remove_returns_true_if_existed() {
        let idx = TmuxSessionIndex::new();
        idx.insert("cv-abc".into()).await;
        assert!(idx.remove("cv-abc").await);
        assert!(!idx.remove("cv-abc").await);
        assert!(!idx.contains("cv-abc").await);
    }

    #[tokio::test]
    async fn from_names_prepopulates() {
        let idx = TmuxSessionIndex::from_names(vec!["cv-1".into(), "cv-2".into()]);
        assert_eq!(idx.len().await, 2);
        assert!(idx.contains("cv-1").await);
    }
}
