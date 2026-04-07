// crates/server/src/sidecar/process.rs
//! Process utilities: port cleanup and sidecar directory discovery.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::error::SidecarError;

/// Kill stale node/sidecar processes holding a TCP port.
///
/// Only kills processes whose command name contains "node" (sidecar runs
/// via `node dist/index.js`). Leaves other apps alone.
pub(crate) fn kill_port_holder(port: u16) {
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
pub(crate) fn find_sidecar_dir() -> Result<PathBuf, SidecarError> {
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
