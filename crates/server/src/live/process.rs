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
/// Returns `(processes_by_cwd, total_process_count)`.
///
/// The map deduplicates by cwd (keeping the newest process per directory).
/// The total count is the raw number of Claude processes found before dedup.
pub fn detect_claude_processes() -> (HashMap<PathBuf, ClaudeProcess>, u32) {
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

        // sysinfo can read cwd on Linux but NOT on macOS (returns None due to
        // security restrictions). Fall back to lsof on macOS when cwd is None.
        let cwd = process
            .cwd()
            .map(|p| p.to_path_buf())
            .or_else(|| get_cwd_via_lsof(pid_u32));

        if let Some(cwd) = cwd {
            total_count += 1;
            let cp = ClaudeProcess {
                pid: pid_u32,
                cwd: cwd.clone(),
                start_time,
            };
            // If there's already a process for this cwd, keep the newer one
            result
                .entry(cwd)
                .and_modify(|existing: &mut ClaudeProcess| {
                    if cp.start_time > existing.start_time {
                        *existing = cp.clone();
                    }
                })
                .or_insert(cp);
        }
    }
    (result, total_count)
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
        let (processes, total_count) = detect_claude_processes();
        // total_count >= deduplicated map size
        assert!(total_count as usize >= processes.len());
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
