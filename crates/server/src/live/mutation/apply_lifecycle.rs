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

/// Bind PID if not already set. Extracted to avoid repetition.
fn bind_pid(hook: &mut HookFields, pid: Option<u32>) {
    if hook.pid.is_none() {
        if let Some(p) = pid {
            hook.pid = Some(p);
        }
    }
}

/// Apply a lifecycle event to hook fields, returning a new SessionStatus
/// when the event changes it (StateChange, End, Stop, StopFailure, etc.).
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
        LifecycleEvent::Start { pid, source, .. } => {
            bind_pid(hook, *pid);
            if source.as_deref() == Some("clear") {
                hook.turn_count = 0;
                hook.current_turn_started_at = None;
            }
            None
        }

        LifecycleEvent::Prompt { text, pid } => {
            hook.last_user_message = text.chars().take(500).collect();
            if hook.title.is_empty() {
                hook.title = hook.last_user_message.clone();
            }
            hook.turn_count += 1;
            hook.current_turn_started_at = Some(now);
            bind_pid(hook, *pid);
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
                bind_pid(hook, *pid);
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
            bind_pid(hook, *pid);
            Some(status)
        }

        LifecycleEvent::Stop {
            agent_state,
            last_assistant_message,
            pid,
        } => {
            // Context preservation (same logic as StateChange)
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
            hook.current_turn_started_at = None; // turn ended

            // Store truncated preview
            hook.last_assistant_preview = last_assistant_message
                .as_ref()
                .map(|m| m.chars().take(200).collect());

            // Clear error state (session recovered from previous StopFailure)
            hook.last_error = None;
            hook.last_error_details = None;

            bind_pid(hook, *pid);
            Some(status)
        }

        LifecycleEvent::StopFailure {
            error,
            error_details,
            pid,
        } => {
            hook.agent_state = AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "error".into(),
                label: format!("API error: {}", error.as_deref().unwrap_or("unknown")),
                context: error_details
                    .as_ref()
                    .map(|d| serde_json::json!({"details": d})),
            };
            hook.current_activity = hook.agent_state.label.clone();
            hook.current_turn_started_at = None;
            hook.last_error = error.clone();
            hook.last_error_details = error_details.clone();
            bind_pid(hook, *pid);
            Some(SessionStatus::Paused) // NOT Done — may resume after rate limit
        }

        LifecycleEvent::End { reason } => {
            let label = match reason.as_deref() {
                Some(r) if !r.is_empty() => format!("Session ended ({})", r),
                _ => "Session ended".into(),
            };
            hook.agent_state = AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "session_ended".into(),
                label,
                context: None,
            };
            hook.hook_events.clear();
            Some(SessionStatus::Done)
        }

        LifecycleEvent::Compacted {
            trigger: _,
            summary,
            pid,
        } => {
            hook.compact_count += 1;
            hook.agent_state = AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Context compacted".into(),
                context: summary.as_ref().map(
                    |s| serde_json::json!({"summary": s.chars().take(500).collect::<String>()}),
                ),
            };
            hook.current_activity = hook.agent_state.label.clone();
            bind_pid(hook, *pid);
            Some(SessionStatus::Paused)
        }

        LifecycleEvent::CwdChanged { pid, .. } => {
            // Observability only — don't change agent_state
            bind_pid(hook, *pid);
            None
        }

        LifecycleEvent::Observability { pid, .. } => {
            // Observability only — don't change agent_state
            bind_pid(hook, *pid);
            None
        }

        LifecycleEvent::SubagentStarted {
            agent_state,
            agent_type,
            agent_id,
            pid,
        } => {
            hook.agent_state = agent_state.clone();
            hook.current_activity = agent_state.label.clone();

            hook.sub_agents
                .push(claude_view_core::subagent::SubAgentInfo {
                    tool_use_id: String::new(), // Hook events don't carry tool_use_id
                    agent_id: agent_id.clone(),
                    agent_type: agent_type.clone(),
                    description: String::new(),
                    status: claude_view_core::subagent::SubAgentStatus::Running,
                    started_at: now,
                    completed_at: None,
                    duration_ms: None,
                    tool_use_count: None,
                    model: None,
                    input_tokens: None,
                    output_tokens: None,
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                    cost_usd: None,
                    current_activity: None,
                });

            bind_pid(hook, *pid);
            Some(status_from_agent_state(agent_state))
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
                SubEntityEvent::TaskCreated {
                    task_id,
                    subject,
                    description: _,
                } => {
                    hook.progress_items
                        .push(claude_view_core::progress::ProgressItem {
                            id: Some(task_id.clone()),
                            tool_use_id: None,
                            title: subject.clone().unwrap_or_else(|| "Task created".into()),
                            status: claude_view_core::progress::ProgressStatus::InProgress,
                            active_form: None,
                            source: claude_view_core::progress::ProgressSource::Task,
                        });
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

        let result = apply_lifecycle(&mut hook, &LifecycleEvent::End { reason: None }, 2000);
        assert_eq!(result, Some(SessionStatus::Done));
        assert!(hook.hook_events.is_empty());
        assert_eq!(hook.agent_state.state, "session_ended");
    }

    #[test]
    fn end_with_reason_includes_reason_in_label() {
        let mut hook = make_hook_fields();
        let result = apply_lifecycle(
            &mut hook,
            &LifecycleEvent::End {
                reason: Some("clear".into()),
            },
            2000,
        );
        assert_eq!(result, Some(SessionStatus::Done));
        assert_eq!(hook.agent_state.label, "Session ended (clear)");
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

    // ── New variant tests ──

    #[test]
    fn stop_clears_turn_and_stores_preview() {
        let mut hook = make_hook_fields();
        hook.current_turn_started_at = Some(999);

        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting".into(),
            context: None,
        };
        let event = LifecycleEvent::Stop {
            agent_state: state,
            last_assistant_message: Some("I've completed the task.".into()),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(result, Some(SessionStatus::Paused));
        assert!(hook.current_turn_started_at.is_none());
        assert_eq!(
            hook.last_assistant_preview.as_deref(),
            Some("I've completed the task.")
        );
    }

    #[test]
    fn stop_failure_sets_error_state() {
        let mut hook = make_hook_fields();
        let event = LifecycleEvent::StopFailure {
            error: Some("rate_limit".into()),
            error_details: Some("429 Too Many Requests".into()),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(result, Some(SessionStatus::Paused));
        assert_eq!(hook.agent_state.state, "error");
        assert_eq!(hook.agent_state.group, AgentStateGroup::NeedsYou);
        assert_eq!(hook.last_error.as_deref(), Some("rate_limit"));
    }

    #[test]
    fn stop_clears_previous_error() {
        let mut hook = make_hook_fields();
        hook.last_error = Some("rate_limit".into());

        let state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting".into(),
            context: None,
        };
        let event = LifecycleEvent::Stop {
            agent_state: state,
            last_assistant_message: None,
            pid: None,
        };
        apply_lifecycle(&mut hook, &event, 1000);
        assert!(hook.last_error.is_none());
    }

    #[test]
    fn compacted_increments_count() {
        let mut hook = make_hook_fields();
        assert_eq!(hook.compact_count, 0);

        let event = LifecycleEvent::Compacted {
            trigger: Some("auto".into()),
            summary: Some("Summary of conversation".into()),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(result, Some(SessionStatus::Paused));
        assert_eq!(hook.compact_count, 1);
    }

    #[test]
    fn observability_preserves_state() {
        let mut hook = make_hook_fields();
        hook.agent_state = make_autonomous_state("acting", "Running Bash");

        let event = LifecycleEvent::Observability {
            event_name: "FileChanged".into(),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(result, None); // no status change
        assert_eq!(hook.agent_state.state, "acting"); // preserved
    }

    #[test]
    fn subagent_started_pushes_and_updates_state() {
        let mut hook = make_hook_fields();
        assert!(hook.sub_agents.is_empty());

        let state = make_autonomous_state("delegating", "Running Explore agent");
        let event = LifecycleEvent::SubagentStarted {
            agent_state: state,
            agent_type: "Explore".into(),
            agent_id: Some("abc123".into()),
            pid: None,
        };
        let result = apply_lifecycle(&mut hook, &event, 1000);
        assert!(result.is_some());
        assert_eq!(hook.sub_agents.len(), 1);
        assert_eq!(hook.sub_agents[0].agent_type, "Explore");
        assert_eq!(hook.agent_state.state, "delegating");
    }

    #[test]
    fn task_created_pushes_progress_item() {
        let mut hook = make_hook_fields();
        assert!(hook.progress_items.is_empty());

        let event = LifecycleEvent::SubEntity(SubEntityEvent::TaskCreated {
            task_id: "task-001".into(),
            subject: Some("Fix the bug".into()),
            description: Some("Fix auth flow".into()),
        });
        apply_lifecycle(&mut hook, &event, 1000);
        assert_eq!(hook.progress_items.len(), 1);
        assert_eq!(hook.progress_items[0].title, "Fix the bug");
    }
}
