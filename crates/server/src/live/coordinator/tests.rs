#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tokio::sync::{broadcast, RwLock};

    use crate::live::coordinator::dispatch::{apply_mutation_to_session, common_post_mutation};
    use crate::live::coordinator::pipeline::SessionCoordinator;
    use crate::live::coordinator::planning::plan_side_effects;
    use crate::live::coordinator::session_factory::{
        create_session_from_start, create_session_shell,
    };
    use crate::live::coordinator::types::MutationContext;
    use crate::live::manager::{LiveSessionMap, TranscriptMap};
    use crate::live::mutation::types::{
        InteractionAction, LifecycleEvent, MutationResult, ReconcileData, SessionMutation,
        SideEffect,
    };
    use crate::live::state::{HookEvent, LiveSession, SessionEvent, SessionStatus};

    #[test]
    fn create_session_sets_basic_fields() {
        let session = create_session_from_start(
            "test-session-123",
            &Some("/home/user/project".to_string()),
            &Some("claude-sonnet-4-20250514".to_string()),
            &Some(12345),
            1700000000,
        );

        assert_eq!(session.id, "test-session-123");
        assert_eq!(session.status, SessionStatus::Working);
        assert_eq!(session.started_at, Some(1700000000));
        assert!(session.closed_at.is_none());
        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.hook.pid, Some(12345));
        assert_eq!(session.hook.last_activity_at, 1700000000);
        assert_eq!(session.jsonl.project_path, "/home/user/project");
    }

    #[test]
    fn create_session_shell_from_reconcile() {
        let data = ReconcileData {
            project: Some("my-project".into()),
            project_display_name: Some("My Project".into()),
            project_path: Some("/home/user/my-project".into()),
            model: Some("claude-sonnet-4-20250514".into()),
            model_display_name: Some("Sonnet".into()),
            tokens: None,
            context_window_tokens: Some(200_000),
            cost: None,
            turn_count: None,
            edit_count: None,
            phase: None,
        };

        let session = create_session_shell("reconcile-123", &data, 1700000000);

        assert_eq!(session.id, "reconcile-123");
        assert_eq!(session.jsonl.project, "my-project");
        assert_eq!(session.jsonl.project_display_name, "My Project");
        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.context_window_tokens, 200_000);
    }

    #[test]
    fn plan_side_effects_for_end_captures_events() {
        let mut session = create_session_from_start("end-test", &None, &None, &None, 1700000000);

        // Add some hook events
        session.hook.hook_events.push(HookEvent {
            timestamp: 1700000001,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Reading file".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook".into(),
        });
        session.hook.hook_events.push(HookEvent {
            timestamp: 1700000002,
            event_name: "PostToolUse".into(),
            tool_name: Some("Read".into()),
            label: "Read complete".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook".into(),
        });

        let mutation = SessionMutation::Lifecycle(LifecycleEvent::End { reason: None });
        let effects = plan_side_effects("end-test", &session, &mutation, 1700000003);

        // Should have: PersistHookEvents, RemoveAccumulator, CleanHookEventChannel
        assert_eq!(effects.len(), 3);

        // Verify PersistHookEvents captured the events
        let persist = effects
            .iter()
            .find(|e| matches!(e, SideEffect::PersistHookEvents { .. }));
        assert!(persist.is_some(), "Expected PersistHookEvents side effect");
        if let Some(SideEffect::PersistHookEvents { events, .. }) = persist {
            assert_eq!(events.len(), 2);
        }

        // Verify RemoveAccumulator is planned
        let remove = effects
            .iter()
            .any(|e| matches!(e, SideEffect::RemoveAccumulator { .. }));
        assert!(remove, "Expected RemoveAccumulator side effect");

        // Verify CleanHookEventChannel is planned
        let clean = effects
            .iter()
            .any(|e| matches!(e, SideEffect::CleanHookEventChannel { .. }));
        assert!(clean, "Expected CleanHookEventChannel side effect");
    }

    #[test]
    fn plan_side_effects_for_start_creates_accumulator() {
        let session =
            create_session_from_start("start-test", &Some("/tmp".into()), &None, &None, 1700000000);

        let mutation = SessionMutation::Lifecycle(LifecycleEvent::Start {
            cwd: Some("/tmp".into()),
            model: None,
            source: None,
            pid: None,
            transcript_path: None,
        });
        let effects = plan_side_effects("start-test", &session, &mutation, 1700000000);

        let has_create = effects
            .iter()
            .any(|e| matches!(e, SideEffect::CreateAccumulator { .. }));
        assert!(has_create, "Expected CreateAccumulator side effect");
    }

    #[test]
    fn apply_statusline_mutation_updates_model() {
        let mut session = create_session_from_start("model-test", &None, &None, &None, 1700000000);

        assert!(session.model.is_none());

        let payload = crate::routes::statusline::StatuslinePayload {
            session_id: "model-test".into(),
            model: Some(crate::routes::statusline::StatuslineModel {
                id: Some("claude-sonnet-4-20250514".into()),
                display_name: Some("Sonnet".into()),
            }),
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        };

        let mutation = SessionMutation::Statusline(Box::new(payload));
        apply_mutation_to_session(&mut session, &mutation, 1700000001);

        assert_eq!(session.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(session.model_display_name.as_deref(), Some("Sonnet"));
    }

    #[test]
    fn common_post_mutation_extracts_pid() {
        let mutation = SessionMutation::Lifecycle(LifecycleEvent::Prompt {
            text: "hello".into(),
            pid: Some(999),
        });
        let post = common_post_mutation(&mutation, None, 1700000000);
        assert_eq!(post.bind_pid, Some(999));
        assert_eq!(post.update_activity_at, Some(1700000000));
    }

    #[test]
    fn common_post_mutation_falls_back_to_caller_pid() {
        let mutation =
            SessionMutation::Statusline(Box::new(crate::routes::statusline::StatuslinePayload {
                session_id: "test".into(),
                model: None,
                cwd: None,
                workspace: None,
                cost: None,
                context_window: None,
                exceeds_200k_tokens: None,
                transcript_path: None,
                version: None,
                output_style: None,
                vim: None,
                agent: None,
                worktree: None,
                rate_limits: None,
                extra: Default::default(),
            }));
        let post = common_post_mutation(&mutation, Some(555), 1700000000);
        assert_eq!(post.bind_pid, Some(555));
    }

    #[tokio::test]
    async fn buffer_phase_stores_and_drains() {
        let coordinator = SessionCoordinator::new();
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let (live_tx, _rx) = broadcast::channel(16);
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let transcript_to_session: TranscriptMap = Arc::new(RwLock::new(HashMap::new()));
        let hook_event_channels: Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let cli_sessions = Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new());
        let interaction_data: Arc<RwLock<HashMap<String, claude_view_types::InteractionBlock>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &transcript_to_session,
            hook_event_channels: &hook_event_channels,
            cli_sessions: &cli_sessions,
            interaction_data: &interaction_data,
        };

        // Send a statusline mutation for a session that doesn't exist yet
        let payload = crate::routes::statusline::StatuslinePayload {
            session_id: "buffered-session".into(),
            model: Some(crate::routes::statusline::StatuslineModel {
                id: Some("claude-sonnet-4-20250514".into()),
                display_name: Some("Sonnet".into()),
            }),
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        };

        let result = coordinator
            .handle(
                &ctx,
                "buffered-session",
                SessionMutation::Statusline(Box::new(payload)),
                None,
                1700000000,
                None,
                None, // no cwd -> buffer
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Buffered),
            "Expected Buffered, got different result"
        );

        // Now send a Birth event which can create the session
        let active = claude_view_core::session_files::ActiveSession {
            pid: 111,
            session_id: "buffered-session".to_string(),
            cwd: "/tmp".to_string(),
            started_at: 1700000001000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        let result = coordinator
            .handle(
                &ctx,
                "buffered-session",
                SessionMutation::Birth(active),
                Some(111),
                1700000001,
                None,
                Some("/tmp"),
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Created(_)),
            "Expected Created after Birth"
        );

        // Verify the buffered statusline was drained and applied
        let sessions = ctx.sessions.read().await;
        let session = sessions.get("buffered-session").unwrap();
        assert_eq!(
            session.model.as_deref(),
            Some("claude-sonnet-4-20250514"),
            "Buffered statusline model should have been applied"
        );
    }

    // =========================================================================
    // Upsert regression tests — any hook with cwd must create-if-missing
    // =========================================================================

    /// Helper: create a MutationContext for upsert tests (no live_manager).
    async fn make_upsert_ctx() -> (
        SessionCoordinator,
        Arc<RwLock<HashMap<String, LiveSession>>>,
        broadcast::Sender<SessionEvent>,
        claude_view_db::Database,
        TranscriptMap,
        Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
        Arc<crate::routes::cli_sessions::store::CliSessionStore>,
        Arc<RwLock<HashMap<String, claude_view_types::InteractionBlock>>>,
    ) {
        let coordinator = SessionCoordinator::new();
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let (live_tx, _rx) = broadcast::channel(16);
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let transcript_to_session: TranscriptMap = Arc::new(RwLock::new(HashMap::new()));
        let hook_event_channels: Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let cli_sessions = Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new());
        let interaction_data: Arc<RwLock<HashMap<String, claude_view_types::InteractionBlock>>> =
            Arc::new(RwLock::new(HashMap::new()));
        (
            coordinator,
            sessions,
            live_tx,
            db,
            transcript_to_session,
            hook_event_channels,
            cli_sessions,
            interaction_data,
        )
    }

    #[tokio::test]
    async fn hook_with_cwd_buffers_without_birth() {
        // After pid.json change: hooks with cwd are BUFFERED, not upserted.
        // Only Birth and Reconcile can create sessions.
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // PreToolUse with cwd — was upsert, now buffered
        let result = coordinator
            .handle(
                &ctx,
                "upsert-session",
                SessionMutation::Lifecycle(LifecycleEvent::StateChange {
                    agent_state: crate::live::state::AgentState {
                        group: crate::live::state::AgentStateGroup::Autonomous,
                        state: "acting".into(),
                        label: "Running Bash".into(),
                        context: None,
                    },
                    event_name: "PreToolUse".into(),
                    pid: Some(12345),
                }),
                Some(12345),
                1700000000,
                None,
                Some("/tmp/my-project"),
                Some("/home/user/.claude/projects/my-project/upsert-session.jsonl"),
            )
            .await;

        assert!(
            matches!(result, MutationResult::Buffered),
            "Hooks with cwd must buffer after pid.json change"
        );
    }

    #[tokio::test]
    async fn observability_with_cwd_buffers_without_birth() {
        // After pid.json change: observability hooks with cwd are BUFFERED.
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // ConfigChange (Observability) with cwd — was upsert, now buffered
        let result = coordinator
            .handle(
                &ctx,
                "obs-upsert",
                SessionMutation::Lifecycle(LifecycleEvent::Observability {
                    event_name: "ConfigChange".into(),
                    pid: Some(555),
                }),
                Some(555),
                1700000000,
                None,
                Some("/tmp/project"),
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Buffered),
            "Observability with cwd must buffer after pid.json change"
        );
    }

    #[tokio::test]
    async fn no_cwd_still_buffers() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // StateChange without cwd — must buffer, not upsert
        let result = coordinator
            .handle(
                &ctx,
                "no-cwd-session",
                SessionMutation::Lifecycle(LifecycleEvent::StateChange {
                    agent_state: crate::live::state::AgentState {
                        group: crate::live::state::AgentStateGroup::Autonomous,
                        state: "acting".into(),
                        label: "Working".into(),
                        context: None,
                    },
                    event_name: "PreToolUse".into(),
                    pid: None,
                }),
                None,
                1700000000,
                None,
                None, // no cwd
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Buffered),
            "Without cwd, non-Start events must buffer"
        );
        assert!(ctx.sessions.read().await.get("no-cwd-session").is_none());
    }

    #[tokio::test]
    async fn birth_drains_previously_buffered_events() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // 1) First event: hook without Birth -> buffered
        coordinator
            .handle(
                &ctx,
                "drain-test",
                SessionMutation::Lifecycle(LifecycleEvent::Prompt {
                    text: "Hello world".into(),
                    pid: None,
                }),
                None,
                1700000000,
                None,
                None,
                None,
            )
            .await;
        assert!(ctx.sessions.read().await.get("drain-test").is_none());

        // 2) Birth event -> creates session + drains buffered Prompt
        let active = claude_view_core::session_files::ActiveSession {
            pid: 100,
            session_id: "drain-test".to_string(),
            cwd: "/tmp/proj".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        coordinator
            .handle(
                &ctx,
                "drain-test",
                SessionMutation::Birth(active),
                Some(100),
                1700000001,
                None,
                Some("/tmp/proj"),
                None,
            )
            .await;

        let sessions = ctx.sessions.read().await;
        let session = sessions.get("drain-test").unwrap();
        // Buffered Prompt should have been drained -> turn_count incremented
        assert_eq!(
            session.hook.turn_count, 1,
            "Buffered Prompt must be drained on Birth"
        );
        assert_eq!(session.hook.last_user_message, "Hello world");
    }

    #[tokio::test]
    async fn birth_then_stop_sets_needs_you_state() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // 1) Birth creates session
        let active = claude_view_core::session_files::ActiveSession {
            pid: 100,
            session_id: "stop-upsert".to_string(),
            cwd: "/tmp/proj".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        coordinator
            .handle(
                &ctx,
                "stop-upsert",
                SessionMutation::Birth(active),
                Some(100),
                1700000000,
                None,
                Some("/tmp/proj"),
                None,
            )
            .await;

        // 2) Stop event enriches with NeedsYou state
        let result = coordinator
            .handle(
                &ctx,
                "stop-upsert",
                SessionMutation::Lifecycle(LifecycleEvent::Stop {
                    agent_state: crate::live::state::AgentState {
                        group: crate::live::state::AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting".into(),
                        context: None,
                    },
                    last_assistant_message: Some("Done.".into()),
                    pid: None,
                }),
                None,
                1700000001,
                None,
                None,
                None,
            )
            .await;

        // Stop on an existing session = Closed (status -> Paused -> Done)
        // Actually Stop sets Paused which is != Done, so it's Updated
        assert!(
            matches!(result, MutationResult::Updated(_)),
            "Stop on existing session should return Updated"
        );

        let sessions = ctx.sessions.read().await;
        let session = sessions.get("stop-upsert").unwrap();
        assert_eq!(
            session.hook.agent_state.state, "idle",
            "Stop must reflect idle state, not forced acting"
        );
        assert!(matches!(
            session.hook.agent_state.group,
            crate::live::state::AgentStateGroup::NeedsYou
        ));
        assert_eq!(session.status, SessionStatus::Paused);
    }

    #[tokio::test]
    async fn persist_hook_events_writes_to_db() {
        use claude_view_db::{hook_events_queries, Database};

        let db = Database::new_in_memory().await.unwrap();
        let events = vec![
            HookEvent {
                timestamp: 1000,
                event_name: "PreToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Running: git status".into(),
                group: "autonomous".into(),
                context: None,
                source: "hook".into(),
            },
            HookEvent {
                timestamp: 1001,
                event_name: "PostToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Completed".into(),
                group: "autonomous".into(),
                context: Some(r#"{"exit_code":0}"#.into()),
                source: "hook".into(),
            },
        ];

        let rows: Vec<_> = events.iter().map(|e| e.to_row()).collect();
        hook_events_queries::insert_hook_events(&db, "test-persist", &rows)
            .await
            .unwrap();

        let stored = hook_events_queries::get_hook_events(&db, "test-persist")
            .await
            .unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].event_name, "PreToolUse");
        assert_eq!(stored[0].group_name, "autonomous");
        assert_eq!(stored[1].event_name, "PostToolUse");
        assert_eq!(stored[1].context.as_deref(), Some(r#"{"exit_code":0}"#));
    }

    // =========================================================================
    // Birth pipeline integration test
    // =========================================================================

    #[tokio::test]
    async fn birth_mutation_creates_session_through_pipeline() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        let active = claude_view_core::session_files::ActiveSession {
            pid: 46567,
            session_id: "birth-pipeline-test".to_string(),
            cwd: "/Users/test/project".to_string(),
            started_at: 1775492920444,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };

        let result = coordinator
            .handle(
                &ctx,
                "birth-pipeline-test",
                SessionMutation::Birth(active),
                Some(46567),
                1700000000,
                None,
                Some("/Users/test/project"),
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Created(_)),
            "Birth mutation must create session"
        );

        let sessions = ctx.sessions.read().await;
        let session = sessions.get("birth-pipeline-test").unwrap();
        assert_eq!(session.jsonl.project_path, "/Users/test/project");
        assert_eq!(session.jsonl.project_display_name, "project");
        assert_eq!(session.session_kind.as_deref(), Some("interactive"));
        assert_eq!(session.entrypoint.as_deref(), Some("cli"));
        assert_eq!(session.hook.pid, Some(46567));
    }

    // =========================================================================
    // Interaction mutation tests
    // =========================================================================

    /// Helper: create a session via Birth mutation. Use this in all tests that
    /// need a pre-existing session — Start can no longer create sessions.
    async fn create_session_via_birth(
        coordinator: &SessionCoordinator,
        ctx: &MutationContext<'_>,
        session_id: &str,
        pid: u32,
    ) {
        let active = claude_view_core::session_files::ActiveSession {
            pid,
            session_id: session_id.to_string(),
            cwd: "/tmp".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        coordinator
            .handle(
                ctx,
                session_id,
                SessionMutation::Birth(active),
                Some(pid),
                1700000000,
                None,
                Some("/tmp"),
                None,
            )
            .await;
    }

    fn make_interaction_meta(request_id: &str) -> claude_view_types::PendingInteractionMeta {
        claude_view_types::PendingInteractionMeta {
            variant: claude_view_types::InteractionVariant::Permission,
            request_id: request_id.to_string(),
            preview: "Allow file write?".to_string(),
        }
    }

    fn make_interaction_block(request_id: &str) -> claude_view_types::InteractionBlock {
        claude_view_types::InteractionBlock {
            id: format!("block-{request_id}"),
            variant: claude_view_types::InteractionVariant::Permission,
            request_id: Some(request_id.to_string()),
            resolved: false,
            historical_source: None,
            data: serde_json::json!({"tool": "Bash", "command": "rm -rf /"}),
        }
    }

    #[test]
    fn interaction_set_updates_pending_interaction() {
        let mut session =
            create_session_from_start("int-set", &Some("/tmp".into()), &None, &None, 1700000000);
        assert!(session.pending_interaction.is_none());

        let meta = make_interaction_meta("req-001");
        let full_data = make_interaction_block("req-001");
        let mutation = SessionMutation::Interaction(InteractionAction::Set {
            meta: meta.clone(),
            full_data,
        });
        apply_mutation_to_session(&mut session, &mutation, 1700000001);

        assert!(session.pending_interaction.is_some());
        let stored = session.pending_interaction.as_ref().unwrap();
        assert_eq!(stored.request_id, "req-001");
        assert_eq!(stored.preview, "Allow file write?");
        assert!(matches!(
            stored.variant,
            claude_view_types::InteractionVariant::Permission
        ));
    }

    #[test]
    fn interaction_clear_removes_pending_interaction() {
        let mut session =
            create_session_from_start("int-clr", &Some("/tmp".into()), &None, &None, 1700000000);
        session.pending_interaction = Some(make_interaction_meta("req-001"));
        assert!(session.pending_interaction.is_some());

        let mutation = SessionMutation::Interaction(InteractionAction::Clear {
            request_id: "req-001".into(),
        });
        apply_mutation_to_session(&mut session, &mutation, 1700000002);

        assert!(session.pending_interaction.is_none());
    }

    #[test]
    fn interaction_set_plans_set_side_effect() {
        let session =
            create_session_from_start("int-se", &Some("/tmp".into()), &None, &None, 1700000000);
        let meta = make_interaction_meta("req-001");
        let full_data = make_interaction_block("req-001");
        let mutation = SessionMutation::Interaction(InteractionAction::Set { meta, full_data });

        let effects = plan_side_effects("int-se", &session, &mutation, 1700000001);
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], SideEffect::SetInteractionData { request_id, .. } if request_id == "req-001"),
            "Expected SetInteractionData side effect"
        );
    }

    #[test]
    fn interaction_clear_plans_clear_side_effect() {
        let session =
            create_session_from_start("int-ce", &Some("/tmp".into()), &None, &None, 1700000000);
        let mutation = SessionMutation::Interaction(InteractionAction::Clear {
            request_id: "req-001".into(),
        });

        let effects = plan_side_effects("int-ce", &session, &mutation, 1700000001);
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], SideEffect::ClearInteractionData { request_id } if request_id == "req-001"),
            "Expected ClearInteractionData side effect"
        );
    }

    // =========================================================================
    // Birth mutation tests — pid.json as single root for session detection
    // =========================================================================

    #[test]
    fn birth_can_create_session_with_cwd() {
        let active = claude_view_core::session_files::ActiveSession {
            pid: 12345,
            session_id: "birth-test".to_string(),
            cwd: "/Users/test/project".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        let mutation = SessionMutation::Birth(active);
        assert!(mutation.can_create_session(), "Birth with cwd must create");
    }

    #[test]
    fn birth_cannot_create_session_with_empty_cwd() {
        let active = claude_view_core::session_files::ActiveSession {
            pid: 12345,
            session_id: "birth-empty".to_string(),
            cwd: "".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };
        let mutation = SessionMutation::Birth(active);
        assert!(
            !mutation.can_create_session(),
            "Birth with empty cwd must not create"
        );
    }

    #[test]
    fn create_session_from_birth_sets_all_fields() {
        use crate::live::coordinator::session_factory::create_session_from_birth;

        let active = claude_view_core::session_files::ActiveSession {
            pid: 46567,
            session_id: "79a5eefa-1234".to_string(),
            cwd: "/Users/dev/@acme/my-project".to_string(),
            started_at: 1775492920444,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: Some("my-feature".to_string()),
        };

        let session = create_session_from_birth(&active, 1700000000);

        assert_eq!(session.id, "79a5eefa-1234");
        assert_eq!(session.status, SessionStatus::Working);
        assert_eq!(session.started_at, Some(1775492920)); // ms -> s
        assert_eq!(session.hook.pid, Some(46567));
        assert_eq!(session.hook.last_activity_at, 1700000000);
        assert_eq!(session.hook.title, "my-feature"); // from name field
        assert_eq!(session.jsonl.project_path, "/Users/dev/@acme/my-project");
        assert_eq!(session.jsonl.project, "-Users-dev--acme-my-project");
        assert_eq!(session.jsonl.project_display_name, "my-project");
        assert_eq!(session.session_kind.as_deref(), Some("interactive"));
        assert_eq!(session.entrypoint.as_deref(), Some("cli"));
        assert!(session.model.is_none());
        assert!(session.closed_at.is_none());
        assert!(session.pending_interaction.is_none());
    }

    #[test]
    fn create_session_from_birth_without_name() {
        use crate::live::coordinator::session_factory::create_session_from_birth;

        let active = claude_view_core::session_files::ActiveSession {
            pid: 100,
            session_id: "no-name-sess".to_string(),
            cwd: "/tmp/project".to_string(),
            started_at: 1700000000000,
            kind: "background".to_string(),
            entrypoint: "claude-vscode".to_string(),
            name: None,
        };

        let session = create_session_from_birth(&active, 1700000000);
        assert_eq!(session.hook.title, ""); // no name -> empty
        assert_eq!(session.session_kind.as_deref(), Some("background"));
        assert_eq!(session.entrypoint.as_deref(), Some("claude-vscode"));
    }

    #[test]
    fn lifecycle_start_cannot_create_session() {
        // After the change, hooks can no longer create sessions
        let mutation = SessionMutation::Lifecycle(LifecycleEvent::Start {
            cwd: Some("/tmp".into()),
            model: None,
            source: None,
            pid: Some(111),
            transcript_path: None,
        });
        assert!(
            !mutation.can_create_session(),
            "Lifecycle::Start must NOT create sessions after pid.json change"
        );
    }

    #[test]
    fn plan_side_effects_for_birth_is_empty() {
        use crate::live::coordinator::session_factory::create_session_from_birth;

        let active = claude_view_core::session_files::ActiveSession {
            pid: 100,
            session_id: "birth-plan".to_string(),
            cwd: "/tmp".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };

        let session = create_session_from_birth(&active, 1700000000);
        let mutation = SessionMutation::Birth(active);
        let effects = plan_side_effects("birth-plan", &session, &mutation, 1700000000);
        assert!(effects.is_empty(), "Birth should have no side effects");
    }

    #[test]
    fn apply_birth_mutation_returns_none() {
        use crate::live::coordinator::session_factory::create_session_from_birth;

        let active = claude_view_core::session_files::ActiveSession {
            pid: 100,
            session_id: "birth-apply".to_string(),
            cwd: "/tmp".to_string(),
            started_at: 1700000000000,
            kind: "interactive".to_string(),
            entrypoint: "cli".to_string(),
            name: None,
        };

        let mut session = create_session_from_birth(&active, 1700000000);
        let mutation = SessionMutation::Birth(active);
        let result = apply_mutation_to_session(&mut session, &mutation, 1700000001);
        assert!(result.is_none(), "Birth mutation should not change status");
        assert_eq!(session.status, SessionStatus::Working);
    }

    #[test]
    fn interaction_cannot_create_session() {
        let set_mutation = SessionMutation::Interaction(InteractionAction::Set {
            meta: make_interaction_meta("req-001"),
            full_data: make_interaction_block("req-001"),
        });
        assert!(
            !set_mutation.can_create_session(),
            "Interaction::Set must not create sessions"
        );

        let clear_mutation = SessionMutation::Interaction(InteractionAction::Clear {
            request_id: "req-001".into(),
        });
        assert!(
            !clear_mutation.can_create_session(),
            "Interaction::Clear must not create sessions"
        );
    }

    #[tokio::test]
    async fn interaction_set_broadcasts_updated() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let mut rx = live_tx.subscribe();
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // First create a session so it exists (via Birth)
        create_session_via_birth(&coordinator, &ctx, "int-bc", 111).await;
        let _ = rx.recv().await; // drain Created

        // Now send an interaction Set
        let result = coordinator
            .handle(
                &ctx,
                "int-bc",
                SessionMutation::Interaction(InteractionAction::Set {
                    meta: make_interaction_meta("req-001"),
                    full_data: make_interaction_block("req-001"),
                }),
                None,
                1700000001,
                None,
                None,
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Updated(_)),
            "Interaction::Set must return Updated"
        );

        // Verify SSE broadcast was Updated
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, SessionEvent::SessionUpsert { .. }),
            "Expected SessionUpdated broadcast"
        );

        // Verify pending_interaction is set on the session
        let sessions = ctx.sessions.read().await;
        let session = sessions.get("int-bc").unwrap();
        assert!(session.pending_interaction.is_some());
        assert_eq!(
            session.pending_interaction.as_ref().unwrap().request_id,
            "req-001"
        );
    }

    #[tokio::test]
    async fn interaction_clear_broadcasts_updated() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let mut rx = live_tx.subscribe();
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // Create session via Birth + set interaction
        create_session_via_birth(&coordinator, &ctx, "int-bc2", 222).await;
        let _ = rx.recv().await; // drain Created

        coordinator
            .handle(
                &ctx,
                "int-bc2",
                SessionMutation::Interaction(InteractionAction::Set {
                    meta: make_interaction_meta("req-002"),
                    full_data: make_interaction_block("req-002"),
                }),
                None,
                1700000001,
                None,
                None,
                None,
            )
            .await;
        let _ = rx.recv().await; // drain Updated from Set

        // Now clear
        let result = coordinator
            .handle(
                &ctx,
                "int-bc2",
                SessionMutation::Interaction(InteractionAction::Clear {
                    request_id: "req-002".into(),
                }),
                None,
                1700000002,
                None,
                None,
                None,
            )
            .await;

        assert!(
            matches!(result, MutationResult::Updated(_)),
            "Interaction::Clear must return Updated"
        );

        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, SessionEvent::SessionUpsert { .. }),
            "Expected SessionUpdated broadcast after Clear"
        );

        // Verify pending_interaction is cleared
        let sessions = ctx.sessions.read().await;
        let session = sessions.get("int-bc2").unwrap();
        assert!(session.pending_interaction.is_none());
    }

    #[tokio::test]
    async fn interaction_set_stores_full_data_in_side_map() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // Create session via Birth
        create_session_via_birth(&coordinator, &ctx, "int-side", 333).await;

        // Set interaction
        coordinator
            .handle(
                &ctx,
                "int-side",
                SessionMutation::Interaction(InteractionAction::Set {
                    meta: make_interaction_meta("req-side"),
                    full_data: make_interaction_block("req-side"),
                }),
                None,
                1700000001,
                None,
                None,
                None,
            )
            .await;

        // Verify side-map has the full data
        let map = idata.read().await;
        assert!(
            map.contains_key("req-side"),
            "Side-map must contain the interaction data"
        );
        let block = map.get("req-side").unwrap();
        assert_eq!(block.id, "block-req-side");
        assert!(!block.resolved);
    }

    #[tokio::test]
    async fn interaction_clear_removes_from_side_map() {
        let (coordinator, sessions, live_tx, db, tmap, hec, cli_sessions, idata) =
            make_upsert_ctx().await;
        let ctx = MutationContext {
            sessions: &sessions,
            live_tx: &live_tx,
            live_manager: None,
            db: &db,
            transcript_to_session: &tmap,
            hook_event_channels: &hec,
            cli_sessions: &cli_sessions,
            interaction_data: &idata,
        };

        // Create session via Birth + set interaction
        create_session_via_birth(&coordinator, &ctx, "int-rm", 444).await;

        coordinator
            .handle(
                &ctx,
                "int-rm",
                SessionMutation::Interaction(InteractionAction::Set {
                    meta: make_interaction_meta("req-rm"),
                    full_data: make_interaction_block("req-rm"),
                }),
                None,
                1700000001,
                None,
                None,
                None,
            )
            .await;

        // Verify it was inserted
        assert!(idata.read().await.contains_key("req-rm"));

        // Clear it
        coordinator
            .handle(
                &ctx,
                "int-rm",
                SessionMutation::Interaction(InteractionAction::Clear {
                    request_id: "req-rm".into(),
                }),
                None,
                1700000002,
                None,
                None,
                None,
            )
            .await;

        // Verify it was removed
        assert!(
            !idata.read().await.contains_key("req-rm"),
            "Side-map must be cleared after Interaction::Clear"
        );
    }
}
