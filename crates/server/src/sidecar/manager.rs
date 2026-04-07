// crates/server/src/sidecar/manager.rs
//! SidecarManager struct and core accessors.

use std::process::Child;
use std::sync::Mutex;

/// Default sidecar TCP port.
const SIDECAR_PORT: u16 = 3001;

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
        }
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
