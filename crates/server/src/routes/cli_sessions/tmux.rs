//! Trait abstraction for tmux commands.
//!
//! `RealTmux` shells out to the tmux binary via `std::process::Command`.
//! `MockTmux` (cfg(test)) uses an in-memory `HashSet` for tracking sessions.

use std::process::Command;

/// Abstraction over tmux operations for testability.
pub trait TmuxCommand: Send + Sync {
    /// Create a new tmux session running `claude` with given args.
    ///
    /// Runs:
    ///   tmux new-session -d -s {name} -x 120 -y 40 'claude {args}'
    ///   tmux set-option -t {name} aggressive-resize on
    fn new_session(
        &self,
        name: &str,
        project_dir: Option<&str>,
        args: &[String],
    ) -> Result<(), String>;

    /// Kill a tmux session by name.
    fn kill_session(&self, name: &str) -> Result<(), String>;

    /// Check if a tmux session exists.
    fn has_session(&self, name: &str) -> bool;

    /// Get the PID of the process running in the tmux pane.
    /// Returns None if the session doesn't exist or pane PID can't be read.
    fn pane_pid(&self, name: &str) -> Option<u32>;

    /// Check if the tmux binary is available.
    fn is_available(&self) -> bool;

    /// List all tmux session names.
    fn list_sessions(&self) -> Vec<String>;
}

/// Production implementation that shells out to the real tmux binary.
pub struct RealTmux;

impl TmuxCommand for RealTmux {
    fn new_session(
        &self,
        name: &str,
        project_dir: Option<&str>,
        args: &[String],
    ) -> Result<(), String> {
        // Build the shell command that tmux will run inside the session.
        let mut claude_cmd = String::from("claude");
        for arg in args {
            claude_cmd.push(' ');
            // Shell-escape each arg with single quotes.
            claude_cmd.push_str(&shell_escape(arg));
        }

        let mut cmd = Command::new("tmux");
        cmd.args(["new-session", "-d", "-s", name, "-x", "120", "-y", "40"]);

        if let Some(dir) = project_dir {
            cmd.args(["-c", dir]);
        }

        cmd.arg(&claude_cmd);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to spawn tmux: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux new-session failed: {stderr}"));
        }

        // Enable aggressive-resize so the session adapts to client size.
        let resize_output = Command::new("tmux")
            .args(["set-option", "-t", name, "aggressive-resize", "on"])
            .output()
            .map_err(|e| format!("Failed to set aggressive-resize: {e}"))?;

        if !resize_output.status.success() {
            tracing::warn!(
                session = name,
                "Failed to set aggressive-resize (non-fatal)"
            );
        }

        Ok(())
    }

    fn kill_session(&self, name: &str) -> Result<(), String> {
        let output = Command::new("tmux")
            .args(["kill-session", "-t", name])
            .output()
            .map_err(|e| format!("Failed to spawn tmux: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux kill-session failed: {stderr}"));
        }

        Ok(())
    }

    fn has_session(&self, name: &str) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn pane_pid(&self, name: &str) -> Option<u32> {
        let output = Command::new("tmux")
            .args(["list-panes", "-t", name, "-F", "#{pane_pid}"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<u32>().ok()
    }

    fn is_available(&self) -> bool {
        Command::new("tmux")
            .arg("-V")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_sessions(&self) -> Vec<String> {
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output();
        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Shell-escape a string with single quotes.
fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    // If no special chars, return as-is.
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
    {
        return s.to_string();
    }
    // Wrap in single quotes, escaping existing single quotes.
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ============================================================================
// Mock implementation for tests
// ============================================================================

#[cfg(test)]
pub mod mock {
    use super::TmuxCommand;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// Mock tmux that tracks sessions in memory.
    pub struct MockTmux {
        sessions: Mutex<HashSet<String>>,
        available: bool,
    }

    impl MockTmux {
        /// Create a mock tmux that reports as available.
        pub fn new() -> Self {
            Self {
                sessions: Mutex::new(HashSet::new()),
                available: true,
            }
        }

        /// Create a mock tmux that reports as unavailable.
        pub fn unavailable() -> Self {
            Self {
                sessions: Mutex::new(HashSet::new()),
                available: false,
            }
        }

        /// Get the set of active session names (for assertions).
        pub fn active_sessions(&self) -> HashSet<String> {
            self.sessions.lock().unwrap().clone()
        }
    }

    impl TmuxCommand for MockTmux {
        fn new_session(
            &self,
            name: &str,
            _project_dir: Option<&str>,
            _args: &[String],
        ) -> Result<(), String> {
            if !self.available {
                return Err("tmux not available".to_string());
            }
            let mut sessions = self.sessions.lock().unwrap();
            if sessions.contains(name) {
                return Err(format!("duplicate session: {name}"));
            }
            sessions.insert(name.to_string());
            Ok(())
        }

        fn kill_session(&self, name: &str) -> Result<(), String> {
            let mut sessions = self.sessions.lock().unwrap();
            if sessions.remove(name) {
                Ok(())
            } else {
                Err(format!("session not found: {name}"))
            }
        }

        fn has_session(&self, name: &str) -> bool {
            self.sessions.lock().unwrap().contains(name)
        }

        fn pane_pid(&self, _name: &str) -> Option<u32> {
            None
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn list_sessions(&self) -> Vec<String> {
            self.sessions.lock().unwrap().iter().cloned().collect()
        }
    }
}
