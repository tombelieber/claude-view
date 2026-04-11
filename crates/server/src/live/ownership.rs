//! Resolve which tier owns a session: SDK > Tmux > Observed.

use claude_view_types::SessionOwnership;

use crate::live::state::LiveSession;
use crate::routes::cli_sessions::store::CliSessionStore;

/// Convert the structured `SessionSourceInfo` into the flat string used by
/// `SessionOwnership`. Uses the serde `rename_all = "snake_case"` form of the
/// category enum (e.g. "terminal", "ide", "agent_sdk").
fn source_label(session: &LiveSession) -> Option<String> {
    session.jsonl.source.as_ref().map(|info| {
        // Use serde to get the canonical snake_case name.
        serde_json::to_value(&info.category)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", info.category))
    })
}

/// Resolve which tier owns a session. Priority: SDK > Tmux > Observed.
pub async fn resolve_ownership(
    session: &LiveSession,
    cli_sessions: &CliSessionStore,
) -> SessionOwnership {
    let source = source_label(session);
    let entrypoint = session.entrypoint.clone();

    // SDK control binding present -> Sdk tier
    if let Some(control) = &session.control {
        return SessionOwnership::Sdk {
            control_id: control.control_id.clone(),
            source,
            entrypoint,
        };
    }

    // Tmux CLI session bound to this session UUID -> Tmux tier
    if let Some(cli) = cli_sessions.find_by_claude_session_id(&session.id).await {
        return SessionOwnership::Tmux {
            cli_session_id: cli.id.clone(),
            source,
            entrypoint,
        };
    }

    // No control channel -> Observed
    SessionOwnership::Observed { source, entrypoint }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::test_live_session;
    use crate::live::state::ControlBinding;
    use crate::live::state::{SessionSource, SessionSourceInfo};
    use crate::routes::cli_sessions::{CliSession, CliSessionStatus};
    use tokio_util::sync::CancellationToken;

    fn make_control_binding(control_id: &str) -> ControlBinding {
        ControlBinding {
            control_id: control_id.to_string(),
            bound_at: 1000,
            cancel: CancellationToken::new(),
        }
    }

    fn make_cli_session(id: &str, claude_session_id: &str) -> CliSession {
        CliSession {
            id: id.to_string(),
            created_at: 1000,
            status: CliSessionStatus::Running,
            project_dir: None,
            args: vec![],
            claude_session_id: Some(claude_session_id.to_string()),
        }
    }

    #[tokio::test]
    async fn sdk_wins_when_control_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-42"));

        let store = CliSessionStore::new();
        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Sdk { control_id, .. } => {
                assert_eq!(control_id, "ctl-42");
            }
            other => panic!("expected Sdk, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn tmux_wins_when_no_control_but_cli_session_matches() {
        let session = test_live_session("sess-1");
        assert!(session.control.is_none());

        let store = CliSessionStore::new();
        store.insert(make_cli_session("cli-99", "sess-1")).await;

        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Tmux { cli_session_id, .. } => {
                assert_eq!(cli_session_id, "cli-99");
            }
            other => panic!("expected Tmux, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn observed_fallback_when_no_control_and_no_cli_session() {
        let session = test_live_session("sess-1");
        assert!(session.control.is_none());

        let store = CliSessionStore::new();
        let result = resolve_ownership(&session, &store).await;

        match &result {
            SessionOwnership::Observed { .. } => {}
            other => panic!("expected Observed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn sdk_wins_over_tmux_when_both_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-77"));

        let store = CliSessionStore::new();
        // Also insert a matching CLI session — SDK should still win
        store.insert(make_cli_session("cli-99", "sess-1")).await;

        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Sdk { control_id, .. } => {
                assert_eq!(control_id, "ctl-77");
            }
            other => panic!("expected Sdk (priority over Tmux), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn source_and_entrypoint_carried_through_sdk() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::Ide,
            label: Some("VS Code".into()),
        });
        session.entrypoint = Some("cli".to_string());
        session.control = Some(make_control_binding("ctl-1"));

        let store = CliSessionStore::new();
        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Sdk {
                source, entrypoint, ..
            } => {
                assert_eq!(source.as_deref(), Some("ide"));
                assert_eq!(entrypoint.as_deref(), Some("cli"));
            }
            other => panic!("expected Sdk, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn source_and_entrypoint_carried_through_tmux() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::Terminal,
            label: None,
        });
        session.entrypoint = Some("claude-vscode".to_string());

        let store = CliSessionStore::new();
        store.insert(make_cli_session("cli-1", "sess-1")).await;

        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Tmux {
                source, entrypoint, ..
            } => {
                assert_eq!(source.as_deref(), Some("terminal"));
                assert_eq!(entrypoint.as_deref(), Some("claude-vscode"));
            }
            other => panic!("expected Tmux, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn source_and_entrypoint_carried_through_observed() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::AgentSdk,
            label: None,
        });
        session.entrypoint = None;

        let store = CliSessionStore::new();
        let result = resolve_ownership(&session, &store).await;

        match result {
            SessionOwnership::Observed { source, entrypoint } => {
                assert_eq!(source.as_deref(), Some("agent_sdk"));
                assert!(entrypoint.is_none());
            }
            other => panic!("expected Observed, got {:?}", other),
        }
    }
}
