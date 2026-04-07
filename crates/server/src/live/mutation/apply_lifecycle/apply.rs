//! Core lifecycle event application logic.

use crate::live::mutation::types::{LifecycleEvent, SubEntityEvent};
use crate::live::state::{AgentState, AgentStateGroup, HookFields, SessionStatus};

use super::helpers::{bind_pid, status_from_agent_state};

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
            match source.as_deref() {
                Some("clear") => {
                    hook.turn_count = 0;
                    hook.current_turn_started_at = None;
                    hook.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Session cleared".into(),
                        context: None,
                    };
                }
                Some("compact") => {
                    hook.agent_state = AgentState {
                        group: AgentStateGroup::Autonomous,
                        state: "thinking".into(),
                        label: "Compacting context...".into(),
                        context: None,
                    };
                }
                Some("resume") => {
                    hook.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Session resumed".into(),
                        context: None,
                    };
                }
                _ => {
                    hook.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for first prompt".into(),
                        context: None,
                    };
                }
            }
            hook.current_activity = hook.agent_state.label.clone();
            Some(status_from_agent_state(&hook.agent_state))
        }

        LifecycleEvent::Prompt { text, pid } => {
            hook.last_user_message = text.chars().take(500).collect();
            if hook.title.is_empty() {
                hook.title = hook.last_user_message.clone();
            }
            hook.turn_count += 1;
            hook.current_turn_started_at = Some(now);
            // User submitted prompt → agent is now thinking (Autonomous)
            hook.agent_state = AgentState {
                group: AgentStateGroup::Autonomous,
                state: "thinking".into(),
                label: "Processing prompt...".into(),
                context: None,
            };
            hook.current_activity = "Processing prompt...".into();
            bind_pid(hook, *pid);
            Some(status_from_agent_state(&hook.agent_state))
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
