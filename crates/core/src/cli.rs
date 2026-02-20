// crates/core/src/cli.rs
//! Claude CLI detection and status checking.
//!
//! Uses the same pattern as VS Code / Cursor / Electron apps: resolve the CLI
//! path via the user's login shell so that nvm, mise, asdf, ~/.local/bin, and
//! other non-standard PATH entries are picked up correctly.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use ts_rs::TS;

/// Timeout for each CLI subprocess call (prevents hangs when claude is already running).
const CLI_TIMEOUT: Duration = Duration::from_secs(3);

/// Cached resolved path to the `claude` binary (process-lifetime singleton).
static RESOLVED_CLI_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Get the resolved path to the `claude` binary, resolving on first call.
///
/// Uses a login-shell waterfall to find the binary regardless of how the
/// server process was started (npx, cargo run, launchd, etc.).
pub fn resolved_cli_path() -> Option<&'static str> {
    RESOLVED_CLI_PATH
        .get_or_init(|| ClaudeCliStatus::find_claude_path())
        .as_deref()
}

/// Claude CLI status information.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ClaudeCliStatus {
    /// Path to claude binary, None if not found.
    pub path: Option<String>,
    /// Version string, None if not found.
    pub version: Option<String>,
    /// Whether CLI is authenticated.
    pub authenticated: bool,
    /// Subscription type if authenticated.
    pub subscription_type: Option<String>,
}

impl ClaudeCliStatus {
    /// Detect Claude CLI installation and status.
    ///
    /// Path resolution is cached via `OnceLock` (first call only).
    /// Auth status is always re-checked (cheap with a known path).
    pub fn detect() -> Self {
        let path = resolved_cli_path().map(|s| s.to_string());

        let Some(ref path_str) = path else {
            return Self::default();
        };

        let version = Self::get_version(path_str);
        let (authenticated, subscription_type) = Self::check_auth(path_str);

        Self {
            path,
            version,
            authenticated,
            subscription_type,
        }
    }

    /// Run a command with a timeout, returning None if it times out or fails to start.
    ///
    /// Removes the `CLAUDECODE` env var so the CLI doesn't refuse to run
    /// when our server was launched from within a Claude Code session.
    fn run_with_timeout(cmd: &mut Command) -> Option<std::process::Output> {
        let mut child = cmd
            .env_remove("CLAUDECODE")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .ok()?;

        let deadline = std::time::Instant::now() + CLI_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return child.wait_with_output().ok(),
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => return None,
            }
        }
    }

    /// Find the path to the claude binary using a login-shell waterfall.
    ///
    /// 1. `$SHELL -lc "which claude"` — user's login shell (handles nvm/mise/asdf/custom PATH)
    /// 2. `which claude` — server's inherited PATH
    /// 3. Filesystem scan of known install locations
    fn find_claude_path() -> Option<String> {
        // Step 1: Login shell resolution (the fix-path pattern from VS Code/Electron)
        if let Some(shell) = std::env::var("SHELL").ok() {
            if let Some(path) = Self::which_via_shell(&shell) {
                return Some(path);
            }
        }

        // Step 2: Direct `which` using server's inherited PATH
        if let Some(path) = Self::which_direct() {
            return Some(path);
        }

        // Step 3: Exhaustive filesystem scan
        Self::scan_known_paths()
    }

    /// Resolve `claude` via the user's login shell.
    fn which_via_shell(shell: &str) -> Option<String> {
        let output = Self::run_with_timeout(
            Command::new(shell).args(["-lc", "which claude"]),
        )?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
        None
    }

    /// Resolve `claude` via the server's inherited PATH.
    fn which_direct() -> Option<String> {
        let output = Self::run_with_timeout(Command::new("which").arg("claude"))?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
        None
    }

    /// Scan known installation locations on the filesystem.
    fn scan_known_paths() -> Option<String> {
        let home = std::env::var("HOME").ok().unwrap_or_default();
        let paths = [
            format!("{home}/.local/bin/claude"),
            "/opt/homebrew/bin/claude".to_string(),
            "/usr/local/bin/claude".to_string(),
            "/usr/bin/claude".to_string(),
        ];
        paths.into_iter().find(|p| std::path::Path::new(p).exists())
    }

    /// Get the claude CLI version.
    fn get_version(path: &str) -> Option<String> {
        let output = Self::run_with_timeout(Command::new(path).arg("--version"))?;

        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Parse version from output like "claude version 1.0.12" or just "1.0.12"
            let trimmed = version_str.trim();
            if trimmed.is_empty() {
                return None;
            }
            // Take the last whitespace-separated token as the version
            if let Some(v) = trimmed.split_whitespace().last() {
                return Some(v.to_string());
            }
        }

        // Also try stderr (some CLIs output version there)
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let trimmed = stderr.trim();
            if let Some(v) = trimmed.split_whitespace().last() {
                if v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    return Some(v.to_string());
                }
            }
        }

        None
    }

    /// Check authentication status.
    fn check_auth(path: &str) -> (bool, Option<String>) {
        // Try to get auth status (with timeout to prevent hangs when claude is running)
        let output = Self::run_with_timeout(Command::new(path).args(["auth", "status"]));

        match output {
            Some(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                // Check both stdout and stderr for auth info
                let combined = format!("{} {}", stdout, stderr);
                let subscription = Self::parse_subscription_type(&combined);
                (true, subscription)
            }
            _ => (false, None),
        }
    }

    /// Parse subscription type from auth status output.
    pub fn parse_subscription_type(output: &str) -> Option<String> {
        // Look for patterns like "(Pro)", "(Free)", "(Team)", "(Enterprise)"
        let types = ["pro", "free", "team", "enterprise", "max"];
        let lower = output.to_lowercase();

        for t in types {
            if lower.contains(&format!("({})", t)) || lower.contains(&format!("{} plan", t)) {
                return Some(t.to_string());
            }
        }

        // Fallback: check if authenticated at all
        // But make sure it's not "not authenticated" or "unauthenticated"
        if (lower.contains("authenticated") && !lower.contains("not authenticated") && !lower.contains("unauthenticated"))
            || lower.contains("logged in")
        {
            return Some("unknown".to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_status_default() {
        let status = ClaudeCliStatus::default();
        assert!(status.path.is_none());
        assert!(status.version.is_none());
        assert!(!status.authenticated);
        assert!(status.subscription_type.is_none());
    }

    #[test]
    fn test_parse_subscription_type_pro() {
        let output = "Authenticated as user@example.com (Pro)";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("pro".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_free() {
        let output = "Authenticated as user@example.com (Free)";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("free".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_team() {
        let output = "Authenticated as user@example.com (Team)";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("team".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_enterprise() {
        let output = "Authenticated as user@example.com (Enterprise)";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("enterprise".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_max() {
        let output = "Authenticated as user@example.com (Max)";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("max".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_plan_format() {
        let output = "You are on the Pro plan";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("pro".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_authenticated_fallback() {
        let output = "Authenticated as user@example.com";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("unknown".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_logged_in_fallback() {
        let output = "Logged in as user@example.com";
        assert_eq!(
            ClaudeCliStatus::parse_subscription_type(output),
            Some("unknown".to_string())
        );
    }

    #[test]
    fn test_parse_subscription_type_none() {
        let output = "Please run: claude auth login";
        assert_eq!(ClaudeCliStatus::parse_subscription_type(output), None);
    }

    #[test]
    fn test_parse_subscription_type_not_authenticated() {
        let output = "Not authenticated. Run: claude auth login";
        assert_eq!(ClaudeCliStatus::parse_subscription_type(output), None);
    }

    #[test]
    fn test_parse_subscription_type_empty() {
        assert_eq!(ClaudeCliStatus::parse_subscription_type(""), None);
    }

    #[test]
    fn test_cli_status_serializes_correctly() {
        let status = ClaudeCliStatus {
            path: Some("/opt/homebrew/bin/claude".to_string()),
            version: Some("1.0.12".to_string()),
            authenticated: true,
            subscription_type: Some("pro".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"path\":\"/opt/homebrew/bin/claude\""));
        assert!(json.contains("\"version\":\"1.0.12\""));
        assert!(json.contains("\"authenticated\":true"));
        assert!(json.contains("\"subscriptionType\":\"pro\""));
    }

    #[test]
    fn test_cli_status_not_installed_serializes() {
        let status = ClaudeCliStatus::default();
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"path\":null"));
        assert!(json.contains("\"version\":null"));
        assert!(json.contains("\"authenticated\":false"));
        assert!(json.contains("\"subscriptionType\":null"));
    }
}
