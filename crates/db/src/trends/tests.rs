//! Tests for trend calculations and index metadata.

use super::*;
use crate::Database;
use chrono::{Datelike, TimeZone, Timelike, Utc, Weekday};

// ============================================================================
// TrendMetric unit tests (A4.3 acceptance tests)
// ============================================================================

#[test]
fn test_trend_metric_positive_delta() {
    // 120 vs 100 -> delta 20, percent 20.0
    let metric = TrendMetric::new(120, 100);
    assert_eq!(metric.current, 120);
    assert_eq!(metric.previous, 100);
    assert_eq!(metric.delta, 20);
    assert_eq!(metric.delta_percent, Some(20.0));
}

#[test]
fn test_trend_metric_negative_delta() {
    // 100 vs 120 -> delta -20, percent -16.7 (rounded)
    let metric = TrendMetric::new(100, 120);
    assert_eq!(metric.current, 100);
    assert_eq!(metric.previous, 120);
    assert_eq!(metric.delta, -20);
    assert_eq!(metric.delta_percent, Some(-16.7));
}

#[test]
fn test_trend_metric_previous_zero() {
    // 50 vs 0 -> delta 50, percent None
    let metric = TrendMetric::new(50, 0);
    assert_eq!(metric.current, 50);
    assert_eq!(metric.previous, 0);
    assert_eq!(metric.delta, 50);
    assert_eq!(metric.delta_percent, None);
}

#[test]
fn test_trend_metric_both_zero() {
    // 0 vs 0 -> delta 0, percent None
    let metric = TrendMetric::new(0, 0);
    assert_eq!(metric.current, 0);
    assert_eq!(metric.previous, 0);
    assert_eq!(metric.delta, 0);
    assert_eq!(metric.delta_percent, None);
}

#[test]
fn test_trend_metric_negative_hundred_percent() {
    // 0 vs 50 -> delta -50, percent -100.0
    let metric = TrendMetric::new(0, 50);
    assert_eq!(metric.current, 0);
    assert_eq!(metric.previous, 50);
    assert_eq!(metric.delta, -50);
    assert_eq!(metric.delta_percent, Some(-100.0));
}

#[test]
fn test_trend_metric_fractional_percent_rounds() {
    // 133 vs 100 -> delta 33, percent 33.0 (not 33.333...)
    let metric = TrendMetric::new(133, 100);
    assert_eq!(metric.delta, 33);
    assert_eq!(metric.delta_percent, Some(33.0));

    // 125 vs 100 -> delta 25, percent 25.0
    let metric = TrendMetric::new(125, 100);
    assert_eq!(metric.delta_percent, Some(25.0));

    // 115 vs 100 -> delta 15, percent 15.0
    let metric = TrendMetric::new(115, 100);
    assert_eq!(metric.delta_percent, Some(15.0));
}

// ============================================================================
// Time bounds tests
// ============================================================================

#[allow(deprecated)]
#[test]
fn test_current_week_bounds_format() {
    let (start, end) = current_week_bounds();

    // Start should be before end
    assert!(start < end, "Start should be before end");

    // Start should be on a Monday at midnight
    let start_dt = Utc.timestamp_opt(start, 0).unwrap();
    assert_eq!(start_dt.weekday(), Weekday::Mon, "Start should be a Monday");
    assert_eq!(start_dt.hour(), 0, "Start should be at 00:00");
    assert_eq!(start_dt.minute(), 0);
    assert_eq!(start_dt.second(), 0);

    // End should be approximately now (within 5 seconds)
    let now = Utc::now().timestamp();
    assert!((end - now).abs() < 5, "End should be approximately now");
}

#[allow(deprecated)]
#[test]
fn test_previous_week_bounds_format() {
    let (start, end) = previous_week_bounds();

    // Start should be before end
    assert!(start < end, "Start should be before end");

    // Start should be on a Monday at midnight
    let start_dt = Utc.timestamp_opt(start, 0).unwrap();
    assert_eq!(start_dt.weekday(), Weekday::Mon, "Start should be a Monday");
    assert_eq!(start_dt.hour(), 0, "Start should be at 00:00");

    // End should be on a Sunday at 23:59:59
    let end_dt = Utc.timestamp_opt(end, 0).unwrap();
    assert_eq!(end_dt.weekday(), Weekday::Sun, "End should be a Sunday");
    assert_eq!(end_dt.hour(), 23, "End should be at 23:59:59");
    assert_eq!(end_dt.minute(), 59);
    assert_eq!(end_dt.second(), 59);

    // Duration should be exactly 7 days minus 1 second
    let duration = end - start;
    assert_eq!(
        duration,
        7 * 24 * 60 * 60 - 1,
        "Duration should be 7 days minus 1 second"
    );
}

#[test]
fn test_week_bounds_relationship() {
    let (curr_start, _curr_end) = current_week_bounds();
    let (_prev_start, prev_end) = previous_week_bounds();

    // Previous week should end exactly 1 second before current week starts
    assert_eq!(
        prev_end + 1,
        curr_start,
        "Previous week should end 1 second before current week starts"
    );
}

// ============================================================================
// Database tests for trends
// ============================================================================

#[tokio::test]
async fn test_get_week_trends_empty_db() {
    let db = Database::new_in_memory().await.unwrap();

    let trends = db.get_week_trends().await.unwrap();

    // All metrics should be 0/0
    assert_eq!(trends.session_count.current, 0);
    assert_eq!(trends.session_count.previous, 0);
    assert_eq!(trends.total_tokens.current, 0);
    assert_eq!(trends.total_files_edited.current, 0);
    assert_eq!(trends.commit_link_count.current, 0);
}

#[tokio::test]
async fn test_get_week_trends_with_data() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert sessions in current week
    let (curr_start, _) = current_week_bounds();

    // Insert + deep-index a session in current week via single UPSERT
    crate::test_support::SessionSeedBuilder::new("sess-curr-1")
        .project_id("project-a")
        .project_display_name("Project A")
        .project_path("/tmp/project-a")
        .file_path("/tmp/curr1.jsonl")
        .preview("Current week session")
        .message_count(5)
        .modified_at(curr_start + 3600) // 1 hour into current week
        .size_bytes(1000)
        .turn_count(3)
        .total_cost_usd(0.0)
        .with_parsed(|s| {
            s.last_message = "Last message".to_string();
            s.tool_counts_edit = 2;
            s.tool_counts_read = 5;
            s.tool_counts_bash = 1;
            s.tool_counts_write = 1;
            s.user_prompt_count = 10;
            s.api_call_count = 8;
            s.tool_call_count = 15;
            s.files_read = r#"["/a.rs"]"#.to_string();
            s.files_edited = r#"["/b.rs", "/c.rs"]"#.to_string();
            s.files_read_count = 1;
            s.files_edited_count = 2;
            s.duration_seconds = 600;
            s.commit_count = 1;
            s.parse_version = 1;
            s.file_size_at_index = 1000;
            s.file_mtime_at_index = 1706200000;
        })
        .seed(&db)
        .await
        .unwrap();

    let trends = db.get_week_trends().await.unwrap();

    // Should have 1 session in current week
    assert_eq!(trends.session_count.current, 1);
    assert_eq!(trends.session_count.previous, 0);
    assert_eq!(trends.session_count.delta, 1);
    assert_eq!(trends.session_count.delta_percent, None); // prev is 0

    // Should have files edited
    assert_eq!(trends.total_files_edited.current, 2);
}

// ============================================================================
// Index metadata tests
// ============================================================================

#[tokio::test]
async fn test_get_index_metadata_default() {
    let db = Database::new_in_memory().await.unwrap();

    let metadata = db.get_index_metadata().await.unwrap();

    // Default values
    assert_eq!(metadata.last_indexed_at, None);
    assert_eq!(metadata.last_index_duration_ms, None);
    assert_eq!(metadata.sessions_indexed, 0);
    assert_eq!(metadata.projects_indexed, 0);
    assert_eq!(metadata.last_git_sync_at, None);
    assert_eq!(metadata.commits_found, 0);
    assert_eq!(metadata.links_created, 0);
    assert!(metadata.updated_at > 0);
}

#[tokio::test]
async fn test_update_index_metadata_on_success() {
    let db = Database::new_in_memory().await.unwrap();

    // Update index metadata
    db.update_index_metadata_on_success(1500, 100, 5)
        .await
        .unwrap();

    let metadata = db.get_index_metadata().await.unwrap();

    assert!(metadata.last_indexed_at.is_some());
    assert_eq!(metadata.last_index_duration_ms, Some(1500));
    assert_eq!(metadata.sessions_indexed, 100);
    assert_eq!(metadata.projects_indexed, 5);
    // Git sync should still be None (not updated)
    assert_eq!(metadata.last_git_sync_at, None);
}

#[tokio::test]
async fn test_update_git_sync_metadata_on_success() {
    let db = Database::new_in_memory().await.unwrap();

    // Update git sync metadata
    db.update_git_sync_metadata_on_success(250, 45)
        .await
        .unwrap();

    let metadata = db.get_index_metadata().await.unwrap();

    assert!(metadata.last_git_sync_at.is_some());
    assert_eq!(metadata.commits_found, 250);
    assert_eq!(metadata.links_created, 45);
    // Index metadata should still be None (not updated)
    assert_eq!(metadata.last_indexed_at, None);
}

#[tokio::test]
async fn test_update_both_metadata() {
    let db = Database::new_in_memory().await.unwrap();

    // Update index first
    db.update_index_metadata_on_success(1200, 50, 3)
        .await
        .unwrap();

    // Then update git sync
    db.update_git_sync_metadata_on_success(100, 20)
        .await
        .unwrap();

    let metadata = db.get_index_metadata().await.unwrap();

    // Both should be set
    assert!(metadata.last_indexed_at.is_some());
    assert_eq!(metadata.last_index_duration_ms, Some(1200));
    assert_eq!(metadata.sessions_indexed, 50);
    assert_eq!(metadata.projects_indexed, 3);

    assert!(metadata.last_git_sync_at.is_some());
    assert_eq!(metadata.commits_found, 100);
    assert_eq!(metadata.links_created, 20);
}

#[tokio::test]
async fn test_metadata_updates_preserve_other_fields() {
    let db = Database::new_in_memory().await.unwrap();

    // Set initial values for both
    db.update_index_metadata_on_success(1000, 30, 2)
        .await
        .unwrap();
    db.update_git_sync_metadata_on_success(80, 15)
        .await
        .unwrap();

    let first_metadata = db.get_index_metadata().await.unwrap();

    // Update only index metadata again
    db.update_index_metadata_on_success(2000, 60, 4)
        .await
        .unwrap();

    let second_metadata = db.get_index_metadata().await.unwrap();

    // Index metadata should be updated
    assert_eq!(second_metadata.last_index_duration_ms, Some(2000));
    assert_eq!(second_metadata.sessions_indexed, 60);
    assert_eq!(second_metadata.projects_indexed, 4);

    // Git sync metadata should be preserved
    assert_eq!(second_metadata.commits_found, 80);
    assert_eq!(second_metadata.links_created, 15);
    // Note: last_git_sync_at timestamp might change due to updated_at, but the data is preserved
    assert_eq!(second_metadata.commits_found, first_metadata.commits_found);
}

#[tokio::test]
async fn test_index_metadata_serializes_correctly() {
    let db = Database::new_in_memory().await.unwrap();

    db.update_index_metadata_on_success(1500, 100, 5)
        .await
        .unwrap();

    let metadata = db.get_index_metadata().await.unwrap();
    let json = serde_json::to_string(&metadata).unwrap();

    // Should use camelCase
    assert!(json.contains("\"lastIndexedAt\""));
    assert!(json.contains("\"lastIndexDurationMs\""));
    assert!(json.contains("\"sessionsIndexed\""));
    assert!(json.contains("\"projectsIndexed\""));
    assert!(json.contains("\"lastGitSyncAt\""));
    assert!(json.contains("\"commitsFound\""));
    assert!(json.contains("\"linksCreated\""));
    assert!(json.contains("\"updatedAt\""));
}

#[tokio::test]
async fn test_trend_metric_serializes_correctly() {
    let metric = TrendMetric::new(120, 100);
    let json = serde_json::to_string(&metric).unwrap();

    // Should use camelCase
    assert!(json.contains("\"current\":120"));
    assert!(json.contains("\"previous\":100"));
    assert!(json.contains("\"delta\":20"));
    assert!(json.contains("\"deltaPercent\":20.0"));
}

#[tokio::test]
async fn test_trend_metric_null_delta_percent_serializes() {
    let metric = TrendMetric::new(50, 0);
    let json = serde_json::to_string(&metric).unwrap();

    // deltaPercent should be null
    assert!(json.contains("\"deltaPercent\":null"));
}

// ============================================================================
// Trends with range and project/branch filter tests
// ============================================================================

fn make_session(id: &str, project: &str, modified_at: i64) -> claude_view_core::SessionInfo {
    claude_view_core::SessionInfo {
        id: id.to_string(),
        project: project.to_string(),
        project_path: format!("/home/user/{}", project),
        display_name: project.to_string(),
        git_root: None,
        file_path: format!("/home/user/.claude/projects/{}/{}.jsonl", project, id),
        modified_at,
        size_bytes: 2048,
        preview: format!("Preview for {}", id),
        last_message: format!("Last message for {}", id),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: claude_view_core::ToolCounts {
            edit: 5,
            read: 10,
            bash: 3,
            write: 2,
        },
        message_count: 20,
        turn_count: 8,
        summary: None,
        git_branch: None,
        is_sidechain: false,
        deep_indexed: false,
        total_input_tokens: None,
        total_output_tokens: None,
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: None,
        primary_model: None,
        user_prompt_count: 0,
        api_call_count: 0,
        tool_call_count: 0,
        files_read: vec![],
        files_edited: vec![],
        files_read_count: 0,
        files_edited_count: 0,
        reedited_files_count: 0,
        duration_seconds: 0,
        commit_count: 0,
        thinking_block_count: 0,
        turn_duration_avg_ms: None,
        turn_duration_max_ms: None,
        api_error_count: 0,
        compaction_count: 0,
        agent_spawn_count: 0,
        bash_progress_count: 0,
        hook_progress_count: 0,
        mcp_progress_count: 0,

        parse_version: 0,
        lines_added: 0,
        lines_removed: 0,
        loc_source: 0,
        category_l1: None,
        category_l2: None,
        category_l3: None,
        category_confidence: None,
        category_source: None,
        classified_at: None,
        prompt_word_count: None,
        correction_count: 0,
        same_file_edit_count: 0,
        total_task_time_seconds: None,
        longest_task_seconds: None,
        longest_task_preview: None,
        first_message_at: None,
        total_cost_usd: None,
        slug: None,
        entrypoint: None,
    }
}

#[tokio::test]
async fn test_get_trends_with_range_and_project_filter() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let from = now - 7 * 86400;
    let to = now;

    // proj-x session within range
    let s1 = claude_view_core::SessionInfo {
        git_branch: Some("main".to_string()),
        user_prompt_count: 5,
        files_edited_count: 3,
        reedited_files_count: 1,
        ..make_session("sess-trend-a", "proj-x", now - 100)
    };
    db.insert_session(&s1, "proj-x", "Project X").await.unwrap();

    // proj-y session within range
    let s2 = claude_view_core::SessionInfo {
        git_branch: Some("develop".to_string()),
        user_prompt_count: 10,
        files_edited_count: 6,
        reedited_files_count: 2,
        ..make_session("sess-trend-b", "proj-y", now - 200)
    };
    db.insert_session(&s2, "proj-y", "Project Y").await.unwrap();

    // No filter — trends include both sessions
    let trends = db
        .get_trends_with_range(from, to, None, None)
        .await
        .unwrap();
    assert_eq!(trends.session_count.current, 2);
    assert_eq!(trends.total_files_edited.current, 9); // 3 + 6

    // Project filter — only proj-x
    let trends = db
        .get_trends_with_range(from, to, Some("proj-x"), None)
        .await
        .unwrap();
    assert_eq!(trends.session_count.current, 1);
    assert_eq!(trends.total_files_edited.current, 3);

    // Project + branch filter
    let trends = db
        .get_trends_with_range(from, to, Some("proj-x"), Some("main"))
        .await
        .unwrap();
    assert_eq!(trends.session_count.current, 1);

    // Project + wrong branch = 0
    let trends = db
        .get_trends_with_range(from, to, Some("proj-x"), Some("develop"))
        .await
        .unwrap();
    assert_eq!(trends.session_count.current, 0);
    assert_eq!(trends.total_files_edited.current, 0);
}
