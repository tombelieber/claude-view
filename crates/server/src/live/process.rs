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
    let cmd_args = process.cmd();
    let own_full = cmd_args
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    if let Some(source) = classify_by_binary_path(&own_full) {
        return source;
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
        let full = ancestor
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
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
}
