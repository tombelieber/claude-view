//! Small, pure helper functions for hook processing.

use crate::live::state::{AgentStateGroup, HookEvent};

/// Build a HookEvent from hook handler data.
pub(super) fn build_hook_event(
    timestamp: i64,
    event_name: &str,
    tool_name: Option<&str>,
    label: &str,
    group: &str,
    context: Option<&serde_json::Value>,
    source: &str,
) -> HookEvent {
    HookEvent {
        timestamp,
        event_name: event_name.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label: label.to_string(),
        group: group.to_string(),
        context: context.map(|v| v.to_string()),
        source: source.to_string(),
    }
}

pub(super) fn group_name_from_agent_group(group: &AgentStateGroup) -> &'static str {
    match group {
        AgentStateGroup::NeedsYou => "needs_you",
        AgentStateGroup::Autonomous => "autonomous",
    }
}

/// Extract and validate a PID from the X-Claude-PID header value.
///
/// Returns None if the header is missing, empty, non-numeric, or <= 1
/// (PID 0 = kernel, PID 1 = init/launchd — indicates reparenting).
pub(super) fn extract_pid_from_header(header_value: Option<&str>) -> Option<u32> {
    let value = header_value?.trim();
    let pid: u32 = value.parse().ok()?;
    if pid <= 1 {
        return None;
    }
    Some(pid)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_hook_event_basic() {
        let event = build_hook_event(
            1708000000,
            "PreToolUse",
            Some("Read"),
            "Reading file.rs",
            "autonomous",
            None,
            "hook",
        );
        assert_eq!(event.event_name, "PreToolUse");
        assert_eq!(event.tool_name, Some("Read".to_string()));
        assert_eq!(event.label, "Reading file.rs");
        assert_eq!(event.group, "autonomous");
        assert_eq!(event.timestamp, 1708000000);
        assert!(event.context.is_none());
        assert_eq!(event.source, "hook");
    }

    #[test]
    fn build_hook_event_with_context() {
        let ctx = serde_json::json!({"command": "git status"});
        let event = build_hook_event(
            1708000000,
            "PreToolUse",
            Some("Bash"),
            "Running: git status",
            "autonomous",
            Some(&ctx),
            "hook",
        );
        assert_eq!(event.context, Some(ctx.to_string()));
        assert_eq!(event.source, "hook");
    }

    #[test]
    fn extract_pid_from_header_valid() {
        let pid = extract_pid_from_header(Some("12345"));
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn extract_pid_from_header_invalid() {
        assert_eq!(extract_pid_from_header(None), None);
        assert_eq!(extract_pid_from_header(Some("")), None);
        assert_eq!(extract_pid_from_header(Some("abc")), None);
        assert_eq!(extract_pid_from_header(Some("0")), None);
        assert_eq!(extract_pid_from_header(Some("1")), None);
    }
}
