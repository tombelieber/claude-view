//! Compute session ownership from independent sources of truth.

use claude_view_types::{SdkBinding, SessionOwnership};

use crate::live::state::LiveSession;

/// Convert the structured `SessionSourceInfo` into the flat string used by
/// `SessionOwnership`. Uses the serde `rename_all = "snake_case"` form of the
/// category enum (e.g. "terminal", "ide", "agent_sdk").
fn source_label(session: &LiveSession) -> Option<String> {
    session.jsonl.source.as_ref().map(|info| {
        serde_json::to_value(&info.category)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", info.category))
    })
}

/// Compute ownership from session state. No external stores needed.
///
/// Tmux binding is now set at POST time (for tmux-spawned sessions) or by
/// the Born handler. The resolver only needs to check for SDK bindings
/// and carry through existing tmux binding from the session record.
pub fn compute_ownership(session: &LiveSession) -> SessionOwnership {
    let source = source_label(session);
    let entrypoint = session.entrypoint.clone();

    let sdk = session.control.as_ref().map(|c| SdkBinding {
        control_id: c.control_id.clone(),
    });

    // Tmux binding is pre-set on the session at creation time (POST handler).
    // Carry it forward from the existing ownership field, if present.
    let tmux = session.ownership.as_ref().and_then(|o| o.tmux.clone());

    SessionOwnership {
        tmux,
        sdk,
        source,
        entrypoint,
    }
}

/// Enrich a LiveSession clone with computed ownership before serialization.
/// Retained for tests — production code computes ownership on session creation
/// (coordinator pipeline Phase 2+3) and updates via `write_ownership`.
#[cfg(test)]
pub fn enrich_with_ownership(session: &LiveSession) -> LiveSession {
    let mut enriched = session.clone();
    enriched.ownership = Some(compute_ownership(session));
    enriched
}

/// Write resolved ownership into the session record and broadcast update.
///
/// Computes ownership from the current session state, stores it directly
/// in the `LiveSession.ownership` field, and broadcasts a `SessionUpsert`
/// so SSE clients see the binding immediately.
pub async fn write_ownership(
    sessions: &crate::live::manager::LiveSessionMap,
    session_id: &str,
    tx: &tokio::sync::broadcast::Sender<crate::live::state::SessionEvent>,
) {
    let ownership = {
        let map = sessions.read().await;
        let Some(session) = map.get(session_id) else {
            return;
        };
        compute_ownership(session)
    };
    let mut map = sessions.write().await;
    if let Some(session) = map.get_mut(session_id) {
        session.ownership = Some(ownership);
        let _ = tx.send(crate::live::state::SessionEvent::SessionUpsert {
            session: session.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::test_live_session;
    use crate::live::state::ControlBinding;
    use crate::live::state::{SessionSource, SessionSourceInfo};
    use claude_view_types::TmuxBinding;
    use tokio_util::sync::CancellationToken;

    fn make_control_binding(control_id: &str) -> ControlBinding {
        ControlBinding {
            control_id: control_id.to_string(),
            bound_at: 1000,
            bound_at_generation: 0,
            cancel: CancellationToken::new(),
        }
    }

    #[test]
    fn sdk_set_when_control_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-42"));

        let result = compute_ownership(&session);

        assert!(result.sdk.is_some());
        assert_eq!(result.sdk.unwrap().control_id, "ctl-42");
    }

    #[test]
    fn tmux_set_when_ownership_has_tmux_binding() {
        let mut session = test_live_session("sess-1");
        // Pre-set tmux binding (as POST handler would)
        session.ownership = Some(SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-abc".to_string(),
            }),
            sdk: None,
            source: None,
            entrypoint: None,
        });

        let result = compute_ownership(&session);

        assert!(result.tmux.is_some());
        assert_eq!(result.tmux.unwrap().cli_session_id, "cv-abc");
        assert!(result.sdk.is_none());
    }

    #[test]
    fn neither_when_no_control_and_no_tmux() {
        let session = test_live_session("sess-1");
        assert!(session.control.is_none());

        let result = compute_ownership(&session);

        assert!(result.tmux.is_none());
        assert!(result.sdk.is_none());
    }

    #[test]
    fn both_set_when_sdk_and_tmux_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-77"));
        session.ownership = Some(SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-99".to_string(),
            }),
            sdk: None,
            source: None,
            entrypoint: None,
        });

        let result = compute_ownership(&session);

        // Both should be set — independent facts coexist
        assert!(result.sdk.is_some());
        assert_eq!(result.sdk.unwrap().control_id, "ctl-77");
        assert!(result.tmux.is_some());
        assert_eq!(result.tmux.unwrap().cli_session_id, "cv-99");
    }

    #[test]
    fn source_and_entrypoint_carried_through_sdk() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("VS Code".into()),
        });
        session.entrypoint = Some("cli".to_string());
        session.control = Some(make_control_binding("ctl-1"));

        let result = compute_ownership(&session);

        assert_eq!(result.source.as_deref(), Some("ide"));
        assert_eq!(result.entrypoint.as_deref(), Some("cli"));
        assert!(result.sdk.is_some());
    }

    #[test]
    fn source_and_entrypoint_carried_through_tmux() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::Terminal,
            label: None,
        });
        session.entrypoint = Some("claude-vscode".to_string());
        session.ownership = Some(SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-1".to_string(),
            }),
            sdk: None,
            source: None,
            entrypoint: None,
        });

        let result = compute_ownership(&session);

        assert_eq!(result.source.as_deref(), Some("terminal"));
        assert_eq!(result.entrypoint.as_deref(), Some("claude-vscode"));
        assert!(result.tmux.is_some());
    }

    #[test]
    fn source_and_entrypoint_carried_through_no_bindings() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::AgentSdk,
            label: None,
        });
        session.entrypoint = None;

        let result = compute_ownership(&session);

        assert_eq!(result.source.as_deref(), Some("agent_sdk"));
        assert!(result.entrypoint.is_none());
        assert!(result.tmux.is_none());
        assert!(result.sdk.is_none());
    }

    #[test]
    fn enrich_with_ownership_sets_ownership_field() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-10"));
        assert!(session.ownership.is_none());

        let enriched = enrich_with_ownership(&session);

        assert!(enriched.ownership.is_some());
        let ownership = enriched.ownership.unwrap();
        assert!(ownership.sdk.is_some());
        assert_eq!(ownership.sdk.unwrap().control_id, "ctl-10");
    }
}
