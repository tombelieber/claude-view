//! Sidecar process lifecycle: spawn, readiness, shutdown, Drop.

use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::sleep;

use super::error::SidecarError;
use super::manager::SidecarManager;
use super::process::{find_sidecar_dir, kill_port_holder};

impl SidecarManager {
    /// Start sidecar if not already running. Returns the base URL.
    ///
    /// Idempotent: if the child is already alive, returns immediately.
    /// If the child died (crash), restarts it — subject to the circuit breaker
    /// (`CIRCUIT_BREAKER_THRESHOLD` spawns per `CIRCUIT_BREAKER_WINDOW`) to
    /// prevent runaway restart loops when the sidecar is persistently failing.
    ///
    /// `caller` labels WHICH code path triggered this — tagged into logs so we
    /// can track down autonomous spawn sources. Valid callers: `"boot"`,
    /// `"interact"`, `"ws_proxy"`, `"http_proxy"`. If you see anything else
    /// in logs, you've added a new spawn path without updating the tag set.
    ///
    /// External sidecar support: if a sidecar is already running on the
    /// configured port (e.g. via `tsx watch` in dev mode), we skip spawning
    /// and use the existing one. This allows `bun dev` to run the sidecar
    /// independently with hot reload.
    ///
    /// Dev-mode opt-out: set `CLAUDE_VIEW_SIDECAR_EXTERNAL=1` to disable all
    /// spawn/kill behaviour. The Rust server will only health-check and
    /// proxy — it will never spawn its own sidecar, and crucially will never
    /// call `kill_port_holder()` (which would kill `tsx watch` in dev).
    ///
    /// This is the industry-standard pattern for shared prod/dev ownership
    /// code (Tauri, Electron, Next.js standalone). Prod code asserts
    /// ownership by default; dev explicitly opts out via env var.
    pub async fn ensure_running(&self, caller: &'static str) -> Result<String, SidecarError> {
        // Dev-mode hands-off path: external orchestrator (e.g. concurrently
        // + tsx watch) owns the sidecar process. We only health-check.
        if Self::is_external_mode() {
            if self.health_check().await.is_ok() {
                return Ok(self.base_url.clone());
            }
            // Not ready yet — wait_for_ready() polls for 3s. If the external
            // sidecar isn't up by then, return HealthCheckTimeout so callers
            // can retry or surface an actionable error.
            return self.wait_for_ready(caller).await;
        }

        // Determine action under the lock, then release lock before any async work.
        // Mutex<Option<Child>> is !Send, so no .await can be held while guard is alive.
        let action = {
            let mut guard = self.child.lock().map_err(|_| {
                SidecarError::RequestError("sidecar mutex poisoned, another thread panicked".into())
            })?;

            if let Some(ref mut child) = *guard {
                match child.try_wait() {
                    Ok(None) => {
                        // Child alive — check if health endpoint responds
                        "check_health"
                    }
                    Ok(Some(status)) => {
                        tracing::warn!(caller, %status, "Sidecar exited, restarting...");
                        "spawn"
                    }
                    Err(e) => {
                        tracing::warn!(caller, error = %e, "Failed to check sidecar status");
                        "spawn"
                    }
                }
            } else {
                "spawn"
            }
        }; // guard dropped here — safe for async

        if action == "check_health" {
            // Quick health check — if it passes, sidecar is ready
            if self.health_check().await.is_ok() {
                return Ok(self.base_url.clone());
            }
            // Health check failed but child alive — wait for readiness
            return self.wait_for_ready(caller).await;
        }

        // Before spawning, check if an external sidecar is already running
        // on the port (e.g. `bun dev` runs sidecar independently via tsx watch).
        if self.health_check().await.is_ok() {
            tracing::info!(
                caller,
                port = self.port,
                "External sidecar detected on port, skipping spawn"
            );
            return Ok(self.base_url.clone());
        }

        // Circuit breaker: if we've spawned too many times recently, refuse
        // to spawn again. Something persistent is wrong; keep trying hides it.
        self.check_and_record_spawn()?;

        // Kill any stale process occupying the sidecar port (zombie from a
        // previous crash). Without this, node's listen() fails with EADDRINUSE.
        kill_port_holder(self.port);

        // Spawn new sidecar process
        let spawned_pid = {
            let mut guard = self
                .child
                .lock()
                .map_err(|_| SidecarError::RequestError("sidecar mutex poisoned".into()))?;

            // Find sidecar directory
            let sidecar_dir = find_sidecar_dir()?;
            let entry_point = sidecar_dir.join("dist/index.js");
            if !entry_point.exists() {
                return Err(SidecarError::SidecarDirNotFound);
            }

            // Verify Node.js is available
            if Command::new("node").arg("--version").output().is_err() {
                return Err(SidecarError::NodeNotFound);
            }

            // CLAUDE.md HARD RULE: Strip ALL `CLAUDE*` env vars when spawning
            // child processes. Use env_clear() then re-add safe vars only.
            let filtered_env: Vec<(String, String)> = std::env::vars()
                .filter(|(k, _)| !k.starts_with("CLAUDE") && k != "ANTHROPIC_API_KEY")
                .collect();
            let child = Command::new("node")
                .arg(&entry_point)
                .env_clear()
                .envs(filtered_env)
                .env("SIDECAR_PORT", self.port.to_string())
                .stdin(Stdio::null())
                // inherit → logs flow to server process stdout/stderr without pipe buffering.
                // piped+unread would fill the 64KB pipe buffer and deadlock the sidecar.
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(SidecarError::SpawnFailed)?;

            let pid = child.id();
            *guard = Some(child);
            pid
        }; // drop lock before async health check

        // Bump generation AFTER spawn is visible to callers — consumers of
        // `generation()` use it to mark their bindings stale post-restart.
        let gen_after = self.spawn_generation.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::info!(
            caller,
            pid = spawned_pid,
            port = self.port,
            generation = gen_after,
            "Spawned sidecar process on TCP"
        );

        self.wait_for_ready(caller).await
    }

    /// Poll sidecar health endpoint until it responds. Used after spawn
    /// and by concurrent callers that see the child alive but not yet ready.
    pub(crate) async fn wait_for_ready(
        &self,
        caller: &'static str,
    ) -> Result<String, SidecarError> {
        for attempt in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if self.health_check().await.is_ok() {
                tracing::info!(caller, attempts = attempt + 1, "Sidecar ready");
                return Ok(self.base_url.clone());
            }
        }
        Err(SidecarError::HealthCheckTimeout)
    }

    /// Gracefully shut down the sidecar: SIGTERM first, SIGKILL fallback.
    ///
    /// Sends SIGTERM so Node.js cleanup handlers (`process.on('SIGTERM')`) can
    /// run, then polls `try_wait()` for up to 3 seconds. Falls back to SIGKILL
    /// only if the process refuses to exit.
    ///
    /// NOTE: `child.wait()` / `child.try_wait()` are blocking calls.
    /// This is acceptable here because:
    /// 1. It's called from Drop and from the shutdown path (not hot path)
    /// 2. The poll loop uses short sleeps (50ms) with a hard 3s deadline
    /// 3. Making this async would require spawn_blocking and complicate Drop
    pub fn shutdown(&self) {
        let Ok(mut guard) = self.child.lock() else {
            tracing::error!("sidecar mutex poisoned, another thread panicked");
            return;
        };
        if let Some(ref mut child) = *guard {
            let pid = child.id();
            tracing::info!(pid, "Shutting down sidecar (SIGTERM)");

            // Send SIGTERM so Node.js cleanup handlers can run.
            // SAFETY: pid comes from a Child we own; the process exists.
            let term_result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if term_result != 0 {
                tracing::warn!(pid, errno = term_result, "SIGTERM send failed");
            }

            // Poll for graceful exit (up to 3s, 50ms intervals).
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tracing::info!(pid, ?status, "Sidecar exited gracefully");
                        *guard = None;
                        return;
                    }
                    Ok(None) => {
                        if std::time::Instant::now() >= deadline {
                            break; // timed out — fall through to SIGKILL
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        tracing::warn!(pid, error = %e, "try_wait failed, falling back to SIGKILL");
                        break;
                    }
                }
            }

            // Graceful shutdown timed out — force kill.
            tracing::warn!(pid, "Sidecar did not exit within 3s, sending SIGKILL");
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}
