//! Pure function: apply a LifecycleEvent to HookFields.
//!
//! Extracted from `routes/hooks.rs` — no IO, no locks, no broadcasts.
//! The coordinator calls this and then handles side effects separately.

use crate::live::mutation::types::{LifecycleEvent, SubEntityEvent};
use crate::live::state::{AgentState, AgentStateGroup, HookFields, SessionStatus};

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
///
/// Re-exported here so mutation callers don't need to reach into `state.rs`.
pub fn status_from_agent_state(state: &AgentState) -> SessionStatus {
    crate::live::state::status_from_agent_state(state)
}

/// Apply a lifecycle event to hook fields, returning a new SessionStatus
/// when the event changes it (StateChange, End).
///
/// Pure function — no IO, no locks. The caller is responsible for:
/// - Setting `session.status` from the returned `Option<SessionStatus>`
/// - Updating `session.hook.last_activity_at` (always `now`)
/// - PID binding (caller checks `hook.pid.is_none()` separately)
/// - Broadcasting SSE events
pub fn apply_lifecycle(
    hook: &mut HookFields,
    event: &LifecycleEvent,
    now: i64,
) -> Option<SessionStatus> {
    match event {
        LifecycleEvent::Start {
            pid,
            source,
            model: _,
            cwd: _,
            transcript_path: _,
        } => {
            // Update PID if not already bound
            if hook.pid.is_none() {
                if let Some(p) = pid {
                    hook.pid = Some(*p);
                }
            }
            // "clear" source resets turn state (e.g. /clear command)
            if source.as_deref() == Some("clear") {
                hook.turn_count = 0;
                hook.current_turn_started_at = None;
            }
            None
        }

        LifecycleEvent::Prompt { text, pid } => {
            // Truncate to 500 chars for display
            hook.last_user_message = text.chars().take(500).collect();
            if hook.title.is_empty() {
                hook.title = hook.last_user_message.clone();
            }
            hook.turn_count += 1;
            hook.current_turn_started_at = Some(now);

            // PID binding
            if hook.pid.is_none() {
                if let Some(p) = pid {
                    hook.pid = Some(*p);
                }
            }
            None
        }

        LifecycleEvent::StateChange {
            agent_state,
            event_name,
            pid,
        } => {
            // PostToolUse during compacting = skip state change
            // (compacting state is sticky until PreCompact clears it)
            if event_name == "PostToolUse" && hook.agent_state.state == "compacting" {
                if hook.pid.is_none() {
                    if let Some(p) = pid {
                        hook.pid = Some(*p);
                    }
                }
                return None;
            }

            // Context preservation: if transitioning to the same state and
            // the new state has no context, keep the existing context.
            let mut new_state = agent_state.clone();
            if new_state.context.is_none()
                && hook.agent_state.state == new_state.state
                && hook.agent_state.context.is_some()
            {
                new_state.context = hook.agent_state.context.clone();
            }

            let status = status_from_agent_state(&new_state);
            hook.agent_state = new_state;
            hook.current_activity = agent_state.label.clone();

            // PID binding
            if hook.pid.is_none() {
                if let Some(p) = pid {
                    hook.pid = Some(*p);
                }
            }

            Some(status)
        }

        LifecycleEvent::End => {
            hook.agent_state = AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "session_ended".into(),
                label: "Session ended".into(),
                context: None,
            };
            hook.hook_events.clear();
            Some(SessionStatus::Done)
        }

        LifecycleEvent::SubEntity(sub) => {
            match sub {
                SubEntityEvent::SubagentComplete {
                    agent_type,
                    agent_id,
                } => {
                    for agent in &mut hook.sub_agents {
                        let type_match = !agent_type.is_empty() && agent.agent_type == *agent_type;
                        let id_match = agent_id.as_ref().is_some_and(|id| !id.is_empty())
                            && agent.agent_id.as_deref() == agent_id.as_deref();
                        if type_match || id_match {
                            agent.status = claude_view_core::subagent::SubAgentStatus::Complete;
                        }
                    }
                }
                SubEntityEvent::TaskComplete { task_id } => {
                    for item in &mut hook.progress_items {
                        if item.id.as_deref() == Some(task_id.as_str()) {
                            item.status = claude_view_core::progress::ProgressStatus::Completed;
                        }
                    }
                }
                SubEntityEvent::TeammateIdle => {
                    // Informational only — no state mutation needed
                }
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{AgentState, AgentStateGroup, HookEvent, HookFields};

    fn make_hook_fields() -> HookFields {
        HookFields::default()
    }

    fn make_autonomous_state(state: &str, label: &str) -> AgentState {
        AgentState {
            group: AgentStateGroup::Autonomous,
            state: state.into(),
            label: label.into(),
            context: None,
        }
    }

    #[test]
    fn prompt_increments_turn_count() {
        let mut hook = make_hook_fields();
        assert_eq!(hook.turn_count, 0);

        let event = LifecycleEvent::Prompt {
            text: "Hello world".into(),
            pid: None,
        };
        apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(hook.turn_count, 1);

        apply_lifecycle(&mut hook, &event, 1001);
        assert_eq!(hook.turn_count, 2);
    }

    #[test]
    fn prompt_sets_title_if_empty() {
        let mut hook = make_hook_fields();
        assert!(hook.title.is_empty());

        let event = LifecycleEvent::Prompt {
            text: "Fix the bug in main.rs".into(),
            pid: None,
        };
        apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(hook.title, "Fix the bug in main.rs");

        // Second prompt should NOT overwrite existing title
        let event2 = LifecycleEvent::Prompt {
            text: "Also update tests".into(),
            pid: None,
        };
        apply_lifecycle(&mut hook, &event2, 1001);
        assert_eq!(hook.title, "Fix the bug in main.rs");
    }

    #[test]
    fn end_clears_hook_events_and_sets_done() {
        let mut hook = make_hook_fields();
        hook.hook_events.push(HookEvent {
            timestamp: 1,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Reading".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook".into(),
        });
        assert!(!hook.hook_events.is_empty());

        let result = apply_lifecycle(&mut hook, &LifecycleEvent::End, 2000);
        assert_eq!(result, Some(SessionStatus::Done));
        assert!(hook.hook_events.is_empty());
        assert_eq!(hook.agent_state.state, "session_ended");
    }

    #[test]
    fn state_change_returns_status() {
        let mut hook = make_hook_fields();
        let state = make_autonomous_state("acting", "Working on task");

        let event = LifecycleEvent::StateChange {
            agent_state: state,
            event_name: "PreToolUse".into(),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(result, Some(SessionStatus::Working));
        assert_eq!(hook.agent_state.state, "acting");
    }

    #[test]
    fn post_tool_use_during_compacting_skips_state_change() {
        let mut hook = make_hook_fields();
        // Set current state to compacting
        hook.agent_state = make_autonomous_state("compacting", "Compacting context");

        let new_state = make_autonomous_state("acting", "Working");
        let event = LifecycleEvent::StateChange {
            agent_state: new_state,
            event_name: "PostToolUse".into(),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);

        // Should return None (skipped) and state should remain "compacting"
        assert!(result.is_none());
        assert_eq!(hook.agent_state.state, "compacting");
    }

    #[test]
    fn context_preserved_on_same_state() {
        let mut hook = make_hook_fields();
        // Set initial state with context
        hook.agent_state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "tool_use".into(),
            label: "Using Read".into(),
            context: Some(serde_json::json!({"file": "main.rs"})),
        };

        // Transition to same state but without context
        let new_state = AgentState {
            group: AgentStateGroup::Autonomous,
            state: "tool_use".into(),
            label: "Using Read".into(),
            context: None,
        };
        let event = LifecycleEvent::StateChange {
            agent_state: new_state,
            event_name: "PreToolUse".into(),
            pid: None,
        };
        apply_lifecycle(&mut hook, &event, 1000);

        // Context should be preserved from the previous state
        assert!(hook.agent_state.context.is_some());
        assert_eq!(
            hook.agent_state.context.as_ref().unwrap().to_string(),
            r#"{"file":"main.rs"}"#
        );
    }
}
