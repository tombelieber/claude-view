//! CLI interaction: spawning `claude plugin` subcommands, parsing output,
//! and caching CLI responses.

use serde::Deserialize;
use tokio::process::Command;

use crate::error::ApiError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// CLI JSON deserialization (matches `claude plugin list --json`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliInstalledPlugin {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    pub scope: String,
    pub enabled: bool,
    pub installed_at: String,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub git_commit_sha: Option<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliAvailablePlugin {
    pub plugin_id: String,
    pub name: String,
    pub description: String,
    pub marketplace_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub install_count: Option<u64>,
}

/// Combined response from `claude plugin list --available --json`.
/// `pub(crate)` so `AppState` can hold `CachedUpstream<CliAvailableResponse>`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CliAvailableResponse {
    #[serde(default)]
    pub(crate) installed: Vec<CliInstalledPlugin>,
    #[serde(default)]
    pub(crate) available: Vec<CliAvailablePlugin>,
}

/// CLI JSON shape for `claude plugin marketplace list --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliMarketplace {
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub repo: Option<String>,
}

// ---------------------------------------------------------------------------
// ANSI stripping
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences (color codes, cursor moves) from CLI output.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    // Consume CSI params until final byte (letter, ~, or @)
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if p.is_ascii_alphabetic() || p == '~' || p == '@' {
                            break;
                        }
                    }
                } else {
                    chars.next();
                    if next == '(' {
                        chars.next(); // charset designator
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// CLI execution
// ---------------------------------------------------------------------------

/// Run a `claude plugin` subcommand and return stdout as String.
/// Strips ALL CLAUDE* env vars and ANSI codes per CLAUDE.md hard rules.
/// Optional `cwd` sets the working directory (needed for project-scoped uninstall).
///
/// Stdout is redirected to a temp file instead of a pipe. Node.js (which powers
/// the `claude` CLI) uses async I/O for piped stdout and can exit before its
/// internal write queue drains. On macOS the kernel pipe buffer is 64KB, so any
/// output beyond that is silently lost. File I/O is synchronous in Node.js,
/// guaranteeing all data is on disk before the process exits.
///
/// We use `spawn()` instead of `output()` because `output()` internally overrides
/// stdout to `Stdio::piped()`, defeating the file redirect.
pub(crate) async fn run_claude_plugin_in(
    args: &[&str],
    cwd: Option<&str>,
    timeout_secs: u64,
) -> Result<String, ApiError> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

    let cli_path = claude_view_core::resolved_cli_path().unwrap_or("claude");

    let stdout_file = tempfile::NamedTempFile::new()
        .map_err(|e| ApiError::Internal(format!("Failed to create temp file: {e}")))?;
    let stdout_fd: Stdio = stdout_file
        .as_file()
        .try_clone()
        .map_err(|e| ApiError::Internal(format!("Failed to clone temp file handle: {e}")))?
        .into();

    let mut cmd = Command::new(cli_path);
    cmd.arg("plugin");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(stdout_fd);
    cmd.stderr(Stdio::piped());

    // Suppress ANSI color codes in CLI output (https://no-color.org/)
    cmd.env("NO_COLOR", "1");

    // Strip ALL CLAUDE* + ANTHROPIC_API_KEY
    let vars_to_strip: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("CLAUDE") || k == "ANTHROPIC_API_KEY")
        .map(|(k, _)| k)
        .collect();
    for key in &vars_to_strip {
        cmd.env_remove(key);
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::Internal(
                "Claude CLI not found. Install: npm install -g @anthropic-ai/claude-code".into(),
            )
        } else {
            ApiError::Internal(format!("Failed to spawn claude CLI: {e}"))
        }
    })?;

    // Read stderr while waiting for exit (stderr is piped)
    let mut stderr_buf = Vec::new();
    let stderr_handle = child.stderr.take();

    let status = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
        // Drain stderr concurrently with waiting for exit
        let stderr_fut = async {
            if let Some(mut stderr) = stderr_handle {
                let _ = stderr.read_to_end(&mut stderr_buf).await;
            }
        };
        let (status, _) = tokio::join!(child.wait(), stderr_fut);
        status
    })
    .await
    .map_err(|_| {
        let _ = child.start_kill();
        ApiError::Internal(format!("claude CLI timed out after {timeout_secs}s"))
    })?
    .map_err(|e| ApiError::Internal(format!("Failed to wait for claude CLI: {e}")))?;

    if !status.success() {
        let stderr = strip_ansi(&String::from_utf8_lossy(&stderr_buf));
        return Err(ApiError::Internal(format!(
            "claude plugin {} failed: {stderr}",
            args.join(" ")
        )));
    }

    let stdout = tokio::fs::read_to_string(stdout_file.path())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read CLI output: {e}")))?;
    Ok(strip_ansi(&stdout))
}

/// Convenience: run `claude plugin` in the default CWD with the default 30s timeout.
pub(crate) async fn run_claude_plugin(args: &[&str]) -> Result<String, ApiError> {
    run_claude_plugin_in(args, None, 30).await
}

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

/// Bust the plugin CLI cache after a mutation so the next GET reflects changes.
pub(crate) async fn invalidate_plugin_cache(state: &AppState) {
    let _ = state
        .plugin_cli_cache
        .force_refresh(std::time::Duration::ZERO, fetch_plugin_cli_data)
        .await;
}

/// Fetch installed + available plugins from the CLI.
/// Signature matches `CachedUpstream::get_or_fetch` requirements.
pub(crate) async fn fetch_plugin_cli_data() -> Result<CliAvailableResponse, String> {
    let json = run_claude_plugin(&["list", "--available", "--json"])
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str::<CliAvailableResponse>(&json)
        .map_err(|e| format!("Failed to parse plugin data: {e}"))
}
