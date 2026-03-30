// crates/server/src/local_llm/process.rs

use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// An oMLX child process owned by claude-view.
pub struct ManagedProcess {
    child: Child,
    port: u16,
    _stderr_drain: JoinHandle<()>,
}

impl ManagedProcess {
    /// Spawn `omlx serve --model-dir <dir> --port <port>`.
    pub async fn spawn(binary_path: &Path, model_dir: &Path, port: u16) -> Result<Self, String> {
        info!(
            binary = %binary_path.display(),
            model_dir = %model_dir.display(),
            port,
            "spawning oMLX"
        );

        // Ensure model_dir exists
        tokio::fs::create_dir_all(model_dir)
            .await
            .map_err(|e| format!("create model dir: {e}"))?;

        let mut child = Command::new(binary_path)
            .args([
                "serve",
                "--model-dir",
                &model_dir.to_string_lossy(),
                "--port",
                &port.to_string(),
                "--host",
                "127.0.0.1",
                "--log-level",
                "info",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false) // We handle shutdown ourselves
            .spawn()
            .map_err(|e| format!("spawn omlx: {e}"))?;

        // Drain stderr in a background task to prevent 64KB pipe buffer deadlock.
        let stderr = child.stderr.take().ok_or("failed to capture stderr")?;
        let stderr_drain = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(target: "omlx", "{}", line);
            }
        });

        let pid = child.id().unwrap_or(0);
        info!(pid, port, "oMLX process spawned");

        Ok(Self {
            child,
            port,
            _stderr_drain: stderr_drain,
        })
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Graceful shutdown: SIGTERM → poll 3s → SIGKILL.
    /// Follows the Sidecar Shutdown Protocol.
    pub async fn shutdown(&mut self) {
        let pid = match self.child.id() {
            Some(pid) => pid,
            None => {
                // Already exited
                let _ = self.child.wait().await;
                return;
            }
        };

        info!(pid, port = self.port, "shutting down oMLX");

        // 1. Send SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        }

        // 2. Poll try_wait for 3 seconds
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok(Some(_)) = self.child.try_wait() {
                info!(pid, "oMLX exited gracefully after SIGTERM");
                return;
            }
        }

        // 3. SIGKILL fallback
        warn!(pid, "oMLX did not exit after 3s, sending SIGKILL");
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }

    /// Return the port this process is bound to.
    pub fn port(&self) -> u16 {
        self.port
    }
}
