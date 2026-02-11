//! Integration tests for Database session query methods.

use chrono::Utc;
use vibe_recall_core::{BranchFilter, SessionInfo};
use vibe_recall_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_insert_and_list_projects() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert 3 sessions across 2 projects
    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = make_session("sess-2", "project-a", 2000);
    let s3 = make_session("sess-3", "project-b", 3000);

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();
    db.insert_session(&s3, "project-b", "Project B").await.unwrap();

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
    db.insert_session(&s1, "project-a", "Project A").await.unwrap();

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
    assert_eq!(projects[0].sessions.len(), 1, "Should still have 1 session (upsert, not duplicate)");
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

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();
    db.insert_session(&s3, "project-b", "Project B").await.unwrap();

    // Also add indexer state for the sessions
    db.update_indexer_state(&s1.file_path, 2048, 1000).await.unwrap();
    db.update_indexer_state(&s2.file_path, 2048, 2000).await.unwrap();
    db.update_indexer_state(&s3.file_path, 2048, 3000).await.unwrap();

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

    db.insert_session(&s_active, "project-a", "Project A").await.unwrap();
    db.insert_session(&s_old, "project-a", "Project A").await.unwrap();

    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].active_count, 1, "Only 1 session should be active (within 5 min)");
    assert_eq!(projects[0].sessions.len(), 2, "Both sessions should be listed");
}

#[tokio::test]
async fn test_list_projects_returns_camelcase_json() {
    let db = Database::new_in_memory().await.unwrap();
    let now = Utc::now().timestamp();

    let s1 = make_session("sess-1", "project-a", now);
    db.insert_session(&s1, "project-a", "Project A").await.unwrap();

    let projects = db.list_projects().await.unwrap();
    let json = serde_json::to_string(&projects).unwrap();

    // Verify camelCase keys in ProjectInfo
    assert!(json.contains("\"displayName\""), "Should use camelCase: displayName");
    assert!(json.contains("\"activeCount\""), "Should use camelCase: activeCount");

    // Verify camelCase keys in SessionInfo
    assert!(json.contains("\"projectPath\""), "Should use camelCase: projectPath");
    assert!(json.contains("\"filePath\""), "Should use camelCase: filePath");
    assert!(json.contains("\"modifiedAt\""), "Should use camelCase: modifiedAt");
    assert!(json.contains("\"sizeBytes\""), "Should use camelCase: sizeBytes");
    assert!(json.contains("\"lastMessage\""), "Should use camelCase: lastMessage");
    assert!(json.contains("\"filesTouched\""), "Should use camelCase: filesTouched");
    assert!(json.contains("\"skillsUsed\""), "Should use camelCase: skillsUsed");
    assert!(json.contains("\"toolCounts\""), "Should use camelCase: toolCounts");
    assert!(json.contains("\"messageCount\""), "Should use camelCase: messageCount");
    assert!(json.contains("\"turnCount\""), "Should use camelCase: turnCount");

    // Verify new fields use camelCase
    assert!(json.contains("\"isSidechain\""), "Should use camelCase: isSidechain");
    assert!(json.contains("\"deepIndexed\""), "Should use camelCase: deepIndexed");
    // summary and git_branch are None, so they should be omitted (skip_serializing_if)
    assert!(!json.contains("\"summary\""), "summary=None should be omitted");
    assert!(!json.contains("\"gitBranch\""), "gitBranch=None should be omitted");

    // modifiedAt should be a Unix timestamp number (not an ISO string)
    let expected_fragment = format!("\"modifiedAt\":{}", now);
    assert!(
        json.contains(&expected_fragment),
        "modifiedAt should be a number: {}",
        json
    );
}

#[tokio::test]
async fn test_list_project_summaries() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = make_session("sess-2", "project-a", 2000);
    let s3 = make_session("sess-3", "project-b", 3000);

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();
    db.insert_session(&s3, "project-b", "Project B").await.unwrap();

    let summaries = db.list_project_summaries().await.unwrap();
    assert_eq!(summaries.len(), 2);

    // Sorted by last_activity_at DESC
    assert_eq!(summaries[0].name, "project-b");
    assert_eq!(summaries[0].session_count, 1);
    assert_eq!(summaries[0].display_name, "Project B");

    assert_eq!(summaries[1].name, "project-a");
    assert_eq!(summaries[1].session_count, 2);

    // No sessions array on summaries
    let json = serde_json::to_string(&summaries).unwrap();
    assert!(!json.contains("\"sessions\""), "Summaries should NOT include sessions array");
    assert!(json.contains("\"sessionCount\""), "Should have sessionCount field");
}

#[tokio::test]
async fn test_list_sessions_for_project_pagination() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert 5 sessions for project-a
    for i in 1..=5 {
        let s = make_session(&format!("sess-{}", i), "project-a", i as i64 * 1000);
        db.insert_session(&s, "project-a", "Project A").await.unwrap();
    }

    // Page 1: limit=2, offset=0
    let page1 = db.list_sessions_for_project("project-a", 2, 0, "recent", &BranchFilter::All, false).await.unwrap();
    assert_eq!(page1.total, 5);
    assert_eq!(page1.sessions.len(), 2);
    assert_eq!(page1.sessions[0].id, "sess-5"); // Most recent first

    // Page 2: limit=2, offset=2
    let page2 = db.list_sessions_for_project("project-a", 2, 2, "recent", &BranchFilter::All, false).await.unwrap();
    assert_eq!(page2.total, 5);
    assert_eq!(page2.sessions.len(), 2);
    assert_eq!(page2.sessions[0].id, "sess-3");
}

#[tokio::test]
async fn test_list_sessions_for_project_sort() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = SessionInfo { message_count: 100, ..make_session("sess-1", "project-a", 1000) };
    let s2 = SessionInfo { message_count: 5, ..make_session("sess-2", "project-a", 3000) };
    let s3 = SessionInfo { message_count: 50, ..make_session("sess-3", "project-a", 2000) };

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();
    db.insert_session(&s3, "project-a", "Project A").await.unwrap();

    // Sort by oldest
    let oldest = db.list_sessions_for_project("project-a", 10, 0, "oldest", &BranchFilter::All, false).await.unwrap();
    assert_eq!(oldest.sessions[0].id, "sess-1");

    // Sort by messages
    let by_msg = db.list_sessions_for_project("project-a", 10, 0, "messages", &BranchFilter::All, false).await.unwrap();
    assert_eq!(by_msg.sessions[0].id, "sess-1"); // 100 messages
}

#[tokio::test]
async fn test_list_sessions_excludes_sidechains() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = SessionInfo { is_sidechain: true, ..make_session("sess-2", "project-a", 2000) };

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();

    // Default: exclude sidechains
    let page = db.list_sessions_for_project("project-a", 10, 0, "recent", &BranchFilter::All, false).await.unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.sessions[0].id, "sess-1");

    // Include sidechains
    let page = db.list_sessions_for_project("project-a", 10, 0, "recent", &BranchFilter::All, true).await.unwrap();
    assert_eq!(page.total, 2);
}

#[tokio::test]
async fn test_project_summaries_exclude_sidechains() {
    let db = Database::new_in_memory().await.unwrap();

    let s1 = make_session("sess-1", "project-a", 1000);
    let s2 = SessionInfo { is_sidechain: true, ..make_session("sess-2", "project-a", 2000) };

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();

    let summaries = db.list_project_summaries().await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_count, 1, "Sidechain should be excluded from count");
}

#[tokio::test]
async fn test_update_session_deep_fields_phase3() {
    let db = Database::new_in_memory().await.unwrap();

    // First insert a session via Pass 1 (from index)
    db.insert_session_from_index(
        "test-sess-deep",
        "project-deep",
        "Project Deep",
        "/tmp/project-deep",
        "/tmp/test-deep.jsonl",
        "Test preview",
        None,
        10,
        1000,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    // Update with Pass 2 deep fields including Phase 3 metrics
    let files_read = r#"["/path/to/file1.rs", "/path/to/file2.rs"]"#;
    let files_edited = r#"["/path/to/file1.rs", "/path/to/file1.rs", "/path/to/file3.rs"]"#;

    db.update_session_deep_fields(
        "test-sess-deep",
        "Last message content",
        5,   // turn_count
        10,  // tool_edit
        15,  // tool_read
        3,   // tool_bash
        2,   // tool_write
        r#"["/path/to/file1.rs", "/path/to/file2.rs", "/path/to/file3.rs"]"#, // files_touched
        r#"["/commit", "/review"]"#, // skills_used
        // Phase 3: Atomic unit metrics
        8,   // user_prompt_count
        12,  // api_call_count
        25,  // tool_call_count
        files_read,
        files_edited,
        2,   // files_read_count
        2,   // files_edited_count (unique: file1.rs, file3.rs)
        1,   // reedited_files_count (file1.rs edited twice)
        600, // duration_seconds (10 minutes)
        3,   // commit_count
        Some(1000), // first_message_at
        // Phase 3.5: Full parser metrics
        5000,  // total_input_tokens
        3000,  // total_output_tokens
        1000,  // cache_read_tokens
        500,   // cache_creation_tokens
        2,     // thinking_block_count
        Some(150), // turn_duration_avg_ms
        Some(300), // turn_duration_max_ms
        Some(750), // turn_duration_total_ms
        1,     // api_error_count
        0,     // api_retry_count
        0,     // compaction_count
        0,     // hook_blocked_count
        1,     // agent_spawn_count
        2,     // bash_progress_count
        0,     // hook_progress_count
        1,     // mcp_progress_count
        Some("Session summary text"), // summary_text
        1,     // parse_version
        5000,  // file_size
        1706200000, // file_mtime
        0,     // lines_added
        0,     // lines_removed
        0,     // loc_source
        0,     // ai_lines_added
        0,     // ai_lines_removed
        None,  // work_type
        None,  // git_branch
        None,  // primary_model
        None,  // last_message_at
        None,  // first_user_prompt
    )
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
    assert!(session.deep_indexed, "Session should be marked as deep_indexed");

    // Verify Phase 3 atomic unit metrics
    assert_eq!(session.user_prompt_count, 8, "user_prompt_count mismatch");
    assert_eq!(session.api_call_count, 12, "api_call_count mismatch");
    assert_eq!(session.tool_call_count, 25, "tool_call_count mismatch");
    assert_eq!(session.files_read.len(), 2, "files_read count mismatch");
    assert_eq!(session.files_read, vec!["/path/to/file1.rs", "/path/to/file2.rs"]);
    assert_eq!(session.files_edited.len(), 3, "files_edited count mismatch (includes duplicates)");
    assert_eq!(session.files_read_count, 2, "files_read_count mismatch");
    assert_eq!(session.files_edited_count, 2, "files_edited_count mismatch");
    assert_eq!(session.reedited_files_count, 1, "reedited_files_count mismatch");
    assert_eq!(session.duration_seconds, 600, "duration_seconds mismatch");
    assert_eq!(session.commit_count, 3, "commit_count mismatch");
}

#[tokio::test]
async fn test_list_sessions_for_project_includes_phase3_fields() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert via Pass 1
    db.insert_session_from_index(
        "phase3-paginated",
        "proj-paginated",
        "Project Paginated",
        "/tmp/proj-paginated",
        "/tmp/paginated.jsonl",
        "Preview",
        None,
        5,
        2000,
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Update via Pass 2 with Phase 3 fields
    db.update_session_deep_fields(
        "phase3-paginated",
        "Last msg",
        3,   // turn_count
        2, 4, 1, 0,  // tool counts
        "[]", "[]", // files_touched, skills_used
        15, 20, 30, // user_prompt_count, api_call_count, tool_call_count
        r#"["/a.rs"]"#, r#"["/b.rs"]"#, // files_read, files_edited
        1, 1, 0,    // counts
        120, 2,     // duration_seconds, commit_count
        None, // first_message_at
        // Phase 3.5: Full parser metrics
        0, 0, 0, 0, // token counts
        0,           // thinking_block_count
        None, None, None, // turn durations
        0, 0, 0, 0, // error/retry/compaction/hook_blocked
        0, 0, 0, 0, // progress counts
        None,        // summary_text
        1,           // parse_version
        1000,        // file_size
        1706200000,  // file_mtime
        0,           // lines_added
        0,           // lines_removed
        0,           // loc_source
        0,           // ai_lines_added
        0,           // ai_lines_removed
        None,        // work_type
        None,        // git_branch
        None,        // primary_model
        None,        // last_message_at
        None,        // first_user_prompt
    )
    .await
    .unwrap();

    // Test paginated retrieval includes Phase 3 fields
    let page = db.list_sessions_for_project("proj-paginated", 10, 0, "recent", &BranchFilter::All, false).await.unwrap();
    assert_eq!(page.sessions.len(), 1);

    let session = &page.sessions[0];
    assert_eq!(session.user_prompt_count, 15);
    assert_eq!(session.api_call_count, 20);
    assert_eq!(session.tool_call_count, 30);
    assert_eq!(session.files_read, vec!["/a.rs"]);
    assert_eq!(session.files_edited, vec!["/b.rs"]);
    assert_eq!(session.duration_seconds, 120);
    assert_eq!(session.commit_count, 2);
}

#[tokio::test]
async fn test_phase3_fields_default_to_zero() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert a session but don't run Pass 2
    db.insert_session_from_index(
        "no-deep-index",
        "proj-no-deep",
        "Project No Deep",
        "/tmp/proj",
        "/tmp/no-deep.jsonl",
        "Preview",
        None,
        5,
        1000,
        None,
        false,
        500,
    )
    .await
    .unwrap();

    // Retrieve and verify Phase 3 fields default to 0/empty
    let projects = db.list_projects().await.unwrap();
    let session = &projects[0].sessions[0];

    assert!(!session.deep_indexed, "Session should not be deep_indexed yet");
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
