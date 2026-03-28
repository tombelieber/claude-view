use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::live::mutation::types::{LifecycleEvent, SessionMutation, SubEntityEvent};
use crate::live::state::{AgentState, AgentStateGroup, HookEvent, SessionStatus};
use crate::state::AppState;

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
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

/// Build a HookEvent from hook handler data.
fn build_hook_event(
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

fn group_name_from_agent_group(group: &AgentStateGroup) -> &'static str {
    match group {
        AgentStateGroup::NeedsYou => "needs_you",
        AgentStateGroup::Autonomous => "autonomous",
    }
}

#[utoipa::path(post, path = "/api/live/hook", tag = "live",
    request_body = HookPayload,
    responses(
        (status = 200, description = "Hook event accepted and processed", body = serde_json::Value),
    )
)]
pub async fn handle_hook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {
    // Early return for no-op events (auth_success notification)
    if payload.hook_event_name == "Notification"
        && payload.notification_type.as_deref() == Some("auth_success")
    {
        return Json(serde_json::json!({ "ok": true }));
    }

    let pid = extract_pid_from_header(headers.get("x-claude-pid").and_then(|v| v.to_str().ok()));

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

    // ── Debug log: full raw payload before mutation consumes it ──
    #[cfg(debug_assertions)]
    let debug_line = serde_json::to_string(&payload).unwrap_or_default();

    // ── Build hook event context (for event log) ────────────────────────
    let hook_event_context: Option<serde_json::Value> = payload.tool_input.clone().or_else(|| {
        payload
            .error
            .as_ref()
            .map(|e| serde_json::json!({"error": e}))
    });

    // ── Construct SessionMutation from hook event name ──────────────────
    let mutation = match payload.hook_event_name.as_str() {
        "SessionStart" => SessionMutation::Lifecycle(LifecycleEvent::Start {
            cwd: payload.cwd.clone(),
            model: payload.model.clone(),
            source: payload.source.clone(),
            pid,
            transcript_path: payload.transcript_path.clone(),
        }),
        "UserPromptSubmit" => SessionMutation::Lifecycle(LifecycleEvent::Prompt {
            text: payload.prompt.clone().unwrap_or_default(),
            pid,
        }),
        "Stop" | "PreCompact" | "PostToolUse" => {
            SessionMutation::Lifecycle(LifecycleEvent::StateChange {
                agent_state,
                event_name: payload.hook_event_name.clone(),
                pid,
            })
        }
        "SessionEnd" => SessionMutation::Lifecycle(LifecycleEvent::End {
            reason: payload.reason.clone(),
        }),
        "SubagentStop" => SessionMutation::Lifecycle(LifecycleEvent::SubEntity(
            SubEntityEvent::SubagentComplete {
                agent_type: payload.agent_type.clone().unwrap_or_default(),
                agent_id: payload.agent_id.clone(),
            },
        )),
        "TaskCompleted" => {
            SessionMutation::Lifecycle(LifecycleEvent::SubEntity(SubEntityEvent::TaskComplete {
                task_id: payload.task_id.clone().unwrap_or_default(),
            }))
        }
        "TeammateIdle" => {
            SessionMutation::Lifecycle(LifecycleEvent::SubEntity(SubEntityEvent::TeammateIdle))
        }
        // All other state-changing events: PreToolUse, PostToolUseFailure,
        // PermissionRequest, Notification, SubagentStart
        _ => SessionMutation::Lifecycle(LifecycleEvent::StateChange {
            agent_state,
            event_name: payload.hook_event_name.clone(),
            pid,
        }),
    };

    // ── Build hook event BEFORE coordinator call ────────────────────────
    // For SessionEnd the coordinator clears hook_events; the hook event
    // is not appended (matches previous behavior).
    let hook_event = if payload.hook_event_name != "SessionEnd" {
        // We use the resolved agent_state's label for the event label,
        // but the GROUP is determined by the coordinator after mutation
        // (the coordinator reads session.hook.agent_state.group post-mutation).
        // For sub-entity events (SubagentStop, TeammateIdle, TaskCompleted),
        // the mutation does NOT change agent_state, so the session's existing
        // group is preserved — matching the old behavior that used the
        // session's actual group, not the resolved state's group.
        let resolved_state = resolve_state_from_hook(&payload);
        Some(build_hook_event(
            now,
            &payload.hook_event_name,
            payload.tool_name.as_deref(),
            &resolved_state.label,
            // Placeholder — coordinator will use session's actual group.
            // For new sessions (no prior group), use resolved state's group.
            group_name_from_agent_group(&resolved_state.group),
            hook_event_context.as_ref(),
            "hook",
        ))
    } else {
        None
    };

    // ── PID uniqueness eviction (SessionStart only) ─────────────────────
    // If this PID already belongs to a different session, close the old one.
    // Must happen before coordinator.handle() so the new session doesn't
    // race with the stale one.
    if matches!(
        mutation,
        SessionMutation::Lifecycle(LifecycleEvent::Start { .. })
    ) {
        if let Some(start_pid) = pid {
            evict_stale_sessions_for_pid(&state, start_pid, &payload.session_id, now).await;
        }
    }

    // ── Delegate to coordinator ─────────────────────────────────────────
    let ctx = state.mutation_context();
    state
        .coordinator
        .handle(&ctx, &payload.session_id, mutation, pid, now, hook_event)
        .await;

    // ── Append to debug log (fire-and-forget, non-blocking) ─────────────
    #[cfg(debug_assertions)]
    if let Some(ref log) = state.debug_hooks_log {
        log.append(debug_line);
    }

    Json(serde_json::json!({ "ok": true }))
}

/// Evict stale sessions that share the same PID as the new session.
///
/// PID uniqueness: one PID = one active session. When a new session starts
/// on the same PID (e.g. session resume), the old entry is immediately
/// evicted — no 10s reconciliation delay.
///
/// Ghost sessions (no JSONL, zero turns) are removed entirely.
/// Real sessions move to "recently closed".
/// Sidecar sessions are never evicted.
async fn evict_stale_sessions_for_pid(state: &AppState, pid: u32, new_session_id: &str, now: i64) {
    use crate::live::state::SessionEvent;

    let mut sessions = state.live_sessions.write().await;

    // Collect eviction targets (session_id, transcript_path, is_ghost)
    let mut pid_evicted: Vec<(String, Option<std::path::PathBuf>, bool)> = Vec::new();
    for (id, session) in sessions.iter_mut() {
        if *id == new_session_id {
            continue;
        }
        if session.hook.pid != Some(pid) {
            continue;
        }
        if session.status == SessionStatus::Done {
            continue;
        }
        // Sidecar sessions: lifecycle managed by SDK, never evict
        if session.control.is_some() {
            continue;
        }
        // Same PID, different session_id -> stale. Close it.
        session.status = SessionStatus::Done;
        session.closed_at = Some(now);
        session.hook.agent_state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session ended".into(),
            context: None,
        };
        session.hook.hook_events.clear();
        let is_ghost = session.jsonl.file_path.is_empty() && session.hook.turn_count == 0;
        let tp = session
            .statusline
            .statusline_transcript_path
            .get()
            .map(std::path::PathBuf::from);
        pid_evicted.push((id.clone(), tp, is_ghost));
        tracing::info!(
            evicted_id = %id,
            new_id = %new_session_id,
            pid = pid,
            "PID uniqueness: closed stale session (same PID, new session_id)"
        );
    }

    // Ghost sessions removed entirely; real sessions stay as "recently closed"
    let mut evicted_real: Vec<crate::live::state::LiveSession> = Vec::new();
    let mut evicted_ghost_ids: Vec<String> = Vec::new();
    for (id, _, is_ghost) in &pid_evicted {
        if *is_ghost {
            sessions.remove(id);
            evicted_ghost_ids.push(id.clone());
        } else if let Some(s) = sessions.get(id) {
            evicted_real.push(s.clone());
        }
    }
    let evicted_transcript_paths: Vec<std::path::PathBuf> = pid_evicted
        .into_iter()
        .filter_map(|(_, tp, _)| tp)
        .collect();

    // Drop sessions lock before any other async work
    drop(sessions);

    // Clean transcript map for evicted sessions
    if !evicted_transcript_paths.is_empty() {
        let mut tmap = state.transcript_to_session.write().await;
        for tp in &evicted_transcript_paths {
            tmap.remove(tp);
        }
    }

    // Clean accumulators for evicted sessions
    if let Some(mgr) = &state.live_manager {
        for s in &evicted_real {
            mgr.remove_accumulator(&s.id).await;
        }
        for id in &evicted_ghost_ids {
            mgr.remove_accumulator(id).await;
        }
    }

    // Broadcast evictions
    let total_evicted = evicted_real.len() + evicted_ghost_ids.len();
    if total_evicted > 1 {
        tracing::warn!(
            count = total_evicted,
            pid = pid,
            "Multiple sessions evicted for same PID — unexpected (possible rapid PID reuse)"
        );
    }
    for evicted in &evicted_real {
        let _ = state.live_tx.send(SessionEvent::SessionClosed {
            session: evicted.clone(),
        });
    }
    for id in &evicted_ghost_ids {
        let _ = state.live_tx.send(SessionEvent::SessionCompleted {
            session_id: id.clone(),
        });
    }
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
                // Tool failures are transient — agent usually retries immediately.
                // Keep as autonomous to avoid false-positive notification dings.
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
            group: AgentStateGroup::Autonomous,
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
                state: "compacting".into(),
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
            label: "Session ended".into(),
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
        // Claude Code renamed "Task" to "Agent" ~v0.10. Handle both.
        // Agent uses input.name as display name, Task uses input.description.
        "Task" | "Agent" => input
            .and_then(|v| v.get("name").or_else(|| v.get("description")))
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

/// Extract and validate a PID from the X-Claude-PID header value.
///
/// Returns None if the header is missing, empty, non-numeric, or <= 1
/// (PID 0 = kernel, PID 1 = init/launchd — indicates reparenting).
fn extract_pid_from_header(header_value: Option<&str>) -> Option<u32> {
    let value = header_value?.trim();
    let pid: u32 = value.parse().ok()?;
    if pid <= 1 {
        return None;
    }
    Some(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{status_from_agent_state, AgentStateGroup, HookFields, SessionStatus};
    use tower::ServiceExt;

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

    /// Create a LiveSession in autonomous/acting state for integration tests.
    fn make_autonomous_session(id: &str) -> crate::live::state::LiveSession {
        crate::live::state::LiveSession {
            id: id.to_string(),
            status: crate::live::state::SessionStatus::Working,
            started_at: Some(1000),
            closed_at: None,
            control: None,
            model: None,
            model_display_name: None,
            model_set_at: 0,
            context_window_tokens: 0,
            statusline: crate::live::state::StatuslineFields::default(),
            hook: HookFields {
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: "Working".into(),
                    context: None,
                },
                pid: None,
                title: "Test session".into(),
                last_user_message: String::new(),
                current_activity: "Working".into(),
                turn_count: 5,
                last_activity_at: 1000,
                current_turn_started_at: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                compact_count: 0,
                agent_state_set_at: 0,
                last_assistant_preview: None,
                last_error: None,
                last_error_details: None,
                hook_events: Vec::new(),
            },
            jsonl: crate::live::state::JsonlFields {
                project: String::new(),
                project_display_name: "test".to_string(),
                project_path: "/tmp/test".to_string(),
                file_path: "/tmp/test.jsonl".to_string(),
                ..crate::live::state::JsonlFields::default()
            },
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
        // Transient failures stay autonomous — agent usually retries
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
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
        // Subagent task completion is autonomous — main agent keeps working
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Fix login bug"));
    }

    #[test]
    fn test_pre_compact_auto_returns_compacting() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("auto".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "compacting");
        assert!(matches!(state.group, AgentStateGroup::Autonomous));
        assert!(state.label.contains("Auto-compacting"));
    }

    #[test]
    fn test_pre_compact_manual_returns_compacting() {
        let mut payload = minimal_payload("PreCompact");
        payload.trigger = Some("manual".into());
        let state = resolve_state_from_hook(&payload);
        assert_eq!(state.state, "compacting");
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
    fn test_build_hook_event() {
        let event = super::build_hook_event(
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
    fn test_build_hook_event_with_context() {
        let ctx = serde_json::json!({"command": "git status"});
        let event = super::build_hook_event(
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
    fn test_extract_pid_from_header_valid() {
        let pid = extract_pid_from_header(Some("12345"));
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn test_extract_pid_from_header_invalid() {
        assert_eq!(extract_pid_from_header(None), None);
        assert_eq!(extract_pid_from_header(Some("")), None);
        assert_eq!(extract_pid_from_header(Some("abc")), None);
        assert_eq!(extract_pid_from_header(Some("0")), None);
        assert_eq!(extract_pid_from_header(Some("1")), None);
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

    #[tokio::test]
    async fn test_session_start_missing_cwd_buffers_without_creating_session() {
        let session_id = "missing-cwd-start";

        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "SessionStart"
        });

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // SessionStart without cwd: coordinator buffers mutation (no session created)
        let sessions = state.live_sessions.read().await;
        assert!(
            sessions.get(session_id).is_none(),
            "SessionStart without cwd must not create a live session"
        );
    }

    #[tokio::test]
    async fn test_buffered_events_promote_on_session_start_with_cwd() {
        let session_id = "promote-on-valid-cwd";

        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // 1) State-changing hook without cwd: buffered by coordinator
        let body_buffered = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "git status"}
        });
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body_buffered).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(state.live_sessions.read().await.get(session_id).is_none());

        // 2) SessionStart with cwd creates session and drains buffered mutations
        let body_start = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "SessionStart",
            "cwd": "/tmp/promoted-project"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body_start).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get(session_id).expect("session should be created");
        assert_eq!(session.jsonl.project_path, "/tmp/promoted-project");
        // Buffered PreToolUse hook event + SessionStart hook event
        assert_eq!(session.hook.hook_events.len(), 2);
        assert_eq!(session.hook.hook_events[0].event_name, "PreToolUse");
        assert_eq!(session.hook.hook_events[1].event_name, "SessionStart");
    }

    #[tokio::test]
    async fn test_session_end_for_unknown_session_returns_ok() {
        // SessionEnd for a never-created session: coordinator cannot create
        // a session from End, so it's buffered (and eventually expires).
        // The handler still returns 200 OK.
        let session_id = "unknown-session-end";

        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        let body_end = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "SessionEnd"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body_end).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // No session was ever created
        assert!(state.live_sessions.read().await.get(session_id).is_none());
    }

    #[tokio::test]
    async fn test_existing_session_receives_hook_events() {
        let session_id = "existing-session-hooks";

        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // 1) Pre-populate session (simulating startup snapshot promotion)
        {
            let mut sessions = state.live_sessions.write().await;
            sessions.insert(session_id.to_string(), make_autonomous_session(session_id));
        }

        // 2) Send a hook event — should be appended to session's hook_events
        let body_update = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "UserPromptSubmit",
            "cwd": "/tmp/promoted-project",
            "prompt": "Continue"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body_update).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get(session_id).expect("session should exist");
        assert_eq!(session.hook.hook_events.len(), 1);
        assert_eq!(session.hook.hook_events[0].event_name, "UserPromptSubmit");
    }

    #[tokio::test]
    async fn test_task_completed_hook_event_records_actual_session_group() {
        // Setup: create AppState with a session in autonomous state
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Pre-populate a session in autonomous state (simulating a working session)
        {
            let mut sessions = state.live_sessions.write().await;
            sessions.insert(
                "test-session".to_string(),
                make_autonomous_session("test-session"),
            );
        }

        // Send a TaskCompleted hook
        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "TaskCompleted",
            "task_id": "task-1",
            "task_subject": "Fix login bug"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // Verify: hook event should record "autonomous" (actual session group)
        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook.hook_events.len(), 1);
        let event = &session.hook.hook_events[0];
        assert_eq!(event.event_name, "TaskCompleted");
        assert_eq!(event.label, "Fix login bug"); // label still from resolved state
        assert_eq!(
            event.group, "autonomous",
            "TaskCompleted hook event should record the session's actual group (autonomous)"
        );
        // Also verify session.hook.agent_state was NOT changed
        assert!(matches!(
            session.hook.agent_state.group,
            AgentStateGroup::Autonomous
        ));
    }

    #[tokio::test]
    async fn test_subagent_stop_hook_event_records_actual_session_group() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Pre-populate session in autonomous/delegating state
        {
            let mut sessions = state.live_sessions.write().await;
            sessions.insert(
                "test-session".to_string(),
                make_autonomous_session("test-session"),
            );
        }

        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "SubagentStop",
            "agent_type": "code-explorer",
            "agent_id": "agent-1"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook.hook_events.len(), 1);
        assert_eq!(
            session.hook.hook_events[0].group, "autonomous",
            "SubagentStop hook event should record session's actual group"
        );
    }

    #[tokio::test]
    async fn test_teammate_idle_hook_event_records_actual_session_group() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        {
            let mut sessions = state.live_sessions.write().await;
            sessions.insert(
                "test-session".to_string(),
                make_autonomous_session("test-session"),
            );
        }

        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "TeammateIdle",
            "teammate_name": "researcher"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook.hook_events.len(), 1);
        assert_eq!(
            session.hook.hook_events[0].group, "autonomous",
            "TeammateIdle hook event should record session's actual group"
        );
    }

    #[tokio::test]
    async fn test_state_changing_event_hook_event_records_new_group() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Session starts autonomous
        {
            let mut sessions = state.live_sessions.write().await;
            sessions.insert(
                "test-session".to_string(),
                make_autonomous_session("test-session"),
            );
        }

        // Send PreToolUse/AskUserQuestion — this SHOULD transition to needs_you
        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "PreToolUse",
            "tool_name": "AskUserQuestion",
            "tool_input": {"question": "Which approach?"}
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook.hook_events.len(), 1);
        assert_eq!(
            session.hook.hook_events[0].group, "needs_you",
            "AskUserQuestion hook event should record needs_you (state was applied)"
        );
        // Verify session.hook.agent_state was updated too
        assert!(matches!(
            session.hook.agent_state.group,
            AgentStateGroup::NeedsYou
        ));
    }

    #[tokio::test]
    async fn test_post_tool_use_does_not_override_compacting() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        // Pre-populate a session in compacting state (as if PreCompact just fired)
        {
            let mut session = make_autonomous_session("test-session");
            session.hook.agent_state = AgentState {
                group: AgentStateGroup::Autonomous,
                state: "compacting".into(),
                label: "Auto-compacting context...".into(),
                context: None,
            };
            session.status = SessionStatus::Working;
            session.hook.current_activity = "Auto-compacting context...".into();
            let mut sessions = state.live_sessions.write().await;
            sessions.insert("test-session".to_string(), session);
        }

        // Send PostToolUse (racing from the previous tool)
        let app = crate::api_routes(state.clone());
        let body = serde_json::json!({
            "session_id": "test-session",
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash"
        });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/live/hook")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // Verify: agent_state should still be "compacting", NOT "thinking"
        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(
            session.hook.agent_state.state, "compacting",
            "PostToolUse must not overwrite compacting state"
        );
        assert_eq!(
            session.hook.current_activity, "Auto-compacting context...",
            "PostToolUse must not overwrite current_activity during compaction"
        );
        assert_eq!(session.status, SessionStatus::Working);
    }

    // =========================================================================
    // PID uniqueness + ghost session tests
    // =========================================================================

    /// Helper: send a SessionStart hook with a PID header.
    async fn send_session_start(
        app: &axum::Router,
        session_id: &str,
        cwd: &str,
        pid: Option<u32>,
    ) -> axum::http::StatusCode {
        let body = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "SessionStart",
            "cwd": cwd,
        });
        let mut req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/live/hook")
            .header("content-type", "application/json");
        if let Some(pid) = pid {
            req = req.header("x-claude-pid", pid.to_string());
        }
        let response = app
            .clone()
            .oneshot(
                req.body(axum::body::Body::from(
                    serde_json::to_string(&body).unwrap(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        response.status()
    }

    /// Helper: send a SessionEnd hook.
    async fn send_session_end(
        app: &axum::Router,
        session_id: &str,
        pid: Option<u32>,
    ) -> axum::http::StatusCode {
        let body = serde_json::json!({
            "session_id": session_id,
            "hook_event_name": "SessionEnd",
        });
        let mut req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/live/hook")
            .header("content-type", "application/json");
        if let Some(pid) = pid {
            req = req.header("x-claude-pid", pid.to_string());
        }
        let response = app
            .clone()
            .oneshot(
                req.body(axum::body::Body::from(
                    serde_json::to_string(&body).unwrap(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        response.status()
    }

    #[tokio::test]
    async fn test_pid_uniqueness_evicts_ghost_session_on_same_pid() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Session A starts with PID 99999 (hook skeleton: no JSONL, 0 turns → ghost)
        let status = send_session_start(&app, "session-a", "/tmp/proj", Some(99999)).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        {
            let sessions = state.live_sessions.read().await;
            let a = sessions.get("session-a").expect("session-a must exist");
            assert_eq!(a.hook.pid, Some(99999));
        }

        // Session B starts with SAME PID 99999 → ghost session A must be REMOVED entirely
        let status = send_session_start(&app, "session-b", "/tmp/proj2", Some(99999)).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        {
            let sessions = state.live_sessions.read().await;
            let b = sessions.get("session-b").expect("session-b must exist");
            assert_eq!(b.hook.pid, Some(99999));
            assert_ne!(b.status, SessionStatus::Done);
            // Ghost session A is fully removed (not recently closed)
            assert!(
                sessions.get("session-a").is_none(),
                "ghost session-a (no JSONL, 0 turns) must be removed from map entirely"
            );
        }
    }

    #[tokio::test]
    async fn test_pid_uniqueness_evicts_real_session_to_recently_closed() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Insert a REAL session (has file_path + turns) with PID 99998
        {
            let mut sessions = state.live_sessions.write().await;
            let mut real = make_autonomous_session("real-old");
            real.hook.pid = Some(99998);
            real.jsonl.file_path = "/tmp/real.jsonl".into();
            real.hook.turn_count = 5;
            sessions.insert("real-old".into(), real);
        }

        // New session with same PID → real session must move to recently closed (not removed)
        send_session_start(&app, "real-new", "/tmp/proj", Some(99998)).await;

        let sessions = state.live_sessions.read().await;
        let old = sessions
            .get("real-old")
            .expect("real session must stay in map as recently closed");
        assert_eq!(old.status, SessionStatus::Done);
        assert!(old.closed_at.is_some());
        assert!(sessions.get("real-new").is_some());
    }

    #[tokio::test]
    async fn test_pid_uniqueness_does_not_evict_different_pid() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Two sessions with different PIDs → both stay active
        send_session_start(&app, "session-x", "/tmp/proj-x", Some(10001)).await;
        send_session_start(&app, "session-y", "/tmp/proj-y", Some(10002)).await;

        let sessions = state.live_sessions.read().await;
        assert!(
            sessions.get("session-x").is_some(),
            "session-x must survive"
        );
        assert!(
            sessions.get("session-y").is_some(),
            "session-y must survive"
        );
        assert_ne!(sessions["session-x"].status, SessionStatus::Done);
        assert_ne!(sessions["session-y"].status, SessionStatus::Done);
    }

    #[tokio::test]
    async fn test_pid_uniqueness_skips_done_sessions() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Session A starts and ends
        send_session_start(&app, "done-a", "/tmp/done", Some(20001)).await;
        send_session_end(&app, "done-a", Some(20001)).await;

        // Verify A is Done
        {
            let sessions = state.live_sessions.read().await;
            let a = sessions.get("done-a").expect("done-a must exist");
            assert_eq!(a.status, SessionStatus::Done);
        }

        // Session B starts with same PID → should NOT try to re-evict A (already Done)
        send_session_start(&app, "done-b", "/tmp/done2", Some(20001)).await;

        let sessions = state.live_sessions.read().await;
        assert!(sessions.get("done-b").is_some(), "done-b must be created");
        // A should still be in map, unchanged (already Done, not double-evicted)
        let a = sessions.get("done-a").expect("done-a must still exist");
        assert_eq!(a.status, SessionStatus::Done);
    }

    #[tokio::test]
    async fn test_pid_uniqueness_skips_sidecar_sessions() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Insert a sidecar-controlled session with PID 30001
        {
            let mut sessions = state.live_sessions.write().await;
            let mut session = make_autonomous_session("sidecar-session");
            session.hook.pid = Some(30001);
            session.control = Some(crate::live::state::ControlBinding {
                control_id: "ctrl-123".into(),
                bound_at: 1000,
                cancel: tokio_util::sync::CancellationToken::new(),
            });
            sessions.insert("sidecar-session".into(), session);
        }

        // New session with same PID → sidecar must NOT be evicted
        send_session_start(&app, "new-session", "/tmp/proj", Some(30001)).await;

        let sessions = state.live_sessions.read().await;
        let sidecar = sessions
            .get("sidecar-session")
            .expect("sidecar must survive");
        assert_ne!(
            sidecar.status,
            SessionStatus::Done,
            "sidecar must not be evicted"
        );
        assert!(
            sidecar.control.is_some(),
            "sidecar control binding must remain"
        );
        assert!(
            sessions.get("new-session").is_some(),
            "new session must also be created"
        );
    }

    #[tokio::test]
    async fn test_ghost_session_evicted_by_pid_is_removed_not_recently_closed() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Insert a ghost session: has PID but no file_path and zero turns
        {
            let mut sessions = state.live_sessions.write().await;
            let mut ghost = make_autonomous_session("ghost-session");
            ghost.hook.pid = Some(40001);
            ghost.jsonl.file_path = String::new(); // no JSONL
            ghost.hook.turn_count = 0; // zero turns
            sessions.insert("ghost-session".into(), ghost);
        }

        // New session with same PID → ghost must be REMOVED (not recently closed)
        send_session_start(&app, "real-session", "/tmp/proj", Some(40001)).await;

        let sessions = state.live_sessions.read().await;
        assert!(
            sessions.get("ghost-session").is_none(),
            "ghost session must be fully removed from map, not kept as recently closed"
        );
        assert!(
            sessions.get("real-session").is_some(),
            "real session must be created"
        );
    }

    #[tokio::test]
    async fn test_no_pid_means_no_eviction() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Session A with PID
        send_session_start(&app, "with-pid", "/tmp/proj1", Some(50001)).await;

        // Session B without PID → must not evict A
        send_session_start(&app, "no-pid", "/tmp/proj2", None).await;

        let sessions = state.live_sessions.read().await;
        assert!(sessions.get("with-pid").is_some());
        assert!(sessions.get("no-pid").is_some());
        assert_ne!(sessions["with-pid"].status, SessionStatus::Done);
    }

    #[tokio::test]
    async fn test_same_session_id_same_pid_is_update_not_eviction() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);
        let app = crate::api_routes(state.clone());

        // Start session, then start again with same ID + PID (resume)
        send_session_start(&app, "resume-me", "/tmp/proj", Some(60001)).await;
        send_session_start(&app, "resume-me", "/tmp/proj", Some(60001)).await;

        let sessions = state.live_sessions.read().await;
        // Should still be 1 session, not evicted
        let s = sessions.get("resume-me").expect("session must exist");
        assert_ne!(
            s.status,
            SessionStatus::Done,
            "resume must not mark session as Done"
        );
        assert_eq!(s.hook.pid, Some(60001));
    }
}
