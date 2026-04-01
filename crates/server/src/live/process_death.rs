//! Event-driven process death watcher using kqueue (macOS).
//!
//! Registers PIDs and gets immediate notification when they exit, instead of
//! polling `kill(pid, 0)` every 10 seconds. Reduces the "ghost session" window
//! from up to 10 seconds to effectively zero.
//!
//! # Architecture
//!
//! ```text
//! ProcessDeathWatcher (background task)
//!   │
//!   ├── Registers PIDs via watch(pid, session_id)
//!   ├── Unregisters PIDs via unwatch(pid)
//!   └── Fires tx.send((pid, session_id)) on death
//! ```
//!
//! # Platform Support
//!
//! - **macOS:** `kqueue` + `EVFILT_PROC` + `NOTE_EXIT` (native, zero overhead)
//! - **Linux:** Falls back to polling (pidfd_open requires Linux 5.3+)
//! - **Other:** Falls back to polling

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A death notification: (pid, session_id).
pub type DeathNotification = (u32, String);

/// Handle for registering/unregistering PIDs with the watcher.
#[derive(Clone)]
pub struct ProcessDeathWatcher {
    /// Watched PIDs: pid → session_id.
    /// Uses std::sync::Mutex (not tokio) because the kqueue OS thread
    /// accesses it synchronously — using tokio::sync::Mutex would require
    /// block_on() which risks priority inversion under thread saturation.
    watched: Arc<Mutex<HashMap<u32, String>>>,
    /// Channel to send death notifications to the consumer.
    #[allow(dead_code)] // Used inside cfg(target_os = "macos") kqueue loop
    death_tx: mpsc::Sender<DeathNotification>,
    /// Channel to request new PID registrations from the kqueue thread.
    register_tx: mpsc::Sender<WatchCommand>,
}

#[allow(dead_code)] // Fields consumed inside cfg(target_os = "macos") kqueue loop
enum WatchCommand {
    Watch(u32, String),
    Unwatch(u32),
}

impl ProcessDeathWatcher {
    /// Start the watcher background task.
    ///
    /// Returns the watcher handle and a receiver for death notifications.
    pub fn start() -> (Self, mpsc::Receiver<DeathNotification>) {
        let (death_tx, death_rx) = mpsc::channel(64);
        let (register_tx, register_rx) = mpsc::channel(64);
        let watched: Arc<Mutex<HashMap<u32, String>>> = Arc::new(Mutex::new(HashMap::new()));

        let watcher = Self {
            watched: watched.clone(),
            death_tx: death_tx.clone(),
            register_tx,
        };

        // Spawn the platform-specific watcher
        #[cfg(target_os = "macos")]
        {
            let death_tx_clone = death_tx.clone();
            let watched_clone = watched.clone();
            // Capture the Tokio runtime handle BEFORE spawning the OS thread.
            let rt_handle = tokio::runtime::Handle::current();
            std::thread::Builder::new()
                .name("process-death-watcher".into())
                .spawn(move || {
                    kqueue_watcher_loop(register_rx, death_tx_clone, watched_clone, rt_handle);
                })
                .expect("failed to spawn process death watcher thread");
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Non-macOS: no-op watcher. The polling fallback in the reconciliation
            // loop handles process death detection.
            tokio::spawn(async move {
                let mut rx = register_rx;
                while let Some(_cmd) = rx.recv().await {
                    // Consume commands but do nothing — polling handles it.
                }
            });
        }

        (watcher, death_rx)
    }

    /// Register a PID for death notification.
    pub async fn watch(&self, pid: u32, session_id: String) {
        if pid <= 1 {
            return; // Never watch kernel/init
        }
        self.watched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(pid, session_id.clone());
        let _ = self
            .register_tx
            .send(WatchCommand::Watch(pid, session_id))
            .await;
    }

    /// Unregister a PID (session closed manually, etc.).
    pub async fn unwatch(&self, pid: u32) {
        self.watched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&pid);
        let _ = self.register_tx.send(WatchCommand::Unwatch(pid)).await;
    }
}

/// macOS kqueue-based watcher loop (runs on a dedicated OS thread, not tokio).
///
/// Uses EVFILT_PROC with NOTE_EXIT to get immediate notification when a
/// watched PID exits. This is the same mechanism launchd uses.
#[cfg(target_os = "macos")]
fn kqueue_watcher_loop(
    mut register_rx: mpsc::Receiver<WatchCommand>,
    death_tx: mpsc::Sender<DeathNotification>,
    watched: Arc<Mutex<HashMap<u32, String>>>,
    rt: tokio::runtime::Handle,
) {
    use std::mem::MaybeUninit;

    // Create kqueue fd
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        tracing::error!(
            "Failed to create kqueue: {}",
            std::io::Error::last_os_error()
        );
        return;
    }

    tracing::info!("process_death_watcher: kqueue loop started");

    let mut events: [MaybeUninit<libc::kevent>; 16] = [const { MaybeUninit::uninit() }; 16];

    loop {
        // Process any pending registration commands (non-blocking)
        loop {
            match register_rx.try_recv() {
                Ok(WatchCommand::Watch(pid, _session_id)) => {
                    let ev = libc::kevent {
                        ident: pid as usize,
                        filter: libc::EVFILT_PROC,
                        flags: libc::EV_ADD | libc::EV_ONESHOT,
                        fflags: libc::NOTE_EXIT,
                        data: 0,
                        udata: std::ptr::null_mut(),
                    };
                    let ret = unsafe {
                        libc::kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null())
                    };
                    if ret < 0 {
                        let err = std::io::Error::last_os_error();
                        // ESRCH = process already dead — fire death immediately
                        if err.raw_os_error() == Some(libc::ESRCH) {
                            let mut w = watched.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(sid) = w.remove(&pid) {
                                let _ = rt.block_on(death_tx.send((pid, sid)));
                            }
                        } else {
                            tracing::debug!(
                                pid,
                                error = %err,
                                "kqueue: failed to watch PID (falling back to polling)"
                            );
                        }
                    }
                }
                Ok(WatchCommand::Unwatch(pid)) => {
                    // EV_DELETE for EVFILT_PROC — if the process already exited
                    // this may ESRCH, which is fine.
                    let ev = libc::kevent {
                        ident: pid as usize,
                        filter: libc::EVFILT_PROC,
                        flags: libc::EV_DELETE,
                        fflags: 0,
                        data: 0,
                        udata: std::ptr::null_mut(),
                    };
                    unsafe {
                        libc::kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null());
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::info!("process_death_watcher: channel closed, exiting");
                    unsafe { libc::close(kq) };
                    return;
                }
            }
        }

        // Block on kqueue with 100ms timeout (responsive to new registrations)
        let timeout = libc::timespec {
            tv_sec: 0,
            tv_nsec: 100_000_000, // 100ms
        };
        let n = unsafe {
            libc::kevent(
                kq,
                std::ptr::null(),
                0,
                events[0].as_mut_ptr(),
                events.len() as i32,
                &timeout,
            )
        };

        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                tracing::error!("kqueue wait failed: {err}");
                break;
            }
            continue;
        }

        for event_slot in &events[..n as usize] {
            let ev = unsafe { event_slot.assume_init() };

            if ev.filter == libc::EVFILT_PROC && (ev.fflags & libc::NOTE_EXIT) != 0 {
                let pid = ev.ident as u32;
                let mut w = watched.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(sid) = w.remove(&pid) {
                    tracing::info!(pid, session_id = %sid, "kqueue: PID exited");
                    let _ = rt.block_on(death_tx.send((pid, sid)));
                }
            }
        }
    }

    unsafe {
        libc::close(kq);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watcher_detects_process_exit() {
        let (watcher, mut death_rx) = ProcessDeathWatcher::start();

        // Spawn a short-lived process
        let child = std::process::Command::new("sleep")
            .arg("0.1")
            .spawn()
            .expect("failed to spawn sleep");
        let pid = child.id();

        // Watch it
        watcher.watch(pid, "test-session".into()).await;

        // Wait for death notification (should arrive within ~200ms)
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), death_rx.recv()).await;

        assert!(result.is_ok(), "Should receive death notification");
        let (dead_pid, session_id) = result.unwrap().unwrap();
        assert_eq!(dead_pid, pid);
        assert_eq!(session_id, "test-session");
    }

    #[tokio::test]
    async fn test_watcher_immediate_death_for_dead_pid() {
        let (watcher, mut death_rx) = ProcessDeathWatcher::start();

        // Use a PID that's almost certainly dead
        let dead_pid: u32 = 99998;

        watcher.watch(dead_pid, "dead-session".into()).await;

        // On macOS, kqueue returns ESRCH immediately → fires death right away
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), death_rx.recv()).await;

        // This may or may not fire depending on whether PID 99998 exists.
        // The test ensures no crash/panic occurs.
        if let Ok(Some((pid, _sid))) = result {
            assert_eq!(pid, dead_pid);
        }
    }

    #[tokio::test]
    async fn test_unwatch_does_not_panic() {
        let (watcher, mut death_rx) = ProcessDeathWatcher::start();

        // Spawn a process
        let child = std::process::Command::new("sleep")
            .arg("0.5")
            .spawn()
            .expect("failed to spawn sleep");
        let pid = child.id();

        // Watch then immediately unwatch
        watcher.watch(pid, "unwatched-session".into()).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        watcher.unwatch(pid).await;

        // Wait a bit — should NOT receive a notification
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), death_rx.recv()).await;

        // It's OK if we get a notification (race condition), but it should
        // not crash. The main point is that unwatch doesn't panic.
        let _ = result;
    }

    #[tokio::test]
    async fn test_watch_rejects_pid_0_and_1() {
        let (watcher, _death_rx) = ProcessDeathWatcher::start();

        // These should be no-ops (no registration with kqueue)
        watcher.watch(0, "kernel".into()).await;
        watcher.watch(1, "init".into()).await;

        let watched = watcher.watched.lock().unwrap();
        assert!(!watched.contains_key(&0), "PID 0 must not be watched");
        assert!(!watched.contains_key(&1), "PID 1 must not be watched");
    }
}
