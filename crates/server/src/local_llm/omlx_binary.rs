// crates/server/src/local_llm/omlx_binary.rs

use std::path::PathBuf;
use std::process::Command;

use tracing::debug;

#[derive(Debug, Clone)]
pub struct OmlxBinary {
    pub path: PathBuf,
}

/// Find the omlx binary. Checks:
/// 1. `OMLX_PATH` env var (explicit override)
/// 2. `which omlx` on PATH
pub fn detect() -> Option<OmlxBinary> {
    // 1. Explicit override
    if let Ok(path) = std::env::var("OMLX_PATH") {
        let p = PathBuf::from(&path);
        if p.is_file() {
            debug!(path = %p.display(), "omlx found via OMLX_PATH");
            return Some(OmlxBinary { path: p });
        }
    }

    // 2. which omlx
    let output = Command::new("which")
        .arg("omlx")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return None;
    }

    let p = PathBuf::from(&path);
    debug!(path = %p.display(), "omlx found on PATH");
    Some(OmlxBinary { path: p })
}

/// Verify the binary runs: `omlx --help` exits 0.
pub fn verify(binary: &OmlxBinary) -> bool {
    Command::new(&binary.path)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if a port is already in use (another process listening).
pub fn is_port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_finds_omlx_if_installed() {
        // This test passes on machines with omlx installed, skips otherwise
        if let Some(binary) = detect() {
            assert!(binary.path.exists());
            assert!(verify(&binary));
        }
    }

    #[test]
    fn is_port_in_use_returns_false_for_random_port() {
        // Port 39999 is very unlikely to be in use
        assert!(!is_port_in_use(39999));
    }
}
