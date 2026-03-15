// crates/core/src/cli.rs
//! Claude CLI detection and status checking.
//!
//! Uses the same pattern as VS Code / Cursor / Electron apps: resolve the CLI
//! path via the user's login shell so that nvm, mise, asdf, ~/.local/bin, and
//! other non-standard PATH entries are picked up correctly.
//!
//! Auth is checked via the shared `credentials` module (file first, then
//! macOS Keychain fallback). This avoids spawning `claude auth status`,
//! which gets SIGKILL'd when run inside a Claude Code session.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use tracing;
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
        .get_or_init(ClaudeCliStatus::find_claude_path)
        .as_deref()
}

/// Claude CLI status information.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
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
    /// Known fallback paths for Claude CLI discovery.
    ///
    /// Last-resort fallback — only includes truly universal Unix paths.
    /// Platform-specific locations (Homebrew, snap, etc.) are discovered
    /// by which_via_shell / which_direct via the user's PATH.
    /// - `.local/bin`: XDG Base Directory Spec ($HOME-relative). npm global installs,
    ///   pip3 --user, and manual installs land here on Linux.
    /// - `/usr/local/bin`: Standard location for locally-compiled software (FHS).
    /// - `/usr/bin`: System binaries (FHS). Package-manager installs (apt, dnf).
    ///
    /// macOS-specific paths (/opt/homebrew, /Applications) are intentionally excluded.
    /// They are discovered by which_via_shell (step 1 of the waterfall) via the
    /// user's login shell PATH.
    pub(crate) const KNOWN_CLI_PATHS: &[&str] = &[
        ".local/bin/claude",     // $HOME-relative (XDG)
        "/usr/local/bin/claude", // locally-compiled (FHS)
        "/usr/bin/claude",       // system package (FHS)
    ];

    /// Detect Claude CLI installation and status.
    ///
    /// Path resolution is cached via `OnceLock` (first call only).
    /// Auth is read from `~/.claude/.credentials.json` — no subprocess needed.
    pub fn detect() -> Self {
        let path = resolved_cli_path().map(|s| s.to_string());
        let version = path.as_ref().and_then(|p| Self::get_version(p));
        let (authenticated, subscription_type) = Self::check_auth_from_credentials();

        Self {
            path,
            version,
            authenticated,
            subscription_type,
        }
    }

    /// Run a command with a timeout, returning None if it times out or fails to start.
    ///
    /// Strips all `CLAUDE*` env vars so the CLI doesn't refuse to run or
    /// try to connect to an SSE port when our server was launched from
    /// within a Claude Code session.
    fn run_with_timeout(cmd: &mut Command) -> Option<std::process::Output> {
        // Collect all CLAUDE* env vars to remove (dynamic prefix scan)
        let claude_vars: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("CLAUDE"))
            .map(|(k, _)| k)
            .collect();

        for var in &claude_vars {
            cmd.env_remove(var);
        }

        let mut child = cmd
            .stdin(std::process::Stdio::null())
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
        if let Ok(shell) = std::env::var("SHELL") {
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
        let output = Self::run_with_timeout(Command::new(shell).args(["-lc", "which claude"]))?;
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
        Self::KNOWN_CLI_PATHS
            .iter()
            .map(|p| {
                if p.starts_with('/') {
                    p.to_string()
                } else {
                    format!("{home}/{p}")
                }
            })
            .find(|p| std::path::Path::new(p).exists())
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
                if v.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                {
                    return Some(v.to_string());
                }
            }
        }

        None
    }

    /// Check authentication by reading `~/.claude/.credentials.json` directly.
    ///
    /// This is the ground truth — the CLI reads this same file. Reading it
    /// directly avoids spawning `claude auth status`, which gets SIGKILL'd
    /// when the server runs inside a Claude Code session (the subprocess is
    /// killed before it can produce any output).
    fn check_auth_from_credentials() -> (bool, Option<String>) {
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => {
                tracing::warn!("CLI auth: HOME not set, cannot read credentials");
                return (false, None);
            }
        };

        let home_path = std::path::Path::new(&home);
        let data = match crate::credentials::load_credentials_bytes(home_path) {
            Some(d) => d,
            None => {
                tracing::debug!("CLI auth: no credentials found (file or keychain)");
                return (false, None);
            }
        };

        let oauth = match crate::credentials::parse_credentials(&data) {
            Some(o) => o,
            None => {
                tracing::debug!("CLI auth: no valid claudeAiOauth in credentials");
                return (false, None);
            }
        };

        if crate::credentials::is_token_expired(oauth.expires_at) {
            tracing::debug!("CLI auth: token expired");
            return (false, None);
        }

        let subscription = oauth
            .subscription_type
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty());
        tracing::debug!("CLI auth: authenticated (subscription={subscription:?})");
        (true, subscription)
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
    fn test_cli_status_serializes_correctly() {
        let status = ClaudeCliStatus {
            path: Some("/usr/local/bin/claude".to_string()),
            version: Some("1.0.12".to_string()),
            authenticated: true,
            subscription_type: Some("pro".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"path\":\"/usr/local/bin/claude\""));
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

    #[test]
    fn known_cli_paths_contain_only_universal_locations() {
        for path in ClaudeCliStatus::KNOWN_CLI_PATHS {
            assert!(
                !path.contains("homebrew"),
                "KNOWN_CLI_PATHS must not contain macOS-specific paths, found: {path}"
            );
            assert!(
                !path.contains("Applications"),
                "KNOWN_CLI_PATHS must not contain macOS app bundle paths, found: {path}"
            );
        }
    }

    // --- Credentials file parsing ---

    fn parse_creds(json: &str) -> (bool, Option<String>) {
        let oauth = match crate::credentials::parse_credentials(json.as_bytes()) {
            Some(o) => o,
            None => return (false, None),
        };
        if crate::credentials::is_token_expired(oauth.expires_at) {
            return (false, None);
        }
        let sub = oauth
            .subscription_type
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty());
        (true, sub)
    }

    #[test]
    fn test_creds_max_subscription() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"max","expiresAt":9999999999999}}"#,
        );
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("max"));
    }

    #[test]
    fn test_creds_pro_subscription() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"Pro","expiresAt":9999999999999}}"#,
        );
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("pro"));
    }

    #[test]
    fn test_creds_free_subscription() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"Free","expiresAt":9999999999999}}"#,
        );
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("free"));
    }

    #[test]
    fn test_creds_no_subscription_type() {
        let (auth, sub) =
            parse_creds(r#"{"claudeAiOauth":{"accessToken":"sk-test","expiresAt":9999999999999}}"#);
        assert!(auth);
        assert_eq!(sub, None);
    }

    #[test]
    fn test_creds_expired_token() {
        let (auth, _) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"max","expiresAt":1000}}"#,
        );
        assert!(!auth);
    }

    #[test]
    fn test_creds_no_oauth_section() {
        let (auth, _) = parse_creds(r#"{"mcpOAuth":{}}"#);
        assert!(!auth);
    }

    #[test]
    fn test_creds_empty_file() {
        let (auth, _) = parse_creds(r#"{}"#);
        assert!(!auth);
    }

    #[test]
    fn test_creds_malformed_json() {
        let (auth, _) = parse_creds(r#"not json"#);
        assert!(!auth);
    }

    #[test]
    fn test_creds_zero_expiry_treated_as_no_expiry() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"max","expiresAt":0}}"#,
        );
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("max"));
    }

    #[test]
    fn test_creds_missing_expiry_treated_as_valid() {
        let (auth, sub) =
            parse_creds(r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"pro"}}"#);
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("pro"));
    }

    #[test]
    fn test_creds_empty_subscription_type_filtered() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-test","subscriptionType":"","expiresAt":9999999999999}}"#,
        );
        assert!(auth);
        assert_eq!(sub, None);
    }

    #[test]
    fn test_creds_with_extra_fields_ignored() {
        let (auth, sub) = parse_creds(
            r#"{"claudeAiOauth":{"accessToken":"sk-xxx","refreshToken":"sk-yyy","subscriptionType":"max","expiresAt":9999999999999,"scopes":["user:inference"]},"mcpOAuth":{}}"#,
        );
        assert!(auth);
        assert_eq!(sub.as_deref(), Some("max"));
    }
}
