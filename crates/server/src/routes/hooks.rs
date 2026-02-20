use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::live::state::{
    status_from_agent_state, AgentState, AgentStateGroup, HookEvent, LiveSession, SessionEvent,
};
use crate::state::AppState;
use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

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

/// Maximum hook events kept in memory per session.
const MAX_HOOK_EVENTS_PER_SESSION: usize = 5000;

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
) -> HookEvent {
    HookEvent {
        timestamp,
        event_name: event_name.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label: label.to_string(),
        group: group.to_string(),
        context: context.map(|v| v.to_string()),
    }
}

async fn handle_hook(
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

    let claude_pid = extract_pid_from_header(
        headers.get("x-claude-pid").and_then(|v| v.to_str().ok()),
    );
    let mut pid_newly_bound = false;
    let mut state_changed = false;

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

    let hook_event_context: Option<serde_json::Value> = payload
        .tool_input
        .clone()
        .or_else(|| payload.error.as_ref().map(|e| serde_json::json!({"error": e})));

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
                pid: claude_pid,
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
                tools_used: Vec::new(),
                last_cache_hit_at: None,
                hook_events: Vec::new(),
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
                state_changed = true;
                if let Some(m) = &payload.model {
                    existing.model = Some(m.clone());
                }
                if payload.source.as_deref() == Some("clear") {
                    existing.turn_count = 0;
                    existing.current_turn_started_at = None;
                }
                if existing.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        existing.pid = Some(pid);
                    }
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
                    pid: claude_pid,
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
                    tools_used: Vec::new(),
                    last_cache_hit_at: None,
                    hook_events: Vec::new(),
                };
                sessions.insert(session.id.clone(), session.clone());
                state_changed = true;
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
                state_changed = true;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
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
                state_changed = true;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "SessionEnd" => {
            let session_id = payload.session_id.clone();

            // Persist hook events to SQLite before removing from memory.
            // Per CLAUDE.md: batch writes in transactions (insert_hook_events does this).
            {
                let sessions = state.live_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    if !session.hook_events.is_empty() {
                        let rows: Vec<claude_view_db::HookEventRow> = session
                            .hook_events
                            .iter()
                            .map(|e| claude_view_db::HookEventRow {
                                timestamp: e.timestamp,
                                event_name: e.event_name.clone(),
                                tool_name: e.tool_name.clone(),
                                label: e.label.clone(),
                                group_name: e.group.clone(),
                                context: e.context.clone(),
                            })
                            .collect();
                        if let Err(e) =
                            claude_view_db::hook_events_queries::insert_hook_events(
                                &state.db, &session_id, &rows,
                            )
                            .await
                        {
                            tracing::warn!(
                                session_id = %session_id,
                                error = %e,
                                "Failed to persist hook events to SQLite"
                            );
                        } else {
                            tracing::info!(
                                session_id = %session_id,
                                count = rows.len(),
                                "Persisted hook events to SQLite"
                            );
                        }
                    }
                }
            }

            state.live_sessions.write().await.remove(&session_id);
            if let Some(mgr) = &state.live_manager {
                mgr.remove_accumulator(&session_id).await;
            }
            // Clean up hook event broadcast channel
            state
                .hook_event_channels
                .write()
                .await
                .remove(&session_id);
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
                        agent.status = claude_view_core::subagent::SubAgentStatus::Complete;
                    }
                }
                session.last_activity_at = now;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
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
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
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
                            item.status = claude_view_core::progress::ProgressStatus::Completed;
                        }
                    }
                }
                session.last_activity_at = now;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "PreCompact" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                session.agent_state = agent_state.clone();
                session.status = status_from_agent_state(&agent_state);
                session.current_activity = agent_state.label.clone();
                session.last_activity_at = now;
                state_changed = true;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "PostToolUse" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                if session.agent_state.state == "compacting" {
                    session.last_activity_at = now;
                    if session.pid.is_none() {
                        if let Some(pid) = claude_pid {
                            session.pid = Some(pid);
                            pid_newly_bound = true;
                        }
                    }
                    let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                } else {
                    session.agent_state = agent_state.clone();
                    session.status = status_from_agent_state(&agent_state);
                    session.current_activity = agent_state.label.clone();
                    session.last_activity_at = now;
                    state_changed = true;
                    if session.pid.is_none() {
                        if let Some(pid) = claude_pid {
                            session.pid = Some(pid);
                            pid_newly_bound = true;
                        }
                    }
                    let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                }
            }
        }
        // ── All other state-changing events ──────────────────────────────
        // PreToolUse, PostToolUseFailure, PermissionRequest,
        // Notification, SubagentStart
        _ => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
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
                state_changed = true;
                if session.pid.is_none() {
                    if let Some(pid) = claude_pid {
                        session.pid = Some(pid);
                        pid_newly_bound = true;
                    }
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
    }

    // ── Append hook event to session (unified, after all match arms) ──
    // SessionEnd removes the session, so skip appending for it.
    // IMPORTANT: Build the hook event HERE (after match arms), using the
    // session's actual agent_state.group. For metadata-only events
    // (TaskCompleted, SubagentStop, TeammateIdle), the resolved state from
    // resolve_state_from_hook is never applied to session.agent_state.
    // Recording the resolved group would create visual false positives
    // in the hook event log (e.g., TaskCompleted showing as "needs_you"
    // when the session is still autonomous).
    if payload.hook_event_name != "SessionEnd" {
        let mut sessions = state.live_sessions.write().await;
        if let Some(session) = sessions.get_mut(&payload.session_id) {
            let actual_group = match &session.agent_state.group {
                AgentStateGroup::NeedsYou => "needs_you",
                AgentStateGroup::Autonomous => "autonomous",
                AgentStateGroup::Delivered => "delivered",
            };

            let hook_event = build_hook_event(
                now,
                &payload.hook_event_name,
                payload.tool_name.as_deref(),
                &agent_state.label,
                actual_group,
                hook_event_context.as_ref(),
            );

            if session.hook_events.len() >= MAX_HOOK_EVENTS_PER_SESSION {
                session.hook_events.drain(..100); // drop oldest 100
            }
            session.hook_events.push(hook_event.clone());
            drop(sessions);

            // Broadcast to any connected WS listeners
            let channels = state.hook_event_channels.read().await;
            if let Some(tx) = channels.get(&payload.session_id) {
                let _ = tx.send(hook_event);
            }
        }
    }

    // Persist session snapshot when PID binding or agent state changed
    if pid_newly_bound || state_changed {
        if let Some(mgr) = &state.live_manager {
            mgr.save_session_snapshot_from_state().await;
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
    use crate::live::state::{AgentStateGroup, SessionStatus};
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
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            status: crate::live::state::SessionStatus::Working,
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            git_branch: None,
            pid: None,
            title: "Test session".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 5,
            started_at: Some(1000),
            last_activity_at: 1000,
            model: None,
            tokens: claude_view_core::pricing::TokenUsage::default(),
            context_window_tokens: 0,
            cost: claude_view_core::pricing::CostBreakdown::default(),
            cache_status: claude_view_core::pricing::CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            tools_used: Vec::new(),
            last_cache_hit_at: None,
            hook_events: Vec::new(),
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
        );
        assert_eq!(event.event_name, "PreToolUse");
        assert_eq!(event.tool_name, Some("Read".to_string()));
        assert_eq!(event.label, "Reading file.rs");
        assert_eq!(event.group, "autonomous");
        assert_eq!(event.timestamp, 1708000000);
        assert!(event.context.is_none());
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
        );
        assert_eq!(event.context, Some(ctx.to_string()));
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
                    .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // Verify: hook event should record "autonomous" (actual session group),
        // NOT "needs_you" (what resolve_state_from_hook returns for TaskCompleted)
        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook_events.len(), 1);
        let event = &session.hook_events[0];
        assert_eq!(event.event_name, "TaskCompleted");
        assert_eq!(event.label, "Fix login bug"); // label still from resolved state
        assert_eq!(
            event.group, "autonomous",
            "TaskCompleted hook event should record the session's actual group (autonomous), \
             not the resolved group (needs_you)"
        );
        // Also verify session.agent_state was NOT changed
        assert!(matches!(
            session.agent_state.group,
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
                    .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook_events.len(), 1);
        assert_eq!(
            session.hook_events[0].group, "autonomous",
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
                    .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook_events.len(), 1);
        assert_eq!(
            session.hook_events[0].group, "autonomous",
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
                    .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.hook_events.len(), 1);
        assert_eq!(
            session.hook_events[0].group, "needs_you",
            "AskUserQuestion hook event should record needs_you (state was applied)"
        );
        // Verify session.agent_state was updated too
        assert!(matches!(
            session.agent_state.group,
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
            session.agent_state = AgentState {
                group: AgentStateGroup::Autonomous,
                state: "compacting".into(),
                label: "Auto-compacting context...".into(),
                context: None,
            };
            session.status = SessionStatus::Working;
            session.current_activity = "Auto-compacting context...".into();
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
                    .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        // Verify: agent_state should still be "compacting", NOT "thinking"
        let sessions = state.live_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        assert_eq!(
            session.agent_state.state, "compacting",
            "PostToolUse must not overwrite compacting state"
        );
        assert_eq!(
            session.current_activity, "Auto-compacting context...",
            "PostToolUse must not overwrite current_activity during compaction"
        );
        assert_eq!(session.status, SessionStatus::Working);
    }
}
