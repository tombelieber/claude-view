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
                    if session.pid != pid {
                        warn!(
                            filename_pid = pid,
                            json_pid = session.pid,
                            "Session file PID mismatch — skipping"
                        );
                        continue;
                    }
                    if let Err(e) = tx.try_send(SessionLifecycleEvent::Born { pid, session }) {
                        warn!(
                            pid,
                            "Sessions watcher: channel full, dropped Born event: {e}"
                        );
                    }
                }
            }
            EventKind::Remove(_) => {
                if let Err(e) = tx.try_send(SessionLifecycleEvent::Exited { pid }) {
                    warn!(
                        pid,
                        "Sessions watcher: channel full, dropped Exited event: {e}"
                    );
                }
            }
            EventKind::Modify(_) => {
                // Session files are write-once, but some FSes report Create as Modify
                if let Some(session) = session_files::parse_session_file(path) {
                    if session.pid != pid {
                        warn!(
                            filename_pid = pid,
                            json_pid = session.pid,
                            "Session file PID mismatch (Modify) — skipping"
                        );
                        continue;
                    }
                    if let Err(e) = tx.try_send(SessionLifecycleEvent::Born { pid, session }) {
                        warn!(
                            pid,
                            "Sessions watcher: channel full, dropped Born (Modify) event: {e}"
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::io::Write;

    /// Helper: create a temp session JSON file and return (dir, path).
    fn write_session_file(dir: &Path, filename: &str, pid: u32) -> std::path::PathBuf {
        let path = dir.join(filename);
        let json = format!(
            r#"{{"pid":{},"sessionId":"sess-{}","cwd":"/tmp","startedAt":1700000000000,"kind":"interactive","entrypoint":"cli"}}"#,
            pid, pid
        );
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        f.flush().unwrap();
        path
    }

    #[test]
    fn test_pid_mismatch_skips_create_event() {
        let tmp = tempfile::tempdir().unwrap();
        // Filename says PID 99999, but JSON says PID 11111
        let path = tmp.path().join("99999.json");
        let json = r#"{"pid":11111,"sessionId":"sess-mismatch","cwd":"/tmp","startedAt":1700000000000,"kind":"interactive","entrypoint":"cli"}"#;
        std::fs::write(&path, json).unwrap();

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        // Channel should be empty — mismatched PID is skipped
        assert!(
            rx.try_recv().is_err(),
            "PID mismatch must cause event to be skipped"
        );
    }

    #[test]
    fn test_pid_mismatch_skips_modify_event() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("99999.json");
        let json = r#"{"pid":11111,"sessionId":"sess-mismatch","cwd":"/tmp","startedAt":1700000000000,"kind":"interactive","entrypoint":"cli"}"#;
        std::fs::write(&path, json).unwrap();

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        assert!(
            rx.try_recv().is_err(),
            "PID mismatch on Modify must skip event"
        );
    }

    #[test]
    fn test_matching_pid_sends_born_event() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_session_file(tmp.path(), "12345.json", 12345);

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        match rx.try_recv() {
            Ok(SessionLifecycleEvent::Born { pid, session }) => {
                assert_eq!(pid, 12345);
                assert_eq!(session.pid, 12345);
                assert_eq!(session.session_id, "sess-12345");
            }
            other => panic!("Expected Born event, got: {:?}", other),
        }
    }

    #[test]
    fn test_remove_event_sends_exited() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("12345.json");
        // File doesn't need to exist for Remove events

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        match rx.try_recv() {
            Ok(SessionLifecycleEvent::Exited { pid }) => {
                assert_eq!(pid, 12345);
            }
            other => panic!("Expected Exited event, got: {:?}", other),
        }
    }

    #[test]
    fn test_channel_full_does_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_session_file(tmp.path(), "12345.json", 12345);

        // Channel with capacity 1 — fill it first, then overflow
        let (tx, _rx) = mpsc::channel::<SessionLifecycleEvent>(1);
        // Fill the channel
        let _ = tx.try_send(SessionLifecycleEvent::Exited { pid: 0 });

        // This should log a warning but NOT panic
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);
        // If we get here without panic, the test passes
    }

    #[test]
    fn test_non_json_files_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("12345.txt");
        std::fs::write(&path, "not json").unwrap();

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        assert!(rx.try_recv().is_err(), "Non-.json files must be ignored");
    }

    #[test]
    fn test_non_numeric_filename_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("not-a-pid.json");
        std::fs::write(&path, r#"{"pid":123,"sessionId":"s","cwd":"/","startedAt":0,"kind":"interactive","entrypoint":"cli"}"#).unwrap();

        let (tx, mut rx) = mpsc::channel::<SessionLifecycleEvent>(64);
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, tmp.path(), event);

        assert!(
            rx.try_recv().is_err(),
            "Non-numeric filenames must be ignored"
        );
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
