//! Process detection for running Claude Code instances.
//!
//! Scans the system process table for processes whose name contains "claude"
//! and extracts their working directories for correlation with JSONL session files.

use std::collections::HashMap;
use std::path::PathBuf;
use sysinfo::{ProcessesToUpdate, System};

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

/// Detect all running Claude Code processes on the system.
///
/// Returns a map from working directory to process info. If multiple Claude
/// processes share the same cwd, only the most recent one is kept.
///
/// This function does synchronous system calls and should be called from
/// `tokio::task::spawn_blocking`.
pub fn detect_claude_processes() -> HashMap<PathBuf, ClaudeProcess> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut result = HashMap::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        if !name.contains("claude") {
            continue;
        }
        if let Some(cwd) = process.cwd() {
            let cp = ClaudeProcess {
                pid: pid.as_u32(),
                cwd: cwd.to_path_buf(),
                start_time: process.start_time(),
            };
            // If there's already a process for this cwd, keep the newer one
            result
                .entry(cwd.to_path_buf())
                .and_modify(|existing: &mut ClaudeProcess| {
                    if cp.start_time > existing.start_time {
                        *existing = cp.clone();
                    }
                })
                .or_insert(cp);
        }
    }
    result
}

/// Check if there is a running Claude process whose cwd matches the given
/// decoded project path.
///
/// The comparison is done by checking whether the process cwd starts with or
/// equals the project path (to handle worktrees and nested directories).
pub fn find_process_for_project<'a>(
    processes: &'a HashMap<PathBuf, ClaudeProcess>,
    project_path: &str,
) -> Option<&'a ClaudeProcess> {
    let project = PathBuf::from(project_path);
    // Exact match first
    if let Some(p) = processes.get(&project) {
        return Some(p);
    }
    // Check if any process cwd starts with the project path
    for (cwd, proc) in processes {
        if cwd.starts_with(&project) || project.starts_with(cwd) {
            return Some(proc);
        }
    }
    None
}

/// Convenience: check and return just whether there's a matching process + its PID.
pub fn has_running_process(
    processes: &HashMap<PathBuf, ClaudeProcess>,
    project_path: &str,
) -> (bool, Option<u32>) {
    match find_process_for_project(processes, project_path) {
        Some(p) => (true, Some(p.pid)),
        None => (false, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude_processes_runs() {
        // Just verify it doesn't panic — we can't guarantee any Claude processes
        // are running during tests.
        let result = detect_claude_processes();
        // result could be empty, that's fine
        let _ = result;
    }

    #[test]
    fn test_find_process_for_project_empty() {
        let processes = HashMap::new();
        assert!(find_process_for_project(&processes, "/some/path").is_none());
    }

    #[test]
    fn test_find_process_for_project_exact_match() {
        let mut processes = HashMap::new();
        let path = PathBuf::from("/Users/test/project");
        processes.insert(
            path.clone(),
            ClaudeProcess {
                pid: 1234,
                cwd: path,
                start_time: 100,
            },
        );

        let result = find_process_for_project(&processes, "/Users/test/project");
        assert!(result.is_some());
        assert_eq!(result.unwrap().pid, 1234);
    }

    #[test]
    fn test_has_running_process_not_found() {
        let processes = HashMap::new();
        let (running, pid) = has_running_process(&processes, "/nonexistent");
        assert!(!running);
        assert!(pid.is_none());
    }

    #[test]
    fn test_has_running_process_found() {
        let mut processes = HashMap::new();
        let path = PathBuf::from("/Users/test/project");
        processes.insert(
            path.clone(),
            ClaudeProcess {
                pid: 5678,
                cwd: path,
                start_time: 200,
            },
        );

        let (running, pid) = has_running_process(&processes, "/Users/test/project");
        assert!(running);
        assert_eq!(pid, Some(5678));
    }
}
