#![allow(deprecated)]
//! Regression tests: polymorphic project_id / git_root filter matching.
//!
//! The sidebar sends `ProjectSummary.name` which is
//! `COALESCE(NULLIF(git_root, ''), project_id)` — so for 98%+ of sessions
//! the filter value is a `git_root` path (e.g. `/Users/test/project`), NOT
//! the encoded `project_id` (e.g. `-Users-test-project`).
//!
//! Every SQL WHERE clause that accepts a project filter MUST match on EITHER
//! `project_id` OR `git_root`. These tests guard against that invariant
//! regressing — which has happened 10+ times.

use chrono::Utc;
use claude_view_core::SessionInfo;
use claude_view_db::Database;

mod queries_shared;
use queries_shared::make_session;

const PROJECT_ID: &str = "-Users-test-project";
const GIT_ROOT: &str = "/Users/test/project";

/// Insert a session that has BOTH project_id and git_root set,
/// mimicking real-world data where 98%+ of sessions have git_root.
/// Sets non-zero counts so aggregate queries return > 0.
async fn setup_session_with_git_root(db: &Database) {
    let now = Utc::now().timestamp();
    let s = SessionInfo {
        git_root: Some(GIT_ROOT.to_string()),
        git_branch: Some("main".to_string()),
        modified_at: now - 100,
        files_edited_count: 3,
        total_input_tokens: Some(1000),
        total_output_tokens: Some(500),
        ..make_session("s-poly", PROJECT_ID, now - 100)
    };
    db.insert_session(&s, PROJECT_ID, "test-project")
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Dashboard stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_stats_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let stats = db.get_dashboard_stats(Some(GIT_ROOT), None).await.unwrap();
    assert!(
        stats.total_sessions > 0,
        "get_dashboard_stats: filtering by git_root must find the session"
    );
}

#[tokio::test]
async fn dashboard_stats_filter_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let stats = db
        .get_dashboard_stats(Some(PROJECT_ID), None)
        .await
        .unwrap();
    assert!(
        stats.total_sessions > 0,
        "get_dashboard_stats: filtering by project_id must also work"
    );
}

#[tokio::test]
async fn dashboard_stats_filter_by_git_root_and_branch() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let stats = db
        .get_dashboard_stats(Some(GIT_ROOT), Some("main"))
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 1);

    let stats = db
        .get_dashboard_stats(Some(GIT_ROOT), Some("nonexistent"))
        .await
        .unwrap();
    assert_eq!(stats.total_sessions, 0);
}

// ---------------------------------------------------------------------------
// Dashboard stats with range
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_stats_with_range_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let stats = db
        .get_dashboard_stats_with_range(Some(0), None, Some(GIT_ROOT), None)
        .await
        .unwrap();
    assert!(
        stats.total_sessions > 0,
        "get_dashboard_stats_with_range: git_root filter must work"
    );
}

// ---------------------------------------------------------------------------
// All-time metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn all_time_metrics_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let (sessions, _, _, _) = db.get_all_time_metrics(Some(GIT_ROOT), None).await.unwrap();
    assert!(
        sessions > 0,
        "get_all_time_metrics: git_root filter must find the session"
    );
}

#[tokio::test]
async fn all_time_metrics_filter_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let (sessions, _, _, _) = db
        .get_all_time_metrics(Some(PROJECT_ID), None)
        .await
        .unwrap();
    assert!(
        sessions > 0,
        "get_all_time_metrics: project_id filter must also work"
    );
}

// ---------------------------------------------------------------------------
// Oldest session date
// ---------------------------------------------------------------------------

#[tokio::test]
async fn oldest_session_date_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let oldest = db
        .get_oldest_session_date(Some(GIT_ROOT), None)
        .await
        .unwrap();
    assert!(
        oldest.is_some(),
        "get_oldest_session_date: git_root filter must find the session"
    );
}

#[tokio::test]
async fn oldest_session_date_filter_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let oldest = db
        .get_oldest_session_date(Some(PROJECT_ID), None)
        .await
        .unwrap();
    assert!(
        oldest.is_some(),
        "get_oldest_session_date: project_id filter must also work"
    );
}

// ---------------------------------------------------------------------------
// Trends
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trends_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let now = Utc::now().timestamp();
    let trends = db
        .get_trends_with_range(now - 86400 * 7, now, Some(GIT_ROOT), None)
        .await
        .unwrap();
    assert!(
        trends.session_count.current > 0,
        "get_trends_with_range: git_root filter must find the session"
    );
}

#[tokio::test]
async fn trends_filter_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let now = Utc::now().timestamp();
    let trends = db
        .get_trends_with_range(now - 86400 * 7, now, Some(PROJECT_ID), None)
        .await
        .unwrap();
    assert!(
        trends.session_count.current > 0,
        "get_trends_with_range: project_id filter must also work"
    );
}

// ---------------------------------------------------------------------------
// AI generation stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ai_generation_stats_filter_by_git_root() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let now = Utc::now().timestamp();
    let stats = db
        .get_ai_generation_stats(Some(0), Some(now + 86400), Some(GIT_ROOT), None)
        .await
        .unwrap();
    assert!(
        stats.files_created > 0,
        "get_ai_generation_stats: git_root filter must find the session"
    );
}

#[tokio::test]
async fn ai_generation_stats_filter_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();
    setup_session_with_git_root(&db).await;

    let now = Utc::now().timestamp();
    let stats = db
        .get_ai_generation_stats(Some(0), Some(now + 86400), Some(PROJECT_ID), None)
        .await
        .unwrap();
    assert!(
        stats.files_created > 0,
        "get_ai_generation_stats: project_id filter must also work"
    );
}

// ---------------------------------------------------------------------------
// Worktree consolidation: multiple project_ids share one git_root
// ---------------------------------------------------------------------------

#[tokio::test]
async fn worktree_consolidation_git_root_matches_all() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    let shared_git_root = "/Users/test/monorepo";

    // Two different project_ids (worktrees) sharing the same git_root
    let s1 = SessionInfo {
        git_root: Some(shared_git_root.to_string()),
        modified_at: now - 100,
        ..make_session("s-wt-1", "-Users-test-monorepo", now - 100)
    };
    let s2 = SessionInfo {
        git_root: Some(shared_git_root.to_string()),
        modified_at: now - 200,
        ..make_session("s-wt-2", "-Users-test-monorepo-worktree-feat", now - 200)
    };

    db.insert_session(&s1, "-Users-test-monorepo", "monorepo")
        .await
        .unwrap();
    db.insert_session(&s2, "-Users-test-monorepo-worktree-feat", "monorepo")
        .await
        .unwrap();

    // Filtering by git_root should find BOTH sessions
    let stats = db
        .get_dashboard_stats(Some(shared_git_root), None)
        .await
        .unwrap();
    assert_eq!(
        stats.total_sessions, 2,
        "worktree consolidation: git_root filter must match both worktree sessions"
    );

    // Filtering by one project_id should find only that one
    let stats = db
        .get_dashboard_stats(Some("-Users-test-monorepo"), None)
        .await
        .unwrap();
    assert_eq!(
        stats.total_sessions, 1,
        "project_id filter should match only its own session"
    );
}

// ---------------------------------------------------------------------------
// Null/empty git_root fallback to project_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_without_git_root_still_matches_by_project_id() {
    let db = Database::new_in_memory().await.unwrap();

    let now = Utc::now().timestamp();
    // Session with NO git_root (the ~2% case)
    let s = SessionInfo {
        git_root: None,
        modified_at: now - 100,
        ..make_session("s-no-root", "-Users-test-legacy", now - 100)
    };
    db.insert_session(&s, "-Users-test-legacy", "legacy")
        .await
        .unwrap();

    // Must still match by project_id
    let stats = db
        .get_dashboard_stats(Some("-Users-test-legacy"), None)
        .await
        .unwrap();
    assert_eq!(
        stats.total_sessions, 1,
        "session without git_root must still be findable by project_id"
    );

    // git_root path should NOT accidentally match (it doesn't exist)
    let stats = db
        .get_dashboard_stats(Some("/Users/test/legacy"), None)
        .await
        .unwrap();
    assert_eq!(
        stats.total_sessions, 0,
        "nonexistent git_root path should not match"
    );
}
