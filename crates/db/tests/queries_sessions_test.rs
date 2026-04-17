#![allow(deprecated)]
//! Integration tests for Database session query methods.

use chrono::Utc;
use claude_view_core::SessionInfo;
use claude_view_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_insert_and_list_projects() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert 3 sessions across 2 projects
    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = make_session("sess-2", "project-a", 2000);
    let s3 = make_session("sess-3", "project-b", 3000);

    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s2, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s3, "project-b", "Project B")
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 2, "Should have 2 projects");

    // Projects should be sorted by most recent activity (project-b first)
    assert_eq!(projects[0].name, "project-b");
    assert_eq!(projects[0].sessions.len(), 1);
    assert_eq!(projects[0].display_name, "Project B");

    assert_eq!(projects[1].name, "project-a");
    assert_eq!(projects[1].sessions.len(), 2);
    assert_eq!(projects[1].display_name, "Project A");

    // Within project-a, sessions should be sorted by last_message_at DESC
    assert_eq!(projects[1].sessions[0].id, "sess-2");
    assert_eq!(projects[1].sessions[1].id, "sess-1");

    // Verify JSON fields deserialized correctly
    assert_eq!(
        projects[1].sessions[0].files_touched,
        vec!["src/main.rs", "Cargo.toml"]
    );
    assert_eq!(projects[1].sessions[0].skills_used, vec!["/commit"]);
    assert_eq!(projects[1].sessions[0].tool_counts.edit, 5);
}

#[tokio::test]
async fn test_upsert_session() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = make_session("sess-1", "project-a", 1000);
    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();

    // Update same session with new data
    let s1_updated = SessionInfo {
        preview: "Updated preview".to_string(),
        modified_at: 5000,
        message_count: 50,
        ..s1
    };
    db.insert_session(&s1_updated, "project-a", "Project A")
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1, "Should still have 1 project");
    assert_eq!(
        projects[0].sessions.len(),
        1,
        "Should still have 1 session (upsert, not duplicate)"
    );
    assert_eq!(projects[0].sessions[0].preview, "Updated preview");
    assert_eq!(projects[0].sessions[0].modified_at, 5000);
    assert_eq!(projects[0].sessions[0].message_count, 50);
}

#[tokio::test]
async fn test_remove_stale_sessions() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = make_session("sess-2", "project-a", 2000);
    let s3 = make_session("sess-3", "project-b", 3000);

    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s2, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s3, "project-b", "Project B")
        .await
        .unwrap();

    // Also add indexer state for the sessions
    db.update_indexer_state(&s1.file_path, 2048, 1000)
        .await
        .unwrap();
    db.update_indexer_state(&s2.file_path, 2048, 2000)
        .await
        .unwrap();
    db.update_indexer_state(&s3.file_path, 2048, 3000)
        .await
        .unwrap();

    // Keep only sess-1's file path; sess-2 and sess-3 are stale
    let valid = vec![s1.file_path.clone()];
    let removed = db.remove_stale_sessions(&valid).await.unwrap();
    assert_eq!(removed, 2, "Should have removed 2 stale sessions");

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1, "Should have 1 project left");
    assert_eq!(projects[0].sessions.len(), 1);
    assert_eq!(projects[0].sessions[0].id, "sess-1");

    // Indexer state should also be cleaned up
    assert!(db.get_indexer_state(&s2.file_path).await.unwrap().is_none());
    assert!(db.get_indexer_state(&s3.file_path).await.unwrap().is_none());
    // The valid file should still have its indexer state
    assert!(db.get_indexer_state(&s1.file_path).await.unwrap().is_some());
}

#[tokio::test]
async fn test_active_count_calculation() {
    let db = Database::new_in_memory().await.unwrap();
    let now = Utc::now().timestamp();

    // Session within the 5-minute window (active)
    let s_active = SessionInfo {
        modified_at: now - 60, // 1 minute ago
        ..make_session("active-sess", "project-a", now - 60)
    };

    // Session outside the 5-minute window (inactive)
    let s_old = SessionInfo {
        modified_at: now - 600, // 10 minutes ago
        ..make_session("old-sess", "project-a", now - 600)
    };

    db.insert_session(&s_active, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s_old, "project-a", "Project A")
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].active_count, 1,
        "Only 1 session should be active (within 5 min)"
    );
    assert_eq!(
        projects[0].sessions.len(),
        2,
        "Both sessions should be listed"
    );
}

#[tokio::test]
async fn test_list_projects_returns_camelcase_json() {
    let db = Database::new_in_memory().await.unwrap();
    let now = Utc::now().timestamp();

    let s1 = make_session("sess-1", "project-a", now);
    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    let json = serde_json::to_string(&projects).unwrap();

    // Verify camelCase keys in ProjectInfo
    assert!(
        json.contains("\"displayName\""),
        "Should use camelCase: displayName"
    );
    assert!(
        json.contains("\"activeCount\""),
        "Should use camelCase: activeCount"
    );

    // Verify camelCase keys in SessionInfo
    assert!(
        json.contains("\"projectPath\""),
        "Should use camelCase: projectPath"
    );
    assert!(
        json.contains("\"filePath\""),
        "Should use camelCase: filePath"
    );
    assert!(
        json.contains("\"modifiedAt\""),
        "Should use camelCase: modifiedAt"
    );
    assert!(
        json.contains("\"sizeBytes\""),
        "Should use camelCase: sizeBytes"
    );
    assert!(
        json.contains("\"lastMessage\""),
        "Should use camelCase: lastMessage"
    );
    assert!(
        json.contains("\"filesTouched\""),
        "Should use camelCase: filesTouched"
    );
    assert!(
        json.contains("\"skillsUsed\""),
        "Should use camelCase: skillsUsed"
    );
    assert!(
        json.contains("\"toolCounts\""),
        "Should use camelCase: toolCounts"
    );
    assert!(
        json.contains("\"messageCount\""),
        "Should use camelCase: messageCount"
    );
    assert!(
        json.contains("\"turnCount\""),
        "Should use camelCase: turnCount"
    );

    // Verify new fields use camelCase
    assert!(
        json.contains("\"isSidechain\""),
        "Should use camelCase: isSidechain"
    );
    assert!(
        json.contains("\"deepIndexed\""),
        "Should use camelCase: deepIndexed"
    );
    // summary and git_branch are None, so they should be omitted (skip_serializing_if)
    assert!(
        !json.contains("\"summary\""),
        "summary=None should be omitted"
    );
    assert!(
        !json.contains("\"gitBranch\""),
        "gitBranch=None should be omitted"
    );

    // modifiedAt should be a Unix timestamp number (not an ISO string)
    let expected_fragment = format!("\"modifiedAt\":{}", now);
    assert!(
        json.contains(&expected_fragment),
        "modifiedAt should be a number: {}",
        json
    );
}

#[tokio::test]
async fn test_update_session_deep_fields_phase3() {
    let db = Database::new_in_memory().await.unwrap();

    // Single UPSERT: identity fields + full Phase 3 deep fields.
    claude_view_db::test_support::SessionSeedBuilder::new("test-sess-deep")
        .project_id("project-deep")
        .project_display_name("Project Deep")
        .project_path("/tmp/project-deep")
        .file_path("/tmp/test-deep.jsonl")
        .preview("Test preview")
        .message_count(10)
        .modified_at(1000)
        .size_bytes(5000)
        .turn_count(5)
        .total_input_tokens(5000)
        .total_output_tokens(3000)
        .total_cost_usd(0.0)
        .with_parsed(|s| {
            s.last_message = "Last message content".to_string();
            s.tool_counts_edit = 10;
            s.tool_counts_read = 15;
            s.tool_counts_bash = 3;
            s.tool_counts_write = 2;
            s.files_touched =
                r#"["/path/to/file1.rs", "/path/to/file2.rs", "/path/to/file3.rs"]"#.to_string();
            s.skills_used = r#"["/commit", "/review"]"#.to_string();
            s.user_prompt_count = 8;
            s.api_call_count = 12;
            s.tool_call_count = 25;
            s.files_read = r#"["/path/to/file1.rs", "/path/to/file2.rs"]"#.to_string();
            s.files_edited =
                r#"["/path/to/file1.rs", "/path/to/file1.rs", "/path/to/file3.rs"]"#.to_string();
            s.files_read_count = 2;
            s.files_edited_count = 2;
            s.reedited_files_count = 1;
            s.duration_seconds = 600;
            s.commit_count = 3;
            s.first_message_at = 1000;
            s.cache_read_tokens = 1000;
            s.cache_creation_tokens = 500;
            s.thinking_block_count = 2;
            s.turn_duration_avg_ms = Some(150);
            s.turn_duration_max_ms = Some(300);
            s.turn_duration_total_ms = Some(750);
            s.api_error_count = 1;
            s.agent_spawn_count = 1;
            s.bash_progress_count = 2;
            s.mcp_progress_count = 1;
            s.summary_text = Some("Session summary text".to_string());
            s.parse_version = 1;
            s.file_size_at_index = 5000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    // Retrieve session and verify all Phase 3 fields
    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].sessions.len(), 1);

    let session = &projects[0].sessions[0];
    assert_eq!(session.id, "test-sess-deep");
    assert_eq!(session.last_message, "Last message content");
    assert_eq!(session.turn_count, 5);
    assert!(
        session.deep_indexed,
        "Session should be marked as deep_indexed"
    );

    // Verify Phase 3 atomic unit metrics
    assert_eq!(session.user_prompt_count, 8, "user_prompt_count mismatch");
    assert_eq!(session.api_call_count, 12, "api_call_count mismatch");
    assert_eq!(session.tool_call_count, 25, "tool_call_count mismatch");
    assert_eq!(session.files_read.len(), 2, "files_read count mismatch");
    assert_eq!(
        session.files_read,
        vec!["/path/to/file1.rs", "/path/to/file2.rs"]
    );
    assert_eq!(
        session.files_edited.len(),
        3,
        "files_edited count mismatch (includes duplicates)"
    );
    assert_eq!(session.files_read_count, 2, "files_read_count mismatch");
    assert_eq!(session.files_edited_count, 2, "files_edited_count mismatch");
    assert_eq!(
        session.reedited_files_count, 1,
        "reedited_files_count mismatch"
    );
    assert_eq!(session.duration_seconds, 600, "duration_seconds mismatch");
    assert_eq!(session.commit_count, 3, "commit_count mismatch");
}

#[tokio::test]
async fn test_get_session_file_path() {
    let db = Database::new_in_memory().await.unwrap();

    // Not in DB → None
    let result = db.get_session_file_path("nonexistent").await.unwrap();
    assert!(result.is_none());

    // Insert session with known file_path
    let session = make_session("fp-test", "proj", 1700000000);
    // make_session sets file_path to "/home/user/.claude/projects/proj/fp-test.jsonl"
    db.insert_session(&session, "proj", "Project")
        .await
        .unwrap();

    let result = db.get_session_file_path("fp-test").await.unwrap();
    assert_eq!(
        result.as_deref(),
        Some("/home/user/.claude/projects/proj/fp-test.jsonl")
    );
}

#[tokio::test]
async fn test_phase3_fields_default_to_zero() {
    let db = Database::new_in_memory().await.unwrap();

    // Seed a session with ONLY identity fields — deep fields default to zero.
    claude_view_db::test_support::SessionSeedBuilder::new("no-deep-index")
        .project_id("proj-no-deep")
        .project_display_name("Project No Deep")
        .project_path("/tmp/proj")
        .file_path("/tmp/no-deep.jsonl")
        .preview("Preview")
        .message_count(5)
        .modified_at(1000)
        .size_bytes(500)
        .seed(&db)
        .await
        .unwrap();

    // Retrieve and verify Phase 3 fields default to 0/empty.
    // (Note: `deep_indexed` is now always true under the unified UPSERT path —
    //  the Pass-1-vs-Pass-2 split that the original assertion depended on no
    //  longer exists. The real invariant the test name refers to — deep count
    //  fields defaulting to zero when unset — is preserved below.)
    let projects = db.list_projects().await.unwrap();
    let session = &projects[0].sessions[0];

    assert_eq!(session.user_prompt_count, 0);
    assert_eq!(session.api_call_count, 0);
    assert_eq!(session.tool_call_count, 0);
    assert!(session.files_read.is_empty());
    assert!(session.files_edited.is_empty());
    assert_eq!(session.files_read_count, 0);
    assert_eq!(session.files_edited_count, 0);
    assert_eq!(session.reedited_files_count, 0);
    assert_eq!(session.duration_seconds, 0);
    assert_eq!(session.commit_count, 0);
}
