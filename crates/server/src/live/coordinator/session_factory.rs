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

    // Resolve JSONL path eagerly when cwd is available.
    let (project, file_path) = if let Some(ref cwd) = cwd {
        let proj = claude_view_core::discovery::encode_project_name(cwd);
        let fp = claude_view_core::discovery::claude_projects_dir()
            .map(|d| {
                d.join(&proj)
                    .join(format!("{session_id}.jsonl"))
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default();
        (proj, fp)
    } else {
        (String::new(), String::new())
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
            project,
            project_path: cwd.clone().unwrap_or_default(),
            file_path,
            ..JsonlFields::default()
        },
        session_kind: None,
        entrypoint: None,
        ownership: None,
        pending_interaction: None,
    }
}

/// Create a `LiveSession` from a pid.json Birth event — the sole creation path
/// for live sessions after the pid.json-as-single-root change.
pub fn create_session_from_birth(
    session: &claude_view_core::session_files::ActiveSession,
    now: i64,
) -> LiveSession {
    let project = claude_view_core::discovery::encode_project_name(&session.cwd);
    let resolved =
        claude_view_core::discovery::resolve_project_path_with_cwd(&project, Some(&session.cwd));

    // Resolve JSONL path eagerly so the WS handler can watch it immediately.
    // Path is deterministic: ~/.claude/projects/{project}/{session_id}.jsonl
    let file_path = claude_view_core::discovery::claude_projects_dir()
        .map(|d| {
            d.join(&project)
                .join(format!("{}.jsonl", session.session_id))
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();

    LiveSession {
        id: session.session_id.clone(),
        status: SessionStatus::Working,
        started_at: Some(crate::live::manager::helpers::ms_to_secs(
            session.started_at,
        )),
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            pid: Some(session.pid),
            last_activity_at: now,
            title: session.name.clone().unwrap_or_default(),
            ..HookFields::default()
        },
        jsonl: JsonlFields {
            project,
            project_path: session.cwd.clone(),
            project_display_name: resolved.display_name,
            file_path,
            ..JsonlFields::default()
        },
        session_kind: Some(session.kind.clone()),
        entrypoint: Some(session.entrypoint.clone()),
        ownership: None,
        pending_interaction: None,
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
        ownership: None,
        pending_interaction: None,
    }
}
