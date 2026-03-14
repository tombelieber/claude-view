//! Cross-platform utilities for process management and IPC.
//!
//! Abstracts differences between Unix (signals, Unix sockets) and Windows
//! (taskkill, TCP sockets) so the rest of the server code is platform-agnostic.

/// Connect to the sidecar at the given address.
///
/// On Unix: connects via Unix domain socket (addr is a file path).
/// On Windows: connects via TCP (addr is "host:port").
pub async fn connect_sidecar(
    addr: &str,
) -> std::io::Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(addr).await
    }
    #[cfg(windows)]
    {
        tokio::net::TcpStream::connect(addr).await
    }
}

/// Check if a process with the given PID is alive.
///
/// Returns `false` for PIDs <= 1 (kernel/init) to guard against reparented processes.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill with signal 0 does not send a signal, only checks existence.
        // Returns 0 if process exists and we have permission, -1 with ESRCH if not.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use sysinfo::{ProcessesToUpdate, System};
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        sys.process(sysinfo::Pid::from_u32(pid)).is_some()
    }
}

/// Send a termination signal to a process (SIGTERM on Unix, taskkill on Windows).
pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }
}

/// Force-kill a process (SIGKILL on Unix, taskkill /F on Windows).
pub fn force_kill_process(pid: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }
}

/// Kill a process, with optional force (SIGKILL vs SIGTERM).
pub fn kill_process(pid: u32, force: bool) -> std::io::Result<()> {
    if force {
        force_kill_process(pid)
    } else {
        terminate_process(pid)
    }
}

/// Generate a sidecar address suitable for the current platform.
///
/// On Unix: returns a Unix socket path like `/tmp/claude-view-sidecar-{pid}.sock`
/// On Windows: finds an available TCP port and returns `127.0.0.1:{port}`
pub fn sidecar_address() -> String {
    let pid = std::process::id();
    #[cfg(unix)]
    {
        format!("/tmp/claude-view-sidecar-{pid}.sock")
    }
    #[cfg(windows)]
    {
        // Find an available port by binding to port 0, then release it.
        // Small TOCTOU race is acceptable for localhost-only usage.
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to find free TCP port");
        let port = listener
            .local_addr()
            .expect("failed to get local addr")
            .port();
        drop(listener);
        let _ = pid; // suppress unused warning
        format!("127.0.0.1:{port}")
    }
}

/// Clean up the sidecar address (remove socket file on Unix, no-op on Windows).
pub fn cleanup_sidecar_address(addr: &str) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(addr);
    }
    #[cfg(windows)]
    {
        let _ = addr;
    }
}
