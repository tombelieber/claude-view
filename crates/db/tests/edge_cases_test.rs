// Edge case tests for Phase 3 Metrics Engine (A10.1-A10.5).
//
// These tests verify robustness of the parsing and correlation pipeline
// when encountering malformed, missing, or unusual data.

use vibe_recall_db::indexer_parallel::{
    extract_commit_skill_invocations, parse_bytes, pass_1_read_indexes, pass_2_deep_index,
    CommitSkillInvocation, RawInvocation,
};
use vibe_recall_db::git_correlation::{
    scan_repo_commits, tier1_match, tier2_match, GitCommit,
};
use vibe_recall_db::Database;
use vibe_recall_core::metrics::{
    edit_velocity, read_to_edit_ratio, reedit_rate, tokens_per_prompt, tool_density,
};

// ============================================================================
// A10.1: JSONL Parsing Edge Cases
// ============================================================================

#[test]
fn a10_1_malformed_json_line_skipped() {
    // Mix of valid and malformed lines
    let data = br#"{"type":"user","message":{"content":"Hello"}}
this is not valid JSON {{{
{"type":"assistant","message":{"content":"Response"}}
{"type":"user","message":{"content":"Thanks"}}
"#;
    let result = parse_bytes(data);

    // Should skip the malformed line and continue
    assert_eq!(result.deep.user_prompt_count, 2, "Should count 2 valid user messages");
    assert_eq!(result.deep.api_call_count, 1, "Should count 1 valid assistant message");
    assert_eq!(result.deep.last_message, "Thanks", "Should capture last valid user message");
}

#[test]
fn a10_1_empty_file_zero_counts() {
    let result = parse_bytes(b"");

    assert_eq!(result.deep.user_prompt_count, 0);
    assert_eq!(result.deep.api_call_count, 0);
    assert_eq!(result.deep.tool_call_count, 0);
    assert_eq!(result.deep.files_read_count, 0);
    assert_eq!(result.deep.files_edited_count, 0);
    assert_eq!(result.deep.reedited_files_count, 0);
    assert_eq!(result.deep.duration_seconds, 0);
    assert!(result.deep.first_timestamp.is_none());
    assert!(result.deep.last_timestamp.is_none());
    assert!(result.turns.is_empty());
    assert!(result.models_seen.is_empty());
}

#[test]
fn a10_1_only_system_messages_zero_user_prompts() {
    // Only assistant messages (no user prompts)
    let data = br#"{"type":"assistant","message":{"content":"I can help!"}}
{"type":"assistant","message":{"content":"What would you like?"}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.user_prompt_count, 0, "No user messages");
    assert_eq!(result.deep.api_call_count, 2, "Should count assistant messages");
    assert_eq!(result.deep.turn_count, 0, "No complete turns without user prompts");
}

#[test]
fn a10_1_missing_timestamps_skipped_for_duration() {
    // Some lines have timestamps, some don't
    let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"First"}}
{"type":"assistant","message":{"content":"Response without timestamp"}}
{"type":"user","message":{"content":"No timestamp here"}}
{"type":"assistant","timestamp":"2026-01-27T10:10:00Z","message":{"content":"Last with timestamp"}}
"#;
    let result = parse_bytes(data);

    // Duration should be calculated from first and last timestamps found
    assert_eq!(result.deep.duration_seconds, 600, "10 minutes between available timestamps");
    assert!(result.deep.first_timestamp.is_some());
    assert!(result.deep.last_timestamp.is_some());
}

#[test]
fn a10_1_invalid_timestamp_format_skipped() {
    let data = br#"{"type":"user","timestamp":"not-a-valid-timestamp","message":{"content":"Bad timestamp"}}
{"type":"assistant","timestamp":"2026-01-27T10:00:00Z","message":{"content":"Good timestamp"}}
"#;
    let result = parse_bytes(data);

    // Should still work, just skip the invalid timestamp
    assert_eq!(result.deep.user_prompt_count, 1);
    assert_eq!(result.deep.api_call_count, 1);
    // Only one valid timestamp, so duration is 0
    assert_eq!(result.deep.duration_seconds, 0);
}

#[test]
fn a10_1_utf8_bom_handled() {
    // UTF-8 BOM (EF BB BF) at the start of file
    let mut data = vec![0xEF, 0xBB, 0xBF];
    data.extend_from_slice(br#"{"type":"user","message":{"content":"Hello after BOM"}}
{"type":"assistant","message":{"content":"Response"}}
"#);

    let result = parse_bytes(&data);

    // Should parse correctly despite BOM (first line might fail JSON parse, but no panic)
    // The BOM will cause the first line to be invalid JSON, so it's skipped
    assert!(result.deep.api_call_count >= 1, "Should parse at least the assistant message");
}

#[test]
fn a10_1_unknown_type_skipped() {
    let data = br#"{"type":"user","message":{"content":"Hello"}}
{"type":"unknown_type","message":{"content":"Unknown"}}
{"type":"system","message":{"content":"System message"}}
{"type":"assistant","message":{"content":"Response"}}
"#;
    let result = parse_bytes(data);

    // Only count user and assistant types
    assert_eq!(result.deep.user_prompt_count, 1);
    assert_eq!(result.deep.api_call_count, 1);
}

#[test]
fn a10_1_whitespace_only_lines_skipped() {
    let data = br#"{"type":"user","message":{"content":"Hello"}}


{"type":"assistant","message":{"content":"Response"}}

"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.user_prompt_count, 1);
    assert_eq!(result.deep.api_call_count, 1);
}

#[test]
fn a10_1_very_long_line_handled() {
    // Create a very long content string (100KB)
    let long_content = "x".repeat(100_000);
    let data = format!(
        r#"{{"type":"user","message":{{"content":"{}"}}}}"#,
        long_content
    );

    let result = parse_bytes(data.as_bytes());

    assert_eq!(result.deep.user_prompt_count, 1);
    // Message should be truncated to 200 chars + "..."
    assert_eq!(result.deep.last_message.len(), 203);
    assert!(result.deep.last_message.ends_with("..."));
}

// ============================================================================
// A10.2: Tool Extraction Edge Cases
// ============================================================================

#[test]
fn a10_2_missing_input_field_skipped() {
    // tool_use without input field
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"}]}}
"#;
    let result = parse_bytes(data);

    // Should count the tool call but not extract file path
    assert_eq!(result.deep.tool_call_count, 1);
    assert_eq!(result.deep.files_read_count, 0, "No valid file_path");
    assert!(result.deep.files_read.is_empty());
}

#[test]
fn a10_2_missing_file_path_skipped() {
    // tool_use with input but no file_path
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"other_field":"value"}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.tool_call_count, 1);
    assert_eq!(result.deep.files_read_count, 0);
}

#[test]
fn a10_2_null_file_path_skipped() {
    // file_path is null
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":null}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.tool_call_count, 1);
    assert_eq!(result.deep.files_read_count, 0);
}

#[test]
fn a10_2_empty_file_path_skipped() {
    // file_path is empty string
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":""}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.tool_call_count, 1);
    assert_eq!(result.deep.files_read_count, 0);
}

#[test]
fn a10_2_special_chars_in_path_stored_as_is() {
    // Path with special characters
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/with spaces/and-special_chars/@#$%.rs"}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.files_read_count, 1);
    assert!(result.deep.files_read.contains(&"/path/with spaces/and-special_chars/@#$%.rs".to_string()));
}

#[test]
fn a10_2_unicode_in_path_stored_as_is() {
    // Path with unicode characters
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file_\u4e2d\u6587.rs"}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.files_read_count, 1);
    // The unicode should be preserved
    assert_eq!(result.deep.files_read.len(), 1);
}

#[test]
fn a10_2_multiple_tool_use_in_content_counted() {
    // Multiple tool_use blocks in one message (must be single line for JSONL)
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/b.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/c.rs"}},{"type":"tool_use","name":"Bash","input":{"command":"ls"}},{"type":"tool_use","name":"Write","input":{"file_path":"/d.rs"}}]}}
"#;
    let result = parse_bytes(data);

    assert_eq!(result.deep.tool_call_count, 5, "Should count all 5 tool calls");
    assert_eq!(result.deep.files_read_count, 2);
    assert_eq!(result.deep.files_edited_count, 2); // Edit + Write
    assert_eq!(result.raw_invocations.len(), 5);
}

#[test]
fn a10_2_tool_use_without_name_skipped() {
    // tool_use block without name field (must be single line for JSONL)
    let data = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","input":{"file_path":"/a.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/b.rs"}}]}}
"#;
    let result = parse_bytes(data);

    // First tool_use lacks name, should be skipped in raw_invocations
    // The tool counts are based on SIMD pattern matching, not full JSON parse
    // So Read pattern will match for second one
    assert!(result.deep.files_read_count >= 1);
}

// ============================================================================
// A10.3: Git Correlation Edge Cases
// ============================================================================

#[tokio::test]
async fn a10_3_non_git_dir_returns_zero_commits_no_error() {
    // Use a temp directory that definitely isn't a git repo
    // Create it in /tmp to ensure we're outside any git repo
    let tmp = tempfile::TempDir::new_in("/tmp").unwrap();

    let result = scan_repo_commits(tmp.path(), None, None).await;

    // Should either mark as not_a_repo or return empty commits
    assert!(
        result.not_a_repo || result.commits.is_empty(),
        "Non-git directory should return 0 commits without error"
    );
    // Should NOT panic
}

#[test]
fn a10_3_tier1_match_uses_exact_repo_path() {
    let skills = vec![CommitSkillInvocation {
        skill_name: "commit".to_string(),
        timestamp_unix: 1706400100,
    }];

    // Commit from a different repo (path differs slightly)
    let commits = vec![GitCommit {
        hash: "a".repeat(40),
        repo_path: "/repo/path/sub".to_string(), // Different!
        message: "Test".to_string(),
        author: None,
        timestamp: 1706400100,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);

    assert!(matches.is_empty(), "Repo path must match exactly");
}

#[test]
fn a10_3_both_tiers_match_use_tier1() {
    // A commit that would match both Tier 1 (commit skill) and Tier 2 (session range)
    let skills = vec![CommitSkillInvocation {
        skill_name: "commit".to_string(),
        timestamp_unix: 1706400100,
    }];

    let commits = vec![GitCommit {
        hash: "a".repeat(40),
        repo_path: "/repo/path".to_string(),
        message: "Test".to_string(),
        author: None,
        timestamp: 1706400120, // Within Tier 1 window
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    // Tier 1 match
    let tier1_matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
    assert_eq!(tier1_matches.len(), 1);
    assert_eq!(tier1_matches[0].tier, 1);

    // Tier 2 also matches (commit during session)
    let tier2_matches = tier2_match("sess-1", "/repo/path", 1706400000, 1706400200, &commits);
    assert_eq!(tier2_matches.len(), 1);
    assert_eq!(tier2_matches[0].tier, 2);

    // When correlating, Tier 1 takes precedence (tested separately in correlation pipeline)
}

#[test]
fn a10_3_tier1_window_boundaries() {
    let skills = vec![CommitSkillInvocation {
        skill_name: "commit".to_string(),
        timestamp_unix: 1706400100,
    }];

    // Test exact window boundaries: [T-60, T+300]
    let commits = vec![
        GitCommit {
            hash: "a".repeat(40),
            repo_path: "/repo".to_string(),
            message: "At T-60".to_string(),
            author: None,
            timestamp: 1706400100 - 60, // Exactly at lower bound
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "b".repeat(40),
            repo_path: "/repo".to_string(),
            message: "At T+300".to_string(),
            author: None,
            timestamp: 1706400100 + 300, // Exactly at upper bound
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "c".repeat(40),
            repo_path: "/repo".to_string(),
            message: "At T-61".to_string(),
            author: None,
            timestamp: 1706400100 - 61, // Just outside
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "d".repeat(40),
            repo_path: "/repo".to_string(),
            message: "At T+301".to_string(),
            author: None,
            timestamp: 1706400100 + 301, // Just outside
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
    ];

    let matches = tier1_match("sess-1", "/repo", &skills, &commits);

    // Should match exactly 2 (at boundaries)
    assert_eq!(matches.len(), 2);
    let hashes: Vec<_> = matches.iter().map(|m| m.commit_hash.as_str()).collect();
    assert!(hashes.contains(&"a".repeat(40).as_str()), "T-60 should match");
    assert!(hashes.contains(&"b".repeat(40).as_str()), "T+300 should match");
}

// ============================================================================
// A10.4: Concurrent Access (Tested at integration level)
// ============================================================================

// Note: The concurrent access test for POST /api/sync/git returning 409 is already
// tested in crates/server/src/routes/sync.rs. Here we add additional edge cases.

#[tokio::test]
async fn a10_4_parallel_indexing_no_data_corruption() {
    // Create multiple sessions and verify parallel indexing doesn't corrupt data
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().to_path_buf();
    let project_dir = claude_dir.join("projects").join("test-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    // Create 10 JSONL files
    let mut entries = Vec::new();
    for i in 0..10 {
        let session_id = format!("sess-{:03}", i);
        let jsonl_path = project_dir.join(format!("{}.jsonl", &session_id));
        let content = format!(
            r#"{{"type":"user","message":{{"content":"Question {}"}}}}{}"#,
            i, "\n"
        );
        std::fs::write(&jsonl_path, content).unwrap();

        entries.push(serde_json::json!({
            "sessionId": session_id,
            "fullPath": jsonl_path.to_string_lossy(),
            "firstPrompt": format!("Question {}", i),
            "messageCount": 1,
            "modified": "2026-01-28T10:00:00.000Z"
        }));
    }

    let index_json = serde_json::to_string_pretty(&entries).unwrap();
    std::fs::write(project_dir.join("sessions-index.json"), index_json).unwrap();

    let db = Database::new_in_memory().await.unwrap();

    // Run parallel indexing
    pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    let (indexed, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();

    assert_eq!(indexed, 10, "All 10 sessions should be indexed");

    // Verify no data corruption
    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects[0].sessions.len(), 10);

    for (i, session) in projects[0].sessions.iter().enumerate() {
        assert!(session.deep_indexed, "Session {} should be deep indexed", i);
        assert_eq!(session.user_prompt_count, 1, "Each session has 1 user prompt");
    }
}

// ============================================================================
// A10.5: Data Integrity Edge Cases
// ============================================================================

#[tokio::test]
async fn a10_5_session_deleted_cascades_session_commits() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a session
    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project",
        "/repo",
        "/tmp/sess-1.jsonl",
        "Test",
        None,
        10,
        1706400000,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    // Insert a commit and link it
    let commit = vibe_recall_db::git_correlation::GitCommit {
        hash: "a".repeat(40),
        repo_path: "/repo".to_string(),
        message: "Test commit".to_string(),
        author: None,
        timestamp: 1706400100,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    };
    db.batch_upsert_commits(&[commit]).await.unwrap();

    let correlation = vibe_recall_db::git_correlation::CorrelationMatch {
        session_id: "sess-1".to_string(),
        commit_hash: "a".repeat(40),
        tier: 1,
        evidence: vibe_recall_db::git_correlation::CorrelationEvidence {
            rule: "commit_skill".to_string(),
            skill_ts: Some(1706400100),
            commit_ts: Some(1706400100),
            skill_name: Some("commit".to_string()),
            session_start: None,
            session_end: None,
        },
    };
    db.batch_insert_session_commits(&[correlation]).await.unwrap();

    // Verify link exists
    let count = db.count_commits_for_session("sess-1").await.unwrap();
    assert_eq!(count, 1);

    // Delete session
    sqlx::query("DELETE FROM sessions WHERE id = 'sess-1'")
        .execute(db.pool())
        .await
        .unwrap();

    // Verify cascade delete
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits WHERE session_id = 'sess-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count.0, 0, "session_commits should cascade delete with session");

    // Commit itself should still exist (no cascade on commit side)
    let commit_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM commits WHERE hash = ?1")
        .bind(&"a".repeat(40))
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(commit_count.0, 1, "Commit should not be deleted");
}

#[test]
fn a10_5_zero_duration_derived_metrics_return_none() {
    // When duration is 0, edit_velocity should return None
    let result = edit_velocity(10, 0);
    assert_eq!(result, None, "edit_velocity with 0 duration should return None");
}

#[test]
fn a10_5_zero_divisors_all_return_none() {
    // All derived metrics with zero divisor
    assert_eq!(tokens_per_prompt(1000, 500, 0), None);
    assert_eq!(reedit_rate(5, 0), None);
    assert_eq!(tool_density(50, 0), None);
    assert_eq!(edit_velocity(10, 0), None);
    assert_eq!(read_to_edit_ratio(20, 0), None);
}

#[test]
fn a10_5_valid_zero_numerators() {
    // Zero numerators should return Some(0.0), not None
    assert_eq!(tokens_per_prompt(0, 0, 10), Some(0.0));
    assert_eq!(reedit_rate(0, 5), Some(0.0));
    assert_eq!(tool_density(0, 10), Some(0.0));
    assert_eq!(edit_velocity(0, 600), Some(0.0));
    assert_eq!(read_to_edit_ratio(0, 5), Some(0.0));
}

// ============================================================================
// Additional Edge Cases: Commit Skill Extraction
// ============================================================================

#[test]
fn a10_commit_skill_empty_invocations() {
    let raw: Vec<RawInvocation> = vec![];
    let result = extract_commit_skill_invocations(&raw);
    assert!(result.is_empty());
}

#[test]
fn a10_commit_skill_non_skill_tools() {
    let raw = vec![
        RawInvocation {
            name: "Read".to_string(),
            input: Some(serde_json::json!({"file_path": "/foo"})),
            byte_offset: 0,
            timestamp: 1706400000,
        },
        RawInvocation {
            name: "Edit".to_string(),
            input: Some(serde_json::json!({"file_path": "/bar"})),
            byte_offset: 100,
            timestamp: 1706400100,
        },
    ];

    let result = extract_commit_skill_invocations(&raw);
    assert!(result.is_empty(), "Non-Skill tools should not produce commit invocations");
}

#[test]
fn a10_commit_skill_missing_skill_field() {
    let raw = vec![RawInvocation {
        name: "Skill".to_string(),
        input: Some(serde_json::json!({"other": "value"})), // No "skill" field
        byte_offset: 0,
        timestamp: 1706400000,
    }];

    let result = extract_commit_skill_invocations(&raw);
    assert!(result.is_empty());
}

#[test]
fn a10_commit_skill_non_string_skill_field() {
    let raw = vec![RawInvocation {
        name: "Skill".to_string(),
        input: Some(serde_json::json!({"skill": 123})), // Not a string
        byte_offset: 0,
        timestamp: 1706400000,
    }];

    let result = extract_commit_skill_invocations(&raw);
    assert!(result.is_empty());
}

#[test]
fn a10_commit_skill_non_commit_skill() {
    let raw = vec![RawInvocation {
        name: "Skill".to_string(),
        input: Some(serde_json::json!({"skill": "debug"})), // Not commit-related
        byte_offset: 0,
        timestamp: 1706400000,
    }];

    let result = extract_commit_skill_invocations(&raw);
    assert!(result.is_empty(), "Non-commit skills should not be extracted");
}

#[test]
fn a10_commit_skill_all_variants() {
    let raw = vec![
        RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit"})),
            byte_offset: 0,
            timestamp: 1706400100,
        },
        RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit-commands:commit"})),
            byte_offset: 100,
            timestamp: 1706400200,
        },
        RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit-commands:commit-push-pr"})),
            byte_offset: 200,
            timestamp: 1706400300,
        },
    ];

    let result = extract_commit_skill_invocations(&raw);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].skill_name, "commit");
    assert_eq!(result[1].skill_name, "commit-commands:commit");
    assert_eq!(result[2].skill_name, "commit-commands:commit-push-pr");
}

// ============================================================================
// Edge Cases: Large Numbers
// ============================================================================

#[test]
fn a10_large_token_counts() {
    // Test with very large token values (u64 range)
    let total_input: u64 = 1_000_000_000; // 1 billion
    let total_output: u64 = 500_000_000;
    let prompts: u32 = 1000;

    let result = tokens_per_prompt(total_input, total_output, prompts);
    assert_eq!(result, Some(1_500_000.0)); // (1B + 500M) / 1000
}

#[test]
fn a10_overflow_protection() {
    // Verify no overflow with max values
    let result = tokens_per_prompt(u64::MAX / 2, u64::MAX / 2, 1);
    assert!(result.is_some());
    // The result might be imprecise but should not panic
}
