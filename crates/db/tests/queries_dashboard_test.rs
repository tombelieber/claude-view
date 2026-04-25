#![allow(deprecated)]
//! Integration tests for Database dashboard/analytics query methods.

use chrono::Utc;
use claude_view_core::SessionInfo;
use claude_view_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_get_dashboard_stats() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        modified_at: now - 86400,
        ..make_session("sess-1", "project-a", now - 86400)
    };
    let s2 = SessionInfo {
        modified_at: now - 172800,
        ..make_session("sess-2", "project-a", now - 172800)
    };
    let s3 = SessionInfo {
        modified_at: now - 86400,
        ..make_session("sess-3", "project-b", now - 86400)
    };

    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s2, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s3, "project-b", "Project B")
        .await
        .unwrap();

    let stats = db.get_dashboard_stats(None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 3);
    assert_eq!(stats.total_projects, 2);
    assert!(!stats.heatmap.is_empty());
    assert!(!stats.top_projects.is_empty());
    assert_eq!(stats.top_projects[0].session_count, 2); // project-a has most
    assert!(stats.tool_totals.edit > 0); // sessions have tool counts
}

#[tokio::test]
async fn test_get_dashboard_stats_with_range() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert 3 sessions at different timestamps
    let s1 = SessionInfo {
        modified_at: 1000,
        ..make_session("sess-1", "project-a", 1000)
    };
    let s2 = SessionInfo {
        modified_at: 2000,
        ..make_session("sess-2", "project-a", 2000)
    };
    let s3 = SessionInfo {
        modified_at: 3000,
        ..make_session("sess-3", "project-b", 3000)
    };

    db.insert_session(&s1, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s2, "project-a", "Project A")
        .await
        .unwrap();
    db.insert_session(&s3, "project-b", "Project B")
        .await
        .unwrap();

    // Filter to only sess-2 (last_message_at = 2000)
    let stats = db
        .get_dashboard_stats_with_range(Some(1500), Some(2500), None, None)
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 1, "Only 1 session within range");
    assert_eq!(stats.total_projects, 1, "Only 1 project within range");

    // Tool totals should reflect only the filtered session
    assert_eq!(stats.tool_totals.edit, 5);
    assert_eq!(stats.tool_totals.read, 10);
    assert_eq!(stats.tool_totals.bash, 3);
    assert_eq!(stats.tool_totals.write, 2);

    // Full range should include all 3
    let all = db
        .get_dashboard_stats_with_range(None, None, None, None)
        .await
        .unwrap();
    assert_eq!(all.total_sessions, 3);
    assert_eq!(all.total_projects, 2);
}

#[tokio::test]
async fn test_get_all_time_metrics() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert + deep-index 2 sessions with known files_edited_count (single UPSERT each)
    claude_view_db::test_support::SessionSeedBuilder::new("metrics-1")
        .project_id("proj-m")
        .project_display_name("Project M")
        .project_path("/tmp/proj-m")
        .file_path("/tmp/m1.jsonl")
        .preview("Preview 1")
        .message_count(10)
        .modified_at(1000)
        .size_bytes(2000)
        .turn_count(3)
        .total_cost_usd(0.0)
        .with_parsed(|s| {
            s.last_message = "Last msg 1".to_string();
            s.tool_counts_edit = 2;
            s.tool_counts_read = 4;
            s.tool_counts_bash = 1;
            s.user_prompt_count = 5;
            s.api_call_count = 8;
            s.tool_call_count = 15;
            s.files_edited_count = 3;
            s.duration_seconds = 120;
            s.commit_count = 1;
            s.first_message_at = 900;
            s.parse_version = 1;
            s.file_size_at_index = 2000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    claude_view_db::test_support::SessionSeedBuilder::new("metrics-2")
        .project_id("proj-m")
        .project_display_name("Project M")
        .project_path("/tmp/proj-m")
        .file_path("/tmp/m2.jsonl")
        .preview("Preview 2")
        .message_count(5)
        .modified_at(2000)
        .size_bytes(1000)
        .turn_count(2)
        .total_cost_usd(0.0)
        .with_parsed(|s| {
            s.last_message = "Last msg 2".to_string();
            s.tool_counts_edit = 1;
            s.tool_counts_read = 2;
            s.tool_counts_write = 1;
            s.user_prompt_count = 3;
            s.api_call_count = 5;
            s.tool_call_count = 10;
            s.files_edited_count = 2;
            s.duration_seconds = 60;
            s.first_message_at = 1900;
            s.parse_version = 1;
            s.file_size_at_index = 1000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    let (session_count, total_tokens, total_files_edited, commit_count) =
        db.get_all_time_metrics(None, None).await.unwrap();

    assert_eq!(session_count, 2, "Should have 2 sessions");
    // Tokens come from valid_sessions.total_input_tokens+total_output_tokens;
    // this test seeds neither, so the sum is 0.
    assert_eq!(total_tokens, 0, "No token data seeded, so 0 tokens");
    // files_edited_count: 3 + 2 = 5
    assert_eq!(total_files_edited, 5, "Sum of files_edited_count");
    // commit_count from session_commits table (not populated in this test)
    assert_eq!(commit_count, 0, "No session_commits data");
}

#[tokio::test]
async fn test_get_dashboard_stats_with_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        duration_seconds: 600,
        ..make_session("sess-filter-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let s2 = SessionInfo {
        git_branch: Some("develop".to_string()),
        duration_seconds: 300,
        ..make_session("sess-filter-b", "proj-y", now - 200)
    };
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // Set longest_task_seconds (not written by insert_session, only by parallel indexer)
    sqlx::query(
        "UPDATE session_stats SET longest_task_seconds = 400, longest_task_preview = 'Longest prompt A' WHERE session_id = 'sess-filter-a'",
    )
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE session_stats SET longest_task_seconds = 200 WHERE session_id = 'sess-filter-b'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // No filter — should see both
    let stats = db.get_dashboard_stats(None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.total_projects, 2);

    // Project filter — should see only proj-x
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_projects, 1);

    // Project + branch filter — matching
    let stats = db
        .get_dashboard_stats(Some("proj-x"), Some("main"))
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Project + wrong branch = 0
    let stats = db
        .get_dashboard_stats(Some("proj-x"), Some("develop"))
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 0);

    // Branch-only filter (no project)
    let stats = db.get_dashboard_stats(None, Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Tool totals should reflect filtered sessions
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.tool_totals.edit, 5); // make_session sets edit=5

    // Longest tasks should be filtered (longest_task_seconds > 0, so they appear)
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.longest_sessions.len(), 1, "only proj-x's session");
    assert_eq!(stats.longest_sessions[0].id, "sess-filter-a");
    assert_eq!(stats.longest_sessions[0].preview, "Longest prompt A");

    let stats = db
        .get_dashboard_stats(Some("proj-x"), Some("develop"))
        .await
        .unwrap();
    assert_eq!(
        stats.longest_sessions.len(),
        0,
        "wrong branch = no sessions"
    );
}

#[tokio::test]
async fn test_get_all_time_metrics_with_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        ..make_session("sess-atm-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let mut s2 = make_session("sess-atm-b", "proj-y", now - 200);
    s2.git_branch = Some("develop".to_string());
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter
    let (sessions, _, _, _) = db.get_all_time_metrics(None, None).await.unwrap();
    assert_eq!(sessions, 2);

    // Project filter
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), None).await.unwrap();
    assert_eq!(sessions, 1);

    // Project + branch filter
    let (sessions, _, _, _) = db
        .get_all_time_metrics(Some("proj-x"), Some("main"))
        .await
        .unwrap();
    assert_eq!(sessions, 1);

    // Project + wrong branch
    let (sessions, _, _, _) = db
        .get_all_time_metrics(Some("proj-x"), Some("develop"))
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn test_get_oldest_session_date_with_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo {
        git_branch: Some("main".to_string()),
        ..make_session("sess-old-a", "proj-x", now - 200)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    let mut s2 = make_session("sess-old-b", "proj-y", now - 100);
    s2.git_branch = Some("develop".to_string());
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter — oldest across all
    let oldest = db.get_oldest_session_date(None, None).await.unwrap();
    assert!(oldest.is_some());

    // Filter proj-y — should get session_b's timestamp
    let oldest = db
        .get_oldest_session_date(Some("proj-y"), None)
        .await
        .unwrap();
    assert!(oldest.is_some());

    // Filter non-existent project — should be None
    let oldest = db
        .get_oldest_session_date(Some("proj-z"), None)
        .await
        .unwrap();
    assert!(oldest.is_none());
}

#[tokio::test]
async fn test_get_dashboard_stats_with_range_and_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    // 3 sessions: proj-x at t=1000, proj-x at t=2000, proj-y at t=2000
    let s1 = SessionInfo {
        modified_at: 1000,
        git_branch: Some("main".to_string()),
        ..make_session("sess-rp-1", "proj-x", 1000)
    };
    let s2 = SessionInfo {
        modified_at: 2000,
        git_branch: Some("main".to_string()),
        ..make_session("sess-rp-2", "proj-x", 2000)
    };
    let mut s3 = SessionInfo {
        modified_at: 2000,
        ..make_session("sess-rp-3", "proj-y", 2000)
    };
    s3.git_branch = Some("develop".to_string());

    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();
    db.insert_session(&s2, "proj-x", "Project X").await.unwrap();
    db.insert_session(&s3, "proj-y", "Project Y").await.unwrap();

    // Time range 1500-2500 + no project filter: sess-rp-2 and sess-rp-3
    let stats = db
        .get_dashboard_stats_with_range(Some(1500), Some(2500), None, None)
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 2);

    // Time range 1500-2500 + project filter proj-x: only sess-rp-2
    let stats = db
        .get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), None)
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Time range 1500-2500 + project proj-x + branch develop: 0
    let stats = db
        .get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), Some("develop"))
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 0);
}
