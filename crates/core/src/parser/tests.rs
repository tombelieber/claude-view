// crates/core/src/parser/tests.rs
//! Integration and fixture-based tests for the JSONL session parser.

#[cfg(test)]
mod tests {
    use crate::parser::{parse_session, parse_session_paginated, parse_session_with_raw};
    use crate::types::Role;
    use std::path::PathBuf;

    fn fixtures_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    // ============================================================================
    // Happy Path Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_simple_session() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 4);
        assert_eq!(session.metadata.total_messages, 4);
        assert_eq!(session.turn_count(), 2);
    }

    #[tokio::test]
    async fn test_parse_simple_session_message_content() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[0].content, "Hello, Claude!");
        assert_eq!(session.messages[1].role, Role::Assistant);
        assert_eq!(
            session.messages[1].content,
            "Hello! How can I help you today?"
        );
    }

    #[tokio::test]
    async fn test_parse_session_preserves_timestamps() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(
            session.messages[0].timestamp,
            Some("2026-01-27T10:00:00Z".to_string())
        );
        assert_eq!(
            session.messages[1].timestamp,
            Some("2026-01-27T10:00:01Z".to_string())
        );
    }

    // ============================================================================
    // Tool Calls Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_with_tool_calls() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 6);
        assert!(session.metadata.tool_call_count > 0);
    }

    #[tokio::test]
    async fn test_tool_calls_aggregation() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Second message (first assistant) should have Read tool calls
        let assistant_msg = &session.messages[1];
        assert_eq!(assistant_msg.role, Role::Assistant);
        assert!(assistant_msg.tool_calls.is_some());

        let tools = assistant_msg.tool_calls.as_ref().unwrap();
        let read_count = tools.iter().filter(|t| t.name == "Read").count();
        assert_eq!(read_count, 2); // Two individual Read tool calls
    }

    #[tokio::test]
    async fn test_tool_calls_count_in_metadata() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Count all tool uses: 2 Read + 1 Edit + 2 Bash + 1 Write = 6
        assert_eq!(session.metadata.tool_call_count, 6);
    }

    #[tokio::test]
    async fn test_assistant_text_extracted_with_tools() {
        let path = fixtures_path().join("with_tools.jsonl");
        let session = parse_session(&path).await.unwrap();

        let assistant_msg = &session.messages[1];
        assert!(assistant_msg.content.contains("Let me read that file"));
    }

    // ============================================================================
    // Meta Message Skipping Tests
    // ============================================================================

    #[tokio::test]
    async fn test_skip_meta_messages() {
        let path = fixtures_path().join("with_meta.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Original has 7 lines: 3 meta (should be skipped), 2 user, 2 assistant
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_meta_messages_not_in_content() {
        let path = fixtures_path().join("with_meta.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Verify no meta content appears
        for msg in &session.messages {
            assert!(!msg.content.contains("System initialization"));
            assert!(!msg.content.contains("Meta command"));
            assert!(!msg.content.contains("Another meta"));
        }
    }

    // ============================================================================
    // Command Tag Cleaning Tests
    // ============================================================================

    #[tokio::test]
    async fn test_clean_command_tags() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user message should have command tag cleaned
        let first_user = &session.messages[0];
        assert!(!first_user.content.contains("<command-name>"));
        assert!(!first_user.content.contains("</command-name>"));
        assert!(first_user.content.contains("Please commit my changes"));
    }

    #[tokio::test]
    async fn test_clean_command_args_tags() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Second user message should extract command-args content as the message
        let second_user = &session.messages[2];
        assert!(!second_user.content.contains("<command-args>"));
        assert!(!second_user.content.contains("</command-args>"));
        assert!(!second_user.content.contains("<command-name>"));
        assert_eq!(second_user.content, "Review this PR #123");
    }

    #[tokio::test]
    async fn test_command_only_message_removed() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // The "/help" message should be effectively empty after cleaning
        // and should result in fewer messages
        // Original: 4 user + 4 assistant = 8, but one becomes empty = 7
        assert_eq!(session.messages.len(), 7);
    }

    #[tokio::test]
    async fn test_normal_message_unchanged() {
        let path = fixtures_path().join("with_commands.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Find the normal message without commands
        let normal_msg = session.messages.iter().find(|m| {
            m.role == Role::User && m.content == "Just a normal message without commands"
        });
        assert!(normal_msg.is_some());
    }

    // ============================================================================
    // Large Session Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_large_session() {
        let path = fixtures_path().join("large_session.jsonl");
        let session = parse_session(&path).await.unwrap();

        // 100 Q&A pairs = 200 messages
        assert_eq!(session.messages.len(), 200);
        assert_eq!(session.turn_count(), 100);
    }

    #[tokio::test]
    async fn test_large_session_first_and_last() {
        let path = fixtures_path().join("large_session.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert!(session.messages[0].content.contains("Question number 1"));
        assert!(session.messages[199].content.contains("200"));
    }

    // ============================================================================
    // Error Cases Tests
    // ============================================================================

    #[tokio::test]
    async fn test_file_not_found() {
        let path = fixtures_path().join("nonexistent.jsonl");
        let result = parse_session(&path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::ParseError::NotFound { path: p } => {
                assert!(p.to_string_lossy().contains("nonexistent.jsonl"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_empty_file() {
        let path = fixtures_path().join("empty.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert!(session.is_empty());
        assert_eq!(session.messages.len(), 0);
        assert_eq!(session.turn_count(), 0);
    }

    #[tokio::test]
    async fn test_malformed_lines_skipped() {
        let path = fixtures_path().join("malformed.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should parse valid lines only: 2 user + 2 assistant = 4
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_malformed_lines_content_correct() {
        let path = fixtures_path().join("malformed.jsonl");
        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages[0].content, "Valid first message");
        assert_eq!(session.messages[1].content, "Valid response");
        assert_eq!(session.messages[2].content, "Another valid message");
        assert_eq!(session.messages[3].content, "Final valid response");
    }

    // ============================================================================
    // Edge Cases Tests
    // ============================================================================

    #[tokio::test]
    async fn test_whitespace_only_lines_skipped() {
        let path = fixtures_path().join("whitespace.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should skip blank lines and parse 4 valid messages
        assert_eq!(session.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_user_only_session() {
        let path = fixtures_path().join("user_only.jsonl");
        let session = parse_session(&path).await.unwrap();

        // 3 user messages, 0 assistant
        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.turn_count(), 0); // min(3, 0)
    }

    #[tokio::test]
    async fn test_unknown_entry_types_ignored() {
        let path = fixtures_path().join("unknown_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Should parse user/assistant only, ignore unknown types
        // File has: user, future_type (ignored), assistant, telemetry (ignored),
        // user, metadata (ignored), assistant
        // = 2 user + 2 assistant = 4 valid messages
        assert_eq!(session.messages.len(), 4);

        // Verify correct messages were parsed
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(session.messages[1].content, "Hi there!");
        assert_eq!(session.messages[2].content, "Goodbye");
        assert_eq!(session.messages[3].content, "Goodbye!");
    }

    // ============================================================================
    // Integration Tests with Temporary Files
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_with_temp_file() {
        use tempfile::NamedTempFile;
        use tokio::io::AsyncWriteExt;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let content = r#"{"type":"user","message":{"content":"Test question"},"timestamp":"2026-01-27T12:00:00Z"}
{"type":"assistant","message":{"content":"Test answer"},"timestamp":"2026-01-27T12:00:01Z"}"#;

        let mut file = tokio::fs::File::create(&path).await.unwrap();
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        let session = parse_session(&path).await.unwrap();

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].content, "Test question");
        assert_eq!(session.messages[1].content, "Test answer");
    }

    #[tokio::test]
    async fn test_parse_session_empty_temp_file() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let session = parse_session(&path).await.unwrap();
        assert!(session.is_empty());
    }

    // ============================================================================
    // All 8 JSONL Line Types Tests
    // ============================================================================

    /// Fixture: all_types.jsonl has 12 lines:
    ///   1. user (string content)         -> Role::User
    ///   2. assistant (text+thinking+tool) -> Role::Assistant
    ///   3. user (tool_result array)       -> Role::ToolResult
    ///   4. assistant (tool-only)          -> Role::ToolUse
    ///   5. assistant (text-only)          -> Role::Assistant
    ///   6. system                         -> Role::System
    ///   7. progress                       -> Role::Progress
    ///   8. queue-operation (enqueue)      -> Role::System
    ///   9. queue-operation (dequeue)      -> Role::System
    ///  10. file-history-snapshot          -> Role::System
    ///  11. user (isMeta=true)            -> skipped
    /// = 10 messages total

    #[tokio::test]
    async fn test_parse_all_types_count() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();
        assert_eq!(session.messages.len(), 10);
    }

    #[tokio::test]
    async fn test_parse_user_string_is_user_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        let msg = &session.messages[0];
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Read and fix auth.rs");
    }

    #[tokio::test]
    async fn test_parse_user_array_is_tool_result_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        let msg = &session.messages[2];
        assert_eq!(msg.role, Role::ToolResult);
        assert!(msg.content.contains("tool_result") || msg.content.contains("Tool result"));
    }

    #[tokio::test]
    async fn test_parse_assistant_with_text_is_assistant_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 1 is assistant with text+thinking+tools -> Role::Assistant
        let msg = &session.messages[1];
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.content.contains("I'll read the file first"));
        assert!(msg.tool_calls.is_some());
        assert!(msg.thinking.is_some());
    }

    #[tokio::test]
    async fn test_parse_assistant_tool_only_is_tool_use_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 3 is assistant with only tool_use blocks -> Role::ToolUse
        let msg = &session.messages[3];
        assert_eq!(msg.role, Role::ToolUse);
        assert!(msg.tool_calls.is_some());
        let tools = msg.tool_calls.as_ref().unwrap();
        let edit_tool = tools.iter().find(|t| t.name == "Edit");
        assert!(edit_tool.is_some());
    }

    #[tokio::test]
    async fn test_parse_system_role_and_metadata() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 5 is system (turn_duration)
        let msg = &session.messages[5];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("turn_duration"));
        assert!(msg.content.contains("5000"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(
            meta.get("subtype").unwrap().as_str().unwrap(),
            "turn_duration"
        );
        assert_eq!(meta.get("durationMs").unwrap().as_u64().unwrap(), 5000);
    }

    #[tokio::test]
    async fn test_parse_progress_role_and_metadata() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 6 is progress (hook_progress)
        let msg = &session.messages[6];
        assert_eq!(msg.role, Role::Progress);
        assert!(msg.content.contains("hook_progress"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(meta.get("type").unwrap().as_str().unwrap(), "hook_progress");
        assert_eq!(
            meta.get("hookName").unwrap().as_str().unwrap(),
            "lint-check"
        );
    }

    #[tokio::test]
    async fn test_parse_queue_operation_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 7 is queue-operation (enqueue)
        let msg = &session.messages[7];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("enqueue"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(
            meta.get("type").unwrap().as_str().unwrap(),
            "queue-operation"
        );
        assert_eq!(meta.get("operation").unwrap().as_str().unwrap(), "enqueue");
        assert_eq!(meta.get("content").unwrap().as_str().unwrap(), "next task");

        // Message index 8 is queue-operation (dequeue)
        let msg2 = &session.messages[8];
        assert_eq!(msg2.role, Role::System);
        assert!(msg2.content.contains("dequeue"));
        let meta2 = msg2.metadata.as_ref().unwrap();
        assert_eq!(meta2.get("operation").unwrap().as_str().unwrap(), "dequeue");
    }

    #[tokio::test]
    async fn test_parse_file_history_snapshot_role() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // Message index 9 is file-history-snapshot
        let msg = &session.messages[9];
        assert_eq!(msg.role, Role::System);
        assert!(msg.content.contains("file-history-snapshot"));

        let meta = msg.metadata.as_ref().unwrap();
        assert_eq!(
            meta.get("type").unwrap().as_str().unwrap(),
            "file-history-snapshot"
        );
        assert_eq!(
            meta.get("isSnapshotUpdate").unwrap().as_bool().unwrap(),
            false
        );
        assert!(meta.get("snapshot").is_some());
    }

    #[tokio::test]
    async fn test_parse_uuid_passthrough() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user message has uuid "u1"
        assert_eq!(session.messages[0].uuid, Some("u1".to_string()));
        // First assistant has uuid "a1"
        assert_eq!(session.messages[1].uuid, Some("a1".to_string()));
        // System has uuid "s1"
        assert_eq!(session.messages[5].uuid, Some("s1".to_string()));
        // Progress has uuid "p1"
        assert_eq!(session.messages[6].uuid, Some("p1".to_string()));
    }

    #[tokio::test]
    async fn test_parse_parent_uuid_passthrough() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // First user has no parentUuid
        assert_eq!(session.messages[0].parent_uuid, None);
        // First assistant has parentUuid "u1"
        assert_eq!(session.messages[1].parent_uuid, Some("u1".to_string()));
        // Tool result user (index 2) has parentUuid "a1"
        assert_eq!(session.messages[2].parent_uuid, Some("a1".to_string()));
    }

    #[tokio::test]
    async fn test_parse_meta_user_still_skipped() {
        let path = fixtures_path().join("all_types.jsonl");
        let session = parse_session(&path).await.unwrap();

        // The last line is a user with isMeta=true, should be skipped.
        // 12 lines - 1 skipped (isMeta) - 1 unknown type ignored = 10 parsed messages
        assert_eq!(session.messages.len(), 10);
        // No message should contain "System init"
        for msg in &session.messages {
            assert!(!msg.content.contains("System init"));
        }
    }

    // ============================================================================
    // Paginated Parsing Tests
    // ============================================================================

    #[tokio::test]
    async fn test_parse_session_paginated_first_page() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 0).await.unwrap();
        assert_eq!(result.messages.len(), 10);
        assert_eq!(result.total, 200);
        assert_eq!(result.offset, 0);
        assert_eq!(result.limit, 10);
        assert!(result.has_more);
        assert!(result.messages[0].content.contains("Question number 1"));
    }

    #[tokio::test]
    async fn test_parse_session_paginated_last_page() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 195).await.unwrap();
        assert_eq!(result.messages.len(), 5); // only 5 remaining
        assert_eq!(result.total, 200);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_parse_session_paginated_beyond_end() {
        let path = fixtures_path().join("large_session.jsonl");
        let result = parse_session_paginated(&path, 10, 999).await.unwrap();
        assert_eq!(result.messages.len(), 0);
        assert_eq!(result.total, 200);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_parse_session_with_raw_populates_raw_json() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session_with_raw(&path).await.unwrap();
        for msg in &session.messages {
            assert!(
                msg.raw_json.is_some(),
                "raw_json should be Some for role {:?}",
                msg.role
            );
            assert!(msg.raw_json.as_ref().unwrap().is_object());
        }
    }

    #[tokio::test]
    async fn test_parse_session_without_raw_has_none() {
        let path = fixtures_path().join("simple.jsonl");
        let session = parse_session(&path).await.unwrap();
        for msg in &session.messages {
            assert!(
                msg.raw_json.is_none(),
                "raw_json should be None without _with_raw"
            );
        }
    }
}
