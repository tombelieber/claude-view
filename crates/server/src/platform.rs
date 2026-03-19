//! Minimal platform abstraction for Unix-only APIs.
//!
//! Provides compile-time stubs for non-Unix platforms so the crate compiles
//! everywhere, while only running on macOS/Linux (enforced by the platform
//! gate in main.rs).

/// Connect to the sidecar Unix domain socket.
///
/// Returns a stream suitable for hyper HTTP/1.1 handshake.
#[cfg(unix)]
pub async fn connect_sidecar(addr: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(addr).await
}

#[cfg(not(unix))]
pub async fn connect_sidecar(addr: &str) -> std::io::Result<tokio::net::TcpStream> {
    let _ = addr;
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Unix sockets not available on this platform",
    ))
}

/// Check if a process with the given PID is alive.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill with signal 0 does not send a signal, only checks existence.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Send SIGTERM to a process.
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
    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Process signals not available on this platform",
        ))
    }
}

/// Kill a process (SIGKILL if force, SIGTERM otherwise).
pub fn kill_process(pid: u32, force: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        let result = unsafe { libc::kill(pid as i32, signal) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, force);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Process signals not available on this platform",
        ))
    }
}

/// Generate the sidecar Unix socket path.
pub fn sidecar_address() -> String {
    let pid = std::process::id();
    format!("/tmp/claude-view-sidecar-{pid}.sock")
}

/// Clean up the sidecar socket file.
pub fn cleanup_sidecar_address(addr: &str) {
    let _ = std::fs::remove_file(addr);
}
