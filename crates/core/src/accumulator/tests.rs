#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::accumulator::helpers::{derive_cache_status, parse_timestamp_to_unix};
    use crate::accumulator::types::SessionAccumulator;
    use crate::live_parser::{LineType, LiveLine};
    use crate::pricing::CacheStatus;
    use crate::progress::{ProgressSource, ProgressStatus};
    use crate::subagent::SubAgentStatus;

    /// Helper: build a minimal LiveLine with defaults.
    fn empty_line() -> LiveLine {
        LiveLine {
            line_type: LineType::Other,
            role: None,
            content_preview: String::new(),
            content_extended: String::new(),
            tool_names: Vec::new(),
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cache_creation_5m_tokens: None,
            cache_creation_1hr_tokens: None,
            timestamp: None,
            stop_reason: None,
            git_branch: None,
            cwd: None,
            is_meta: false,
            is_tool_result_continuation: false,
            has_system_prefix: false,
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
            sub_agent_progress: None,
            sub_agent_notification: None,
            todo_write: None,
            task_creates: Vec::new(),
            task_updates: Vec::new(),
            task_id_assignments: Vec::new(),
            skill_names: Vec::new(),
            bash_commands: Vec::new(),
            edited_files: Vec::new(),
            is_compact_boundary: false,
            ide_file: None,
            message_id: None,
            request_id: None,
            hook_progress: None,
            slug: None,
            team_name: None,
            at_files: Vec::new(),
            pasted_paths: Vec::new(),
            entrypoint: None,
            ai_title: None,
        }
    }

    #[test]
    fn test_accumulate_empty() {
        let acc = SessionAccumulator::new();
        let pricing = HashMap::new();
        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.total_tokens, 0);
        assert_eq!(data.turn_count, 0);
        assert!(data.cost.total_usd == 0.0);
        assert_eq!(data.cache_status, CacheStatus::Unknown);
        assert!(data.first_user_message.is_none());
        assert!(data.last_user_message.is_none());
        assert!(data.model.is_none());
        assert!(data.sub_agents.is_empty());
        assert!(data.progress_items.is_empty());
    }

    #[test]
    fn test_token_accumulation() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(1000);
        line1.output_tokens = Some(500);
        line1.cache_read_tokens = Some(200);
        line1.cache_creation_tokens = Some(50);

        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(2000);
        line2.output_tokens = Some(800);

        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.input_tokens, 3000);
        assert_eq!(data.tokens.output_tokens, 1300);
        assert_eq!(data.tokens.cache_read_tokens, 200);
        assert_eq!(data.tokens.cache_creation_tokens, 50);
        assert_eq!(data.tokens.total_tokens, 3000 + 1300 + 200 + 50);
    }

    #[test]
    fn test_content_block_dedup() {
        // Claude Code writes one JSONL line per content block (thinking, text, tool_use),
        // each carrying the same message-level usage tokens. We must only count once per
        // unique message_id:requestId pair.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // First content block of API response "msg1:req1"
        let mut block1 = empty_line();
        block1.line_type = LineType::Assistant;
        block1.input_tokens = Some(1000);
        block1.output_tokens = Some(500);
        block1.message_id = Some("msg1".to_string());
        block1.request_id = Some("req1".to_string());
        acc.process_line(&block1, 0, &pricing);

        // Second content block of the SAME API response -- should be deduped
        let mut block2 = empty_line();
        block2.line_type = LineType::Assistant;
        block2.input_tokens = Some(1000);
        block2.output_tokens = Some(500);
        block2.message_id = Some("msg1".to_string());
        block2.request_id = Some("req1".to_string());
        acc.process_line(&block2, 0, &pricing);

        // Third content block, still same response
        let mut block3 = empty_line();
        block3.line_type = LineType::Assistant;
        block3.input_tokens = Some(1000);
        block3.output_tokens = Some(500);
        block3.message_id = Some("msg1".to_string());
        block3.request_id = Some("req1".to_string());
        acc.process_line(&block3, 0, &pricing);

        let data = acc.finish(&pricing);
        // Should count tokens only once, not 3x
        assert_eq!(data.tokens.input_tokens, 1000);
        assert_eq!(data.tokens.output_tokens, 500);
        assert_eq!(data.tokens.total_tokens, 1500);
    }

    #[test]
    fn test_content_block_dedup_different_requests() {
        // Two different API responses should both be counted
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(1000);
        line1.output_tokens = Some(500);
        line1.message_id = Some("msg1".to_string());
        line1.request_id = Some("req1".to_string());
        acc.process_line(&line1, 0, &pricing);

        // Duplicate of first response -- should be deduped
        let mut dup = empty_line();
        dup.line_type = LineType::Assistant;
        dup.input_tokens = Some(1000);
        dup.output_tokens = Some(500);
        dup.message_id = Some("msg1".to_string());
        dup.request_id = Some("req1".to_string());
        acc.process_line(&dup, 0, &pricing);

        // Different API response -- should be counted
        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(2000);
        line2.output_tokens = Some(800);
        line2.message_id = Some("msg2".to_string());
        line2.request_id = Some("req2".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.input_tokens, 3000); // 1000 + 2000
        assert_eq!(data.tokens.output_tokens, 1300); // 500 + 800
    }

    #[test]
    fn test_content_block_dedup_no_ids_fallback() {
        // Lines without message_id/request_id should always be counted (legacy/live data)
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(1000);
        line1.output_tokens = Some(500);
        // No message_id or request_id set
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(2000);
        line2.output_tokens = Some(800);
        // No IDs either
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Both counted since no IDs for dedup
        assert_eq!(data.tokens.input_tokens, 3000);
        assert_eq!(data.tokens.output_tokens, 1300);
    }

    #[test]
    fn test_content_block_dedup_first_block_without_usage() {
        // If the first block has IDs but no usage/cost fields, it should NOT
        // consume the dedup key. Later block with usage must still be counted.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut empty_first = empty_line();
        empty_first.line_type = LineType::Assistant;
        empty_first.message_id = Some("msg1".to_string());
        empty_first.request_id = Some("req1".to_string());
        acc.process_line(&empty_first, 0, &pricing);

        let mut usage_later = empty_line();
        usage_later.line_type = LineType::Assistant;
        usage_later.input_tokens = Some(1200);
        usage_later.output_tokens = Some(300);
        usage_later.message_id = Some("msg1".to_string());
        usage_later.request_id = Some("req1".to_string());
        acc.process_line(&usage_later, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.input_tokens, 1200);
        assert_eq!(data.tokens.output_tokens, 300);
        assert_eq!(data.tokens.total_tokens, 1500);
    }

    #[test]
    fn test_content_block_dedup_partial_ids_fallback() {
        // Partial IDs are treated as fallback/no-dedup and both lines count.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(400);
        line1.message_id = Some("msg-partial".to_string());
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(600);
        line2.message_id = Some("msg-partial".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.input_tokens, 1000);
        assert_eq!(data.tokens.total_tokens, 1000);
    }

    #[test]
    fn test_cache_creation_split_accumulation() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.cache_creation_5m_tokens = Some(100);
        line.cache_creation_1hr_tokens = Some(500);

        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.tokens.cache_creation_5m_tokens, 100);
        assert_eq!(data.tokens.cache_creation_1hr_tokens, 500);
    }

    #[test]
    fn test_model_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.model = Some("claude-sonnet-4-6".to_string());
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.model = Some("claude-opus-4-6".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Last model seen wins
        assert_eq!(data.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn test_user_message_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Meta message should be skipped for content
        let mut meta = empty_line();
        meta.line_type = LineType::User;
        meta.is_meta = true;
        meta.content_preview = "system stuff".to_string();
        acc.process_line(&meta, 0, &pricing);

        // First real user message
        let mut first = empty_line();
        first.line_type = LineType::User;
        first.content_preview = "Fix the bug".to_string();
        acc.process_line(&first, 0, &pricing);

        // Second real user message
        let mut second = empty_line();
        second.line_type = LineType::User;
        second.content_preview = "Now add tests".to_string();
        acc.process_line(&second, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.turn_count, 3); // meta counts as a turn
        assert_eq!(data.first_user_message.as_deref(), Some("Fix the bug"));
        assert_eq!(data.last_user_message.as_deref(), Some("Now add tests"));
    }

    #[test]
    fn test_git_branch_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.git_branch = Some("feature/auth".to_string());
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.git_branch.as_deref(), Some("feature/auth"));
    }

    #[test]
    fn test_context_window_tracking() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // First assistant turn
        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.input_tokens = Some(5000);
        line1.cache_read_tokens = Some(3000);
        line1.cache_creation_tokens = Some(1000);
        acc.process_line(&line1, 0, &pricing);

        assert_eq!(acc.context_window_tokens, 9000); // 5000+3000+1000

        // Second assistant turn with larger context
        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.input_tokens = Some(10000);
        line2.cache_read_tokens = Some(5000);
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Last assistant turn's context window
        assert_eq!(data.context_window_tokens, 15000); // 10000+5000
    }

    #[test]
    fn test_started_at_from_first_timestamp() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line1 = empty_line();
        line1.timestamp = Some("2026-02-20T10:00:00Z".to_string());
        acc.process_line(&line1, 0, &pricing);

        let mut line2 = empty_line();
        line2.timestamp = Some("2026-02-20T10:05:00Z".to_string());
        acc.process_line(&line2, 0, &pricing);

        let data = acc.finish(&pricing);
        // Should capture the first timestamp only
        assert!(data.tokens.total_tokens == 0); // no tokens in these lines
        let started = acc.started_at.unwrap();
        assert!(started > 0);
        // Second line should not overwrite started_at
        let ts1 = parse_timestamp_to_unix("2026-02-20T10:00:00Z").unwrap();
        assert_eq!(started, ts1);
    }

    #[test]
    fn test_sub_agent_spawn_and_completion() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line.timestamp = Some("2026-02-20T10:00:00Z".to_string());
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01ABC".to_string(),
                agent_type: "Explore".to_string(),
                description: "Search codebase".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        assert_eq!(acc.sub_agents.len(), 1);
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);

        // Completion
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.timestamp = Some("2026-02-20T10:01:00Z".to_string());
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: Some("a951849".to_string()),
            status: "completed".to_string(),
            total_duration_ms: Some(60000),
            total_tool_use_count: Some(15),
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.sub_agents.len(), 1);
        assert_eq!(data.sub_agents[0].status, SubAgentStatus::Complete);
        assert_eq!(data.sub_agents[0].agent_id.as_deref(), Some("a951849"));
        assert_eq!(data.sub_agents[0].duration_ms, Some(60000));
        assert_eq!(data.sub_agents[0].tool_use_count, Some(15));
    }

    #[test]
    fn test_sub_agent_async_launched_stays_running() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01BG".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "Backend work".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);

        // Background launch result (async_launched)
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01BG".to_string(),
            agent_id: Some("ab897bc".to_string()),
            status: "async_launched".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        // Should stay Running, not Error
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);
        // But agent_id should be captured
        assert_eq!(acc.sub_agents[0].agent_id.as_deref(), Some("ab897bc"));
        // Completion fields should remain None
        assert_eq!(acc.sub_agents[0].completed_at, None);
        assert_eq!(acc.sub_agents[0].duration_ms, None);
    }

    #[test]
    fn test_sub_agent_background_notification_completed() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01BG2".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "Backend work".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        // async_launched (captures agentId)
        let mut launch_line = empty_line();
        launch_line.line_type = LineType::User;
        launch_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01BG2".to_string(),
            agent_id: Some("ab897bc".to_string()),
            status: "async_launched".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&launch_line, 0, &pricing);
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);
        assert_eq!(acc.sub_agents[0].agent_id.as_deref(), Some("ab897bc"));

        // <task-notification> completed
        let mut notif_line = empty_line();
        notif_line.line_type = LineType::User;
        notif_line.timestamp = Some("2026-02-20T11:00:00Z".to_string());
        notif_line.sub_agent_notification = Some(crate::live_parser::SubAgentNotification {
            agent_id: "ab897bc".to_string(),
            status: "completed".to_string(),
        });
        acc.process_line(&notif_line, 0, &pricing);

        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Complete);
        assert!(acc.sub_agents[0].completed_at.is_some());
        assert_eq!(acc.sub_agents[0].current_activity, None);
    }

    #[test]
    fn test_sub_agent_background_notification_failed() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn + async_launched
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01FAIL".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "Failing work".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        let mut launch_line = empty_line();
        launch_line.line_type = LineType::User;
        launch_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01FAIL".to_string(),
            agent_id: Some("afailed1".to_string()),
            status: "async_launched".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&launch_line, 0, &pricing);

        // <task-notification> failed
        let mut notif_line = empty_line();
        notif_line.line_type = LineType::User;
        notif_line.sub_agent_notification = Some(crate::live_parser::SubAgentNotification {
            agent_id: "afailed1".to_string(),
            status: "failed".to_string(),
        });
        acc.process_line(&notif_line, 0, &pricing);

        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Error);
    }

    #[test]
    fn test_sub_agent_background_notification_killed() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn + async_launched
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01KILL".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "Killed work".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        let mut launch_line = empty_line();
        launch_line.line_type = LineType::User;
        launch_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01KILL".to_string(),
            agent_id: Some("akilled1".to_string()),
            status: "async_launched".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&launch_line, 0, &pricing);

        // <task-notification> killed
        let mut notif_line = empty_line();
        notif_line.line_type = LineType::User;
        notif_line.sub_agent_notification = Some(crate::live_parser::SubAgentNotification {
            agent_id: "akilled1".to_string(),
            status: "killed".to_string(),
        });
        acc.process_line(&notif_line, 0, &pricing);

        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Error);
    }

    #[test]
    fn test_sub_agent_teammate_spawned_stays_running() {
        // Regression: team-spawned agents return "teammate_spawned" in
        // toolUseResult.status. These are async -- agent is still running.
        // Must NOT be marked Error.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01TEAM".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "landing-sync".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);

        // Team spawn result -- agent is running via mailbox
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01TEAM".to_string(),
            agent_id: Some("landing-sync@release-team".to_string()),
            status: "teammate_spawned".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        // MUST stay Running, capture agent_id, no completion fields
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Running);
        assert_eq!(
            acc.sub_agents[0].agent_id.as_deref(),
            Some("landing-sync@release-team")
        );
        assert_eq!(acc.sub_agents[0].completed_at, None);
        assert_eq!(acc.sub_agents[0].duration_ms, None);
    }

    #[test]
    fn test_sub_agent_unknown_nonterminal_status_stays_running() {
        // Forward-compatibility: any unrecognized status that isn't a known
        // terminal ("completed"/"failed"/"killed") should keep the agent Running.
        // This prevents future protocol additions from breaking the UI.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01FUTURE".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "future-agent".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        for status in ["queued", "delegated", "pending_review", "some_new_status"] {
            let mut result_line = empty_line();
            result_line.line_type = LineType::User;
            result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
                tool_use_id: "toolu_01FUTURE".to_string(),
                agent_id: Some("afuture1".to_string()),
                status: status.to_string(),
                total_duration_ms: None,
                total_tool_use_count: None,
                usage_input_tokens: None,
                usage_output_tokens: None,
                usage_cache_read_tokens: None,
                usage_cache_creation_tokens: None,
                usage_cache_creation_5m_tokens: None,
                usage_cache_creation_1hr_tokens: None,
                model: None,
            });
            acc.process_line(&result_line, 0, &pricing);

            assert_eq!(
                acc.sub_agents[0].status,
                SubAgentStatus::Running,
                "status '{status}' should keep agent Running, not mark Error"
            );
        }
    }

    #[test]
    fn test_sub_agent_terminal_statuses_via_result() {
        // "completed" -> Complete, "failed"/"killed" -> Error
        // These are the ONLY statuses that should trigger state transitions
        // in the toolUseResult path (non-notification).
        let pricing = HashMap::new();

        for (status, expected) in [
            ("completed", SubAgentStatus::Complete),
            ("failed", SubAgentStatus::Error),
            ("killed", SubAgentStatus::Error),
        ] {
            let mut acc = SessionAccumulator::new();
            let tid = format!("toolu_{status}");

            let mut spawn_line = empty_line();
            spawn_line.line_type = LineType::Assistant;
            spawn_line
                .sub_agent_spawns
                .push(crate::live_parser::SubAgentSpawn {
                    tool_use_id: tid.clone(),
                    agent_type: "general-purpose".to_string(),
                    description: format!("{status}-agent"),
                    team_name: None,
                    model: None,
                });
            acc.process_line(&spawn_line, 0, &pricing);

            let mut result_line = empty_line();
            result_line.line_type = LineType::User;
            result_line.timestamp = Some("2026-02-20T10:01:00Z".to_string());
            result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
                tool_use_id: tid,
                agent_id: Some(format!("a{status}")),
                status: status.to_string(),
                total_duration_ms: Some(5000),
                total_tool_use_count: Some(3),
                usage_input_tokens: None,
                usage_output_tokens: None,
                usage_cache_read_tokens: None,
                usage_cache_creation_tokens: None,
                usage_cache_creation_5m_tokens: None,
                usage_cache_creation_1hr_tokens: None,
                model: None,
            });
            acc.process_line(&result_line, 0, &pricing);

            assert_eq!(
                acc.sub_agents[0].status, expected,
                "status '{status}' should map to {expected:?}"
            );
            // Terminal statuses should populate completion fields
            assert!(
                acc.sub_agents[0].completed_at.is_some(),
                "terminal status '{status}' should set completed_at"
            );
            assert_eq!(acc.sub_agents[0].duration_ms, Some(5000));
        }
    }

    #[test]
    fn test_sub_agent_notification_unknown_terminal_is_error() {
        // The notification path (task-notification) uses the same logic:
        // "completed" -> Complete, everything else -> Error.
        // Unknown terminal statuses in notifications should be Error (safe default).
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01NTERM".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "notif-agent".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        // Give it an agent_id via async_launched
        let mut launch_line = empty_line();
        launch_line.line_type = LineType::User;
        launch_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_01NTERM".to_string(),
            agent_id: Some("anterm1".to_string()),
            status: "async_launched".to_string(),
            total_duration_ms: None,
            total_tool_use_count: None,
            usage_input_tokens: None,
            usage_output_tokens: None,
            usage_cache_read_tokens: None,
            usage_cache_creation_tokens: None,
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&launch_line, 0, &pricing);

        // Notification with unknown terminal status
        let mut notif_line = empty_line();
        notif_line.line_type = LineType::User;
        notif_line.sub_agent_notification = Some(crate::live_parser::SubAgentNotification {
            agent_id: "anterm1".to_string(),
            status: "timed_out".to_string(),
        });
        acc.process_line(&notif_line, 0, &pricing);

        // Notifications are always terminal -- unknown = Error
        assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Error);
    }

    #[test]
    fn test_sub_agent_dedup() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01ABC".to_string(),
                agent_type: "Explore".to_string(),
                description: "Search".to_string(),
                team_name: None,
                model: None,
            });

        // Process same spawn twice (replay resilience)
        acc.process_line(&line, 0, &pricing);
        acc.process_line(&line, 0, &pricing);

        assert_eq!(acc.sub_agents.len(), 1);
    }

    #[test]
    fn test_sub_agent_progress() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01ABC".to_string(),
                agent_type: "Explore".to_string(),
                description: "Search".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        // Progress event
        let mut progress_line = empty_line();
        progress_line.line_type = LineType::Progress;
        progress_line.sub_agent_progress = Some(crate::live_parser::SubAgentProgress {
            parent_tool_use_id: "toolu_01ABC".to_string(),
            agent_id: "a951849".to_string(),
            current_tool: Some("Read".to_string()),
        });
        acc.process_line(&progress_line, 0, &pricing);

        assert_eq!(acc.sub_agents[0].agent_id.as_deref(), Some("a951849"));
        assert_eq!(acc.sub_agents[0].current_activity.as_deref(), Some("Read"));
    }

    #[test]
    fn test_todo_write_full_replacement() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTodoItem;

        // First TodoWrite
        let mut line1 = empty_line();
        line1.line_type = LineType::Assistant;
        line1.todo_write = Some(vec![
            RawTodoItem {
                content: "Step 1".to_string(),
                status: "completed".to_string(),
                active_form: String::new(),
            },
            RawTodoItem {
                content: "Step 2".to_string(),
                status: "pending".to_string(),
                active_form: "Working on step 2".to_string(),
            },
        ]);
        acc.process_line(&line1, 0, &pricing);
        assert_eq!(acc.todo_items.len(), 2);

        // Second TodoWrite replaces all
        let mut line2 = empty_line();
        line2.line_type = LineType::Assistant;
        line2.todo_write = Some(vec![RawTodoItem {
            content: "New plan".to_string(),
            status: "in_progress".to_string(),
            active_form: String::new(),
        }]);
        acc.process_line(&line2, 0, &pricing);
        assert_eq!(acc.todo_items.len(), 1);
        assert_eq!(acc.todo_items[0].title, "New plan");
        assert_eq!(acc.todo_items[0].status, ProgressStatus::InProgress);
    }

    #[test]
    fn test_task_create_and_update() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate};

        // TaskCreate
        let mut create_line = empty_line();
        create_line.line_type = LineType::Assistant;
        create_line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_task1".to_string(),
            subject: "Write tests".to_string(),
            description: "Add unit tests".to_string(),
            active_form: "Writing tests".to_string(),
        });
        acc.process_line(&create_line, 0, &pricing);
        assert_eq!(acc.task_items.len(), 1);
        assert!(acc.task_items[0].id.is_none()); // no ID yet

        // TaskIdAssignment
        let mut assign_line = empty_line();
        assign_line.line_type = LineType::User;
        assign_line.task_id_assignments.push(RawTaskIdAssignment {
            tool_use_id: "toolu_task1".to_string(),
            task_id: "1".to_string(),
        });
        acc.process_line(&assign_line, 0, &pricing);
        assert_eq!(acc.task_items[0].id.as_deref(), Some("1"));

        // TaskUpdate
        let mut update_line = empty_line();
        update_line.line_type = LineType::Assistant;
        update_line.task_updates.push(RawTaskUpdate {
            task_id: "1".to_string(),
            status: Some("completed".to_string()),
            subject: None,
            active_form: None,
        });
        acc.process_line(&update_line, 0, &pricing);
        assert_eq!(acc.task_items[0].status, ProgressStatus::Completed);
    }

    #[test]
    fn test_task_create_dedup() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTaskCreate;

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_task1".to_string(),
            subject: "Task 1".to_string(),
            description: "".to_string(),
            active_form: "".to_string(),
        });

        acc.process_line(&line, 0, &pricing);
        acc.process_line(&line, 0, &pricing);

        assert_eq!(acc.task_items.len(), 1);
    }

    #[test]
    fn test_finish_combines_todo_and_task_items() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        use crate::progress::RawTaskCreate;
        use crate::progress::RawTodoItem;

        // Add a todo
        let mut todo_line = empty_line();
        todo_line.line_type = LineType::Assistant;
        todo_line.todo_write = Some(vec![RawTodoItem {
            content: "Todo item".to_string(),
            status: "pending".to_string(),
            active_form: String::new(),
        }]);
        acc.process_line(&todo_line, 0, &pricing);

        // Add a task
        let mut task_line = empty_line();
        task_line.line_type = LineType::Assistant;
        task_line.task_creates.push(RawTaskCreate {
            tool_use_id: "toolu_1".to_string(),
            subject: "Task item".to_string(),
            description: "".to_string(),
            active_form: "".to_string(),
        });
        acc.process_line(&task_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.progress_items.len(), 2);
        assert_eq!(data.progress_items[0].source, ProgressSource::Todo);
        assert_eq!(data.progress_items[1].source, ProgressSource::Task);
    }

    #[test]
    fn test_cost_calculation_with_known_model() {
        let mut acc = SessionAccumulator::new();
        let pricing = crate::pricing::load_pricing();

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.model = Some("claude-sonnet-4-6".to_string());
        line.input_tokens = Some(1_000_000);
        line.output_tokens = Some(100_000);
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert!(!data.cost.has_unpriced_usage);
        assert_eq!(data.cost.priced_token_coverage, 1.0);
        assert!(data.cost.total_usd > 0.0);
    }

    #[test]
    fn test_cost_calculation_with_unknown_model() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new(); // no pricing data

        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.model = Some("unknown-model-xyz".to_string());
        line.input_tokens = Some(1000);
        line.output_tokens = Some(500);
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert!(data.cost.has_unpriced_usage);
        assert_eq!(data.cost.total_usd, 0.0);
        assert_eq!(data.cost.unpriced_input_tokens, 1000);
        assert_eq!(data.cost.unpriced_output_tokens, 500);
        assert_eq!(data.cost.priced_token_coverage, 0.0);
    }

    #[test]
    fn test_cache_status_unknown_no_activity() {
        let acc = SessionAccumulator::new();
        let pricing = HashMap::new();
        let data = acc.finish(&pricing);
        assert_eq!(data.cache_status, CacheStatus::Unknown);
    }

    #[test]
    fn test_cache_status_cold_old_timestamp() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Use a timestamp far in the past
        let mut line = empty_line();
        line.cache_read_tokens = Some(100);
        line.timestamp = Some("2020-01-01T00:00:00Z".to_string());
        acc.process_line(&line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.cache_status, CacheStatus::Cold);
    }

    #[test]
    fn test_parse_timestamp_to_unix_rfc3339() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00Z");
        assert!(ts.is_some());
        assert!(ts.unwrap() > 0);
    }

    #[test]
    fn test_parse_timestamp_to_unix_with_offset() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00+08:00");
        assert!(ts.is_some());
    }

    #[test]
    fn test_parse_timestamp_to_unix_fallback_format() {
        let ts = parse_timestamp_to_unix("2026-02-20T10:30:00");
        assert!(ts.is_some());
    }

    #[test]
    fn test_parse_timestamp_to_unix_invalid() {
        let ts = parse_timestamp_to_unix("not-a-timestamp");
        assert!(ts.is_none());
    }

    #[test]
    fn test_derive_cache_status_none() {
        assert_eq!(derive_cache_status(None), CacheStatus::Unknown);
    }

    #[test]
    fn test_derive_cache_status_recent() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 30 seconds ago = warm
        assert_eq!(derive_cache_status(Some(now - 30)), CacheStatus::Warm);
    }

    #[test]
    fn test_derive_cache_status_old() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // 10 minutes ago = cold
        assert_eq!(derive_cache_status(Some(now - 600)), CacheStatus::Cold);
    }

    #[test]
    fn test_from_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::File::create(&path).unwrap();

        let pricing = HashMap::new();
        let data = SessionAccumulator::from_file(&path, &pricing).unwrap();
        assert_eq!(data.tokens.total_tokens, 0);
        assert_eq!(data.turn_count, 0);
    }

    #[test]
    fn test_from_file_with_content() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello"}},"gitBranch":"main","timestamp":"2026-02-20T10:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"Hi there","model":"claude-sonnet-4-6","usage":{{"input_tokens":1000,"output_tokens":200}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let pricing = crate::pricing::load_pricing();
        let data = SessionAccumulator::from_file(&path, &pricing).unwrap();
        assert_eq!(data.turn_count, 1);
        assert_eq!(data.first_user_message.as_deref(), Some("Hello"));
        assert_eq!(data.git_branch.as_deref(), Some("main"));
        assert_eq!(data.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(data.tokens.input_tokens, 1000);
        assert_eq!(data.tokens.output_tokens, 200);
        assert!(!data.cost.has_unpriced_usage);
    }

    #[test]
    fn test_sub_agent_cost_calculation() {
        let mut acc = SessionAccumulator::new();
        let pricing = crate::pricing::load_pricing();

        // Set model first
        let mut model_line = empty_line();
        model_line.model = Some("claude-sonnet-4-6".to_string());
        acc.process_line(&model_line, 0, &pricing);

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_cost".to_string(),
                agent_type: "code".to_string(),
                description: "Write code".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        // Complete with token usage
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_cost".to_string(),
            agent_id: Some("abc1234".to_string()),
            status: "completed".to_string(),
            total_duration_ms: Some(5000),
            total_tool_use_count: Some(3),
            usage_input_tokens: Some(100_000),
            usage_output_tokens: Some(10_000),
            usage_cache_read_tokens: Some(50_000),
            usage_cache_creation_tokens: Some(5_000),
            usage_cache_creation_5m_tokens: None,
            usage_cache_creation_1hr_tokens: None,
            model: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.sub_agents.len(), 1);
        assert!(
            data.sub_agents[0].cost_usd.unwrap() > 0.0,
            "Sub-agent should have a non-zero cost"
        );
    }

    #[test]
    fn test_sub_agent_1hr_cache_cost_uses_1hr_rate() {
        // Regression test: pre-fix, sub-agent cache_creation tokens with 1hr
        // TTL were charged at the 5m rate because usage_cache_creation_5m_tokens
        // and usage_cache_creation_1hr_tokens were hardcoded to 0 at the
        // cost-calculation call site. This test locks in the fix.
        let mut acc = SessionAccumulator::new();
        let pricing = crate::pricing::load_pricing();

        // Set model to opus-4-6 (has distinct 5m/1h rates: $6.25 vs $10 per MTok)
        let mut model_line = empty_line();
        model_line.model = Some("claude-opus-4-6".to_string());
        acc.process_line(&model_line, 0, &pricing);

        // Spawn
        let mut spawn_line = empty_line();
        spawn_line.line_type = LineType::Assistant;
        spawn_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_1hr".to_string(),
                agent_type: "code".to_string(),
                description: "1hr cache test".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&spawn_line, 0, &pricing);

        // Complete: 100k cache_creation tokens, ALL on 1hr TTL
        let mut result_line = empty_line();
        result_line.line_type = LineType::User;
        result_line.sub_agent_result = Some(crate::live_parser::SubAgentResult {
            tool_use_id: "toolu_1hr".to_string(),
            agent_id: Some("abc1234".to_string()),
            status: "completed".to_string(),
            total_duration_ms: Some(5000),
            total_tool_use_count: Some(1),
            usage_input_tokens: Some(0),
            usage_output_tokens: Some(0),
            usage_cache_read_tokens: Some(0),
            usage_cache_creation_tokens: Some(100_000),
            usage_cache_creation_5m_tokens: Some(0),
            usage_cache_creation_1hr_tokens: Some(100_000),
            model: None,
        });
        acc.process_line(&result_line, 0, &pricing);

        let data = acc.finish(&pricing);
        assert_eq!(data.sub_agents.len(), 1);
        let cost = data.sub_agents[0].cost_usd.unwrap();
        // 100k at opus-4-6 1hr rate ($10/MTok) = $1.00
        // Pre-fix (5m rate $6.25/MTok) would have been $0.625
        assert!(
            (cost - 1.0).abs() < 0.001,
            "expected $1.00 (1hr rate), got ${}",
            cost
        );
        assert!(
            cost > 0.8,
            "sub-agent 1hr rate must be applied, got ${}",
            cost
        );
    }

    #[test]
    fn test_team_name_from_top_level_jsonl_field() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Real data: after TeamCreate, every line has top-level `teamName`.
        // The parser extracts it into LiveLine.team_name.
        // Team member spawns carry `team_name` on the SubAgentSpawn from input.team_name.
        let mut line = empty_line();
        line.line_type = LineType::Assistant;
        line.team_name = Some("demo-team".to_string());
        // Agent spawn with team_name -- this is a team member, NOT a sub-agent
        line.sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01AG".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "agent-sysinfo".to_string(),
                team_name: Some("demo-team".to_string()),
                model: None,
            });
        acc.process_line(&line, 0, &pricing);

        // team_name captured from top-level field
        assert_eq!(acc.team_name.as_deref(), Some("demo-team"));
        // Team spawns must NOT be added to sub_agents -- their lifecycle is
        // managed by ~/.claude/teams/, not the JSONL sub-agent tracking system.
        assert_eq!(acc.sub_agents.len(), 0);
    }

    #[test]
    fn test_team_spawn_skipped_but_regular_spawn_kept_in_same_session() {
        // A team lead can spawn BOTH team members (with team_name) and regular
        // sub-agents (without team_name). Only team members should be skipped.
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Line 1: team member spawn (should be SKIPPED)
        let mut team_line = empty_line();
        team_line.line_type = LineType::Assistant;
        team_line.team_name = Some("nvda-demo".to_string());
        team_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_TEAM1".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "researcher".to_string(),
                team_name: Some("nvda-demo".to_string()),
                model: None,
            });
        acc.process_line(&team_line, 0, &pricing);

        // Line 2: another team member (should be SKIPPED)
        let mut team_line2 = empty_line();
        team_line2.line_type = LineType::Assistant;
        team_line2.team_name = Some("nvda-demo".to_string());
        team_line2
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_TEAM2".to_string(),
                agent_type: "general-purpose".to_string(),
                description: "analyst".to_string(),
                team_name: Some("nvda-demo".to_string()),
                model: None,
            });
        acc.process_line(&team_line2, 0, &pricing);

        // Line 3: regular sub-agent in the same session (should be KEPT)
        let mut regular_line = empty_line();
        regular_line.line_type = LineType::Assistant;
        regular_line.team_name = Some("nvda-demo".to_string()); // top-level still present
        regular_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_REG1".to_string(),
                agent_type: "Explore".to_string(),
                description: "Search codebase".to_string(),
                team_name: None, // no team_name on spawn = regular sub-agent
                model: None,
            });
        acc.process_line(&regular_line, 0, &pricing);

        // Verify: 2 team members skipped, 1 regular kept
        assert_eq!(acc.team_name.as_deref(), Some("nvda-demo"));
        assert_eq!(acc.sub_agents.len(), 1);
        assert_eq!(acc.sub_agents[0].tool_use_id, "toolu_REG1");
        assert_eq!(acc.sub_agents[0].description, "Search codebase");
    }

    #[test]
    fn test_regular_spawn_without_team() {
        let mut acc = SessionAccumulator::new();
        let pricing = HashMap::new();

        // Regular spawn: no team context (no top-level teamName)
        let mut reg_line = empty_line();
        reg_line.line_type = LineType::Assistant;
        reg_line
            .sub_agent_spawns
            .push(crate::live_parser::SubAgentSpawn {
                tool_use_id: "toolu_01REG".to_string(),
                agent_type: "Explore".to_string(),
                description: "Search codebase".to_string(),
                team_name: None,
                model: None,
            });
        acc.process_line(&reg_line, 0, &pricing);

        assert_eq!(acc.sub_agents.len(), 1);
        assert_eq!(acc.team_name, None);
    }
}
