use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::live::state::{
    status_from_agent_state, AgentState, AgentStateGroup, LiveSession, SessionEvent,
};
use crate::state::AppState;
use vibe_recall_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    pub hook_event_name: String,
    pub cwd: Option<String>,
    pub transcript_path: Option<String>,
    pub permission_mode: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub tool_use_id: Option<String>,
    pub error: Option<String>,
    pub is_interrupt: Option<bool>,
    pub agent_type: Option<String>,
    pub agent_id: Option<String>,
    pub reason: Option<String>,
    pub task_id: Option<String>,
    pub task_subject: Option<String>,
    pub task_description: Option<String>,
    pub stop_hook_active: Option<bool>,
    pub agent_transcript_path: Option<String>,
    pub teammate_name: Option<String>,
    pub team_name: Option<String>,
    pub source: Option<String>,
    pub prompt: Option<String>,            // UserPromptSubmit
    pub notification_type: Option<String>, // Notification
    pub message: Option<String>,           // Notification
    pub model: Option<String>,             // SessionStart
    pub permission_suggestions: Option<serde_json::Value>, // PermissionRequest
    pub trigger: Option<String>,           // PreCompact: "manual" | "auto"
    pub custom_instructions: Option<String>, // PreCompact
    pub name: Option<String>,              // SubagentStart/Stop name field
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/hook", post(handle_hook))
}

async fn handle_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {
    // Early return for no-op events (auth_success notification)
    if payload.hook_event_name == "Notification"
        && payload.notification_type.as_deref() == Some("auth_success")
    {
        return Json(serde_json::json!({ "ok": true }));
    }

    let agent_state = resolve_state_from_hook(&payload);

    tracing::info!(
        session_id = %payload.session_id,
        event = %payload.hook_event_name,
        state = %agent_state.state,
        group = ?agent_state.group,
        "Hook event received"
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // ── Lazy session creation ────────────────────────────────────────────
    // Sessions that were already running before the server started won't
    // send a SessionStart hook (that event already happened). When any
    // subsequent hook arrives for an unknown session, create a skeleton
    // so the session appears in the live monitor immediately. The JSONL
    // watcher will enrich it with metadata (title, cost, tokens) on the
    // next file event.
    if payload.hook_event_name != "SessionStart" && payload.hook_event_name != "SessionEnd" {
        let needs_creation = !state
            .live_sessions
            .read()
            .await
            .contains_key(&payload.session_id);
        if needs_creation {
            let session = LiveSession {
                id: payload.session_id.clone(),
                project: String::new(),
                project_display_name: extract_project_name(payload.cwd.as_deref()),
                project_path: payload.cwd.clone().unwrap_or_default(),
                file_path: payload.transcript_path.clone().unwrap_or_default(),
                status: status_from_agent_state(&agent_state),
                agent_state: agent_state.clone(),
                git_branch: None,
                pid: None,
                title: String::new(),
                last_user_message: payload
                    .prompt
                    .as_ref()
                    .map(|p| p.chars().take(500).collect())
                    .unwrap_or_default(),
                current_activity: agent_state.label.clone(),
                turn_count: 0,
                started_at: None,
                last_activity_at: now,
                model: payload.model.clone(),
                tokens: TokenUsage::default(),
                context_window_tokens: 0,
                cost: CostBreakdown::default(),
                cache_status: CacheStatus::Unknown,
                current_turn_started_at: None,
                last_turn_task_seconds: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                last_cache_hit_at: None,
            };
            let mut sessions = state.live_sessions.write().await;
            if !sessions.contains_key(&payload.session_id) {
                sessions.insert(session.id.clone(), session.clone());
                drop(sessions);
                if let Some(mgr) = &state.live_manager {
                    mgr.create_accumulator_for_hook(&payload.session_id).await;
                }
                tracing::info!(
                    session_id = %payload.session_id,
                    event = %payload.hook_event_name,
                    "Lazily created session from non-SessionStart hook (was running before server)"
                );
                let _ = state
                    .live_tx
                    .send(SessionEvent::SessionDiscovered { session });
            }
        }
    }

    match payload.hook_event_name.as_str() {
        "SessionStart" => {
            let mut sessions = state.live_sessions.write().await;

            if let Some(existing) = sessions.get_mut(&payload.session_id) {
                // Session already exists (file watcher got there first, OR resume)
                existing.agent_state = agent_state.clone();
                existing.status = status_from_agent_state(&agent_state);
                if let Some(m) = &payload.model {
                    existing.model = Some(m.clone());
                }
                if payload.source.as_deref() == Some("clear") {
                    existing.turn_count = 0;
                    existing.current_turn_started_at = None;
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: existing.clone(),
                });
            } else {
                // Session doesn't exist — create skeleton.
                let session = LiveSession {
                    id: payload.session_id.clone(),
                    project: String::new(),
                    project_display_name: extract_project_name(payload.cwd.as_deref()),
                    project_path: payload.cwd.clone().unwrap_or_default(),
                    file_path: payload.transcript_path.clone().unwrap_or_default(),
                    status: status_from_agent_state(&agent_state),
                    agent_state: agent_state.clone(),
                    git_branch: None,
                    pid: None,
                    title: String::new(),
                    last_user_message: String::new(),
                    current_activity: agent_state.label.clone(),
                    turn_count: 0,
                    started_at: Some(now),
                    last_activity_at: now,
                    model: payload.model.clone(),
                    tokens: TokenUsage::default(),
                    context_window_tokens: 0,
                    cost: CostBreakdown::default(),
                    cache_status: CacheStatus::Unknown,
                    current_turn_started_at: None,
                    last_turn_task_seconds: None,
                    sub_agents: Vec::new(),
                    progress_items: Vec::new(),
                    last_cache_hit_at: None,
                };
                sessions.insert(session.id.clone(), session.clone());
                drop(sessions); // release lock before async manager call
                if let Some(mgr) = &state.live_manager {
                    mgr.create_accumulator_for_hook(&payload.session_id).await;
                }
                let _ = state
                    .live_tx
                    .send(SessionEvent::SessionDiscovered { session });
            }
        }
        "UserPromptSubmit" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                if let Some(prompt) = &payload.prompt {
                    session.last_user_message = prompt.chars().take(500).collect();
                    if session.title.is_empty() {
                        session.title = session.last_user_message.clone();
                    }
                }
                session.current_turn_started_at = Some(now);
                session.turn_count += 1;
                session.agent_state = agent_state.clone();
                session.status = status_from_agent_state(&agent_state);
                session.current_activity = agent_state.label.clone();
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "Stop" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                session.agent_state = agent_state.clone();
                session.status = status_from_agent_state(&agent_state);
                session.current_activity = agent_state.label.clone();
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "SessionEnd" => {
            let session_id = payload.session_id.clone();
            state.live_sessions.write().await.remove(&session_id);
            if let Some(mgr) = &state.live_manager {
                mgr.remove_accumulator(&session_id).await;
            }
            let _ = state
                .live_tx
                .send(SessionEvent::SessionCompleted { session_id });
        }
        // ── Metadata-only events ─────────────────────────────────────────
        // Sub-entity lifecycle: update metadata but NEVER touch agent_state.
        // These events describe sub-agents/teammates/tasks, not the parent.
        "SubagentStop" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                // Mark the sub-agent as complete in the metadata list
                let match_type = payload.agent_type.as_deref().unwrap_or("");
                let match_id = payload.agent_id.as_deref().unwrap_or("");
                for agent in &mut session.sub_agents {
                    if (!match_type.is_empty() && agent.agent_type == match_type)
                        || (!match_id.is_empty() && agent.agent_id.as_deref() == Some(match_id))
                    {
                        agent.status = vibe_recall_core::subagent::SubAgentStatus::Complete;
                    }
                }
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "TeammateIdle" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                // Informational only — teammate status in sub_agents list
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "TaskCompleted" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                if let Some(task_id) = &payload.task_id {
                    for item in &mut session.progress_items {
                        if item.id.as_deref() == Some(task_id.as_str()) {
                            item.status = vibe_recall_core::progress::ProgressStatus::Completed;
                        }
                    }
                }
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        // ── All other state-changing events ──────────────────────────────
        // PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest,
        // Notification, SubagentStart, PreCompact
        _ => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                // Preserve context when re-entering the same state without new context.
                // e.g., Notification/elicitation_dialog fires after PreToolUse/AskUserQuestion
                // and would otherwise overwrite the question context with None.
                let mut new_state = agent_state.clone();
                if new_state.context.is_none()
                    && session.agent_state.state == new_state.state
                    && session.agent_state.context.is_some()
                {
                    new_state.context = session.agent_state.context.clone();
                }
                session.agent_state = new_state;
                session.status = status_from_agent_state(&agent_state);
                session.current_activity = agent_state.label.clone();
                session.last_activity_at = now;
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
    }

    Json(serde_json::json!({ "ok": true }))
}

/// Extract project name from cwd path (last component).
fn extract_project_name(cwd: Option<&str>) -> String {
    cwd.and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown Project")
        .to_string()
}

/// Map a hook event to an `AgentState`.
///
/// This is the SOLE authority for agent state. Every hook maps to exactly one state.
fn resolve_state_from_hook(payload: &HookPayload) -> AgentState {
    match payload.hook_event_name.as_str() {
        "SessionStart" => {
            if payload.source.as_deref() == Some("compact") {
                AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Compacting context...".into(),
                    context: None,
                }
            } else {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Waiting for first prompt".into(),
                    context: None,
                }
            }
        }
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
                    group: AgentStateGroup::NeedsYou,
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
            _ => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
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
        "TeammateIdle" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!(
                "Teammate {} idle",
                payload.teammate_name.as_deref().unwrap_or("unknown")
            ),
            context: None,
        },
        "TaskCompleted" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "task_complete".into(),
            label: payload
                .task_subject
                .clone()
                .unwrap_or_else(|| "Task completed".into()),
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
                state: "thinking".into(),
                label: if trigger == "manual" {
                    "Compacting context...".into()
                } else {
                    "Auto-compacting context...".into()
                },
                context: None,
            }
        }
        "SessionEnd" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session closed".into(),
            context: None,
        },
        _ => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Event: {}", payload.hook_event_name),
            context: None,
        },
    }
}

/// Derive a rich activity label from PreToolUse hook data.
fn activity_from_pre_tool(tool_name: &str, tool_input: &Option<serde_json::Value>) -> String {
    let input = tool_input.as_ref();
    match tool_name {
        "Bash" => input
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .map(|cmd| {
                let truncated: String = cmd.chars().take(60).collect();
                format!("Running: {}", truncated)
            })
            .unwrap_or_else(|| "Running command".into()),
        "Read" => input
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|p| format!("Reading {}", short_path(p)))
            .unwrap_or_else(|| "Reading file".into()),
        "Edit" | "Write" => input
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|p| format!("Editing {}", short_path(p)))
            .unwrap_or_else(|| "Editing file".into()),
        "Grep" => input
            .and_then(|v| v.get("pattern"))
            .and_then(|v| v.as_str())
            .map(|pat| {
                let truncated: String = pat.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching code".into()),
        "Glob" => "Finding files".into(),
        "Task" => input
            .and_then(|v| v.get("description"))
            .and_then(|v| v.as_str())
            .map(|d| {
                let truncated: String = d.chars().take(50).collect();
                format!("Agent: {}", truncated)
            })
            .unwrap_or_else(|| "Dispatching agent".into()),
        "WebFetch" => "Fetching web page".into(),
        "WebSearch" => input
            .and_then(|v| v.get("query"))
            .and_then(|v| v.as_str())
            .map(|q| {
                let truncated: String = q.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching web".into()),
        _ if tool_name.starts_with("mcp__") => {
            let short = tool_name.trim_start_matches("mcp__");
            format!("MCP: {}", short)
        }
        _ => format!("Using {}", tool_name),
    }
}

/// Extract the last path component for display.
fn short_path(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{AgentStateGroup, SessionStatus};

    fn minimal_payload(event: &str) -> HookPayload {
        HookPayload {
            session_id: "test-session".into(),
            hook_event_name: event.into(),
            cwd: None,
            transcript_path: None,
            permission_mode: None,
            tool_name: None,
            tool_input: None,
            tool_response: None,
            tool_use_id: None,
            error: None,
            is_interrupt: None,
            agent_type: None,
            agent_id: None,
            reason: None,
            task_id: None,
            task_subject: None,
            task_description: None,
            stop_hook_active: None,
            agent_transcript_path: None,
            teammate_name: None,
            team_name: None,
            source: None,
            prompt: None,
            notification_type: None,
            message: None,
            model: None,
            permission_suggestions: None,
            trigger: None,
            custom_instructions: None,
            name: None,
        }
    }

    #[test]
    fn test_session_start_startup_returns_idle_state() {
        let mut payload = minimal_payload("SessionStart");
        payload.source = Some("startup".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_session_start_compact_returns_thinking_state() {
        let mut payload = minimal_payload("SessionStart");
        payload.source = Some("compact".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_user_prompt_submit_returns_thinking_state() {
        let payload = minimal_payload("UserPromptSubmit");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_stop_returns_idle_state() {
        let payload = minimal_payload("Stop");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_notification_permission_prompt_returns_needs_permission() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("permission_prompt".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "needs_permission");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_notification_idle_prompt_returns_idle() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("idle_prompt".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "idle");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_notification_elicitation_returns_awaiting_input() {
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("elicitation_dialog".into());
        payload.message = Some("Pick a color".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_input");
        assert!(state.label.contains("Pick a color"));
    }

    #[test]
    fn test_notification_auth_success_is_catchall() {
        // auth_success is handled by early return in handle_hook, but
        // resolve_state_from_hook falls through to the catchall Notification arm
        let mut payload = minimal_payload("Notification");
        payload.notification_type = Some("auth_success".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_input");
        // Was low-confidence catchall — now just maps to awaiting_input
        assert_eq!(state.state, "awaiting_input");
    }

    #[test]
    fn test_post_tool_use_failure_returns_error() {
        let mut payload = minimal_payload("PostToolUseFailure");
        payload.is_interrupt = Some(false);
        payload.tool_name = Some("Bash".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "error");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Bash"));
    }

    #[test]
    fn test_post_tool_use_failure_with_interrupt_returns_interrupted() {
        let mut payload = minimal_payload("PostToolUseFailure");
        payload.is_interrupt = Some(true);
        payload.tool_name = Some("Read".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "interrupted");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Read"));
    }

    #[test]
    fn test_session_end_returns_session_ended() {
        let payload = minimal_payload("SessionEnd");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "session_ended");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    // --- New hook event tests ---

    #[test]
    fn test_pre_tool_use_bash_returns_acting_with_command() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("Bash".into());
        payload.tool_input = Some(serde_json::json!({"command": "git status"}));
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("git status"));
    }

    #[test]
    fn test_pre_tool_use_read_returns_acting_with_filename() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("Read".into());
        payload.tool_input = Some(serde_json::json!({"file_path": "/src/lib.rs"}));
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(state.label.contains("lib.rs"));
    }

    #[test]
    fn test_pre_tool_use_ask_user_question_returns_awaiting_input() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("AskUserQuestion".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_input");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_pre_tool_use_exit_plan_mode_returns_awaiting_approval() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("ExitPlanMode".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "awaiting_approval");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    }

    #[test]
    fn test_post_tool_use_returns_thinking() {
        let payload = minimal_payload("PostToolUse");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_permission_request_returns_needs_permission() {
        let mut payload = minimal_payload("PermissionRequest");
        payload.tool_name = Some("Bash".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "needs_permission");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Bash"));
    }

    #[test]
    fn test_teammate_idle_returns_delegating() {
        let mut payload = minimal_payload("TeammateIdle");
        payload.teammate_name = Some("researcher".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "delegating");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("researcher"));
    }

    #[test]
    fn test_task_completed_returns_task_complete() {
        let mut payload = minimal_payload("TaskCompleted");
        payload.task_subject = Some("Fix login bug".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "task_complete");
        assert!(matches!(state.group, AgentStateGroup::NeedsYou));
        assert!(state.label.contains("Fix login bug"));
    }

    #[test]
    fn test_pre_compact_auto_returns_thinking() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("auto".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Auto-compacting"));
    }

    #[test]
    fn test_pre_compact_manual_returns_thinking() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("manual".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "thinking");
        assert!(state.label.contains("Compacting context"));
        assert!(!state.label.contains("Auto"));
    }

    #[test]
    fn test_subagent_start_returns_delegating() {
        let mut payload = minimal_payload("SubagentStart");
        payload.agent_type = Some("code-explorer".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "delegating");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_subagent_stop_returns_acting() {
        let payload = minimal_payload("SubagentStop");
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_pre_tool_use_mcp_tool() {
        let mut payload = minimal_payload("PreToolUse");
        payload.tool_name = Some("mcp__github__create_issue".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "acting");
        assert!(state.label.starts_with("MCP: "));
    }

    // --- Metadata-only events (no state change) ---
    // SubagentStop, TeammateIdle, TaskCompleted are sub-entity lifecycle
    // events. They update metadata but never touch agent_state. This
    // prevents the race where SubagentStop arrives after Stop and
    // incorrectly flips the session back to Autonomous.

    #[test]
    fn test_subagent_stop_resolves_but_not_applied() {
        // resolve_state_from_hook still maps it (for logging),
        // but handle_hook routes to the metadata-only arm
        let state = resolve_state_from_hook(&minimal_payload("SubagentStop"));
        assert_eq!(state.state, "acting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
    }

    #[test]
    fn test_teammate_idle_resolves_but_not_applied() {
        let mut p = minimal_payload("TeammateIdle");
        p.teammate_name = Some("researcher".into());
        let state = resolve_state_from_hook(&p);
        assert_eq!(state.state, "delegating");
    }

    #[test]
    fn test_task_completed_resolves_but_not_applied() {
        let mut p = minimal_payload("TaskCompleted");
        p.task_subject = Some("Fix bug".into());
        let state = resolve_state_from_hook(&p);
        assert_eq!(state.state, "task_complete");
    }

    #[test]
    fn test_status_from_agent_state_integration() {
        // Verify status derivation works for various hook states
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
