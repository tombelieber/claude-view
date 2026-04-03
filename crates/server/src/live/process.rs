//! Process detection for running Claude Code instances.
//!
//! Scans the system process table for processes whose name contains "claude"
//! and extracts their working directories for correlation with JSONL session files.
//! Session source (terminal, IDE, SDK) is derived from the JSONL `entrypoint` field
//! rather than process tree inspection — see `entrypoint_to_source()`.

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
}

/// Convert a JSONL `entrypoint` string to a `SessionSourceInfo`.
///
/// Mirrors the frontend's `entrypointToSource()` in `SessionCard.tsx`.
/// Values: "cli" → Terminal, "claude-vscode" → IDE/VS Code, "sdk-ts" → AgentSdk,
/// "claude-*" → IDE with label derived from the suffix.
pub fn entrypoint_to_source(entrypoint: &str) -> SessionSourceInfo {
    match entrypoint {
        "cli" => SessionSourceInfo {
            category: SessionSource::Terminal,
            label: None,
        },
        "claude-vscode" => SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("VS Code".to_string()),
        },
        "sdk-ts" => SessionSourceInfo {
            category: SessionSource::AgentSdk,
            label: None,
        },
        other if other.starts_with("claude-") => SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some(other.strip_prefix("claude-").unwrap().to_string()),
        },
        // Unknown entrypoint: default to terminal
        _ => SessionSourceInfo {
            category: SessionSource::Terminal,
            label: None,
        },
    }
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
        let is_claude = name.contains("claude")
            || process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains("@anthropic-ai/claude"));
        if !is_claude {
            continue;
        }
        if let Some(cwd) = process.cwd().map(|p| p.to_path_buf()) {
            total_count += 1;
            result.insert(
                pid.as_u32(),
                ClaudeProcess {
                    pid: pid.as_u32(),
                    cwd,
                    start_time: process.start_time(),
                },
            );
        }
    }

    (result, total_count)
}

/// Detect Claude processes using an already-refreshed `System` instance.
///
/// Same logic as `detect_claude_processes()` but reuses the caller's System
/// instead of creating a new one. Used by `ProcessOracle` to avoid duplicate
/// process table scans.
pub fn detect_claude_processes_with_sys(sys: &System) -> super::process_oracle::ClaudeProcesses {
    let mut result = HashMap::new();
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
        if let Some(cwd) = process.cwd().map(|p| p.to_path_buf()) {
            total_count += 1;
            result.insert(
                pid.as_u32(),
                ClaudeProcess {
                    pid: pid.as_u32(),
                    cwd,
                    start_time: process.start_time(),
                },
            );
        }
    }

    super::process_oracle::ClaudeProcesses {
        processes: result,
        count: total_count,
    }
}

/// Count running Claude Code processes without building the full HashMap.
///
/// Lightweight alternative to `detect_claude_processes()` — returns only the
/// total count (e.g. dashboard metric). Uses the same batch-lsof pattern.
///
/// This function does synchronous system calls and should be called from
/// `tokio::task::spawn_blocking`.
pub fn count_claude_processes() -> u32 {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut count = 0u32;

    for process in sys.processes().values() {
        let name = process.name().to_string_lossy();
        let is_claude = name.contains("claude")
            || process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains("@anthropic-ai/claude"));
        if !is_claude {
            continue;
        }
        if process.cwd().is_some() {
            count += 1;
        }
    }

    count
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
        let (processes, total_count) = detect_claude_processes();
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
        processes.insert(
            1234,
            ClaudeProcess {
                pid: 1234,
                cwd: PathBuf::from("/Users/test/project"),
                start_time: 100,
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
        processes.insert(
            5678,
            ClaudeProcess {
                pid: 5678,
                cwd: PathBuf::from("/Users/test/project"),
                start_time: 200,
            },
        );
        let (running, pid) = has_running_process(&processes, "/Users/test/project");
        assert!(running);
        assert_eq!(pid, Some(5678));
    }

    #[test]
    fn test_is_pid_alive_current_process() {
        assert!(is_pid_alive(std::process::id()));
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

    // =========================================================================
    // entrypoint_to_source tests
    // =========================================================================

    #[test]
    fn entrypoint_cli() {
        let result = entrypoint_to_source("cli");
        assert_eq!(result.category, SessionSource::Terminal);
        assert!(result.label.is_none());
    }

    #[test]
    fn entrypoint_vscode() {
        let result = entrypoint_to_source("claude-vscode");
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn entrypoint_sdk_ts() {
        let result = entrypoint_to_source("sdk-ts");
        assert_eq!(result.category, SessionSource::AgentSdk);
        assert!(result.label.is_none());
    }

    #[test]
    fn entrypoint_unknown_ide_prefix() {
        let result = entrypoint_to_source("claude-cursor");
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("cursor"));
    }

    #[test]
    fn entrypoint_unknown_string_defaults_terminal() {
        let result = entrypoint_to_source("something-else");
        assert_eq!(result.category, SessionSource::Terminal);
        assert!(result.label.is_none());
    }
}
