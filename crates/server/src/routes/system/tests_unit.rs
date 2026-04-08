//! Unit and database-level tests for system module.

use super::*;
use claude_view_db::{Database, HealthStatus};

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB")
}

// ========================================================================
// Performance calculation tests
// ========================================================================

#[test]
fn test_calculate_performance_with_data() {
    let metadata = claude_view_db::IndexMetadata {
        last_indexed_at: Some(1000),
        last_index_duration_ms: Some(2000),
        sessions_indexed: 1000,
        projects_indexed: 10,
        last_git_sync_at: None,
        commits_found: 0,
        links_created: 0,
        updated_at: 1000,
        git_sync_interval_secs: 60,
    };
    let storage = claude_view_db::SystemStorageStats {
        jsonl_bytes: 10_000_000,
        index_bytes: 0,
        db_bytes: 0,
        cache_bytes: 0,
        total_bytes: 10_000_000,
    };

    let perf = handlers::calculate_performance(&metadata, &storage);
    assert_eq!(perf.last_index_duration_ms, Some(2000));
    // 10MB in 2 seconds = 5MB/s = 5_000_000 bytes/sec
    assert_eq!(perf.throughput_bytes_per_sec, Some(5_000_000));
    // 1000 sessions in 2 seconds = 500 sessions/sec
    assert!((perf.sessions_per_sec.unwrap() - 500.0).abs() < 0.01);
}

#[test]
fn test_calculate_performance_empty() {
    let metadata = claude_view_db::IndexMetadata {
        last_indexed_at: None,
        last_index_duration_ms: None,
        sessions_indexed: 0,
        projects_indexed: 0,
        last_git_sync_at: None,
        commits_found: 0,
        links_created: 0,
        updated_at: 0,
        git_sync_interval_secs: 60,
    };
    let storage = claude_view_db::SystemStorageStats {
        jsonl_bytes: 0,
        index_bytes: 0,
        db_bytes: 0,
        cache_bytes: 0,
        total_bytes: 0,
    };

    let perf = handlers::calculate_performance(&metadata, &storage);
    assert!(perf.last_index_duration_ms.is_none());
    assert!(perf.throughput_bytes_per_sec.is_none());
    assert!(perf.sessions_per_sec.is_none());
}

#[test]
fn test_calculate_performance_zero_duration() {
    let metadata = claude_view_db::IndexMetadata {
        last_indexed_at: Some(1000),
        last_index_duration_ms: Some(0),
        sessions_indexed: 100,
        projects_indexed: 5,
        last_git_sync_at: None,
        commits_found: 0,
        links_created: 0,
        updated_at: 1000,
        git_sync_interval_secs: 60,
    };
    let storage = claude_view_db::SystemStorageStats {
        jsonl_bytes: 1000,
        index_bytes: 0,
        db_bytes: 0,
        cache_bytes: 0,
        total_bytes: 1000,
    };

    let perf = handlers::calculate_performance(&metadata, &storage);
    // With 0 duration, throughput should be None (div by zero guard)
    assert!(perf.throughput_bytes_per_sec.is_none());
    assert!(perf.sessions_per_sec.is_none());
}

// ========================================================================
// Health status tests
// ========================================================================

#[tokio::test]
async fn test_health_status_healthy() {
    let db = test_db().await;
    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.status, HealthStatus::Healthy);
}

#[tokio::test]
async fn test_health_status_warning_on_failed_runs() {
    let db = test_db().await;

    // Create one failed index run
    let run_id = db.create_index_run("full", None, None).await.unwrap();
    db.fail_index_run(run_id, "test error").await.unwrap();

    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.errors_count, 1);
    assert_eq!(health.status, HealthStatus::Warning);
}

#[tokio::test]
async fn test_health_status_error_on_many_failed_runs() {
    let db = test_db().await;

    // Create 10+ failed index runs
    for _ in 0..10 {
        let run_id = db.create_index_run("full", None, None).await.unwrap();
        db.fail_index_run(run_id, "test error").await.unwrap();
    }

    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.errors_count, 10);
    assert_eq!(health.status, HealthStatus::Error);
}

// ========================================================================
// Storage stats tests
// ========================================================================

#[tokio::test]
async fn test_storage_stats_empty_db() {
    let db = test_db().await;
    let stats = db.get_storage_stats().await.unwrap();
    assert_eq!(stats.jsonl_bytes, 0);
    assert_eq!(stats.index_bytes, 0);
    // In-memory DB has empty path, so db_bytes = 0
    assert_eq!(stats.db_bytes, 0);
    assert_eq!(stats.cache_bytes, 0);
    assert_eq!(stats.total_bytes, 0);
}

// ========================================================================
// Classification status tests
// ========================================================================

#[tokio::test]
async fn test_classification_status_empty() {
    let db = test_db().await;
    let status = db.get_classification_status().await.unwrap();
    assert_eq!(status.classified_count, 0);
    assert_eq!(status.unclassified_count, 0);
    assert!(!status.is_running);
    assert!(status.progress.is_none());
}

#[tokio::test]
async fn test_classification_status_with_active_job() {
    let db = test_db().await;

    // Insert a session
    db.insert_session_from_index(
        "sess-1",
        "project-a",
        "Project A",
        "/tmp/project-a",
        "/tmp/sess1.jsonl",
        "Test session",
        None,
        5,
        chrono::Utc::now().timestamp(),
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Create a running classification job
    let _job_id = db
        .create_classification_job(1, "claude-cli", "haiku")
        .await
        .unwrap();

    let status = db.get_classification_status().await.unwrap();
    assert!(status.is_running);
    assert_eq!(status.unclassified_count, 1);
}

// ========================================================================
// Reset tests
// ========================================================================

#[tokio::test]
async fn test_reset_all_data() {
    let db = test_db().await;

    // Insert data
    db.insert_session_from_index(
        "sess-1",
        "project-a",
        "Project A",
        "/tmp/project-a",
        "/tmp/sess1.jsonl",
        "Test",
        None,
        5,
        chrono::Utc::now().timestamp(),
        None,
        false,
        1000,
    )
    .await
    .unwrap();

    // Create an index run
    let run_id = db.create_index_run("full", Some(0), None).await.unwrap();
    db.complete_index_run(run_id, Some(1), 100, None, None)
        .await
        .unwrap();

    // Update metadata
    db.update_index_metadata_on_success(100, 1, 1)
        .await
        .unwrap();

    // Verify data exists
    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.sessions_count, 1);

    // Reset
    db.reset_all_data().await.unwrap();

    // Verify data is gone
    let health = db.get_health_stats().await.unwrap();
    assert_eq!(health.sessions_count, 0);
    assert_eq!(health.commits_count, 0);

    // Index runs should be gone
    let runs = db.get_recent_index_runs().await.unwrap();
    assert!(runs.is_empty());

    // Metadata should be reset
    let metadata = db.get_index_metadata().await.unwrap();
    assert!(metadata.last_indexed_at.is_none());
    assert_eq!(metadata.sessions_indexed, 0);
}

// ========================================================================
// Router tests
// ========================================================================

#[test]
fn test_system_router_creation() {
    let _router = router();
}
