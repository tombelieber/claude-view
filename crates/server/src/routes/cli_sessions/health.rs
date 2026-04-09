//! Background health check for CLI sessions.
//!
//! Periodically verifies that tmux sessions are still alive and marks
//! dead ones as `Exited`.

use std::sync::Arc;

use super::store::CliSessionStore;
use super::tmux::TmuxCommand;
use super::types::CliSessionStatus;

/// Interval between health checks.
const HEALTH_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Spawn a background task that periodically checks session health.
///
/// Runs every 30 seconds until the shutdown signal fires.
pub fn spawn_health_check(
    store: Arc<CliSessionStore>,
    tmux: Arc<dyn TmuxCommand>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(HEALTH_CHECK_INTERVAL) => {
                    check_sessions(&store, &*tmux).await;
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::debug!("CLI session health check shutting down");
                        break;
                    }
                }
            }
        }
    })
}

/// Check all sessions against tmux and mark dead ones as Exited.
async fn check_sessions(store: &CliSessionStore, tmux: &dyn TmuxCommand) {
    let sessions = store.list().await;
    for session in sessions {
        if session.status == CliSessionStatus::Exited {
            continue;
        }
        if !tmux.has_session(&session.id) {
            tracing::debug!(id = %session.id, "CLI session no longer alive in tmux, marking Exited");
            store
                .update_status(&session.id, CliSessionStatus::Exited)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::cli_sessions::tmux::mock::MockTmux;
    use crate::routes::cli_sessions::types::CliSession;

    #[tokio::test]
    async fn test_health_check_marks_dead_sessions() {
        let store = Arc::new(CliSessionStore::new());
        let tmux = Arc::new(MockTmux::new());

        // Insert a session into the store but NOT into tmux's tracking.
        // This simulates a tmux session that died externally.
        store
            .insert(CliSession {
                id: "cv-dead".to_string(),
                created_at: 1000,
                status: CliSessionStatus::Running,
                project_dir: None,
                args: vec![],
            })
            .await;

        // Also insert a "live" one that IS in tmux.
        tmux.new_session("cv-alive", None, &[]).unwrap();
        store
            .insert(CliSession {
                id: "cv-alive".to_string(),
                created_at: 2000,
                status: CliSessionStatus::Running,
                project_dir: None,
                args: vec![],
            })
            .await;

        // Run the health check.
        check_sessions(&store, &*tmux).await;

        // Dead session should be Exited.
        let dead = store.get("cv-dead").await.unwrap();
        assert_eq!(dead.status, CliSessionStatus::Exited);

        // Alive session should still be Running.
        let alive = store.get("cv-alive").await.unwrap();
        assert_eq!(alive.status, CliSessionStatus::Running);
    }

    #[tokio::test]
    async fn test_health_check_skips_already_exited() {
        let store = Arc::new(CliSessionStore::new());
        let tmux = Arc::new(MockTmux::new());

        store
            .insert(CliSession {
                id: "cv-old".to_string(),
                created_at: 500,
                status: CliSessionStatus::Exited,
                project_dir: None,
                args: vec![],
            })
            .await;

        // Should not panic or change anything.
        check_sessions(&store, &*tmux).await;

        let s = store.get("cv-old").await.unwrap();
        assert_eq!(s.status, CliSessionStatus::Exited);
    }

    #[tokio::test]
    async fn test_spawn_health_check_shutdown() {
        let store = Arc::new(CliSessionStore::new());
        let tmux: Arc<dyn TmuxCommand> = Arc::new(MockTmux::new());
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = spawn_health_check(store, tmux, shutdown_rx);

        // Signal shutdown immediately.
        shutdown_tx.send(true).unwrap();

        // The task should exit promptly (within the sleep interval).
        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("health check should shut down within timeout")
            .expect("task should not panic");
    }
}
