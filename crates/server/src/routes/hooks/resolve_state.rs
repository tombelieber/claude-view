//! Map hook events to AgentState — the SOLE authority for agent state.
//!
//! All 25 Claude Code events have explicit arms. Unknown events fall through
//! to the wildcard which produces an Observability-safe fallback.

use crate::live::state::{AgentState, AgentStateGroup};

use super::activity::activity_from_pre_tool;
use super::HookPayload;

/// Map a hook event to an `AgentState`.
///
/// This is the SOLE authority for agent state. All 25 events are explicit.
pub(super) fn resolve_state_from_hook(payload: &HookPayload) -> AgentState {
    match payload.hook_event_name.as_str() {
        "SessionStart" => match payload.source.as_deref() {
            Some("compact") => AgentState {
                group: AgentStateGroup::Autonomous,
                state: "thinking".into(),
                label: "Compacting context...".into(),
                context: None,
            },
            Some("resume") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Session resumed".into(),
                context: None,
            },
            Some("clear") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Session cleared".into(),
                context: None,
            },
            _ => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Waiting for first prompt".into(),
                context: None,
            },
        },
        "UserPromptSubmit" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Processing prompt...".into(),
            context: None,
        },
        "PreToolUse" => {
            let tool_name = payload.tool_name.as_deref().unwrap_or("unknown");
            match tool_name {
                "AskUserQuestion" => AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "awaiting_input".into(),
                    label: "Asked you a question".into(),
                    context: payload.tool_input.clone(),
                },
                "ExitPlanMode" => AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "awaiting_approval".into(),
                    label: "Plan ready for review".into(),
                    context: None,
                },
                "EnterPlanMode" => AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Entering plan mode...".into(),
                    context: None,
                },
                _ => AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: activity_from_pre_tool(tool_name, &payload.tool_input),
                    context: None,
                },
            }
        }
        "PostToolUse" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Thinking...".into(),
            context: None,
        },
        "PostToolUseFailure" => {
            if payload.is_interrupt.unwrap_or(false) {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "interrupted".into(),
                    label: format!(
                        "You interrupted {}",
                        payload.tool_name.as_deref().unwrap_or("tool")
                    ),
                    context: None,
                }
            } else {
                AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "error".into(),
                    label: format!("Failed: {}", payload.tool_name.as_deref().unwrap_or("tool")),
                    context: payload
                        .error
                        .as_ref()
                        .map(|e| serde_json::json!({"error": e})),
                }
            }
        }
        "PermissionRequest" => {
            let tool = payload.tool_name.as_deref().unwrap_or("tool");
            AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "needs_permission".into(),
                label: format!("Needs permission: {}", tool),
                context: payload.tool_input.clone(),
            }
        }
        "Stop" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting for your next prompt".into(),
            context: None,
        },
        "StopFailure" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "error".into(),
            label: format!(
                "API error: {}",
                payload.error.as_deref().unwrap_or("unknown")
            ),
            context: payload
                .error_details
                .as_ref()
                .map(|d| serde_json::json!({"details": d})),
        },
        "Notification" => match payload.notification_type.as_deref() {
            Some("permission_prompt") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "needs_permission".into(),
                label: "Needs permission".into(),
                context: None,
            },
            Some("idle_prompt") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Session idle".into(),
                context: None,
            },
            Some("elicitation_dialog") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: payload
                    .message
                    .as_deref()
                    .map(|m| m.chars().take(100).collect::<String>())
                    .unwrap_or_else(|| "Awaiting input".into()),
                context: None,
            },
            // Unknown notification types → preserve state (Observability path).
            // This AgentState is a defensive fallback only.
            _ => AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Notification".into(),
                context: None,
            },
        },
        "SubagentStart" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!(
                "Running {} agent",
                payload.agent_type.as_deref().unwrap_or("sub")
            ),
            context: None,
        },
        "SubagentStop" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!(
                "{} agent finished",
                payload.agent_type.as_deref().unwrap_or("Sub")
            ),
            context: None,
        },
        "TaskCreated" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: payload
                .task_subject
                .clone()
                .unwrap_or_else(|| "Task created".into()),
            context: None,
        },
        "TaskCompleted" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "task_complete".into(),
            label: payload
                .task_subject
                .clone()
                .unwrap_or_else(|| "Task completed".into()),
            context: None,
        },
        "TeammateIdle" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!(
                "Teammate {} idle",
                payload.teammate_name.as_deref().unwrap_or("unknown")
            ),
            context: None,
        },
        "PreCompact" => {
            let trigger = payload
                .trigger
                .as_deref()
                .or(payload.source.as_deref())
                .unwrap_or("auto");
            AgentState {
                group: AgentStateGroup::Autonomous,
                state: "compacting".into(),
                label: if trigger == "manual" {
                    "Compacting context...".into()
                } else {
                    "Auto-compacting context...".into()
                },
                context: None,
            }
        }
        "PostCompact" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Context compacted".into(),
            context: None,
        },
        "Elicitation" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "waiting_mcp_input".into(),
            label: payload
                .message
                .as_deref()
                .map(|m| m.chars().take(100).collect::<String>())
                .unwrap_or_else(|| "MCP input requested".into()),
            context: None,
        },
        "ElicitationResult" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Processing MCP response".into(),
            context: None,
        },
        "SessionEnd" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session ended".into(),
            context: None,
        },
        // Observability events + unknown: resolved state is unused
        // (Observability variant skips state update), but we need a
        // fallback for the hook_event label.
        _ => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Event: {}", payload.hook_event_name),
            context: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{status_from_agent_state, AgentStateGroup, SessionStatus};

    fn minimal_payload(event: &str) -> HookPayload {
        super::super::tests::minimal_payload(event)
    }

    #[test]
    fn session_start_startup_returns_idle_state() {
        let mut payload = minimal_payload("SessionStart");
        payload.source = Some("startup".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn session_start_compact_returns_thinking_state() {
        let mut payload = minimal_payload("SessionStart");
        payload.source = Some("compact".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn user_prompt_submit_returns_thinking_state() {
        let payload = minimal_payload("UserPromptSubmit");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn stop_returns_idle_state() {
        let payload = minimal_payload("Stop");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn notification_permission_prompt_returns_needs_permission() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("permission_prompt".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "needs_permission");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn notification_idle_prompt_returns_idle() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("idle_prompt".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn notification_elicitation_returns_awaiting_input() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("elicitation_dialog".into());
        payload.message = Some("Pick a color".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_input");
        assert!(state.label.contains("Pick a color"));
    }

    #[test]
    fn notification_unknown_type_is_observability() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("auth_success".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn post_tool_use_failure_returns_error() {
        let mut payload = minimal_payload("PostToolUseFailure");
        payload.is_interrupt = Some(false);
        payload.tool_name = Some("Bash".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "error");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Bash"));
    }

    #[test]
    fn post_tool_use_failure_with_interrupt_returns_interrupted() {
        let mut payload = minimal_payload("PostToolUseFailure");
        payload.is_interrupt = Some(true);
        payload.tool_name = Some("Read".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "interrupted");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Read"));
    }

    #[test]
    fn session_end_returns_session_ended() {
        let payload = minimal_payload("SessionEnd");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "session_ended");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn pre_tool_use_bash_returns_acting_with_command() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("Bash".into());
        payload.tool_input = Some(serde_json::json!({"command": "git status"}));
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("git status"));
    }

    #[test]
    fn pre_tool_use_read_returns_acting_with_filename() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("Read".into());
        payload.tool_input = Some(serde_json::json!({"file_path": "/src/lib.rs"}));
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(state.label.contains("lib.rs"));
    }

    #[test]
    fn pre_tool_use_ask_user_question_returns_awaiting_input() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("AskUserQuestion".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_input");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn pre_tool_use_exit_plan_mode_returns_awaiting_approval() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("ExitPlanMode".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_approval");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn post_tool_use_returns_thinking() {
        let payload = minimal_payload("PostToolUse");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn permission_request_returns_needs_permission() {
        let mut payload = minimal_payload("PermissionRequest");
        payload.tool_name = Some("Bash".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "needs_permission");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Bash"));
    }

    #[test]
    fn teammate_idle_returns_delegating() {
        let mut payload = minimal_payload("TeammateIdle");
        payload.teammate_name = Some("researcher".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "delegating");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("researcher"));
    }

    #[test]
    fn task_completed_returns_task_complete() {
        let mut payload = minimal_payload("TaskCompleted");
        payload.task_subject = Some("Fix login bug".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "task_complete");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Fix login bug"));
    }

    #[test]
    fn pre_compact_auto_returns_compacting() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("auto".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "compacting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Auto-compacting"));
    }

    #[test]
    fn pre_compact_manual_returns_compacting() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("manual".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "compacting");
        assert!(state.label.contains("Compacting context"));
        assert!(!state.label.contains("Auto"));
    }

    #[test]
    fn subagent_start_returns_delegating() {
        let mut payload = minimal_payload("SubagentStart");
        payload.agent_type = Some("code-explorer".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "delegating");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn subagent_stop_returns_acting() {
        let payload = minimal_payload("SubagentStop");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn pre_tool_use_mcp_tool() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("mcp__github__create_issue".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(state.label.starts_with("MCP: "));
    }

    // --- Metadata-only events resolve but are not applied ---

    #[test]
    fn subagent_stop_resolves_but_not_applied() {
        let state = resolve_state_from_hook(&minimal_payload("SubagentStop"));
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn teammate_idle_resolves_but_not_applied() {
        let mut p = minimal_payload("TeammateIdle");
        p.teammate_name = Some("researcher".into());
        let state = resolve_state_from_hook(&p);
        assert_eq!(state.state, "delegating");
    }

    #[test]
    fn task_completed_resolves_but_not_applied() {
        let mut p = minimal_payload("TaskCompleted");
        p.task_subject = Some("Fix bug".into());
        let state = resolve_state_from_hook(&p);
        assert_eq!(state.state, "task_complete");
    }

    #[test]
    fn status_from_agent_state_integration() {
        let acting = resolve_state_from_hook(&{
            let mut p = minimal_payload("PostToolUse");
            p.tool_name = Some("Bash".into());
            p
        });
        assert_eq!(status_from_agent_state(&acting), SessionStatus::Working);

        let idle = resolve_state_from_hook(&minimal_payload("Stop"));
        assert_eq!(status_from_agent_state(&idle), SessionStatus::Paused);

        let ended = resolve_state_from_hook(&minimal_payload("SessionEnd"));
        assert_eq!(status_from_agent_state(&ended), SessionStatus::Done);
    }
}
