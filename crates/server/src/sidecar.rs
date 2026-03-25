// crates/server/src/sidecar.rs
//! Node.js sidecar process manager for interactive control.
//!
//! The sidecar wraps the Claude Agent SDK (npm-only) and exposes a local
//! HTTP + WebSocket API on TCP port 3001. The frontend connects directly
//! to the sidecar via Vite proxy; the Rust server uses this manager for
//! lifecycle management (spawn, health check, model fetch).

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

/// Default sidecar TCP port.
const SIDECAR_PORT: u16 = 3001;

/// Errors from sidecar operations.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("Failed to spawn sidecar: {0}")]
    SpawnFailed(std::io::Error),
    #[error("Sidecar health check timed out after 3s")]
    HealthCheckTimeout,
    #[error("Sidecar directory not found (set SIDECAR_DIR or place sidecar/ next to binary)")]
    SidecarDirNotFound,
    #[error("Node.js not found in PATH (required for interactive mode)")]
    NodeNotFound,
    #[error("Sidecar returned error: {0}")]
    RequestError(String),
}

/// Manages the lifecycle of the Node.js sidecar child process.
///
/// Thread-safe: uses `Mutex<Option<Child>>` for the child handle.
/// The sidecar is lazy-started on first `ensure_running()` call.
///
/// Communication is over TCP HTTP (localhost:3001), not Unix socket.
pub struct SidecarManager {
    child: Mutex<Option<Child>>,
    base_url: String,
    port: u16,
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

    /// Start sidecar if not already running. Returns the base URL.
    ///
    /// Idempotent: if the child is already alive, returns immediately.
    /// If the child died (crash), restarts it.
    ///
    /// External sidecar support: if a sidecar is already running on the
    /// configured port (e.g. via `tsx watch` in dev mode), we skip spawning
    /// and use the existing one. This allows `bun dev` to run the sidecar
    /// independently with hot reload.
    pub async fn ensure_running(&self) -> Result<String, SidecarError> {
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
                        tracing::warn!("Sidecar exited with {status}, restarting...");
                        "spawn"
                    }
                    Err(e) => {
                        tracing::warn!("Failed to check sidecar status: {e}");
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
            return self.wait_for_ready().await;
        }

        // Before spawning, check if an external sidecar is already running
        // on the port (e.g. `bun dev` runs sidecar independently via tsx watch).
        if self.health_check().await.is_ok() {
            tracing::info!(
                port = self.port,
                "External sidecar detected on port, skipping spawn"
            );
            return Ok(self.base_url.clone());
        }

        // Kill any stale process occupying the sidecar port (zombie from a
        // previous crash). Without this, node's listen() fails with EADDRINUSE.
        Self::kill_port_holder(self.port);

        // Spawn new sidecar process
        {
            let mut guard = self
                .child
                .lock()
                .map_err(|_| SidecarError::RequestError("sidecar mutex poisoned".into()))?;

            // Find sidecar directory
            let sidecar_dir = Self::find_sidecar_dir()?;
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

            tracing::info!(
                pid = child.id(),
                port = self.port,
                "Spawned sidecar process on TCP"
            );

            *guard = Some(child);
        } // drop lock before async health check

        self.wait_for_ready().await
    }

    /// Poll sidecar health endpoint until it responds. Used after spawn
    /// and by concurrent callers that see the child alive but not yet ready.
    async fn wait_for_ready(&self) -> Result<String, SidecarError> {
        for attempt in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if self.health_check().await.is_ok() {
                tracing::info!(attempts = attempt + 1, "Sidecar ready");
                return Ok(self.base_url.clone());
            }
        }
        Err(SidecarError::HealthCheckTimeout)
    }

    /// Kill the sidecar child process and wait for it to exit.
    ///
    /// NOTE: `child.wait()` is a blocking call (std::process::Child::wait).
    /// This is acceptable here because:
    /// 1. It's called from Drop and from the shutdown path (not hot path)
    /// 2. The child is already killed, so wait() returns almost immediately
    /// 3. Making this async would require spawn_blocking and complicate Drop
    pub fn shutdown(&self) {
        let Ok(mut guard) = self.child.lock() else {
            tracing::error!("sidecar mutex poisoned, another thread panicked");
            return;
        };
        if let Some(ref mut child) = *guard {
            tracing::info!(pid = child.id(), "Shutting down sidecar");
            let _ = child.kill();
            let _ = child.wait(); // blocking but brief — child already killed
        }
        *guard = None;
    }

    /// Check if the sidecar is currently running.
    pub fn is_running(&self) -> bool {
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

    /// HTTP health check over TCP using reqwest.
    async fn health_check(&self) -> Result<(), SidecarError> {
        let url = format!("{}/health", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| SidecarError::RequestError(format!("Build HTTP client: {e}")))?;

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Health check request: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SidecarError::RequestError(format!(
                "Health check returned {}",
                response.status()
            )))
        }
    }

    /// Re-resume all previously controlled sessions after sidecar restart.
    pub async fn recover_controlled_sessions(
        &self,
        session_ids: &[(String, String)], // (session_id, old_control_id)
    ) -> Vec<(String, String)> {
        let mut recovered = Vec::new();
        for (session_id, _old_control_id) in session_ids {
            match self.resume_session(session_id).await {
                Ok(new_control_id) => {
                    tracing::info!(
                        session_id = %session_id,
                        new_control_id = %new_control_id,
                        "Recovered controlled session after sidecar restart"
                    );
                    recovered.push((session_id.clone(), new_control_id));
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to recover controlled session"
                    );
                }
            }
        }
        recovered
    }

    /// Call sidecar POST /api/sidecar/sessions/:id/resume for a single session.
    async fn resume_session(&self, session_id: &str) -> Result<String, SidecarError> {
        let url = format!(
            "{}/api/sidecar/sessions/{}/resume",
            self.base_url, session_id
        );
        let body = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Resume request: {e}")))?;

        if !resp.status().is_success() {
            return Err(SidecarError::RequestError(format!(
                "Resume returned {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Parse JSON: {e}")))?;

        data["controlId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SidecarError::RequestError("No controlId in response".into()))
    }

    /// Kill stale node/sidecar processes holding a TCP port.
    ///
    /// Only kills processes whose command name contains "node" (sidecar runs
    /// via `node dist/index.js`). Leaves other apps alone.
    fn kill_port_holder(port: u16) {
        let output = Command::new("lsof")
            .args(["-ti", &format!(":{port}")])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        let pids = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return,
        };

        let my_pid = std::process::id().to_string();
        for pid in pids.split_whitespace() {
            if pid == my_pid {
                continue;
            }
            // Only kill node processes (sidecar runs as `node dist/index.js`)
            let is_node = Command::new("ps")
                .args(["-p", pid, "-o", "comm="])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .to_lowercase()
                        .contains("node")
                })
                .unwrap_or(false);
            if is_node {
                tracing::info!(pid, port, "Killing stale node process on sidecar port");
                let _ = Command::new("kill").args(["-9", pid]).status();
            } else {
                tracing::warn!(pid, port, "Non-node process on sidecar port, skipping");
            }
        }
    }

    /// Locate the sidecar directory.
    ///
    /// Priority:
    /// 1. `SIDECAR_DIR` env var (set by npx-cli)
    /// 2. `./sidecar/` relative to the binary (npx distribution)
    /// 3. `./sidecar/` relative to CWD (dev mode: `cargo run` from repo root)
    fn find_sidecar_dir() -> Result<PathBuf, SidecarError> {
        // 1. Explicit env var
        if let Ok(dir) = std::env::var("SIDECAR_DIR") {
            let p = PathBuf::from(&dir);
            if p.exists() {
                return Ok(p);
            }
            tracing::warn!(sidecar_dir = %dir, "SIDECAR_DIR set but directory does not exist");
        }

        // 2. Binary-relative (npx distribution)
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(canonical) = exe.canonicalize() {
                if let Some(exe_dir) = canonical.parent() {
                    let bin_sidecar = exe_dir.join("sidecar");
                    if bin_sidecar.join("dist/index.js").exists() {
                        return Ok(bin_sidecar);
                    }
                }
            }
        }

        // 3. CWD-relative (dev mode)
        let cwd_sidecar = PathBuf::from("sidecar");
        if cwd_sidecar.join("dist/index.js").exists() {
            return Ok(cwd_sidecar);
        }

        Err(SidecarError::SidecarDirNotFound)
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_base_url_with_default_port() {
        let mgr = SidecarManager::new();
        assert!(mgr.base_url().starts_with("http://localhost:"));
    }

    #[test]
    fn test_not_running_by_default() {
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
        let result = SidecarManager::find_sidecar_dir();
        let _ = result;
    }
}
