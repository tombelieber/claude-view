// =============================================================================
// Classification helpers
// =============================================================================

/// Check if a process is the claude-view binary (not sidecar, not a directory match).
/// Requires the process name itself to be "claude-view" — pure substring matching
/// on the command line would false-positive on paths like `/backup/claude-view-old/`.
pub(super) fn is_claude_view_binary(name: &str, cmd: &str) -> bool {
    name == "claude-view" && !cmd.contains("sidecar/dist/index.js")
}

/// Check if a process named "claude" is actually from the Anthropic Claude package.
/// Validates against known installation paths and package identifiers to prevent
/// false positives from unrelated binaries that happen to be named "claude".
pub(super) fn is_anthropic_claude(cmd: &str) -> bool {
    // Empty command (SIP-restricted on macOS) — accept name-only match as fallback
    // because the sysctl resolution already ran and couldn't resolve it.
    if cmd.is_empty() {
        return true;
    }
    // High-confidence: known Anthropic package paths
    if cmd.contains("@anthropic-ai/claude")
        || cmd.contains("anthropic.claude-code")
        || cmd.contains(".claude/local")
    {
        return true;
    }
    // Medium-confidence: the first token's basename must be exactly "claude"
    // (not "claude-game", "claude-wrapper", etc.)
    let first_token = cmd.split_whitespace().next().unwrap_or("");
    let basename = first_token.rsplit('/').next().unwrap_or(first_token);
    basename == "claude"
}
