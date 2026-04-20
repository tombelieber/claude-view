//! Server port detection + reclamation helpers.
//!
//! Extracted from `main.rs` in CQRS Phase 7.c so the runtime entry point
//! stays focused on orchestration. Behaviour is unchanged.

/// Default port for the server.
pub const DEFAULT_PORT: u16 = 47892;

/// Get the server port from environment or use default.
///
/// Precedence: `CLAUDE_VIEW_PORT` → `PORT` → [`DEFAULT_PORT`].
pub fn get_port() -> u16 {
    std::env::var("CLAUDE_VIEW_PORT")
        .ok()
        .or_else(|| std::env::var("PORT").ok())
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Check if a process holding a port is a stale claude-view instance.
///
/// Returns true if the process name contains "claude-view" or
/// "claude_view". If we can't determine the process name, returns false
/// (don't kill unknowns).
fn is_claude_view_process(pid: &str) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-p", pid, "-o", "comm="])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let name = String::from_utf8_lossy(&o.stdout).to_lowercase();
            name.contains("claude-view") || name.contains("claude_view")
        }
        _ => false,
    }
}

/// Try to reclaim a port from a stale claude-view process.
///
/// Returns true if the port was freed (stale process killed).
/// Returns false if the port is held by a non-claude-view process.
pub fn try_reclaim_port(port: u16) -> bool {
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output();

    let pids = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return false,
    };

    let my_pid = std::process::id().to_string();
    let mut killed_any = false;

    for pid in pids.split_whitespace() {
        if pid == my_pid {
            continue;
        }
        if is_claude_view_process(pid) {
            eprintln!("  killing stale claude-view (PID {pid}) on port {port}");
            let _ = std::process::Command::new("kill")
                .args(["-9", pid])
                .status();
            killed_any = true;
        } else {
            eprintln!("  port {port} held by another app (PID {pid}), skipping");
        }
    }
    killed_any
}
