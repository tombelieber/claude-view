//! Process detection for running Claude Code instances.
//!
//! Scans the system process table for processes whose name contains "claude"
//! and extracts their working directories for correlation with JSONL session files.
//! Also classifies the **source** of each process (terminal, IDE extension, or Agent SDK)
//! by inspecting the parent process.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use sysinfo::{ProcessesToUpdate, System};
use ts_rs::TS;

/// Cache for source classification results.
/// Key: (pid, start_time) — stable pair that uniquely identifies a process instance.
/// A process's source (IDE/Terminal/AgentSdk) never changes during its lifetime.
static SOURCE_CACHE: std::sync::LazyLock<Mutex<HashMap<(u32, u64), SessionSourceInfo>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

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

    // Pass 1: Find all Claude processes, collect PIDs that need lsof fallback.
    struct Candidate {
        pid: u32,
        start_time: u64,
        cwd: Option<PathBuf>,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut need_lsof: Vec<u32> = Vec::new();

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
        let start_time = process.start_time();

        // sysinfo returns None for cwd on macOS due to security restrictions
        // (sandboxing / SIP). On Linux, sysinfo reads /proc/<pid>/cwd directly.
        let cwd = process.cwd().map(|p| p.to_path_buf());
        if cwd.is_none() {
            need_lsof.push(pid_u32);
        }
        candidates.push(Candidate {
            pid: pid_u32,
            start_time,
            cwd,
        });
    }

    // Pass 2: Batch lsof for all PIDs that need it (single subprocess instead of N).
    let lsof_results = if need_lsof.is_empty() {
        HashMap::new()
    } else {
        batch_get_cwd_via_lsof(&need_lsof)
    };

    // Pass 3: Assemble results with resolved CWDs + classify source (cached).
    let mut result = HashMap::new();
    let mut total_count = 0u32;
    let mut cache = SOURCE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    for candidate in candidates {
        let cwd = candidate
            .cwd
            .or_else(|| lsof_results.get(&candidate.pid).cloned());

        if let Some(cwd) = cwd {
            total_count += 1;
            let cache_key = (candidate.pid, candidate.start_time);
            let source = if let Some(cached) = cache.get(&cache_key) {
                cached.clone()
            } else {
                let computed = match sys.process(sysinfo::Pid::from_u32(candidate.pid)) {
                    Some(p) => classify_source(&sys, p),
                    None => SessionSourceInfo {
                        category: SessionSource::Terminal,
                        label: None,
                    },
                };
                cache.insert(cache_key, computed.clone());
                computed
            };
            let cp = ClaudeProcess {
                pid: candidate.pid,
                cwd,
                start_time: candidate.start_time,
                source,
            };
            result.insert(candidate.pid, cp);
        }
    }

    // Evict stale cache entries (dead processes) to prevent unbounded growth.
    // Only keep entries for PIDs still in this scan's results.
    cache.retain(|&(pid, _), _| result.contains_key(&pid));

    (result, total_count)
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

    let mut need_lsof: Vec<u32> = Vec::new();
    let mut has_cwd_count = 0u32;

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
        if process.cwd().is_some() {
            has_cwd_count += 1;
        } else {
            need_lsof.push(pid.as_u32());
        }
    }

    let lsof_resolved = if need_lsof.is_empty() {
        0
    } else {
        batch_get_cwd_via_lsof(&need_lsof).len() as u32
    };
    has_cwd_count + lsof_resolved
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

/// IDE parent process patterns: (substring to match, human-readable label).
///
/// Checked against the DIRECT parent process only (not grandparents) to
/// distinguish "extension launched Claude" from "user typed `claude` in
/// IDE's integrated terminal." When the parent is a shell, the session is
/// Terminal — even if an IDE is further up the process tree.
const IDE_PARENT_PATTERNS: &[(&str, &str)] = &[
    // VS Code and variants
    ("Visual Studio Code", "VS Code"),
    ("Code Helper", "VS Code"),
    ("code-insiders", "VS Code"),
    // Cursor
    ("Cursor Helper", "Cursor"),
    ("cursor-helper", "Cursor"),
    // Windsurf
    ("Windsurf Helper", "Windsurf"),
    ("windsurf-helper", "Windsurf"),
    // JetBrains family
    ("IntelliJ IDEA", "IntelliJ"),
    ("WebStorm", "WebStorm"),
    ("PyCharm", "PyCharm"),
    ("GoLand", "GoLand"),
    ("RustRover", "RustRover"),
    ("Rider", "Rider"),
    ("CLion", "CLion"),
    ("PhpStorm", "PhpStorm"),
    // Others
    ("Xcode", "Xcode"),
    ("Zed", "Zed"),
    ("Neovim", "Neovim"),
    ("nvim", "Neovim"),
    ("Sublime Text", "Sublime Text"),
    ("sublime_text", "Sublime Text"),
    ("Eclipse", "Eclipse"),
    ("Android Studio", "Android Studio"),
    ("Fleet", "Fleet"),
];

/// Classify where a Claude process was launched from.
///
/// Three-pass approach:
/// 1. Check the process's OWN binary path for IDE extension installs
///    (the most reliable signal — extension bundles its own Claude binary)
/// 2. Walk ancestors: sidecar (any depth) + IDE (direct parent only)
///    - Shell parent → Terminal (user typed `claude` in a terminal, even inside IDE)
///    - IDE parent → IDE (extension launched Claude directly, no shell in between)
///    - Sidecar parent → AgentSdk (defense-in-depth for control binding)
/// 3. Everything else → Terminal
fn classify_source(sys: &System, process: &sysinfo::Process) -> SessionSourceInfo {
    // Pass 1: Check the process's own binary path for IDE extension installs.
    // Try cmd() first (full command line), then exe() as fallback.
    // On macOS, cmd() may return empty due to security restrictions (SIP/sandbox),
    // while exe() uses proc_pidpath which works reliably for same-user processes.
    let cmd_args = process.cmd();
    let own_full = cmd_args
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    if let Some(source) = classify_by_binary_path(&own_full) {
        return source;
    }
    // Fallback: check exe() path (reliable on macOS via proc_pidpath)
    if own_full.is_empty() || cmd_args.is_empty() {
        if let Some(exe_path) = process.exe() {
            let exe_str = exe_path.to_string_lossy();
            if let Some(source) = classify_by_binary_path(&exe_str) {
                return source;
            }
        }
    }

    // Pass 2: Walk ancestors for sidecar + direct-parent IDE detection.
    let ancestors = collect_ancestors(sys, process);
    classify_by_ancestors(&ancestors)
}

/// Pure classification from the Claude binary's own command line.
/// Returns Some(IDE) if the path contains a known extension directory.
fn classify_by_binary_path(own_full: &str) -> Option<SessionSourceInfo> {
    if own_full.contains(".vscode/extensions/") || own_full.contains(".vscode-server/") {
        return Some(SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("VS Code".to_string()),
        });
    }
    if own_full.contains(".cursor/extensions/") || own_full.contains(".cursor-server/") {
        return Some(SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("Cursor".to_string()),
        });
    }
    if own_full.contains(".windsurf/extensions/") || own_full.contains(".windsurf-server/") {
        return Some(SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("Windsurf".to_string()),
        });
    }
    None
}

/// Collect ancestor (name, full_cmd) pairs from the process tree.
/// Uses cmd() for the full command line, with exe() fallback on macOS
/// where cmd() may return empty due to security restrictions.
fn collect_ancestors(sys: &System, process: &sysinfo::Process) -> Vec<(String, String)> {
    let mut result = Vec::new();
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
        let name = ancestor.name().to_string_lossy().to_lowercase();
        let cmd_args = ancestor.cmd();
        let mut full = cmd_args
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        // Fallback: use exe() path when cmd() is empty (macOS security restriction)
        if full.is_empty() {
            if let Some(exe_path) = ancestor.exe() {
                full = exe_path.to_string_lossy().to_string();
            }
        }
        result.push((name, full));
        current_pid = ancestor.parent();
    }
    result
}

/// Pure classification from ancestor data. Testable without real processes.
///
/// Rules:
/// - Sidecar at ANY depth → AgentSdk
/// - Direct parent (index 0) is a shell → Terminal (even if IDE is grandparent)
/// - Direct parent matches IDE pattern → IDE
/// - Everything else → Terminal
fn classify_by_ancestors(ancestors: &[(String, String)]) -> SessionSourceInfo {
    for (depth_0, (anc_name, anc_full)) in ancestors.iter().enumerate() {
        // Sidecar → Agent SDK (at any depth)
        if anc_full.contains("sidecar/dist/index.js") {
            return SessionSourceInfo {
                category: SessionSource::AgentSdk,
                label: None,
            };
        }

        let is_shell = SHELL_NAMES
            .iter()
            .any(|sh| anc_name == sh || anc_name.ends_with(sh));

        // Direct parent (depth 0): also check IDE patterns.
        // Shell parent → skip IDE (user typed `claude` in terminal).
        if depth_0 == 0 && !is_shell {
            for &(pattern, label) in IDE_PARENT_PATTERNS {
                // Substring match on full command line (catches paths and multi-word names).
                // Exact equality on process name (prevents "zed" matching "freezed").
                if anc_full.contains(pattern) || *anc_name == pattern.to_lowercase() {
                    return SessionSourceInfo {
                        category: SessionSource::Ide,
                        label: Some(label.to_string()),
                    };
                }
            }
        }

        // Skip shells — sidecar might be higher up
        if is_shell {
            continue;
        }

        // Non-shell ancestor found, not sidecar or IDE → stop walking
        break;
    }

    // Default: terminal
    SessionSourceInfo {
        category: SessionSource::Terminal,
        label: None,
    }
}

/// Fallback: get a process's working directory via `lsof`.
///
/// On macOS, `sysinfo` cannot read cwd for other processes (security restriction).
/// `lsof -a -p <pid> -d cwd -Fn` reliably returns the cwd for same-user processes.
#[allow(dead_code)] // Kept for single-PID fallback if batch lsof is unavailable
fn get_cwd_via_lsof(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}

/// Batch CWD resolution: single `lsof` call for all PIDs.
///
/// `lsof -a -p <pid1>,<pid2>,... -d cwd -Fn` returns CWDs for all PIDs in one
/// subprocess call. Output format groups by PID (lines starting with 'p') and
/// path (lines starting with 'n').
///
/// 1 call for N PIDs instead of N calls for N PIDs: O(1) subprocess overhead.
fn batch_get_cwd_via_lsof(pids: &[u32]) -> HashMap<u32, PathBuf> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let pid_arg: String = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // Note: lsof exits with code 1 when ANY PID in the batch no longer exists,
    // even if it successfully resolved the rest. We must parse stdout regardless
    // of exit code — it contains valid results for the PIDs that were still alive.
    let output = match std::process::Command::new("lsof")
        .args(["-a", "-p", &pid_arg, "-d", "cwd", "-Fn"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return HashMap::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse grouped output: 'p' lines = PID, 'n' lines = path
    let mut result = HashMap::new();
    let mut current_pid: Option<u32> = None;
    for line in stdout.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.parse().ok();
        } else if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                if let Some(pid) = current_pid {
                    result.insert(pid, PathBuf::from(path));
                }
            }
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

    // =========================================================================
    // classify_by_binary_path tests
    // =========================================================================

    #[test]
    fn binary_path_vscode_extension() {
        let cmd = "/Users/me/.vscode/extensions/anthropic.claude-code-1.0.0/cli.js";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn binary_path_vscode_server() {
        let cmd = "/home/me/.vscode-server/extensions/anthropic.claude-code/cli.js";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn binary_path_cursor_extension() {
        let cmd = "/Users/me/.cursor/extensions/anthropic.claude-code-1.0.0/cli.js";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Cursor"));
    }

    #[test]
    fn binary_path_windsurf_extension() {
        let cmd = "/Users/me/.windsurf/extensions/anthropic.claude-code-1.0.0/cli.js";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Windsurf"));
    }

    #[test]
    fn binary_path_global_binary_returns_none() {
        let cmd = "/usr/local/bin/claude";
        assert!(classify_by_binary_path(cmd).is_none());
    }

    #[test]
    fn binary_path_node_global_returns_none() {
        let cmd = "node /usr/local/lib/node_modules/@anthropic-ai/claude-code/cli.js";
        assert!(classify_by_binary_path(cmd).is_none());
    }

    // =========================================================================
    // classify_by_ancestors tests — the core logic for Issue 1
    // =========================================================================

    #[test]
    fn ancestors_empty_returns_terminal() {
        let result = classify_by_ancestors(&[]);
        assert_eq!(result.category, SessionSource::Terminal);
        assert!(result.label.is_none());
    }

    #[test]
    fn ancestors_shell_parent_returns_terminal() {
        // claude → zsh: user typed `claude` in terminal
        let ancestors = vec![("zsh".to_string(), "/bin/zsh".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Terminal);
    }

    #[test]
    fn ancestors_shell_parent_with_ide_grandparent_returns_terminal() {
        // claude → zsh → Code Helper: user typed `claude` in VS Code terminal
        // MUST be Terminal, not VS Code!
        let ancestors = vec![
            ("zsh".to_string(), "/bin/zsh".to_string()),
            (
                "code helper".to_string(),
                "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app"
                    .to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(
            result.category,
            SessionSource::Terminal,
            "CLI in VS Code terminal must be Terminal, not IDE"
        );
    }

    #[test]
    fn ancestors_vscode_helper_direct_parent_returns_ide() {
        // claude → Code Helper: VS Code extension launched Claude
        let ancestors = vec![(
            "code helper".to_string(),
            "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn ancestors_cursor_helper_direct_parent_returns_ide() {
        let ancestors = vec![(
            "cursor helper".to_string(),
            "/Applications/Cursor.app/Contents/Frameworks/Cursor Helper.app".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Cursor"));
    }

    #[test]
    fn ancestors_windsurf_helper_direct_parent_returns_ide() {
        let ancestors = vec![(
            "windsurf helper".to_string(),
            "/Applications/Windsurf.app/Contents/Frameworks/Windsurf Helper.app".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Windsurf"));
    }

    #[test]
    fn ancestors_jetbrains_direct_parent_returns_ide() {
        let ancestors = vec![(
            "idea".to_string(),
            "/Applications/IntelliJ IDEA.app/Contents/MacOS/idea".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("IntelliJ"));
    }

    #[test]
    fn ancestors_neovim_direct_parent_returns_ide() {
        let ancestors = vec![("nvim".to_string(), "/usr/local/bin/nvim".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Neovim"));
    }

    #[test]
    fn ancestors_sidecar_direct_parent_returns_agent_sdk() {
        let ancestors = vec![(
            "node".to_string(),
            "node /app/sidecar/dist/index.js".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::AgentSdk);
    }

    #[test]
    fn ancestors_sidecar_through_shell_returns_agent_sdk() {
        // claude → bash → sidecar: sidecar spawned via shell wrapper
        let ancestors = vec![
            ("bash".to_string(), "/bin/bash".to_string()),
            (
                "node".to_string(),
                "node /app/sidecar/dist/index.js".to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(
            result.category,
            SessionSource::AgentSdk,
            "Sidecar through shell must still be AgentSdk"
        );
    }

    #[test]
    fn ancestors_sidecar_with_ide_grandparent_returns_agent_sdk() {
        // Sidecar takes priority over IDE even if IDE is in the chain
        let ancestors = vec![
            (
                "node".to_string(),
                "node /app/sidecar/dist/index.js".to_string(),
            ),
            (
                "code helper".to_string(),
                "/Applications/Visual Studio Code.app/Code Helper".to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::AgentSdk);
    }

    #[test]
    fn ancestors_unknown_parent_returns_terminal() {
        // claude → some-random-daemon: not shell, not IDE, not sidecar
        let ancestors = vec![("launchd".to_string(), "/sbin/launchd".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Terminal);
    }

    #[test]
    fn ancestors_fish_shell_returns_terminal() {
        let ancestors = vec![("fish".to_string(), "/usr/bin/fish".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Terminal);
    }

    #[test]
    fn ancestors_powershell_returns_terminal() {
        let ancestors = vec![("pwsh".to_string(), "/usr/local/bin/pwsh".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Terminal);
    }

    #[test]
    fn ancestors_process_containing_zed_substring_not_ide() {
        // "freezed" contains "zed" as substring — must NOT match Zed IDE
        let ancestors = vec![("freezed".to_string(), "/usr/bin/freezed".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(
            result.category,
            SessionSource::Terminal,
            "Process name containing 'zed' as substring must not match Zed IDE"
        );
    }

    #[test]
    fn ancestors_process_named_exactly_zed_is_ide() {
        // Exact "zed" process name = Zed IDE
        let ancestors = vec![("zed".to_string(), "/usr/bin/zed".to_string())];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Zed"));
    }

    #[test]
    fn ancestors_jetbrains_bare_symlink_detected_by_name() {
        // Linux: JetBrains Toolbox symlinks bare `idea` binary
        // anc_full has no "IntelliJ IDEA" string, but process name is "idea"
        // The pattern "IntelliJ IDEA" won't match name or cmd, but we don't
        // have a bare "idea" pattern — this is a known gap (documented).
        let ancestors = vec![("idea".to_string(), "/usr/local/bin/idea".to_string())];
        let result = classify_by_ancestors(&ancestors);
        // Currently Terminal — known gap for bare symlinks without full path
        assert_eq!(result.category, SessionSource::Terminal);
    }

    // =========================================================================
    // Regression tests — real-world process trees from production incidents
    // =========================================================================

    #[test]
    fn regression_vscode_extension_native_binary_path() {
        // Real path from macOS: VS Code extension bundles native binary.
        // This is the primary detection path (Pass 1: binary path).
        let cmd = "/Users/dev/.vscode/extensions/anthropic.claude-code-2.1.81-darwin-arm64/resources/native-binary/claude";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn regression_vscode_extension_node_based_path() {
        // Node-based installs: extension runs via node cli.js
        let cmd = "node /Users/dev/.vscode/extensions/anthropic.claude-code-2.1.0/cli.js";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn regression_vscode_code_helper_plugin_direct_parent() {
        // Real macOS process tree: VS Code extension → Code Helper (Plugin) → claude
        // When exe() fallback is needed (cmd() empty on macOS).
        let ancestors = vec![(
            "code helper (plugin)".to_string(),
            "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin) --type=utility".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }

    #[test]
    fn regression_cli_in_vscode_terminal_not_misclassified() {
        // CRITICAL: User typed `claude` in VS Code's integrated terminal.
        // Parent is zsh, grandparent is Code Helper → MUST be Terminal.
        // This is the exact scenario that broke when parent detection was
        // removed (2026-03-23), and the exact scenario we must NOT break
        // when restoring it.
        let ancestors = vec![
            ("zsh".to_string(), "/bin/zsh -l".to_string()),
            (
                "code helper (plugin)".to_string(),
                "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin) --type=utility".to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(
            result.category,
            SessionSource::Terminal,
            "REGRESSION: CLI typed in VS Code terminal must be Terminal, not VS Code"
        );
    }

    #[test]
    fn regression_cursor_extension_binary_path() {
        let cmd = "/Users/dev/.cursor/extensions/anthropic.claude-code-2.1.0-darwin-arm64/resources/native-binary/claude";
        let result = classify_by_binary_path(cmd).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Cursor"));
    }

    #[test]
    fn regression_cursor_helper_direct_parent() {
        let ancestors = vec![(
            "cursor helper (plugin)".to_string(),
            "/Applications/Cursor.app/Contents/Frameworks/Cursor Helper (Plugin).app/Contents/MacOS/Cursor Helper (Plugin)".to_string(),
        )];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("Cursor"));
    }

    #[test]
    fn regression_sidecar_via_shell_wrapper() {
        // Sidecar spawns claude via a shell script wrapper.
        // Must still be detected as AgentSdk.
        let ancestors = vec![
            ("sh".to_string(), "/bin/sh -c claude".to_string()),
            ("bash".to_string(), "/bin/bash".to_string()),
            (
                "node".to_string(),
                "node /app/sidecar/dist/index.js".to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(
            result.category,
            SessionSource::AgentSdk,
            "REGRESSION: sidecar through multiple shell layers must still be AgentSdk"
        );
    }

    #[test]
    fn regression_global_binary_in_plain_terminal() {
        // Global `claude` binary in a regular terminal.
        // Parent is bash, no IDE or sidecar → Terminal.
        let ancestors = vec![
            ("bash".to_string(), "/bin/bash --login".to_string()),
            (
                "terminal".to_string(),
                "/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal".to_string(),
            ),
        ];
        let result = classify_by_ancestors(&ancestors);
        assert_eq!(result.category, SessionSource::Terminal);
    }

    #[test]
    fn regression_empty_cmd_with_vscode_exe_path() {
        // macOS: cmd() returns empty but exe() has the VS Code extension path.
        // The classify_source function should use exe() as fallback.
        // This tests classify_by_binary_path which is called with exe() path.
        let exe_path = "/Users/dev/.vscode/extensions/anthropic.claude-code-2.1.81-darwin-arm64/resources/native-binary/claude";

        // cmd() is empty → classify_by_binary_path("") returns None
        assert!(classify_by_binary_path("").is_none());
        // exe() fallback → classify_by_binary_path(exe_path) returns VS Code
        let result = classify_by_binary_path(exe_path).unwrap();
        assert_eq!(result.category, SessionSource::Ide);
        assert_eq!(result.label.as_deref(), Some("VS Code"));
    }
}
