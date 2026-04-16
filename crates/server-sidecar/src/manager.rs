//! SidecarManager struct and core accessors.

use std::collections::VecDeque;
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default sidecar TCP port.
const SIDECAR_PORT: u16 = 3001;

/// Circuit breaker: if the sidecar is spawned this many times within
/// `CIRCUIT_BREAKER_WINDOW`, stop spawning and surface an actionable error.
/// 10 spawns / 5min covers normal crash-and-recover; exceeding it means
/// something is persistently broken (Node OOM loop, corrupt binary, etc.)
/// and retrying wastes cycles while hiding the real failure.
pub(crate) const CIRCUIT_BREAKER_THRESHOLD: usize = 10;
pub(crate) const CIRCUIT_BREAKER_WINDOW: Duration = Duration::from_secs(300);

/// Manages the lifecycle of the Node.js sidecar child process.
///
/// Thread-safe: uses `Mutex<Option<Child>>` for the child handle.
/// The sidecar is lazy-started on first `ensure_running()` call.
///
/// Communication is over TCP HTTP (localhost:3001), not Unix socket.
pub struct SidecarManager {
    pub(crate) child: Mutex<Option<Child>>,
    pub(crate) base_url: String,
    pub(crate) port: u16,
    /// Monotonically increasing counter — incremented after each successful spawn.
    /// Consumers (`LiveSessionManager::ensure_session_control_alive`) compare this
    /// to a binding's `bound_at_generation` to detect stale control IDs that need
    /// lazy recovery after the sidecar restarted.
    pub(crate) spawn_generation: AtomicU64,
    /// Sliding window of recent spawn timestamps for the circuit breaker.
    pub(crate) spawn_history: Mutex<VecDeque<Instant>>,
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarManager {
    pub fn new() -> Self {
        let port = std::env::var("SIDECAR_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(SIDECAR_PORT);
        Self {
            child: Mutex::new(None),
            base_url: format!("http://localhost:{port}"),
            port,
            spawn_generation: AtomicU64::new(0),
            spawn_history: Mutex::new(VecDeque::new()),
        }
    }

    /// Current spawn generation. Starts at 0, increments on every successful
    /// spawn. Control bindings created before a restart will have a smaller
    /// generation than this; callers use the mismatch to detect staleness.
    pub fn generation(&self) -> u64 {
        self.spawn_generation.load(Ordering::Relaxed)
    }

    /// Record a spawn in the circuit-breaker window and return `Err(CircuitOpen)`
    /// if too many spawns have happened recently.
    pub(crate) fn check_and_record_spawn(&self) -> Result<(), super::error::SidecarError> {
        let now = Instant::now();
        let Ok(mut history) = self.spawn_history.lock() else {
            return Ok(()); // poisoned lock — don't make things worse by failing
        };
        // Drop entries outside the window.
        while history
            .front()
            .is_some_and(|t| now.duration_since(*t) > CIRCUIT_BREAKER_WINDOW)
        {
            history.pop_front();
        }
        if history.len() >= CIRCUIT_BREAKER_THRESHOLD {
            let oldest = history.front().copied().unwrap_or(now);
            let window_s = now.duration_since(oldest).as_secs();
            return Err(super::error::SidecarError::CircuitOpen(format!(
                "{} sidecar spawns in {}s — refusing to spawn again. Restart claude-view to reset.",
                history.len(),
                window_s
            )));
        }
        history.push_back(now);
        Ok(())
    }

    /// Get the TCP base URL for this sidecar instance.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check if the sidecar is currently running.
    ///
    /// In external mode (`CLAUDE_VIEW_SIDECAR_EXTERNAL=1`) we don't manage a
    /// child process, but the reconciler uses this to decide whether to
    /// trigger session recovery. Returning `true` in external mode is
    /// correct because the reconciler's next step is `ensure_running()`
    /// which health-checks the external sidecar — if it's down, recovery
    /// is correctly skipped by the resulting `HealthCheckTimeout`. This
    /// avoids a 10s-interval "Sidecar not running" warning spam in dev.
    pub fn is_running(&self) -> bool {
        if Self::is_external_mode() {
            return true;
        }
        let Ok(mut guard) = self.child.lock() else {
            tracing::error!("sidecar mutex poisoned, another thread panicked");
            return false;
        };
        if let Some(ref mut child) = *guard {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// Dev-mode opt-out: when set, `ensure_running()` never spawns/kills,
    /// and `is_running()` is optimistic. See `ensure_running()` doc.
    pub(crate) fn is_external_mode() -> bool {
        std::env::var("CLAUDE_VIEW_SIDECAR_EXTERNAL").is_ok()
    }

    /// Get the PID of the managed sidecar child process, if alive.
    pub fn child_pid(&self) -> Option<u32> {
        let Ok(mut guard) = self.child.lock() else {
            return None;
        };
        if let Some(ref mut child) = *guard {
            if matches!(child.try_wait(), Ok(None)) {
                return Some(child.id());
            }
        }
        None
    }
}
