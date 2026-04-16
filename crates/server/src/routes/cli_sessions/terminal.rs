//! Rust-native terminal relay using portable-pty.
//!
//! Replaces the Node.js sidecar's terminal-relay.ts. Spawns
//! `tmux attach-session` via portable-pty, streams PTY I/O over
//! axum WebSocket, supports multi-client fan-out via broadcast channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::{broadcast, mpsc, RwLock};

use super::ring_buffer::RingBuffer;

/// Broadcast channel capacity: 256 messages × ~4KB = ~1MB max buffered.
const BROADCAST_CAPACITY: usize = 256;
/// Scrollback ring buffer size for reconnection replay + lag re-sync.
const SCROLLBACK_BYTES: usize = 64 * 1024;
/// mpsc write channel capacity: 64 keystroke messages.
const WRITE_CHANNEL_CAPACITY: usize = 64;

/// Validate a tmux target name used as a WebSocket path segment.
///
/// Accepts any name that is safe to pass to `tmux attach-session -t <name>`
/// and that tmux itself can parse without ambiguity:
///
/// - length 1..=128
/// - characters restricted to `[A-Za-z0-9._-]`
///
/// This covers both claude-view-spawned sessions (`cv-abcd1234`) and
/// user-spawned sessions discovered via env-probe (`work`, `dev_env`,
/// `my-session`, `0`). Excludes anything that would break tmux argument
/// parsing or (defence in depth) leak into a shell: `/`, `:`, `;`, `|`,
/// `&`, `$`, backticks, quotes, whitespace, control characters.
///
/// Safe because the PTY layer uses `portable-pty`'s `CommandBuilder`
/// which is exec-style (no shell) — but the stricter filter keeps the
/// URL-path / log-line readable and prevents surprising tmux behavior on
/// reserved characters.
pub fn is_valid_session_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// A live terminal session attached to a tmux session via PTY.
pub struct TerminalSession {
    /// Broadcast sender for PTY output -> all WebSocket clients.
    pub tx: broadcast::Sender<Bytes>,
    /// Send keystrokes here — a dedicated task writes them to the PTY.
    pub write_tx: mpsc::Sender<Bytes>,
    /// Recent output for reconnection replay + lag re-sync.
    pub scrollback: Arc<tokio::sync::Mutex<RingBuffer>>,
    /// Master PTY handle — needed for resize operations.
    pub master: Arc<tokio::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    /// Handle to kill the PTY child process on cleanup.
    child: Arc<tokio::sync::Mutex<Box<dyn portable_pty::Child + Send>>>,
    /// Signals PTY death to all WS send tasks.
    pub pty_dead: tokio::sync::watch::Sender<bool>,
    /// Diagnostic: current connected client count.
    client_count: AtomicUsize,
}

impl TerminalSession {
    /// Resize the PTY terminal dimensions.
    pub async fn resize(&self, cols: u16, rows: u16) {
        let master = self.master.lock().await;
        let _ = master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

/// Manages all active terminal sessions.
pub struct TerminalManager {
    sessions: RwLock<HashMap<String, Arc<TerminalSession>>>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a terminal session and atomically register the client.
    ///
    /// Increments client_count BEFORE returning the Arc, preventing the
    /// TOCTOU race where disconnect() kills a session between get and connect.
    pub async fn acquire(&self, tmux_session_id: &str) -> Result<Arc<TerminalSession>, String> {
        // Fast path: session already exists
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(tmux_session_id) {
                session.client_count.fetch_add(1, Ordering::SeqCst);
                return Ok(Arc::clone(session));
            }
        }

        // Slow path: spawn PTY outside the lock to avoid blocking all sessions
        let new_session = self.spawn_pty(tmux_session_id)?;
        let new_session = Arc::new(new_session);

        let mut sessions = self.sessions.write().await;
        // Double-check: another task may have created it while we spawned
        if let Some(session) = sessions.get(tmux_session_id) {
            session.client_count.fetch_add(1, Ordering::SeqCst);
            return Ok(Arc::clone(session));
        }

        new_session.client_count.fetch_add(1, Ordering::SeqCst);
        sessions.insert(tmux_session_id.to_string(), Arc::clone(&new_session));
        Ok(new_session)
    }

    /// Unregister a WS client. If last client, removes session and kills PTY.
    ///
    /// Holds the write lock for the entire check-and-remove to prevent
    /// the TOCTOU race where a connecting client gets a dead session ref.
    pub async fn disconnect(&self, session_id: &str) {
        let removed = {
            let mut sessions = self.sessions.write().await;
            let should_remove = if let Some(session) = sessions.get(session_id) {
                let prev = session.client_count.fetch_sub(1, Ordering::SeqCst);
                prev <= 1
            } else {
                false
            };
            if should_remove {
                sessions.remove(session_id)
            } else {
                None
            }
        }; // write lock released here

        if let Some(session) = removed {
            let _ = session.pty_dead.send(true);
            let mut child = session.child.lock().await;
            let _ = child.kill();
            tracing::debug!(id = %session_id, "terminal session cleaned up (last client)");
        }
    }

    pub fn active_count(&self) -> usize {
        self.sessions.try_read().map(|s| s.len()).unwrap_or(0)
    }

    /// Spawn a PTY running `tmux attach-session -t {id}`.
    fn spawn_pty(&self, tmux_session_id: &str) -> Result<TerminalSession, String> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| format!("Failed to open PTY: {e}"))?;

        let mut cmd = CommandBuilder::new("tmux");
        cmd.arg("attach-session");
        cmd.arg("-t");
        cmd.arg(tmux_session_id);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn tmux attach: {e}"))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

        let (tx, _rx) = broadcast::channel::<Bytes>(BROADCAST_CAPACITY);
        let scrollback = Arc::new(tokio::sync::Mutex::new(RingBuffer::new(SCROLLBACK_BYTES)));
        let (write_tx, mut write_rx) = mpsc::channel::<Bytes>(WRITE_CHANNEL_CAPACITY);
        let (pty_dead_tx, _pty_dead_rx) = tokio::sync::watch::channel(false);

        // Reader task: PTY -> scrollback + broadcast (spawn_blocking for sync IO)
        let tx_clone = tx.clone();
        let scrollback_clone = Arc::clone(&scrollback);
        let pty_dead_clone = pty_dead_tx.clone();
        let _reader_handle = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = Bytes::copy_from_slice(&buf[..n]);
                        let mut sb = scrollback_clone.blocking_lock();
                        sb.write(&buf[..n]);
                        drop(sb);
                        let _ = tx_clone.send(chunk);
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "PTY read error");
                        break;
                    }
                }
            }
            let _ = pty_dead_clone.send(true);
        });

        // Writer task: mpsc channel -> PTY (spawn_blocking for sync IO)
        let _writer_handle = tokio::task::spawn_blocking(move || {
            use std::io::Write;
            let mut writer = writer;
            while let Some(data) = write_rx.blocking_recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
            }
        });

        let master: Box<dyn portable_pty::MasterPty + Send> = pair.master;

        Ok(TerminalSession {
            tx,
            write_tx,
            scrollback,
            master: Arc::new(tokio::sync::Mutex::new(master)),
            child: Arc::new(tokio::sync::Mutex::new(child)),
            pty_dead: pty_dead_tx,
            client_count: AtomicUsize::new(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_claude_view_spawned_session_ids() {
        // Backward compat: cv-*-spawned sessions still valid.
        assert!(is_valid_session_id("cv-abcd1234"));
        assert!(is_valid_session_id("cv-00000000"));
        assert!(is_valid_session_id("cv-ffffffff"));
    }

    #[test]
    fn valid_user_spawned_tmux_names() {
        // Env-probe discovers user-spawned tmux sessions with arbitrary names.
        assert!(is_valid_session_id("work"));
        assert!(is_valid_session_id("my-session"));
        assert!(is_valid_session_id("dev_env"));
        assert!(is_valid_session_id("0"));
        assert!(is_valid_session_id("project.branch"));
        assert!(is_valid_session_id("MixedCase"));
        assert!(is_valid_session_id("a"));
        assert!(is_valid_session_id(&"a".repeat(128)));
    }

    #[test]
    fn rejects_shell_metacharacters_and_path_separators() {
        assert!(!is_valid_session_id("evil; rm -rf /"));
        assert!(!is_valid_session_id("a/b"));
        assert!(!is_valid_session_id("a:b"));
        assert!(!is_valid_session_id("a b"));
        assert!(!is_valid_session_id("$foo"));
        assert!(!is_valid_session_id("`cmd`"));
        assert!(!is_valid_session_id("a|b"));
        assert!(!is_valid_session_id("a&b"));
        assert!(!is_valid_session_id("a\"b"));
        assert!(!is_valid_session_id("a'b"));
        assert!(!is_valid_session_id("a\nb"));
        assert!(!is_valid_session_id("a\tb"));
    }

    #[test]
    fn rejects_empty_and_overlong_names() {
        assert!(!is_valid_session_id(""));
        assert!(!is_valid_session_id(&"a".repeat(129)));
    }

    #[test]
    fn manager_starts_empty() {
        let mgr = TerminalManager::new();
        assert_eq!(mgr.active_count(), 0);
    }
}
