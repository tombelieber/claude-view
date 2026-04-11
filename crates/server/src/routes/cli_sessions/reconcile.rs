//! Startup reconciliation: discover existing `cv-*` tmux sessions
//! that survived a server restart and populate the in-memory store.

use std::time::{SystemTime, UNIX_EPOCH};

use super::handlers::resolve_claude_session_id;
use super::tmux::TmuxCommand;
use super::types::{CliSession, CliSessionStatus};

/// Scan tmux for existing `cv-*` sessions and return them as `CliSession` entries.
///
/// Called once at server startup to reconcile the in-memory store with tmux state.
/// Without this, sessions created in a previous server lifetime appear as orphaned
/// tmux processes that the API doesn't know about.
pub fn reconcile_tmux_sessions(tmux: &dyn TmuxCommand) -> Vec<CliSession> {
    if !tmux.is_available() {
        return Vec::new();
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let all_sessions = tmux.list_sessions();
    let mut reconciled = Vec::new();

    for name in all_sessions {
        if !name.starts_with("cv-") {
            continue;
        }

        let claude_session_id = tmux.pane_pid(&name).and_then(resolve_claude_session_id);

        reconciled.push(CliSession {
            id: name,
            created_at: now_ms,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id,
        });
    }

    if !reconciled.is_empty() {
        tracing::info!(
            count = reconciled.len(),
            "Reconciled existing tmux sessions into CLI session store"
        );
    }

    reconciled
}
