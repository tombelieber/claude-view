//! Read tmux binding from a process's environment.
//!
//! When tmux spawns a shell (or any command), it bakes two env vars into the
//! child at execve time: `TMUX=<socket>,<server_pid>,<session_id>` and
//! `TMUX_PANE=%<n>`. These inherit transitively through every descendant, so
//! reading them from a Claude PID is the authoritative answer to "is this
//! process inside a tmux pane, and if so which one?" — no process-tree
//! walking, no tmux enumeration.
//!
//! Platform notes:
//! - **Linux**: `/proc/{pid}/environ` is readable by the process owner. Env
//!   is NUL-separated key=value pairs.
//! - **macOS**: `ps eww -p {pid}` prints env vars after the command line.
//!   Each var is separated by whitespace. SIP-protected binaries (e.g.
//!   `/bin/sleep`) return a blank env line — but Claude is a user binary
//!   (Homebrew / ~/.bun/bin / /usr/local/bin), so SIP never applies to us.

use std::process::Command;

/// Parsed TMUX env from a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxEnv {
    /// Absolute path to the tmux server socket. e.g. `/private/tmp/tmux-501/default`.
    pub socket: String,
    /// tmux server PID.
    pub server_pid: u32,
    /// tmux pane ID, e.g. `%3`. Globally unique within the tmux server.
    pub pane_id: String,
}

/// Read tmux binding from a PID's environment.
///
/// Returns `Some(TmuxEnv)` if the process has both `TMUX` and `TMUX_PANE`
/// env vars set (i.e. it is a descendant of a tmux pane). Returns `None`
/// if the process isn't in tmux, doesn't exist, or its env is unreadable.
pub fn read_tmux_env(pid: u32) -> Option<TmuxEnv> {
    let raw = read_raw_env(pid)?;
    parse_tmux_env(&raw)
}

/// Parse `TMUX=...` and `TMUX_PANE=...` from a raw env blob.
///
/// Accepts both formats:
/// - NUL-separated (`/proc/{pid}/environ` on Linux)
/// - Whitespace-separated (`ps eww` output on macOS, after the command)
///
/// Extracted for unit testing — the platform-specific I/O wrappers call
/// `read_raw_env` then this.
pub(crate) fn parse_tmux_env(raw: &str) -> Option<TmuxEnv> {
    let mut tmux_val: Option<&str> = None;
    let mut pane_val: Option<&str> = None;

    // Split on both NUL (Linux environ) and whitespace (ps output).
    // Using char-array split to handle both transparently.
    for token in raw.split(['\0', ' ', '\n', '\t']) {
        if let Some(v) = token.strip_prefix("TMUX=") {
            tmux_val = Some(v);
        } else if let Some(v) = token.strip_prefix("TMUX_PANE=") {
            pane_val = Some(v);
        }
        if tmux_val.is_some() && pane_val.is_some() {
            break;
        }
    }

    let tmux = tmux_val?;
    let pane_id = pane_val?.to_string();

    // TMUX format: socket,server_pid,session_id
    let mut parts = tmux.splitn(3, ',');
    let socket = parts.next()?.to_string();
    let server_pid = parts.next()?.parse::<u32>().ok()?;
    // Third part (session_id) is informational, we don't need it.

    if socket.is_empty() || pane_id.is_empty() || !pane_id.starts_with('%') {
        return None;
    }

    Some(TmuxEnv {
        socket,
        server_pid,
        pane_id,
    })
}

#[cfg(target_os = "linux")]
fn read_raw_env(pid: u32) -> Option<String> {
    let path = format!("/proc/{pid}/environ");
    let bytes = std::fs::read(path).ok()?;
    // environ is NUL-separated bytes; lossy decode is fine for env vars
    // which are expected to be ASCII/UTF-8.
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(target_os = "macos")]
fn read_raw_env(pid: u32) -> Option<String> {
    // `ps eww -p PID` prints one line:
    //   "  PID   TT  STAT      TIME COMMAND args... KEY=VALUE KEY=VALUE ..."
    // Env vars are appended after the command line, space-separated.
    // We only care about TMUX= / TMUX_PANE= prefixes so we return the whole
    // line and let parse_tmux_env split it.
    let output = Command::new("ps")
        .args(["eww", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // First line is the header (PID TT STAT TIME COMMAND). Skip it.
    let body = stdout.lines().skip(1).collect::<Vec<_>>().join(" ");
    if body.trim().is_empty() {
        return None;
    }
    Some(body)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_raw_env(_pid: u32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_environ_format() {
        // NUL-separated like /proc/{pid}/environ
        let raw = "PATH=/usr/bin\0HOME=/home/user\0TMUX=/tmp/tmux-1000/default,12345,0\0TMUX_PANE=%7\0SHELL=/bin/zsh\0";
        let env = parse_tmux_env(raw).unwrap();
        assert_eq!(env.socket, "/tmp/tmux-1000/default");
        assert_eq!(env.server_pid, 12345);
        assert_eq!(env.pane_id, "%7");
    }

    #[test]
    fn parses_macos_ps_format() {
        // Space-separated like ps eww output
        let raw = "PATH=/usr/bin HOME=/Users/tom TMUX=/private/tmp/tmux-501/default,12561,0 TMUX_PANE=%0 SHELL=/bin/zsh";
        let env = parse_tmux_env(raw).unwrap();
        assert_eq!(env.socket, "/private/tmp/tmux-501/default");
        assert_eq!(env.server_pid, 12561);
        assert_eq!(env.pane_id, "%0");
    }

    #[test]
    fn returns_none_when_not_in_tmux() {
        let raw = "PATH=/usr/bin\0HOME=/home/user\0SHELL=/bin/zsh\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn returns_none_when_tmux_present_but_pane_missing() {
        // TMUX set but TMUX_PANE missing — malformed, reject.
        let raw = "TMUX=/tmp/tmux-1000/default,12345,0\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn returns_none_when_pane_present_but_tmux_missing() {
        let raw = "TMUX_PANE=%7\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn returns_none_on_malformed_tmux_var() {
        // Missing server_pid comma.
        let raw = "TMUX=/tmp/tmux-1000/default\0TMUX_PANE=%7\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn returns_none_on_non_numeric_server_pid() {
        let raw = "TMUX=/tmp/tmux-1000/default,abc,0\0TMUX_PANE=%7\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn returns_none_on_pane_id_without_percent() {
        // pane_id must start with '%' — tmux format contract.
        let raw = "TMUX=/tmp/tmux-1000/default,12345,0\0TMUX_PANE=7\0";
        assert!(parse_tmux_env(raw).is_none());
    }

    #[test]
    fn handles_multi_char_pane_ids() {
        let raw = "TMUX=/tmp/tmux-1000/default,12345,0\0TMUX_PANE=%142\0";
        let env = parse_tmux_env(raw).unwrap();
        assert_eq!(env.pane_id, "%142");
    }

    #[test]
    fn handles_empty_input() {
        assert!(parse_tmux_env("").is_none());
    }

    #[test]
    fn handles_only_whitespace() {
        assert!(parse_tmux_env("   \n\t  ").is_none());
    }

    #[test]
    fn tolerates_extra_whitespace_in_ps_format() {
        let raw = "  COMMAND=/usr/local/bin/claude   TMUX=/tmp/tmux-501/default,100,0  TMUX_PANE=%3  HOME=/Users/tom";
        let env = parse_tmux_env(raw).unwrap();
        assert_eq!(env.socket, "/tmp/tmux-501/default");
        assert_eq!(env.server_pid, 100);
        assert_eq!(env.pane_id, "%3");
    }

    // Integration test: actually read THIS test process's env.
    // Only runs inside tmux; no-op otherwise.
    #[test]
    fn integration_reads_own_env_when_in_tmux() {
        let in_tmux = std::env::var("TMUX").is_ok() && std::env::var("TMUX_PANE").is_ok();
        if !in_tmux {
            eprintln!("SKIP: not inside tmux");
            return;
        }
        let my_pid = std::process::id();
        let env = read_tmux_env(my_pid);
        assert!(
            env.is_some(),
            "read_tmux_env returned None for own PID {my_pid} despite TMUX vars being set in this test's env"
        );
        let env = env.unwrap();
        let expected_pane = std::env::var("TMUX_PANE").unwrap();
        assert_eq!(env.pane_id, expected_pane);
    }

    // End-to-end integration: spawn a real user-named tmux session (NOT cv-*),
    // read the pane's shell PID, probe its env, resolve pane_id to session name
    // through RealTmux, and assert the round trip matches. This is the full
    // discovery path for user-spawned tmux sessions exercised in a single test.
    //
    // Requires: tmux binary on PATH. Skips otherwise.
    #[test]
    fn integration_user_spawned_tmux_round_trip() {
        use crate::routes::cli_sessions::tmux::{RealTmux, TmuxCommand};
        use std::process::Command;

        let real = RealTmux;
        if !real.is_available() {
            eprintln!("SKIP: tmux not available");
            return;
        }

        // Unique name so parallel test runs don't collide. Intentionally NOT
        // cv-prefixed to exercise the user-spawned path.
        let test_name = format!("envprobe-poc-{}", std::process::id());

        // Create a bare session (default shell, no command override) so the
        // pane runs the user's $SHELL — a user binary that `ps eww` can
        // introspect reliably on macOS.
        let created = Command::new("tmux")
            .args(["new-session", "-d", "-s", &test_name])
            .status();
        if !matches!(created, Ok(s) if s.success()) {
            eprintln!("SKIP: could not create test tmux session");
            return;
        }

        // Teardown guard — always kill the session even if an assertion fails.
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = Command::new("tmux")
                    .args(["kill-session", "-t", &self.0])
                    .status();
            }
        }
        let _cleanup = Cleanup(test_name.clone());

        // Ask tmux for the pane's shell PID — this is the process we will
        // probe. We do NOT care what pane_pid is semantically; we just need
        // a live PID inside this tmux session.
        let pane_pid = real.pane_pid(&test_name).expect("pane_pid should exist");
        assert!(pane_pid > 0, "pane_pid should be non-zero");

        // Probe env. This is the core of the feature: starting from any PID
        // running inside a tmux pane, we recover the tmux binding.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut env = read_tmux_env(pane_pid);
        while env.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            env = read_tmux_env(pane_pid);
        }
        let env = env.unwrap_or_else(|| {
            panic!(
                "read_tmux_env returned None for pane_pid {pane_pid} \
                 of tmux session '{test_name}' — the env-probe approach is broken"
            )
        });
        assert!(
            env.pane_id.starts_with('%'),
            "pane_id should start with '%', got {:?}",
            env.pane_id
        );

        // Resolve pane_id back to the session name. This closes the loop:
        // (PID) → (pane_id via env) → (session name via tmux) → attach target.
        let resolved = real
            .pane_to_session_name(&env.pane_id)
            .unwrap_or_else(|| panic!("pane_to_session_name returned None for {}", env.pane_id));

        assert_eq!(
            resolved, test_name,
            "round-trip mismatch: probed env from pane_pid={pane_pid}, \
             got pane_id={}, resolved to session name {:?}, expected {:?}",
            env.pane_id, resolved, test_name
        );
    }
}
