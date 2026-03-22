//! Process detection for running Claude Code instances.
//!
//! Scans the system process table for processes whose name contains "claude"
//! and extracts their working directories for correlation with JSONL session files.
//! Also classifies the **source** of each process (terminal, IDE extension, or Agent SDK)
//! by inspecting the parent process.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use sysinfo::{ProcessesToUpdate, System};
use ts_rs::TS;

/// Where a Claude Code process was launched from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    /// Interactive shell (zsh, bash, fish, etc.)
    Terminal,
    /// IDE extension (VS Code, Cursor, IntelliJ, etc.)
    Ide,
    /// claude-view Agent SDK sidecar
    AgentSdk,
}

/// Metadata about the source environment of a Claude process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct SessionSourceInfo {
    /// Category: terminal, ide, or agent_sdk.
    pub category: SessionSource,
    /// Human-readable label for the source (e.g. "VS Code", "IntelliJ", "Cursor").
    /// None for terminal sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A running Claude Code process on the system.
#[derive(Debug, Clone)]
pub struct ClaudeProcess {
    /// OS-level process ID.
    pub pid: u32,
    /// Working directory of the process (used to match with project paths).
    pub cwd: PathBuf,
    /// Unix timestamp when the process started.
    pub start_time: u64,
    /// Where this process was launched from.
    pub source: SessionSourceInfo,
}

/// Detect all running Claude Code processes on the system.
///
/// Returns a map from PID to process info, plus the total count.
/// Indexed by PID (unique) — no deduplication. Multiple processes sharing
/// the same CWD (e.g. terminal + IDE sessions in the same worktree) are
/// all returned so the backfill can match each session's PID.
///
/// This function does synchronous system calls and should be called from
/// `tokio::task::spawn_blocking`.
pub fn detect_claude_processes() -> (HashMap<u32, ClaudeProcess>, u32) {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut result = HashMap::new();
    let mut total_count = 0u32;
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();

        // Check process name first (native binary installs where name IS "claude")
        let is_claude = name.contains("claude")
            // Also check command-line args for Node.js-based installs.
            // Claude Code runs as `node /path/to/@anthropic-ai/claude-code/cli.js`
            // where the process name is "node", not "claude".
            || process.cmd().iter().any(|arg| {
                arg.to_string_lossy().contains("@anthropic-ai/claude")
            });

        if !is_claude {
            continue;
        }

        let pid_u32 = pid.as_u32();
        let start_time = process.start_time();

        // sysinfo returns None for cwd on macOS due to security restrictions
        // (sandboxing / SIP). On Linux, sysinfo reads /proc/<pid>/cwd directly.
        // Fall back to lsof when cwd is None (works on both macOS and Linux).
        let cwd = process
            .cwd()
            .map(|p| p.to_path_buf())
            .or_else(|| get_cwd_via_lsof(pid_u32));

        if let Some(cwd) = cwd {
            total_count += 1;
            let source = classify_source(&sys, process);
            let cp = ClaudeProcess {
                pid: pid_u32,
                cwd,
                start_time,
                source,
            };
            result.insert(pid_u32, cp);
        }
    }
    (result, total_count)
}

/// Count running Claude Code processes without building the full HashMap.
///
/// This is a lightweight alternative to `detect_claude_processes()` for call
/// sites that only need the total process count (e.g. the dashboard metric).
/// Avoids allocating the HashMap, cloning PathBufs, and deduplicating by cwd.
///
/// This function does synchronous system calls and should be called from
/// `tokio::task::spawn_blocking`.
pub fn count_claude_processes() -> u32 {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut total_count = 0u32;
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();

        let is_claude = name.contains("claude")
            || process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains("@anthropic-ai/claude"));

        if !is_claude {
            continue;
        }

        let pid_u32 = pid.as_u32();

        // Only count processes where we can resolve a cwd (same filter as detect_claude_processes)
        let cwd = process
            .cwd()
            .map(|p| p.to_path_buf())
            .or_else(|| get_cwd_via_lsof(pid_u32));

        if cwd.is_some() {
            total_count += 1;
        }
    }
    total_count
}

/// Known shell process names — if the parent is one of these, the Claude process
/// was launched from an interactive terminal.
const SHELL_NAMES: &[&str] = &[
    "zsh",
    "bash",
    "fish",
    "sh",
    "dash",
    "tcsh",
    "csh",
    "ksh",
    "nu",
    "pwsh",
    "powershell",
];

/// IDE extension path patterns for own-binary detection.
///
/// IDE classification uses ONLY the Claude binary's own path — not the parent
/// process chain. A global `claude` binary launched from an IDE's integrated
/// terminal is a terminal session, not an IDE session.
///
/// Currently VS Code, Cursor, and Windsurf install their own Claude binary
/// inside their extension directories. Other IDEs (IntelliJ, Neovim, etc.)
/// use the global binary, so they appear as terminal sessions.
/// This is intentional: the IDE badge means "launched BY the IDE extension."
///
/// Classify where a Claude process was launched from.
///
/// Two-pass approach:
/// 1. Check the process's OWN binary path for IDE extension paths
///    (only the extension's bundled binary counts as "IDE" — typing `claude`
///    in an IDE's integrated terminal is still a terminal session)
/// 2. Walk ancestors for sidecar detection (Agent SDK, defense-in-depth)
/// 3. Everything else → Terminal
///
/// IDE detection relies ONLY on the binary path, not the ancestor chain.
/// Finding "VS Code" as a grandparent means nothing — the user may have
/// just typed `claude` in VS Code's integrated terminal.
fn classify_source(sys: &System, process: &sysinfo::Process) -> SessionSourceInfo {
    // Pass 1: Check the process's own binary path for IDE extension installs.
    // This is the ONLY reliable IDE signal. A global `claude` binary launched
    // from an IDE's terminal is still a terminal session.
    let cmd_args = process.cmd();
    let own_full = cmd_args
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    // VS Code extension bundles Claude at .vscode/extensions/anthropic.claude-code-*/
    if own_full.contains(".vscode/extensions/") || own_full.contains(".vscode-server/") {
        return SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("VS Code".to_string()),
        };
    }
    if own_full.contains(".cursor/extensions/") || own_full.contains(".cursor-server/") {
        return SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("Cursor".to_string()),
        };
    }
    if own_full.contains(".windsurf/extensions/") || own_full.contains(".windsurf-server/") {
        return SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("Windsurf".to_string()),
        };
    }

    // Pass 2: Walk ancestors for sidecar detection only (Agent SDK).
    // Defense-in-depth — the authoritative path is control binding → AgentSdk
    // set in manager.rs / live.rs. This catches edge cases where the control
    // binding was missed.
    let mut current_pid = process.parent();
    let mut depth = 0u32;
    const MAX_ANCESTOR_DEPTH: u32 = 5;

    while let Some(pid) = current_pid {
        if depth >= MAX_ANCESTOR_DEPTH {
            break;
        }
        depth += 1;

        let Some(ancestor) = sys.process(pid) else {
            break;
        };
        let anc_name = ancestor.name().to_string_lossy().to_lowercase();
        let anc_full = ancestor
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Sidecar → Agent SDK
        if anc_full.contains("sidecar/dist/index.js") {
            return SessionSourceInfo {
                category: SessionSource::AgentSdk,
                label: None,
            };
        }

        // Skip shells — sidecar might be higher up
        let is_shell = SHELL_NAMES
            .iter()
            .any(|sh| anc_name == *sh || anc_name.ends_with(sh));
        if is_shell {
            current_pid = ancestor.parent();
            continue;
        }

        // Non-shell ancestor found, not sidecar → stop walking
        break;
    }

    // Default: terminal (global binary, any terminal — including IDE integrated terminals)
    SessionSourceInfo {
        category: SessionSource::Terminal,
        label: None,
    }
}

/// Fallback: get a process's working directory via `lsof`.
///
/// On macOS, `sysinfo` cannot read cwd for other processes (security restriction).
/// `lsof -a -p <pid> -d cwd -Fn` reliably returns the cwd for same-user processes.
fn get_cwd_via_lsof(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // lsof -Fn output: lines starting with 'n' contain the path
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}

/// Check if there is a running Claude process whose cwd matches the given
/// decoded project path.
///
/// The comparison is done by checking whether the process cwd starts with or
/// equals the project path (to handle worktrees and nested directories).
pub fn find_process_for_project<'a>(
    processes: &'a HashMap<u32, ClaudeProcess>,
    project_path: &str,
) -> Option<&'a ClaudeProcess> {
    let project = PathBuf::from(project_path);
    // Check all processes — multiple may share the same cwd
    processes.values().find(|proc| {
        proc.cwd == project || proc.cwd.starts_with(&project) || project.starts_with(&proc.cwd)
    })
}

/// Convenience: check and return just whether there's a matching process + its PID.
pub fn has_running_process(
    processes: &HashMap<u32, ClaudeProcess>,
    project_path: &str,
) -> (bool, Option<u32>) {
    match find_process_for_project(processes, project_path) {
        Some(p) => (true, Some(p.pid)),
        None => (false, None),
    }
}

/// Check if a process with the given PID is still alive.
///
/// Uses `kill(pid, 0)` which checks process existence without sending a signal.
/// Returns `false` for PIDs <= 1 (kernel/init) to guard against reparented processes.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    // SAFETY: kill with signal 0 does not send a signal, only checks existence.
    // Returns 0 if process exists and we have permission, -1 with ESRCH if not.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude_processes_runs() {
        // Just verify it doesn't panic — we can't guarantee any Claude processes
        // are running during tests.
        let (processes, total_count) = detect_claude_processes();
        // PID-indexed map: total_count == map size (no dedup)
        assert_eq!(total_count as usize, processes.len());
    }

    #[test]
    fn test_find_process_for_project_empty() {
        let processes: HashMap<u32, ClaudeProcess> = HashMap::new();
        assert!(find_process_for_project(&processes, "/some/path").is_none());
    }

    #[test]
    fn test_find_process_for_project_exact_match() {
        let mut processes = HashMap::new();
        let path = PathBuf::from("/Users/test/project");
        processes.insert(
            1234,
            ClaudeProcess {
                pid: 1234,
                cwd: path,
                start_time: 100,
                source: SessionSourceInfo {
                    category: SessionSource::Terminal,
                    label: None,
                },
            },
        );

        let result = find_process_for_project(&processes, "/Users/test/project");
        assert!(result.is_some());
        assert_eq!(result.unwrap().pid, 1234);
    }

    #[test]
    fn test_has_running_process_not_found() {
        let processes: HashMap<u32, ClaudeProcess> = HashMap::new();
        let (running, pid) = has_running_process(&processes, "/nonexistent");
        assert!(!running);
        assert!(pid.is_none());
    }

    #[test]
    fn test_has_running_process_found() {
        let mut processes = HashMap::new();
        let path = PathBuf::from("/Users/test/project");
        processes.insert(
            5678,
            ClaudeProcess {
                pid: 5678,
                cwd: path,
                start_time: 200,
                source: SessionSourceInfo {
                    category: SessionSource::Terminal,
                    label: None,
                },
            },
        );

        let (running, pid) = has_running_process(&processes, "/Users/test/project");
        assert!(running);
        assert_eq!(pid, Some(5678));
    }

    #[test]
    fn test_is_pid_alive_current_process() {
        let pid = std::process::id();
        assert!(is_pid_alive(pid));
    }

    #[test]
    fn test_is_pid_alive_nonexistent() {
        assert!(!is_pid_alive(4_000_000));
    }

    #[test]
    fn test_is_pid_alive_rejects_zero_and_one() {
        assert!(!is_pid_alive(0));
        assert!(!is_pid_alive(1));
    }
}
