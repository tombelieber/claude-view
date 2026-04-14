//! Background health check for tmux CLI sessions.
//!
//! Periodically verifies that tmux sessions are still alive and removes
//! dead ones from the `TmuxSessionIndex`.

use std::sync::Arc;

use super::tmux::TmuxCommand;
use super::tmux_index::TmuxSessionIndex;

/// Interval between health checks.
const HEALTH_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Spawn a background task that periodically checks tmux session health.
///
/// Runs every 30 seconds until the shutdown signal fires.
pub fn spawn_health_check(
    tmux_index: Arc<TmuxSessionIndex>,
    tmux: Arc<dyn TmuxCommand>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(HEALTH_CHECK_INTERVAL) => {
                    check_sessions(&tmux_index, &*tmux).await;
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

/// Check all tracked tmux session names and remove dead ones.
async fn check_sessions(tmux_index: &TmuxSessionIndex, tmux: &dyn TmuxCommand) {
    let names = tmux_index.list().await;
    for name in names {
        if !tmux.has_session(&name) {
            tracing::debug!(name = %name, "Tmux session no longer alive, removing from index");
            tmux_index.remove(&name).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::cli_sessions::tmux::mock::MockTmux;

    #[tokio::test]
    async fn test_health_check_removes_dead_sessions() {
        let tmux_index = Arc::new(TmuxSessionIndex::new());
        let tmux = Arc::new(MockTmux::new());

        // Insert a session into the index but NOT into tmux's tracking.
        // This simulates a tmux session that died externally.
        tmux_index.insert("cv-dead".to_string()).await;

        // Also insert a "live" one that IS in tmux.
        tmux.new_session("cv-alive", None, &[], &[]).unwrap();
        tmux_index.insert("cv-alive".to_string()).await;

        // Run the health check.
        check_sessions(&tmux_index, &*tmux).await;

        // Dead session should be removed from index.
        assert!(!tmux_index.contains("cv-dead").await);

        // Alive session should still be in index.
        assert!(tmux_index.contains("cv-alive").await);
    }

    #[tokio::test]
    async fn test_spawn_health_check_shutdown() {
        let tmux_index = Arc::new(TmuxSessionIndex::new());
        let tmux: Arc<dyn TmuxCommand> = Arc::new(MockTmux::new());
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = spawn_health_check(tmux_index, tmux, shutdown_rx);

        // Signal shutdown immediately.
        shutdown_tx.send(true).unwrap();

        // The task should exit promptly (within the sleep interval).
        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("health check should shut down within timeout")
            .expect("task should not panic");
    }
}
