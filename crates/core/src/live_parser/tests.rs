#[cfg(test)]
mod tests {
    use crate::live_parser::content::{strip_noise_tags, truncate_str};
    use crate::live_parser::finders::TailFinders;
    use crate::live_parser::parse_line::parse_single_line;
    use crate::live_parser::sub_agents::parse_tool_use_result_payload;
    use crate::live_parser::tail_io::parse_tail;
    use crate::live_parser::types::{LineType, PASTED_PATH_PATTERN};

    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_parse_tail_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        File::create(&path).unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_parse_tail_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("single.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello world"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, LineType::User);
        assert_eq!(lines[0].role.as_deref(), Some("user"));
        assert_eq!(lines[0].content_preview, "Hello world");
        assert!(offset > 0);
    }

    #[test]
    fn test_parse_tail_partial_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.jsonl");
        let mut f = File::create(&path).unwrap();
        // Write without trailing newline — simulates a partial write in progress
        write!(f, r#"{{"role":"user","content":"partial"#).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_parse_tail_resets_on_file_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        // Write initial content
        {
            let mut f = File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
            )
            .unwrap();
            writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        }

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(!lines.is_empty());
        assert!(offset > 0);

        // "Replace" the file with smaller content (simulates log rotation)
        {
            let mut f = File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"type":"user","message":{{"role":"user","content":"new session"}}}}"#
            )
            .unwrap();
        }

        // Old offset is larger than new file — should reset and read from start
        let (lines2, offset2) = parse_tail(&path, offset, &finders).unwrap();
        assert!(
            !lines2.is_empty(),
            "Should read new content after file replacement"
        );
        assert!(offset2 > 0);
    }

    #[test]
    fn test_parse_tail_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("incremental.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"first"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();

        // First read
        let (lines1, offset1) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines1.len(), 1);
        assert_eq!(lines1[0].content_preview, "first");

        // Append a second line
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"second"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        // Second read from previous offset
        let (lines2, offset2) = parse_tail(&path, offset1, &finders).unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0].content_preview, "second");
        assert_eq!(lines2[0].line_type, LineType::Assistant);
        assert!(offset2 > offset1);
    }

    #[test]
    fn test_parse_tail_extracts_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}},"stop_reason":"end_turn"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(line.input_tokens, Some(1000));
        assert_eq!(line.output_tokens, Some(500));
        assert_eq!(line.cache_read_tokens, Some(200));
        assert_eq!(line.cache_creation_tokens, Some(50));
        assert_eq!(line.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_parse_tail_content_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blocks.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"role":"assistant","content":[{{"type":"text","text":"Hello"}},{{"type":"tool_use","name":"bash","id":"123","input":{{}}}}]}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.content_preview, "Hello");
        assert_eq!(line.tool_names, vec!["bash"]);
    }

    #[test]
    fn test_parse_tail_nested_message_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","model":"claude-opus-4-6","content":[{{"type":"text","text":"Hello"}}],"usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}},"stop_reason":"end_turn"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.line_type, LineType::Assistant);
        assert_eq!(line.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(line.input_tokens, Some(1000));
        assert_eq!(line.output_tokens, Some(500));
        assert_eq!(line.cache_read_tokens, Some(200));
        assert_eq!(line.cache_creation_tokens, Some(50));
        assert_eq!(line.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(line.content_preview, "Hello");
    }

    #[test]
    fn test_parse_tail_nested_user_with_git_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user_nested.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Fix the bug"}},"gitBranch":"feature/auth","isMeta":false,"timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.line_type, LineType::User);
        assert_eq!(line.content_preview, "Fix the bug");
        assert_eq!(line.git_branch.as_deref(), Some("feature/auth"));
        assert!(!line.is_meta);
    }

    #[test]
    fn test_parse_cwd_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cwd.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"hello"}},"cwd":"/Users/u/dev/repo/.worktrees/feat","gitBranch":"main","timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].cwd.as_deref(),
            Some("/Users/u/dev/repo/.worktrees/feat")
        );
        assert_eq!(lines[0].git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_parse_tail_meta_message_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"system prompt stuff"}},"isMeta":true,"timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_meta);
    }

    #[test]
    fn test_truncation() {
        let long = "a".repeat(250);
        let result = truncate_str(&long, 200);
        // 200 chars + "..." = 203 chars
        assert_eq!(result.len(), 203);
        assert!(result.ends_with("..."));
    }

    // -------------------------------------------------------------------------
    // Turn detection: is_tool_result_continuation
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_result_continuation_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tool_result.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_123","content":"file contents here"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = true for content with tool_result block"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_text_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_only.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Hello world"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for text-only content"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_text_and_tool_use() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_tool_use.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Let me check"}},{{"type":"tool_use","name":"bash","id":"123","input":{{}}}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for text + tool_use content"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_string_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("string_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Just a plain message"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for string content"
        );
    }

    // -------------------------------------------------------------------------
    // Turn detection: has_system_prefix
    // -------------------------------------------------------------------------

    #[test]
    fn test_system_prefix_local_command_caveat() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("caveat.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<local-command-caveat>some caveat text</local-command-caveat>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <local-command-caveat>"
        );
    }

    #[test]
    fn test_system_prefix_local_command_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stdout.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<local-command-stdout>ls -la output</local-command-stdout>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <local-command-stdout>"
        );
    }

    #[test]
    fn test_system_prefix_command_name_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clear.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<command-name>/clear</command-name>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <command-name>/clear"
        );
    }

    #[test]
    fn test_system_prefix_command_name_context() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("context.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<command-name>/context add file.rs</command-name>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <command-name>/context"
        );
    }

    #[test]
    fn test_system_prefix_session_continuation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("continuation.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"This session is being continued from a previous conversation."}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for session continuation marker"
        );
    }

    #[test]
    fn test_system_prefix_task_notification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task_notif.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>Task completed</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <task-notification>"
        );
    }

    #[test]
    fn test_task_notification_extracts_agent_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_completed.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>\n<task-id>ab897bc</task-id>\n<status>completed</status>\n<summary>Agent done</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].has_system_prefix);
        let notif = lines[0].sub_agent_notification.as_ref().unwrap();
        assert_eq!(notif.agent_id, "ab897bc");
        assert_eq!(notif.status, "completed");
    }

    #[test]
    fn test_task_notification_failed_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_failed.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>\n<task-id>afailed1</task-id>\n<status>failed</status>\n<summary>Agent errored</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let notif = lines[0].sub_agent_notification.as_ref().unwrap();
        assert_eq!(notif.agent_id, "afailed1");
        assert_eq!(notif.status, "failed");
    }

    #[test]
    fn test_task_notification_deep_in_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_deep.jsonl");
        let mut f = File::create(&path).unwrap();
        let prefix = "x".repeat(400);
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"{prefix}<task-notification>\n<task-id>adeep01</task-id>\n<status>completed</status>\n<summary>Deep agent done</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let notif = lines[0]
            .sub_agent_notification
            .as_ref()
            .expect("notification must be extracted even past 200-char preview limit");
        assert_eq!(notif.agent_id, "adeep01");
        assert_eq!(notif.status, "completed");
    }

    #[test]
    fn test_task_notification_in_content_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_array.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"<task-notification>\n<task-id>aarray1</task-id>\n<status>killed</status>\n<summary>Killed agent</summary>\n</task-notification>"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let notif = lines[0]
            .sub_agent_notification
            .as_ref()
            .expect("notification must be extracted from content array");
        assert_eq!(notif.agent_id, "aarray1");
        assert_eq!(notif.status, "killed");
    }

    #[test]
    fn test_task_notification_not_on_regular_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("normal.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello world"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines[0].sub_agent_notification.is_none());
    }

    #[test]
    fn test_system_prefix_false_normal_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("normal_user.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Fix the bug in auth.rs"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].has_system_prefix,
            "Expected has_system_prefix = false for normal user message"
        );
    }

    #[test]
    fn test_system_prefix_false_for_assistant_messages() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assistant_with_prefix.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"<local-command-caveat>this text happens to start like a prefix"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].has_system_prefix,
            "Expected has_system_prefix = false for assistant messages even with prefix-like content"
        );
    }

    // -------------------------------------------------------------------------
    // Turn detection: edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_turn_detection_empty_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":""}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
    }

    #[test]
    fn test_turn_detection_missing_content_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"user","message":{{"role":"user"}}}}"#).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
        assert_eq!(lines[0].content_preview, "");
    }

    #[test]
    fn test_turn_detection_empty_content_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_array.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
    }

    // -------------------------------------------------------------------------
    // Sub-agent progress detection
    // -------------------------------------------------------------------------

    #[test]
    fn test_progress_event_agent_activity() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","toolUseID":"agent_msg_01","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);

        assert!(line.sub_agent_progress.is_some());
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.parent_tool_use_id, "toolu_01ABC");
        assert_eq!(progress.agent_id, "a951849");
        assert_eq!(progress.current_tool, Some("Read".to_string()));
    }

    #[test]
    fn test_progress_event_no_tool_use() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"text","text":"Let me think..."}]}}}"#;
        let line = parse_single_line(raw, &finders);
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.current_tool, None);
    }

    #[test]
    fn test_progress_event_non_agent_type() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","data":{"type":"tool_progress","tool":"Bash"}}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none());
    }

    #[test]
    fn test_progress_event_missing_agent_id() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","message":{"role":"assistant","content":[]}}}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none());
    }

    #[test]
    fn test_progress_event_multiple_tool_uses() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{}},{"type":"text","text":"..."},{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
        let line = parse_single_line(raw, &finders);
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.current_tool, Some("Grep".to_string()));
    }

    #[test]
    fn test_simd_prefilter_skips_non_progress() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here is the result."}]},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none());
    }

    #[test]
    fn test_progress_line_classified_as_progress_not_assistant() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::Progress,
            "Progress lines must be classified as Progress, not Assistant"
        );
    }

    #[test]
    fn test_parse_tail_extracts_ephemeral_cache_breakdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ephemeral.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":57339,"cache_creation":{{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":57339}}}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.cache_creation_tokens, Some(57339));
        assert_eq!(line.cache_creation_5m_tokens, Some(0));
        assert_eq!(line.cache_creation_1hr_tokens, Some(57339));
    }

    #[test]
    fn test_parse_tail_no_ephemeral_breakdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_ephemeral.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"hi","usage":{{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":1000}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let line = &lines[0];
        assert_eq!(line.cache_creation_tokens, Some(1000));
        assert_eq!(line.cache_creation_5m_tokens, None);
        assert_eq!(line.cache_creation_1hr_tokens, None);
    }

    #[test]
    fn test_tool_use_result_not_classified_as_result() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01ABC","content":"done"}]},"toolUseResult":{"status":"completed"}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::User,
            "toolUseResult lines must remain User, not Result"
        );
    }

    #[test]
    fn test_spawn_and_progress_same_agent() {
        let finders = TailFinders::new();

        let spawn_raw = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01ABC","name":"Task","input":{"description":"Search auth","subagent_type":"Explore"}}]},"timestamp":"2026-02-16T08:34:00.000Z"}"#;
        let spawn_line = parse_single_line(spawn_raw, &finders);
        assert_eq!(spawn_line.sub_agent_spawns.len(), 1);
        assert!(spawn_line.sub_agent_progress.is_none());

        let progress_raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
        let progress_line = parse_single_line(progress_raw, &finders);
        assert!(progress_line.sub_agent_spawns.is_empty());
        assert!(progress_line.sub_agent_progress.is_some());
    }

    // -------------------------------------------------------------------------
    // Skill name extraction from Skill tool_use
    // -------------------------------------------------------------------------

    #[test]
    fn test_skill_name_extracted_from_skill_tool_use() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me commit."},{"type":"tool_use","id":"toolu_01XYZ","name":"Skill","input":{"skill":"commit","args":"-m 'fix bug'"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert_eq!(line.skill_names, vec!["commit"]);
    }

    #[test]
    fn test_multiple_skill_invocations() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{"skill":"commit"}},{"type":"tool_use","id":"toolu_01B","name":"Skill","input":{"skill":"review-pr","args":"123"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill", "Skill"]);
        assert_eq!(line.skill_names, vec!["commit", "review-pr"]);
    }

    #[test]
    fn test_skill_name_empty_ignored() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{"skill":""}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert!(
            line.skill_names.is_empty(),
            "Empty skill name should be ignored"
        );
    }

    #[test]
    fn test_skill_name_missing_input() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert!(
            line.skill_names.is_empty(),
            "Missing skill field should produce no skill_names"
        );
    }

    #[test]
    fn test_non_skill_tool_use_no_skill_names() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Bash","input":{"command":"ls"}},{"type":"tool_use","id":"toolu_01B","name":"mcp__plugin_playwright_playwright__browser_navigate","input":{"url":"http://example.com"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.tool_names,
            vec![
                "Bash",
                "mcp__plugin_playwright_playwright__browser_navigate"
            ]
        );
        assert!(
            line.skill_names.is_empty(),
            "Non-Skill tools should produce no skill_names"
        );
    }

    #[test]
    fn test_mixed_skill_and_mcp_tools() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"mcp__chrome-devtools__take_screenshot","input":{}},{"type":"tool_use","id":"toolu_01B","name":"Skill","input":{"skill":"pdf"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.tool_names,
            vec!["mcp__chrome-devtools__take_screenshot", "Skill"]
        );
        assert_eq!(line.skill_names, vec!["pdf"]);
    }

    // -------------------------------------------------------------------------
    // strip_noise_tags
    // -------------------------------------------------------------------------

    #[test]
    fn test_strip_noise_tags_system_reminder() {
        let input = "<system-reminder>SessionStart:startup hook success: Success</system-reminder>fix the bug";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "fix the bug");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_ide_opened_file() {
        let input = "<ide_opened_file>The user opened the file /Users/me/project/src/auth.rs in the IDE. This may or may not be related to the current task.</ide_opened_file> continue";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "continue");
        assert_eq!(file.as_deref(), Some("auth.rs"));
    }

    #[test]
    fn test_strip_noise_tags_multiple_tags() {
        let input = "<system-reminder>hook data</system-reminder><ide_opened_file>The user opened the file /path/to/main.rs in the IDE.</ide_opened_file><ide_selection>some code</ide_selection> do the thing";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "do the thing");
        assert_eq!(file.as_deref(), Some("main.rs"));
    }

    #[test]
    fn test_strip_noise_tags_no_tags() {
        let input = "just a normal message";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "just a normal message");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_only_tags() {
        let input = "<system-reminder>hook stuff</system-reminder>";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_command_tags() {
        let input = "<command-name>/clear</command-name><command-message>Clearing context</command-message> hello";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "hello");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_nested_path() {
        let input = "<ide_opened_file>The user opened the file /deep/nested/path/to/component.tsx in the IDE.</ide_opened_file>review this";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "review this");
        assert_eq!(file.as_deref(), Some("component.tsx"));
    }

    #[test]
    fn test_strip_noise_tags_user_prompt_submit_hook() {
        let input = "<user-prompt-submit-hook>hook output</user-prompt-submit-hook>fix tests";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "fix tests");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_whitespace_between_text() {
        let input = "hello <system-reminder>hook data</system-reminder> world";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "hello world");
        assert!(file.is_none());
    }

    #[test]
    fn test_parse_tail_strips_noise_tags_from_preview() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tags.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<system-reminder>hook data</system-reminder>fix the bug"}}}}"#
        ).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content_preview, "fix the bug");
        assert!(lines[0].ide_file.is_none());
    }

    #[test]
    fn test_parse_tail_extracts_ide_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ide.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<ide_opened_file>The user opened the file /src/auth.rs in the IDE.</ide_opened_file> continue"}}}}"#
        ).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content_preview, "continue");
        assert_eq!(lines[0].ide_file.as_deref(), Some("auth.rs"));
    }

    #[test]
    fn test_hook_progress_pre_tool_use() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"PreToolUse:Read","hookId":"h1","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.line_type, LineType::Progress);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "PreToolUse");
        assert_eq!(hp.tool_name, Some("Read".to_string()));
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_hook_progress_session_start_compact() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"SessionStart","hookName":"SessionStart:compact","hookId":"h2","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "SessionStart");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, Some("compact".to_string()));
    }

    #[test]
    fn test_hook_progress_stop_no_colon() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"Stop","hookName":"Stop","hookId":"h3","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "Stop");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_hook_progress_malformed_json() {
        let line = br#"{"type":"progress","data":{"type":"hook_progress","broken"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert!(
            result.hook_progress.is_none(),
            "Malformed JSON should be None"
        );
    }

    #[test]
    fn test_sub_agent_spawn_agent_tool() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ABC","name":"Agent","input":{"name":"Gate 1: Code Quality","description":"General code review","subagent_type":"code-reviewer","prompt":"Review code","run_in_background":true}}]},"timestamp":"2026-03-08T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_01ABC");
        assert_eq!(spawn.agent_type, "code-reviewer");
        assert_eq!(spawn.description, "Gate 1: Code Quality");
    }

    #[test]
    fn test_sub_agent_spawn_legacy_task_tool() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_02DEF","name":"Task","input":{"description":"Search codebase","subagent_type":"Explore"}}]},"timestamp":"2026-02-20T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_02DEF");
        assert_eq!(spawn.agent_type, "Explore");
        assert_eq!(spawn.description, "Search codebase");
    }

    #[test]
    fn test_sub_agent_spawn_agent_without_name_field() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_03GHI","name":"Agent","input":{"description":"Audit the codebase","prompt":"Do audit"}}]},"timestamp":"2026-03-08T11:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_03GHI");
        assert_eq!(spawn.agent_type, "Agent");
        assert_eq!(spawn.description, "Audit the codebase");
    }

    #[test]
    fn test_hook_progress_empty_hook_name() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"","hookId":"h4","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "PreToolUse");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_slug_extraction() {
        let finders = TailFinders::new();
        let line = br#"{"type":"user","slug":"async-greeting-dewdrop","message":{"role":"user","content":"hello"}}"#;
        let parsed = parse_single_line(line, &finders);
        assert_eq!(parsed.slug.as_deref(), Some("async-greeting-dewdrop"));
    }

    #[test]
    fn test_slug_missing() {
        let finders = TailFinders::new();
        let line = br#"{"type":"user","message":{"role":"user","content":"hello"}}"#;
        let parsed = parse_single_line(line, &finders);
        assert!(parsed.slug.is_none());
    }

    // -------------------------------------------------------------------------
    // Team name extraction from top-level `teamName` JSONL field
    // -------------------------------------------------------------------------

    #[test]
    fn test_team_name_from_top_level_field() {
        let line = br#"{"type":"assistant","teamName":"demo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01AG","name":"Agent","input":{"name":"agent-sysinfo","description":"System info agent","prompt":"..."}}]},"timestamp":"2026-03-11T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.team_name.as_deref(), Some("demo-team"));
        assert_eq!(result.sub_agent_spawns.len(), 1);
        assert!(result.sub_agent_spawns[0].team_name.is_none());
    }

    #[test]
    fn test_spawn_with_input_team_name() {
        let line = br#"{"type":"assistant","teamName":"nvda-demo","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_015Y","name":"Agent","input":{"description":"NVDA stock researcher","name":"researcher","team_name":"nvda-demo","subagent_type":"general-purpose","prompt":"...","run_in_background":true}}]},"timestamp":"2026-03-10T19:04:44Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.team_name.as_deref(), Some("nvda-demo"));
        assert_eq!(result.sub_agent_spawns.len(), 1);
        assert_eq!(
            result.sub_agent_spawns[0].team_name.as_deref(),
            Some("nvda-demo")
        );
    }

    #[test]
    fn test_no_team_name_without_top_level_field() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01REG","name":"Agent","input":{"name":"Search auth","description":"Search auth code","subagent_type":"Explore"}}]},"timestamp":"2026-03-08T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        assert!(result.team_name.is_none());
        assert!(result.sub_agent_spawns[0].team_name.is_none());
    }

    #[test]
    fn pasted_path_regex_matches_absolute_paths() {
        let re = regex_lite::Regex::new(PASTED_PATH_PATTERN).unwrap();
        let cases = vec![
            (
                "look at /Users/dev/project/src/auth.rs",
                Some("/Users/dev/project/src/auth.rs"),
            ),
            ("/etc/hosts is the file", Some("/etc/hosts")),
            ("check /tmp/test.txt, please", Some("/tmp/test.txt")),
            ("no path here", None),
            ("relative/path.rs not matched", None),
            ("just a /directory/ not matched", None),
        ];
        for (input, expected) in cases {
            let found = re
                .captures(input)
                .map(|c| c.get(1).unwrap().as_str().to_string());
            assert_eq!(found.as_deref(), expected, "input: {input}");
        }
    }

    #[test]
    fn pasted_path_skips_urls() {
        let line = "see https://github.com/foo/bar.rs for details";
        assert!(line.contains("://"), "pre-filter should skip this line");
    }

    #[test]
    fn pasted_path_regex_compiles_with_regex_lite() {
        let re = regex_lite::Regex::new(PASTED_PATH_PATTERN);
        assert!(
            re.is_ok(),
            "regex must compile with regex-lite (no lookbehinds, no Unicode classes)"
        );
    }

    #[test]
    fn parse_tool_use_result_payload_extracts_ephemeral_5m_and_1h() {
        let tur = serde_json::json!({
            "status": "completed",
            "agentId": "abc123",
            "totalDurationMs": 5000,
            "totalToolUseCount": 3,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 16416,
                "cache_creation_input_tokens": 26109,
                "cache_creation": {
                    "ephemeral_5m_input_tokens": 0,
                    "ephemeral_1h_input_tokens": 26109
                }
            },
            "model": "claude-opus-4-6"
        });
        let parsed = parse_tool_use_result_payload(&tur).unwrap();
        assert_eq!(parsed.usage_input_tokens, Some(100));
        assert_eq!(parsed.usage_cache_creation_tokens, Some(26109));
        assert_eq!(parsed.usage_cache_creation_5m_tokens, Some(0));
        assert_eq!(parsed.usage_cache_creation_1hr_tokens, Some(26109));
    }

    #[test]
    fn parse_tool_use_result_payload_without_ephemeral_breakdown() {
        let tur = serde_json::json!({
            "status": "completed",
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 500
            }
        });
        let parsed = parse_tool_use_result_payload(&tur).unwrap();
        assert_eq!(parsed.usage_cache_creation_tokens, Some(500));
        assert_eq!(parsed.usage_cache_creation_5m_tokens, None);
        assert_eq!(parsed.usage_cache_creation_1hr_tokens, None);
    }

    #[test]
    fn parse_tool_use_result_payload_with_5m_only_caching() {
        let tur = serde_json::json!({
            "status": "completed",
            "usage": {
                "cache_creation_input_tokens": 1000,
                "cache_creation": {
                    "ephemeral_5m_input_tokens": 1000,
                    "ephemeral_1h_input_tokens": 0
                }
            }
        });
        let parsed = parse_tool_use_result_payload(&tur).unwrap();
        assert_eq!(parsed.usage_cache_creation_5m_tokens, Some(1000));
        assert_eq!(parsed.usage_cache_creation_1hr_tokens, Some(0));
    }
}
