//! Integration tests for Database dashboard/analytics query methods.

use chrono::Utc;
use vibe_recall_core::SessionInfo;
use vibe_recall_db::Database;

mod queries_shared;
use queries_shared::make_session;

#[tokio::test]
async fn test_get_dashboard_stats() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let s1 = SessionInfo { modified_at: now - 86400, ..make_session("sess-1", "project-a", now - 86400) };
    let s2 = SessionInfo { modified_at: now - 172800, ..make_session("sess-2", "project-a", now - 172800) };
    let s3 = SessionInfo { modified_at: now - 86400, ..make_session("sess-3", "project-b", now - 86400) };

    db.insert_session(&s1, "project-a", "Project A").await.unwrap();
    db.insert_session(&s2, "project-a", "Project A").await.unwrap();
    db.insert_session(&s3, "project-b", "Project B").await.unwrap();

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

    // Insert 2 sessions with known files_edited_count
    db.insert_session_from_index(
        "metrics-1",
        "proj-m",
        "Project M",
        "/tmp/proj-m",
        "/tmp/m1.jsonl",
        "Preview 1",
        None,
        10,
        1000,
        None,
        false,
        2000,
    )
    .await
    .unwrap();

    db.insert_session_from_index(
        "metrics-2",
        "proj-m",
        "Project M",
        "/tmp/proj-m",
        "/tmp/m2.jsonl",
        "Preview 2",
        None,
        5,
        2000,
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Update deep fields to set files_edited_count
    db.update_session_deep_fields(
        "metrics-1",
        "Last msg 1",
        3,
        2, 4, 1, 0,
        "[]", "[]",
        5, 8, 15,
        "[]", "[]",
        0, 3, 0,
        120, 1,
        Some(900),
        0, 0, 0, 0,
        0,
        None, None, None,
        0, 0, 0, 0,
        0, 0, 0, 0,
        None,
        1,
        2000,
        1706200000,
        0, 0, 0, // lines_added, lines_removed, loc_source
        0, 0,    // ai_lines_added, ai_lines_removed
        None,    // work_type
        None,    // git_branch
        None, // primary_model
        None, // last_message_at
        None, // first_user_prompt
    )
    .await
    .unwrap();

    db.update_session_deep_fields(
        "metrics-2",
        "Last msg 2",
        2,
        1, 2, 0, 1,
        "[]", "[]",
        3, 5, 10,
        "[]", "[]",
        0, 2, 0,
        60, 0,
        Some(1900),
        0, 0, 0, 0,
        0,
        None, None, None,
        0, 0, 0, 0,
        0, 0, 0, 0,
        None,
        1,
        1000,
        1706200000,
        0, 0, 0, // lines_added, lines_removed, loc_source
        0, 0,    // ai_lines_added, ai_lines_removed
        None,    // work_type
        None,    // git_branch
        None, // primary_model
        None, // last_message_at
        None, // first_user_prompt
    )
    .await
    .unwrap();

    let (session_count, total_tokens, total_files_edited, commit_count) =
        db.get_all_time_metrics(None, None).await.unwrap();

    assert_eq!(session_count, 2, "Should have 2 sessions");
    // Tokens come from turns table, which we didn't populate
    assert_eq!(total_tokens, 0, "No turns data, so 0 tokens");
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

    // No filter — should see both
    let stats = db.get_dashboard_stats(None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.total_projects, 2);

    // Project filter — should see only proj-x
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.total_projects, 1);

    // Project + branch filter — matching
    let stats = db.get_dashboard_stats(Some("proj-x"), Some("main")).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Project + wrong branch = 0
    let stats = db.get_dashboard_stats(Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 0);

    // Branch-only filter (no project)
    let stats = db.get_dashboard_stats(None, Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Tool totals should reflect filtered sessions
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.tool_totals.edit, 5); // make_session sets edit=5

    // Longest sessions should be filtered (duration_seconds > 0, so they appear)
    let stats = db.get_dashboard_stats(Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.longest_sessions.len(), 1, "only proj-x's session");
    assert_eq!(stats.longest_sessions[0].id, "sess-filter-a");

    let stats = db.get_dashboard_stats(Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.longest_sessions.len(), 0, "wrong branch = no sessions");
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
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), Some("main")).await.unwrap();
    assert_eq!(sessions, 1);

    // Project + wrong branch
    let (sessions, _, _, _) = db.get_all_time_metrics(Some("proj-x"), Some("develop")).await.unwrap();
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
    let oldest = db.get_oldest_session_date(Some("proj-y"), None).await.unwrap();
    assert!(oldest.is_some());

    // Filter non-existent project — should be None
    let oldest = db.get_oldest_session_date(Some("proj-z"), None).await.unwrap();
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
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), None, None).await.unwrap();
    assert_eq!(stats.total_sessions, 2);

    // Time range 1500-2500 + project filter proj-x: only sess-rp-2
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), None).await.unwrap();
    assert_eq!(stats.total_sessions, 1);

    // Time range 1500-2500 + project proj-x + branch develop: 0
    let stats = db.get_dashboard_stats_with_range(Some(1500), Some(2500), Some("proj-x"), Some("develop")).await.unwrap();
    assert_eq!(stats.total_sessions, 0);
}
