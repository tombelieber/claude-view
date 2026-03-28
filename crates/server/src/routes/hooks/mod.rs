//! Hook handler — receives Claude Code lifecycle events via POST /api/live/hook.
//!
//! Submodules split by concern:
//! - `resolve_state` — maps hook events to AgentState (sole authority)
//! - `activity`      — derives human-readable labels from tool use
//! - `eviction`      — PID-based ghost session eviction
//! - `helpers`       — build_hook_event, group_name, PID extraction

mod activity;
mod eviction;
mod helpers;
mod resolve_state;

use axum::{extract::State, response::Json, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::live::mutation::types::{LifecycleEvent, SessionMutation, SubEntityEvent};
use crate::live::state::AgentState;
use crate::state::AppState;

use eviction::evict_stale_sessions_for_pid;
use helpers::{build_hook_event, extract_pid_from_header, group_name_from_agent_group};
use resolve_state::resolve_state_from_hook;

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
    // ── Fields from Claude Code docs (previously silently dropped) ──
    pub last_assistant_message: Option<String>, // Stop, SubagentStop, StopFailure
    pub compact_summary: Option<String>,        // PostCompact
    pub error_details: Option<String>,          // StopFailure
    pub title: Option<String>,                  // Notification
    pub old_cwd: Option<String>,                // CwdChanged
    pub new_cwd: Option<String>,                // CwdChanged
    pub file_path: Option<String>,              // FileChanged, InstructionsLoaded, ConfigChange
    #[serde(rename = "event")]
    pub file_event: Option<String>, // FileChanged ("change"|"add"|"unlink")
    pub worktree_path: Option<String>,          // WorktreeRemove
    pub memory_type: Option<String>,            // InstructionsLoaded
    pub load_reason: Option<String>,            // InstructionsLoaded
    pub globs: Option<Vec<String>>,             // InstructionsLoaded
    pub trigger_file_path: Option<String>,      // InstructionsLoaded
    pub parent_file_path: Option<String>,       // InstructionsLoaded
    pub mcp_server_name: Option<String>,        // Elicitation/ElicitationResult
    pub mode: Option<String>,                   // Elicitation/ElicitationResult
    pub url: Option<String>,                    // Elicitation
    pub elicitation_id: Option<String>,         // Elicitation/ElicitationResult
    pub requested_schema: Option<serde_json::Value>, // Elicitation
    pub action: Option<String>,                 // ElicitationResult
    pub content: Option<serde_json::Value>,     // ElicitationResult
    /// Safety net: capture any fields Claude Code adds in the future.
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/live/hook", post(handle_hook))
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

    // ── Build hook event context (enriched for new events) ───────────────
    let hook_event_context: Option<serde_json::Value> = match payload.hook_event_name.as_str() {
        "Stop" | "SubagentStop" => payload
            .last_assistant_message
            .as_ref()
            .map(|m| serde_json::json!({"lastAssistantMessage": m.chars().take(200).collect::<String>()}))
            .or_else(|| payload.tool_input.clone()),
        "StopFailure" => Some(serde_json::json!({
            "error": payload.error,
            "details": payload.error_details,
        })),
        "CwdChanged" => Some(serde_json::json!({
            "oldCwd": payload.old_cwd,
            "newCwd": payload.new_cwd,
        })),
        "PostCompact" => payload
            .compact_summary
            .as_ref()
            .map(|s| serde_json::json!({"summary": s.chars().take(500).collect::<String>()})),
        "TaskCreated" => Some(serde_json::json!({
            "taskId": payload.task_id,
            "subject": payload.task_subject,
        })),
        "SubagentStart" => Some(serde_json::json!({
            "agentType": payload.agent_type,
            "agentId": payload.agent_id,
        })),
        _ => payload.tool_input.clone().or_else(|| {
            payload
                .error
                .as_ref()
                .map(|e| serde_json::json!({"error": e}))
        }),
    };

    // ── Construct SessionMutation from hook event name ──────────────────
    let mutation = match payload.hook_event_name.as_str() {
        // ── Session lifecycle ──
        "SessionStart" => SessionMutation::Lifecycle(LifecycleEvent::Start {
            cwd: payload.cwd.clone(),
            model: payload.model.clone(),
            source: payload.source.clone(),
            pid,
            transcript_path: payload.transcript_path.clone(),
        }),
        "SessionEnd" => SessionMutation::Lifecycle(LifecycleEvent::End {
            reason: payload.reason.clone(),
        }),

        // ── User input ──
        "UserPromptSubmit" => SessionMutation::Lifecycle(LifecycleEvent::Prompt {
            text: payload.prompt.clone().unwrap_or_default(),
            pid,
        }),

        // ── Agent turn end ──
        "Stop" => SessionMutation::Lifecycle(LifecycleEvent::Stop {
            agent_state,
            last_assistant_message: payload.last_assistant_message.clone(),
            pid,
        }),
        "StopFailure" => SessionMutation::Lifecycle(LifecycleEvent::StopFailure {
            error: payload.error.clone(),
            error_details: payload.error_details.clone(),
            pid,
        }),

        // ── Tool events ──
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "PermissionRequest" => {
            SessionMutation::Lifecycle(LifecycleEvent::StateChange {
                agent_state,
                event_name: payload.hook_event_name.clone(),
                pid,
            })
        }

        // ── Sub-entities ──
        "SubagentStart" => SessionMutation::Lifecycle(LifecycleEvent::SubagentStarted {
            agent_state,
            agent_type: payload.agent_type.clone().unwrap_or_default(),
            agent_id: payload.agent_id.clone(),
            pid,
        }),
        "SubagentStop" => SessionMutation::Lifecycle(LifecycleEvent::SubEntity(
            SubEntityEvent::SubagentComplete {
                agent_type: payload.agent_type.clone().unwrap_or_default(),
                agent_id: payload.agent_id.clone(),
            },
        )),
        "TaskCreated" => {
            SessionMutation::Lifecycle(LifecycleEvent::SubEntity(SubEntityEvent::TaskCreated {
                task_id: payload.task_id.clone().unwrap_or_default(),
                subject: payload.task_subject.clone(),
                description: payload.task_description.clone(),
            }))
        }
        "TaskCompleted" => {
            SessionMutation::Lifecycle(LifecycleEvent::SubEntity(SubEntityEvent::TaskComplete {
                task_id: payload.task_id.clone().unwrap_or_default(),
            }))
        }
        "TeammateIdle" => {
            SessionMutation::Lifecycle(LifecycleEvent::SubEntity(SubEntityEvent::TeammateIdle))
        }

        // ── Context management ──
        "PreCompact" => SessionMutation::Lifecycle(LifecycleEvent::StateChange {
            agent_state,
            event_name: payload.hook_event_name.clone(),
            pid,
        }),
        "PostCompact" => SessionMutation::Lifecycle(LifecycleEvent::Compacted {
            trigger: payload.trigger.clone(),
            summary: payload.compact_summary.clone(),
            pid,
        }),

        // ── Notifications ──
        "Notification" => {
            // auth_success is already filtered as early return above.
            // Known interactive types → StateChange (NeedsYou via resolve_state_from_hook).
            // Unknown types → Observability (preserve current state).
            match payload.notification_type.as_deref() {
                Some("permission_prompt") | Some("idle_prompt") | Some("elicitation_dialog") => {
                    SessionMutation::Lifecycle(LifecycleEvent::StateChange {
                        agent_state,
                        event_name: payload.hook_event_name.clone(),
                        pid,
                    })
                }
                _ => SessionMutation::Lifecycle(LifecycleEvent::Observability {
                    event_name: payload.hook_event_name.clone(),
                    pid,
                }),
            }
        }

        // ── Environment / observability (do NOT clobber agent_state) ──
        "CwdChanged" => SessionMutation::Lifecycle(LifecycleEvent::CwdChanged {
            old_cwd: payload.old_cwd.clone(),
            new_cwd: payload.new_cwd.clone(),
            pid,
        }),
        "FileChanged" | "InstructionsLoaded" | "ConfigChange" => {
            SessionMutation::Lifecycle(LifecycleEvent::Observability {
                event_name: payload.hook_event_name.clone(),
                pid,
            })
        }

        // ── Worktree ──
        "WorktreeCreate" | "WorktreeRemove" => {
            SessionMutation::Lifecycle(LifecycleEvent::Observability {
                event_name: payload.hook_event_name.clone(),
                pid,
            })
        }

        // ── MCP Elicitation ──
        "Elicitation" => SessionMutation::Lifecycle(LifecycleEvent::StateChange {
            agent_state,
            event_name: payload.hook_event_name.clone(),
            pid,
        }),
        "ElicitationResult" => SessionMutation::Lifecycle(LifecycleEvent::StateChange {
            agent_state,
            event_name: payload.hook_event_name.clone(),
            pid,
        }),

        // ── Unknown = Claude Code added a new event we haven't handled ──
        unknown => {
            tracing::warn!(event = unknown, "Unknown hook event — update handler");
            SessionMutation::Lifecycle(LifecycleEvent::Observability {
                event_name: unknown.to_string(),
                pid,
            })
        }
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

#[cfg(test)]
#[path = "tests.rs"]
pub(super) mod tests;
