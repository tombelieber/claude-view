use std::process::Command;

/// Check if `omlx` is available on PATH (for UX guidance, not for spawning).
pub fn is_installed() -> bool {
    Command::new("which")
        .arg("omlx")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if a port is already in use (TCP connect succeeds).
pub fn is_port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_port_in_use_returns_false_for_random_port() {
        assert!(!is_port_in_use(39999));
    }
}
