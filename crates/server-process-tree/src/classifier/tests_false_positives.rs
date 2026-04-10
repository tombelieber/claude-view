use super::pipeline::classify_process_list;
use super::rules::{is_anthropic_claude, is_claude_view_binary, is_node_running_claude};
use crate::types::{EcosystemTag, RawProcessInfo};

fn make_raw(
    pid: u32,
    ppid: u32,
    name: &str,
    command: &str,
    cpu: f32,
    mem: u64,
    start_time: i64,
) -> RawProcessInfo {
    RawProcessInfo {
        pid,
        ppid,
        name: name.into(),
        command: command.into(),
        cpu_percent: cpu,
        memory_bytes: mem,
        start_time,
    }
}

// =========================================================================
// False-positive prevention tests
// =========================================================================

#[test]
fn claude_wrapper_binary_not_classified() {
    // A wrapper/launcher binary with "claude" in process name but different command basename
    let processes = vec![make_raw(
        900,
        1,
        "claude",
        "/opt/tools/claude-wrapper --mode=chatbot --port=3000",
        1.0,
        50_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert!(
        snap.ecosystem.is_empty(),
        "binary with basename 'claude-wrapper' must not be classified as CLI ecosystem"
    );
}

#[test]
fn anthropic_claude_with_package_path_classified() {
    let processes = vec![make_raw(
        901,
        99,
        "claude",
        "node /home/user/.nvm/versions/node/v22/lib/node_modules/@anthropic-ai/claude/cli.mjs chat",
        5.0,
        200_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert_eq!(snap.ecosystem.len(), 1);
    assert!(matches!(
        snap.ecosystem[0].ecosystem_tag,
        Some(EcosystemTag::Cli)
    ));
}

#[test]
fn claude_binary_at_standard_path_classified() {
    // Standard install: /usr/local/bin/claude
    let processes = vec![make_raw(
        902,
        99,
        "claude",
        "/usr/local/bin/claude --verbose",
        5.0,
        200_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert_eq!(snap.ecosystem.len(), 1);
    assert!(matches!(
        snap.ecosystem[0].ecosystem_tag,
        Some(EcosystemTag::Cli)
    ));
}

#[test]
fn claude_with_empty_command_rejected() {
    // macOS SIP restriction: sysinfo returns empty cmd.
    // Empty command must NOT classify — it was a false-positive source
    // for any process named "claude" when sysctl fails.
    let processes = vec![make_raw(
        903,
        99,
        "claude",
        "",
        5.0,
        200_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert!(
        snap.ecosystem.is_empty(),
        "empty command must not be classified as CLI ecosystem"
    );
}

#[test]
fn claude_view_directory_in_path_not_classified_as_self() {
    // A script running from a directory that happens to contain "claude-view" in its path
    let processes = vec![make_raw(
        904,
        1,
        "bash",
        "/home/user/claude-view-backup/restore.sh",
        0.5,
        10_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert!(
        snap.ecosystem.is_empty(),
        "process from claude-view-named directory must not be classified as Self_"
    );
}

#[test]
fn is_anthropic_claude_helper_cases() {
    // Package path
    assert!(is_anthropic_claude(
        "node /path/to/@anthropic-ai/claude/cli.mjs"
    ));
    // Standard binary path
    assert!(is_anthropic_claude("/usr/local/bin/claude --verbose"));
    // Bare command
    assert!(is_anthropic_claude("claude chat"));
    // Empty command — sysctl failed, do not classify (was a false-positive source)
    assert!(!is_anthropic_claude(""));
    // Binary with different basename (e.g. claude-wrapper, claude-game)
    assert!(!is_anthropic_claude(
        "/opt/tools/claude-wrapper --mode=chatbot"
    ));
    assert!(!is_anthropic_claude("/opt/games/claude-game start"));
    // Binary named "claude" at non-standard path — accepted (basename is "claude")
    assert!(is_anthropic_claude("/opt/custom/bin/claude --flag"));
}

#[test]
fn is_claude_view_binary_helper_cases() {
    assert!(is_claude_view_binary(
        "claude-view",
        "/usr/local/bin/claude-view serve"
    ));
    assert!(!is_claude_view_binary(
        "bash",
        "/home/user/claude-view-backup/script.sh"
    ));
    assert!(!is_claude_view_binary(
        "node",
        "node /path/to/claude-view/sidecar/dist/index.js"
    ));
}

#[test]
fn is_anthropic_claude_empty_command_returns_false() {
    assert!(
        !is_anthropic_claude(""),
        "empty command must return false — sysctl failure is not evidence of Claude"
    );
}

#[test]
fn is_node_running_claude_npx_case() {
    assert!(is_node_running_claude(
        "node",
        "node /path/.bin/claude --version"
    ));
}

#[test]
fn is_node_running_claude_shebang_env_case() {
    assert!(is_node_running_claude(
        "env",
        "/usr/bin/env node /path/.bin/claude"
    ));
}

#[test]
fn is_node_running_claude_npm_global() {
    assert!(is_node_running_claude(
        "node",
        "node /path/@anthropic-ai/claude-code/cli.js"
    ));
}

#[test]
fn is_node_running_claude_unrelated_node_app() {
    assert!(!is_node_running_claude("node", "node /path/to/my-app.js"));
}

#[test]
fn is_node_running_claude_wrong_process_name() {
    assert!(!is_node_running_claude("python", "python claude.py"));
}

#[test]
fn is_node_running_claude_empty_command() {
    assert!(!is_node_running_claude("node", ""));
}

#[test]
fn npx_claude_classified_as_cli_in_pipeline() {
    // npm/npx-installed Claude Code: process name is "node", not "claude"
    let processes = vec![make_raw(
        84064,
        99,
        "node",
        "node /Users/test/.npm/_npx/abc123/node_modules/.bin/claude --version",
        1.0,
        100_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    assert_eq!(snap.ecosystem.len(), 1);
    assert!(matches!(
        snap.ecosystem[0].ecosystem_tag,
        Some(EcosystemTag::Cli)
    ));
}
