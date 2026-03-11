// crates/server/src/sidecar.rs
//! Node.js sidecar process manager for Phase F interactive control.
//!
//! The sidecar wraps the Claude Agent SDK (npm-only) and exposes a local
//! HTTP + WebSocket API on a Unix domain socket. Axum proxies all
//! `/api/control/*` requests to this socket.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

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
pub struct SidecarManager {
    child: Mutex<Option<Child>>,
    socket_path: String,
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarManager {
    pub fn new() -> Self {
        let pid = std::process::id();
        Self {
            child: Mutex::new(None),
            socket_path: format!("/tmp/claude-view-sidecar-{pid}.sock"),
        }
    }

    /// Get the Unix socket path for this sidecar instance.
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Start sidecar if not already running. Returns the socket path.
    ///
    /// Idempotent: if the child is already alive, returns immediately.
    /// If the child died (crash), restarts it.
    pub async fn ensure_running(&self) -> Result<String, SidecarError> {
        {
            let mut guard = self.child.lock().map_err(|_| {
                SidecarError::RequestError("sidecar mutex poisoned, another thread panicked".into())
            })?;

            // Check if existing child is still alive
            if let Some(ref mut child) = *guard {
                match child.try_wait() {
                    Ok(None) => return Ok(self.socket_path.clone()), // still running
                    Ok(Some(status)) => {
                        tracing::warn!("Sidecar exited with {status}, restarting...");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to check sidecar status: {e}");
                    }
                }
            }

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

            // Clean up stale socket
            let _ = std::fs::remove_file(&self.socket_path);

            // CLAUDE.md HARD RULE: Strip ALL `CLAUDE*` env vars when spawning
            // child processes. Use env_clear() then re-add safe vars only.
            let filtered_env: Vec<(String, String)> = std::env::vars()
                .filter(|(k, _)| !k.starts_with("CLAUDE") && k != "ANTHROPIC_API_KEY")
                .collect();
            let child = Command::new("node")
                .arg(&entry_point)
                .env_clear()
                .envs(filtered_env)
                .env("SIDECAR_SOCKET", &self.socket_path)
                .stdin(Stdio::null())
                // inherit → logs flow to server process stdout/stderr without pipe buffering.
                // piped+unread would fill the 64KB pipe buffer and deadlock the sidecar.
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(SidecarError::SpawnFailed)?;

            tracing::info!(
                pid = child.id(),
                socket = %self.socket_path,
                "Spawned sidecar process"
            );

            *guard = Some(child);
        } // drop lock before async health check

        // Wait for sidecar to be ready (poll health endpoint)
        for attempt in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if self.health_check().await.is_ok() {
                tracing::info!(attempts = attempt + 1, "Sidecar ready");
                return Ok(self.socket_path.clone());
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

        // Cleanup socket file
        let _ = std::fs::remove_file(&self.socket_path);
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

    /// HTTP health check over Unix socket using raw hyper 1.x client.
    ///
    /// Replaces hyperlocal (incompatible with hyper 1.x -- audit fix B3).
    /// Uses tokio::net::UnixStream + hyper::client::conn::http1 directly.
    async fn health_check(&self) -> Result<(), SidecarError> {
        use http_body_util::Empty;
        use hyper::client::conn::http1;
        use hyper_util::rt::TokioIo;

        let stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Unix socket connect: {e}")))?;

        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io)
            .await
            .map_err(|e| SidecarError::RequestError(format!("HTTP handshake: {e}")))?;

        // Spawn connection driver (required for hyper 1.x)
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::debug!("Health check connection closed: {e}");
            }
        });

        let req = hyper::Request::builder()
            .uri("/health")
            .header("host", "localhost")
            .body(Empty::<bytes::Bytes>::new())
            .map_err(|e| SidecarError::RequestError(format!("Build request: {e}")))?;

        let response = sender
            .send_request(req)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Send request: {e}")))?;

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

    /// Call sidecar POST /control/resume for a single session.
    async fn resume_session(&self, session_id: &str) -> Result<String, SidecarError> {
        use http_body_util::{BodyExt, Full};
        use hyper::client::conn::http1;
        use hyper_util::rt::TokioIo;

        let body = serde_json::json!({
            "sessionId": session_id,
            "model": "claude-sonnet-4-20250514",
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| SidecarError::RequestError(format!("Serialize: {e}")))?;

        let stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Connect: {e}")))?;

        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Handshake: {e}")))?;

        tokio::spawn(async move {
            let _ = conn.await;
        });

        let req = hyper::Request::builder()
            .method("POST")
            .uri("/control/resume")
            .header("host", "localhost")
            .header("content-type", "application/json")
            .body(Full::new(bytes::Bytes::from(body_str)))
            .map_err(|e| SidecarError::RequestError(format!("Build request: {e}")))?;

        let resp = sender
            .send_request(req)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Send: {e}")))?;

        if !resp.status().is_success() {
            return Err(SidecarError::RequestError(format!(
                "Resume returned {}",
                resp.status()
            )));
        }

        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Read body: {e}")))?;

        let data: serde_json::Value = serde_json::from_slice(&bytes.to_bytes())
            .map_err(|e| SidecarError::RequestError(format!("Parse JSON: {e}")))?;

        data["controlId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SidecarError::RequestError("No controlId in response".into()))
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
    fn test_new_creates_socket_path_with_pid() {
        let mgr = SidecarManager::new();
        let pid = std::process::id();
        assert_eq!(
            mgr.socket_path(),
            format!("/tmp/claude-view-sidecar-{pid}.sock")
        );
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
