//! Session creation helpers — construct `LiveSession` from various sources.

use crate::live::mutation::types::ReconcileData;
use crate::live::state::{HookFields, JsonlFields, LiveSession, SessionStatus, StatuslineFields};

/// Create a new `LiveSession` from a `Start` lifecycle event.
pub fn create_session_from_start(
    session_id: &str,
    cwd: &Option<String>,
    model: &Option<String>,
    pid: &Option<u32>,
    now: i64,
) -> LiveSession {
    let hook = HookFields {
        last_activity_at: now,
        pid: *pid,
        ..HookFields::default()
    };

    LiveSession {
        id: session_id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(now),
        closed_at: None,
        control: None,
        model: model.clone(),
        model_display_name: None,
        model_set_at: now,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook,
        jsonl: JsonlFields {
            project_path: cwd.clone().unwrap_or_default(),
            ..JsonlFields::default()
        },
        session_kind: None,
        entrypoint: None,
    }
}

/// Create a minimal `LiveSession` shell for watcher discovery (Reconcile).
/// JSONL data fills in project/tokens/cost; hooks will follow.
pub fn create_session_shell(session_id: &str, data: &ReconcileData, now: i64) -> LiveSession {
    let hook = HookFields {
        last_activity_at: now,
        ..HookFields::default()
    };

    let mut jsonl = JsonlFields::default();
    if let Some(ref p) = data.project {
        jsonl.project = p.clone();
    }
    if let Some(ref p) = data.project_display_name {
        jsonl.project_display_name = p.clone();
    }
    if let Some(ref p) = data.project_path {
        jsonl.project_path = p.clone();
    }

    LiveSession {
        id: session_id.to_string(),
        status: SessionStatus::Working,
        started_at: Some(now),
        closed_at: None,
        control: None,
        model: data.model.clone(),
        model_display_name: data.model_display_name.clone(),
        model_set_at: now,
        context_window_tokens: data.context_window_tokens.unwrap_or(0),
        statusline: StatuslineFields::default(),
        hook,
        jsonl,
        session_kind: None,
        entrypoint: None,
    }
}
