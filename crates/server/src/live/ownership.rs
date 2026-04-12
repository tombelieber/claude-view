//! Compute session ownership from independent sources of truth.

use claude_view_types::{SdkBinding, SessionOwnership, TmuxBinding};

use crate::live::state::LiveSession;
use crate::routes::cli_sessions::store::CliSessionStore;

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

/// Compute ownership from two independent sources. No priority — facts coexist.
pub async fn compute_ownership(
    session: &LiveSession,
    cli_sessions: &CliSessionStore,
) -> SessionOwnership {
    let source = source_label(session);
    let entrypoint = session.entrypoint.clone();

    let sdk = session.control.as_ref().map(|c| SdkBinding {
        control_id: c.control_id.clone(),
    });

    let tmux = cli_sessions
        .find_by_claude_session_id(&session.id)
        .await
        .map(|cli| TmuxBinding {
            cli_session_id: cli.id,
        });

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
pub async fn enrich_with_ownership(
    session: &LiveSession,
    cli_sessions: &CliSessionStore,
) -> LiveSession {
    let mut enriched = session.clone();
    enriched.ownership = Some(compute_ownership(session, cli_sessions).await);
    enriched
}

/// Write resolved ownership into the session record and broadcast update.
///
/// Computes ownership from the current session state and CLI session store,
/// stores it directly in the `LiveSession.ownership` field, and broadcasts
/// a `SessionUpsert` so SSE clients see the binding immediately.
pub async fn write_ownership(
    sessions: &crate::live::manager::LiveSessionMap,
    session_id: &str,
    cli_sessions: &CliSessionStore,
    tx: &tokio::sync::broadcast::Sender<crate::live::state::SessionEvent>,
) {
    let ownership = {
        let map = sessions.read().await;
        let Some(session) = map.get(session_id) else {
            return;
        };
        compute_ownership(session, cli_sessions).await
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
    async fn sdk_set_when_control_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-42"));

        let store = CliSessionStore::new();
        let result = compute_ownership(&session, &store).await;

        assert!(result.sdk.is_some());
        assert_eq!(result.sdk.unwrap().control_id, "ctl-42");
    }

    #[tokio::test]
    async fn tmux_set_when_cli_session_matches() {
        let session = test_live_session("sess-1");
        assert!(session.control.is_none());

        let store = CliSessionStore::new();
        store.insert(make_cli_session("cli-99", "sess-1")).await;

        let result = compute_ownership(&session, &store).await;

        assert!(result.tmux.is_some());
        assert_eq!(result.tmux.unwrap().cli_session_id, "cli-99");
        assert!(result.sdk.is_none());
    }

    #[tokio::test]
    async fn neither_when_no_control_and_no_cli_session() {
        let session = test_live_session("sess-1");
        assert!(session.control.is_none());

        let store = CliSessionStore::new();
        let result = compute_ownership(&session, &store).await;

        assert!(result.tmux.is_none());
        assert!(result.sdk.is_none());
    }

    #[tokio::test]
    async fn both_set_when_sdk_and_tmux_present() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-77"));

        let store = CliSessionStore::new();
        store.insert(make_cli_session("cli-99", "sess-1")).await;

        let result = compute_ownership(&session, &store).await;

        // Both should be set — independent facts coexist
        assert!(result.sdk.is_some());
        assert_eq!(result.sdk.unwrap().control_id, "ctl-77");
        assert!(result.tmux.is_some());
        assert_eq!(result.tmux.unwrap().cli_session_id, "cli-99");
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
        let result = compute_ownership(&session, &store).await;

        assert_eq!(result.source.as_deref(), Some("ide"));
        assert_eq!(result.entrypoint.as_deref(), Some("cli"));
        assert!(result.sdk.is_some());
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

        let result = compute_ownership(&session, &store).await;

        assert_eq!(result.source.as_deref(), Some("terminal"));
        assert_eq!(result.entrypoint.as_deref(), Some("claude-vscode"));
        assert!(result.tmux.is_some());
    }

    #[tokio::test]
    async fn source_and_entrypoint_carried_through_no_bindings() {
        let mut session = test_live_session("sess-1");
        session.jsonl.source = Some(SessionSourceInfo {
            category: SessionSource::AgentSdk,
            label: None,
        });
        session.entrypoint = None;

        let store = CliSessionStore::new();
        let result = compute_ownership(&session, &store).await;

        assert_eq!(result.source.as_deref(), Some("agent_sdk"));
        assert!(result.entrypoint.is_none());
        assert!(result.tmux.is_none());
        assert!(result.sdk.is_none());
    }

    #[tokio::test]
    async fn enrich_with_ownership_sets_ownership_field() {
        let mut session = test_live_session("sess-1");
        session.control = Some(make_control_binding("ctl-10"));
        assert!(session.ownership.is_none());

        let store = CliSessionStore::new();
        let enriched = enrich_with_ownership(&session, &store).await;

        assert!(enriched.ownership.is_some());
        let ownership = enriched.ownership.unwrap();
        assert!(ownership.sdk.is_some());
        assert_eq!(ownership.sdk.unwrap().control_id, "ctl-10");
    }
}
