//! File watcher for ~/.claude/sessions/ directory — hook-free lifecycle detection.
//!
//! Watches for create/delete events on session JSON files:
//! - Create → new Claude Code session started (extract kind/entrypoint/sessionId)
//! - Delete → session process exited cleanly
//!
//! This is the **primary lifecycle source** — hooks provide rich state (agent_state,
//! activity labels) but sessions/ handles birth/death without requiring hooks.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use claude_view_core::session_files::{self, ActiveSession};

/// Events emitted by the sessions watcher.
#[derive(Debug, Clone)]
pub enum SessionLifecycleEvent {
    /// A new session file was created — process started.
    Born { pid: u32, session: ActiveSession },
    /// A session file was deleted — process exited cleanly.
    Exited { pid: u32 },
    /// A session file exists but kill(pid, 0) fails — crashed.
    Crashed { pid: u32, session_id: String },
}

/// Scan the sessions directory and return all currently alive sessions.
///
/// Called at startup to establish initial state BEFORE snapshot recovery.
pub fn scan_sessions_dir() -> Vec<ActiveSession> {
    match session_files::claude_sessions_dir() {
        Some(dir) => session_files::scan_active_sessions(&dir),
        None => Vec::new(),
    }
}

/// Check which sessions from the sessions dir are actually alive (kill -0).
/// Returns (alive, crashed) partitions.
pub fn partition_by_liveness(
    sessions: Vec<ActiveSession>,
) -> (Vec<ActiveSession>, Vec<ActiveSession>) {
    let mut alive = Vec::new();
    let mut crashed = Vec::new();

    for session in sessions {
        if crate::live::process::is_pid_alive(session.pid) {
            alive.push(session);
        } else {
            crashed.push(session);
        }
    }

    (alive, crashed)
}

/// Start the sessions directory watcher.
///
/// Returns a receiver for lifecycle events and the watcher handle (must be kept alive).
pub fn start_sessions_watcher(
) -> Option<(mpsc::Receiver<SessionLifecycleEvent>, RecommendedWatcher)> {
    let sessions_dir = session_files::claude_sessions_dir()?;

    if !sessions_dir.exists() {
        // Create the directory if it doesn't exist — Claude Code may not have run yet
        if std::fs::create_dir_all(&sessions_dir).is_err() {
            warn!("Failed to create ~/.claude/sessions/ directory");
            return None;
        }
    }

    let (tx, rx) = mpsc::channel::<SessionLifecycleEvent>(64);
    let sessions_dir_clone = sessions_dir.clone();

    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                handle_notify_event(&tx, &sessions_dir_clone, event);
            }
            Err(e) => {
                warn!("Sessions watcher error: {e}");
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    ) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create sessions watcher: {e}");
            return None;
        }
    };

    if let Err(e) = watcher.watch(&sessions_dir, RecursiveMode::NonRecursive) {
        error!("Failed to watch ~/.claude/sessions/: {e}");
        return None;
    }

    info!("Sessions watcher started on {}", sessions_dir.display());

    Some((rx, watcher))
}

/// Handle a notify event from the sessions directory.
fn handle_notify_event(
    tx: &mpsc::Sender<SessionLifecycleEvent>,
    _sessions_dir: &Path,
    event: Event,
) {
    for path in &event.paths {
        // Only care about .json files
        if !path.extension().map(|e| e == "json").unwrap_or(false) {
            continue;
        }

        // Extract PID from filename (e.g. "12345.json" → 12345)
        let pid = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(p) => p,
            None => continue,
        };

        match event.kind {
            EventKind::Create(_) => {
                // Parse the session file
                if let Some(session) = session_files::parse_session_file(path) {
                    let _ = tx.try_send(SessionLifecycleEvent::Born { pid, session });
                }
            }
            EventKind::Remove(_) => {
                let _ = tx.try_send(SessionLifecycleEvent::Exited { pid });
            }
            EventKind::Modify(_) => {
                // Session files are write-once, but some FSes report Create as Modify
                if let Some(session) = session_files::parse_session_file(path) {
                    let _ = tx.try_send(SessionLifecycleEvent::Born { pid, session });
                }
            }
            _ => {}
        }
    }
}

/// Periodic crash detection: for each known session from the sessions dir,
/// check if the PID is still alive. If dead but the file still exists,
/// the process crashed without cleanup.
///
/// Called periodically by the reconciliation loop.
pub fn detect_crashed_sessions(
    known_sessions: &HashMap<u32, String>,
) -> Vec<SessionLifecycleEvent> {
    let mut crashed = Vec::new();

    for (pid, session_id) in known_sessions {
        if !crate::live::process::is_pid_alive(*pid) {
            crashed.push(SessionLifecycleEvent::Crashed {
                pid: *pid,
                session_id: session_id.clone(),
            });
        }
    }

    crashed
}
