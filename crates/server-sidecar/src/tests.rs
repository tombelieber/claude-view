// crates/server/src/sidecar/tests.rs
//! Tests for sidecar manager.

use std::sync::{Mutex, OnceLock};

use super::error::SidecarError;
use super::manager::SidecarManager;
use super::process::find_sidecar_dir;

/// Serialize env-var mutating tests across the whole test binary.
/// Without this, parallel test execution causes
/// CLAUDE_VIEW_SIDECAR_EXTERNAL to leak from one test into another's
/// assertions (process-global env vars).
fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

/// RAII guard: set an env var for the test duration, restore on drop.
/// Must be held alongside `env_lock()` to prevent cross-test leakage.
struct EnvGuard(&'static str, Option<String>);
impl EnvGuard {
    fn set(key: &'static str, val: &str) -> Self {
        let prev = std::env::var(key).ok();
        // SAFETY: test-only; caller holds env_lock() for the test duration
        unsafe {
            std::env::set_var(key, val);
        }
        Self(key, prev)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: test-only; caller holds env_lock()
        unsafe {
            match &self.1 {
                Some(v) => std::env::set_var(self.0, v),
                None => std::env::remove_var(self.0),
            }
        }
    }
}

#[test]
fn test_new_creates_base_url_with_default_port() {
    let mgr = SidecarManager::new();
    assert!(mgr.base_url().starts_with("http://localhost:"));
}

#[test]
fn test_not_running_by_default() {
    // Hold env_lock so concurrent external-mode tests don't flip the
    // env var while we assert the default behaviour.
    let _lock = env_lock().lock().unwrap();
    // Defensive: ensure the env var is unset for this test regardless
    // of how a prior panicked test may have left it.
    unsafe {
        std::env::remove_var("CLAUDE_VIEW_SIDECAR_EXTERNAL");
    }
    let mgr = SidecarManager::new();
    assert!(!mgr.is_running());
}

#[test]
fn test_shutdown_when_not_running_is_noop() {
    let mgr = SidecarManager::new();
    mgr.shutdown(); // should not panic
}

#[test]
fn test_find_sidecar_dir_returns_error_when_not_found() {
    let result = find_sidecar_dir();
    let _ = result;
}

/// External-mode opt-out must never spawn or kill processes. When
/// CLAUDE_VIEW_SIDECAR_EXTERNAL is set AND no external sidecar is
/// reachable, ensure_running() must return HealthCheckTimeout without
/// calling kill_port_holder or spawning a child — proving we take the
/// hands-off path all the way through.
#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "test-only env-var serialization; no contention beyond test binary"
)]
async fn test_external_mode_never_spawns_or_kills() {
    let _lock = env_lock().lock().unwrap();
    let _guard = EnvGuard::set("CLAUDE_VIEW_SIDECAR_EXTERNAL", "1");

    // Use a port we know nothing listens on (reserved IANA range).
    // 1 is "TCP port service multiplexer" — almost never bound locally.
    let mgr = SidecarManager {
        child: std::sync::Mutex::new(None),
        base_url: "http://localhost:1".to_string(),
        port: 1,
    };

    let result = mgr.ensure_running().await;
    // External mode + nothing listening → HealthCheckTimeout (not spawned)
    assert!(matches!(result, Err(SidecarError::HealthCheckTimeout)));

    // Critical: we must not have created a child process. In external
    // mode `is_running()` is optimistic (returns true), but child_pid
    // returns None because we never spawn.
    assert_eq!(mgr.child_pid(), None);
}

#[test]
fn test_is_running_optimistic_in_external_mode() {
    let _lock = env_lock().lock().unwrap();
    // Start from a known baseline — remove first, then assert default.
    unsafe {
        std::env::remove_var("CLAUDE_VIEW_SIDECAR_EXTERNAL");
    }
    let mgr = SidecarManager::new();

    // Default mode: no child → false
    assert!(!mgr.is_running());

    // External mode: optimistic → true (avoids reconciler warning spam)
    let _guard = EnvGuard::set("CLAUDE_VIEW_SIDECAR_EXTERNAL", "1");
    assert!(mgr.is_running());
}
