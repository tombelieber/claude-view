//! Tests for the migrations module.
//!
//! Stays as a single file in PR 2.0 to keep the SQL split surgical. The
//! `>600 line` project rule is acknowledged here — splitting these
//! ~40 tests by theme is a deferred follow-up tracked under the
//! Phase 2 PR sequence.

use sqlx::SqlitePool;

/// Helper to create an in-memory database and run migrations.
async fn setup_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Enable foreign keys (required for CASCADE)
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();

    // Create migration tracking table
    sqlx::query("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    // Run all migrations
    for (i, migration) in super::migrations().iter().enumerate() {
        let version = i + 1;
        // Multi-statement migrations (with BEGIN/COMMIT) use raw_sql()
        let is_multi = migration.contains("BEGIN;") || migration.contains("BEGIN\n");
        let result = if is_multi {
            sqlx::raw_sql(migration).execute(&pool).await.map(|_| ())
        } else {
            sqlx::query(migration).execute(&pool).await.map(|_| ())
        };
        match result {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {}
            Err(e) => panic!("Migration {} failed: {}", version, e),
        }
        sqlx::query("INSERT OR IGNORE INTO _migrations (version) VALUES (?)")
            .bind(version as i64)
            .execute(&pool)
            .await
            .unwrap();
    }

    pool
}

/// Phase 2 PR 2.0 — invariants the per-module split must preserve.
///
/// Catches accidental drops, duplicates, or empty entries introduced
/// while moving migrations between `core` / `indexer` / `features` /
/// `rollups`. Runs without spinning up a DB so it's cheap (sub-millisecond).
#[test]
fn migration_order_invariants() {
    let m = super::migrations();

    assert!(!m.is_empty(), "migrations() must not be empty");

    for (i, sql) in m.iter().enumerate() {
        assert!(
            !sql.trim().is_empty(),
            "Migration version {} is empty (index {})",
            i + 1,
            i
        );
    }

    // Sub-module accounting — sum must equal flat slice length.
    // Catches drops or duplicates during the per-module split.
    let sub_total = super::core::MIGRATIONS.len()
        + super::indexer::MIGRATIONS.len()
        + super::features::MIGRATIONS.len()
        + super::rollups::MIGRATIONS.len()
        + super::events::MIGRATIONS.len();
    assert_eq!(
        sub_total,
        m.len(),
        "sum of sub-module MIGRATIONS lengths ({}) must equal migrations() length ({})",
        sub_total,
        m.len()
    );

    // Memoization: same allocation on second call (OnceLock invariant).
    let m2 = super::migrations();
    assert!(
        std::ptr::eq(m.as_ptr(), m2.as_ptr()),
        "migrations() must return the cached static slice on repeat calls"
    );
}

#[tokio::test]
async fn test_migration8_sessions_new_columns_exist() {
    let pool = setup_db().await;

    // Query the sessions table schema
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    // Verify all new Phase 3 columns exist
    assert!(
        column_names.contains(&"user_prompt_count"),
        "Missing user_prompt_count column"
    );
    assert!(
        column_names.contains(&"api_call_count"),
        "Missing api_call_count column"
    );
    assert!(
        column_names.contains(&"tool_call_count"),
        "Missing tool_call_count column"
    );
    assert!(
        column_names.contains(&"files_read"),
        "Missing files_read column"
    );
    assert!(
        column_names.contains(&"files_edited"),
        "Missing files_edited column"
    );
    assert!(
        column_names.contains(&"files_read_count"),
        "Missing files_read_count column"
    );
    assert!(
        column_names.contains(&"files_edited_count"),
        "Missing files_edited_count column"
    );
    assert!(
        column_names.contains(&"reedited_files_count"),
        "Missing reedited_files_count column"
    );
    assert!(
        column_names.contains(&"duration_seconds"),
        "Missing duration_seconds column"
    );
    assert!(
        column_names.contains(&"commit_count"),
        "Missing commit_count column"
    );
}

#[tokio::test]
async fn test_migration8_commits_table_exists() {
    let pool = setup_db().await;

    // Query commits table schema
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('commits')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"hash"), "Missing hash column");
    assert!(
        column_names.contains(&"repo_path"),
        "Missing repo_path column"
    );
    assert!(column_names.contains(&"message"), "Missing message column");
    assert!(column_names.contains(&"author"), "Missing author column");
    assert!(
        column_names.contains(&"timestamp"),
        "Missing timestamp column"
    );
    assert!(column_names.contains(&"branch"), "Missing branch column");
    assert!(
        column_names.contains(&"created_at"),
        "Missing created_at column"
    );
}

#[tokio::test]
async fn test_migration8_session_commits_table_exists() {
    let pool = setup_db().await;

    // Query session_commits table schema
    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('session_commits')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        column_names.contains(&"session_id"),
        "Missing session_id column"
    );
    assert!(
        column_names.contains(&"commit_hash"),
        "Missing commit_hash column"
    );
    assert!(column_names.contains(&"tier"), "Missing tier column");
    assert!(
        column_names.contains(&"evidence"),
        "Missing evidence column"
    );
    assert!(
        column_names.contains(&"created_at"),
        "Missing created_at column"
    );
}

#[tokio::test]
async fn test_migration8_index_metadata_table_exists() {
    let pool = setup_db().await;

    // Query index_metadata table schema
    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('index_metadata')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(
        column_names.contains(&"last_indexed_at"),
        "Missing last_indexed_at column"
    );
    assert!(
        column_names.contains(&"last_index_duration_ms"),
        "Missing last_index_duration_ms column"
    );
    assert!(
        column_names.contains(&"sessions_indexed"),
        "Missing sessions_indexed column"
    );
    assert!(
        column_names.contains(&"projects_indexed"),
        "Missing projects_indexed column"
    );
    assert!(
        column_names.contains(&"last_git_sync_at"),
        "Missing last_git_sync_at column"
    );
    assert!(
        column_names.contains(&"commits_found"),
        "Missing commits_found column"
    );
    assert!(
        column_names.contains(&"links_created"),
        "Missing links_created column"
    );
    assert!(
        column_names.contains(&"updated_at"),
        "Missing updated_at column"
    );

    // Verify singleton row was inserted
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM index_metadata")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "index_metadata should have exactly 1 row");
}

#[tokio::test]
async fn test_migration8_indexes_created() {
    let pool = setup_db().await;

    // Query all index names
    let indexes: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(&pool)
            .await
            .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    // Verify new Phase 3 indexes
    assert!(
        index_names.contains(&"idx_commits_repo_ts"),
        "Missing idx_commits_repo_ts index"
    );
    assert!(
        index_names.contains(&"idx_commits_timestamp"),
        "Missing idx_commits_timestamp index"
    );
    assert!(
        index_names.contains(&"idx_session_commits_session"),
        "Missing idx_session_commits_session index"
    );
    assert!(
        index_names.contains(&"idx_session_commits_commit"),
        "Missing idx_session_commits_commit index"
    );
    assert!(
        index_names.contains(&"idx_sessions_commit_count"),
        "Missing idx_sessions_commit_count index"
    );
    assert!(
        index_names.contains(&"idx_sessions_reedit"),
        "Missing idx_sessions_reedit index"
    );
    assert!(
        index_names.contains(&"idx_sessions_duration"),
        "Missing idx_sessions_duration index"
    );
}

#[tokio::test]
async fn test_migration12_unused_indexes_dropped() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(&pool)
            .await
            .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    // These unused indexes should be dropped by migration 12
    assert!(
        !index_names.contains(&"idx_invocations_session"),
        "idx_invocations_session should be dropped (unused)"
    );
    assert!(
        !index_names.contains(&"idx_invocations_timestamp"),
        "idx_invocations_timestamp should be dropped (unused)"
    );

    // These used indexes must still exist
    assert!(
        index_names.contains(&"idx_invocations_invocable"),
        "idx_invocations_invocable must still exist (used by skills dashboard)"
    );
    assert!(
        index_names.contains(&"idx_turns_session"),
        "idx_turns_session must still exist (used by session listing)"
    );
    assert!(
        index_names.contains(&"idx_turns_model"),
        "idx_turns_model must still exist (used by models API)"
    );
}

// ========================================================================
// Migration 13 tests (Theme 4: Classification + Index Runs)
// ========================================================================

#[tokio::test]
async fn test_migration13_classification_columns_exist() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    // CQRS Phase D.3 — migration 85 dropped the legacy classification
    // columns from `sessions`. They now live exclusively on
    // `session_flags`; asserting their absence here guards against
    // accidental re-introduction. Previous assertions (migration 13
    // added category_l1..classified_at on `sessions`) moved to
    // `test_migration85_legacy_columns_dropped`.
    for legacy in [
        "category_l1",
        "category_l2",
        "category_l3",
        "category_confidence",
        "category_source",
        "classified_at",
    ] {
        assert!(
            !column_names.contains(&legacy),
            "{legacy} should be dropped by Migration 85",
        );
    }
    // prompt_word_count, correction_count, same_file_edit_count were dropped
    // in Migration 63 (CQRS Phase 0 Step 5) — asserting absence instead.
    assert!(
        !column_names.contains(&"prompt_word_count"),
        "prompt_word_count should be dropped by Migration 63"
    );
    assert!(
        !column_names.contains(&"correction_count"),
        "correction_count should be dropped by Migration 63"
    );
    assert!(
        !column_names.contains(&"same_file_edit_count"),
        "same_file_edit_count should be dropped by Migration 63"
    );
}

#[tokio::test]
async fn test_migration13_classification_jobs_table_exists() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('classification_jobs')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(
        column_names.contains(&"started_at"),
        "Missing started_at column"
    );
    assert!(
        column_names.contains(&"completed_at"),
        "Missing completed_at column"
    );
    assert!(
        column_names.contains(&"total_sessions"),
        "Missing total_sessions column"
    );
    assert!(
        column_names.contains(&"classified_count"),
        "Missing classified_count column"
    );
    assert!(
        column_names.contains(&"skipped_count"),
        "Missing skipped_count column"
    );
    assert!(
        column_names.contains(&"failed_count"),
        "Missing failed_count column"
    );
    assert!(
        column_names.contains(&"provider"),
        "Missing provider column"
    );
    assert!(column_names.contains(&"model"), "Missing model column");
    assert!(column_names.contains(&"status"), "Missing status column");
    assert!(
        column_names.contains(&"error_message"),
        "Missing error_message column"
    );
    assert!(
        !column_names.contains(&"cost_estimate_cents"),
        "cost_estimate_cents should be removed by migration 49"
    );
    assert!(
        column_names.contains(&"actual_cost_cents"),
        "Missing actual_cost_cents column"
    );
    assert!(
        column_names.contains(&"tokens_used"),
        "Missing tokens_used column"
    );
}

#[tokio::test]
async fn test_migration49_classification_jobs_drop_estimate_preserves_data() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE classification_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            total_sessions INTEGER NOT NULL,
            classified_count INTEGER DEFAULT 0,
            skipped_count INTEGER DEFAULT 0,
            failed_count INTEGER DEFAULT 0,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            status TEXT DEFAULT 'running',
            error_message TEXT,
            cost_estimate_cents INTEGER,
            actual_cost_cents INTEGER,
            tokens_used INTEGER,
            CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO classification_jobs (
            id, started_at, completed_at, total_sessions, classified_count, skipped_count, failed_count,
            provider, model, status, error_message, cost_estimate_cents, actual_cost_cents, tokens_used
        ) VALUES (
            7, '2026-03-05T00:00:00Z', '2026-03-05T00:01:00Z', 10, 8, 1, 1,
            'claude-cli', 'claude-haiku-4-5-20251001', 'completed', NULL, 99, 42, 12345
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Find migration 49 by content rather than index — immune to new migrations being appended.
    let migration_49 = super::migrations()
        .iter()
        .find(|m| m.contains("classification_jobs_v2"))
        .expect("migration 49 (classification_jobs drop estimate) not found");
    sqlx::raw_sql(migration_49)
        .execute(&pool)
        .await
        .map(|_| ())
        .unwrap();

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('classification_jobs')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let column_names: Vec<String> = columns.into_iter().map(|(name,)| name).collect();
    assert!(
        !column_names.contains(&"cost_estimate_cents".to_string()),
        "cost_estimate_cents should be dropped by migration 49"
    );

    let row: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT id, total_sessions, actual_cost_cents, tokens_used FROM classification_jobs WHERE id = 7",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 7);
    assert_eq!(row.1, 10);
    assert_eq!(row.2, 42);
    assert_eq!(row.3, 12345);
}

#[tokio::test]
async fn test_migration13_index_runs_table_exists() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('index_runs')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(
        column_names.contains(&"started_at"),
        "Missing started_at column"
    );
    assert!(
        column_names.contains(&"completed_at"),
        "Missing completed_at column"
    );
    assert!(column_names.contains(&"type"), "Missing type column");
    assert!(
        column_names.contains(&"sessions_before"),
        "Missing sessions_before column"
    );
    assert!(
        column_names.contains(&"sessions_after"),
        "Missing sessions_after column"
    );
    assert!(
        column_names.contains(&"duration_ms"),
        "Missing duration_ms column"
    );
    assert!(
        column_names.contains(&"throughput_mb_per_sec"),
        "Missing throughput_mb_per_sec column"
    );
    assert!(column_names.contains(&"status"), "Missing status column");
    assert!(
        column_names.contains(&"error_message"),
        "Missing error_message column"
    );
    assert!(
        column_names.contains(&"unknown_top_level_type_count"),
        "Missing unknown_top_level_type_count column"
    );
    assert!(
        column_names.contains(&"unknown_required_path_count"),
        "Missing unknown_required_path_count column"
    );
    assert!(
        column_names.contains(&"imaginary_path_access_count"),
        "Missing imaginary_path_access_count column"
    );
    assert!(
        column_names.contains(&"legacy_fallback_path_count"),
        "Missing legacy_fallback_path_count column"
    );
    assert!(
        column_names.contains(&"dropped_line_invalid_json_count"),
        "Missing dropped_line_invalid_json_count column"
    );
    assert!(
        column_names.contains(&"schema_mismatch_count"),
        "Missing schema_mismatch_count column"
    );
    assert!(
        column_names.contains(&"unknown_source_role_count"),
        "Missing unknown_source_role_count column"
    );
    assert!(
        column_names.contains(&"derived_source_message_doc_count"),
        "Missing derived_source_message_doc_count column"
    );
    assert!(
        column_names.contains(&"source_message_non_source_provenance_count"),
        "Missing source_message_non_source_provenance_count column"
    );
}

#[tokio::test]
async fn test_migration40_index_runs_integrity_counter_defaults() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'running')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            unknown_top_level_type_count,
            unknown_required_path_count,
            imaginary_path_access_count,
            legacy_fallback_path_count,
            dropped_line_invalid_json_count,
            schema_mismatch_count,
            unknown_source_role_count,
            derived_source_message_doc_count,
            source_message_non_source_provenance_count
        FROM index_runs
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0);
    assert_eq!(row.1, 0);
    assert_eq!(row.2, 0);
    assert_eq!(row.3, 0);
    assert_eq!(row.4, 0);
    assert_eq!(row.5, 0);
    assert_eq!(row.6, 0);
    assert_eq!(row.7, 0);
    assert_eq!(row.8, 0);
}

#[tokio::test]
async fn test_migration40_index_runs_integrity_counter_check_constraints() {
    let pool = setup_db().await;

    let counters = [
        "unknown_top_level_type_count",
        "unknown_required_path_count",
        "imaginary_path_access_count",
        "legacy_fallback_path_count",
        "dropped_line_invalid_json_count",
        "schema_mismatch_count",
        "unknown_source_role_count",
        "derived_source_message_doc_count",
        "source_message_non_source_provenance_count",
    ];

    for column in counters {
        let sql = format!(
            "INSERT INTO index_runs (started_at, type, status, {column}) VALUES ('2026-02-05T12:00:00Z', 'full', 'running', -1)"
        );
        let result = sqlx::query(&sql).execute(&pool).await;
        assert!(
            result.is_err(),
            "Negative value should be rejected for {}",
            column
        );
    }
}

#[tokio::test]
async fn test_migration13_classification_jobs_check_constraints() {
    let pool = setup_db().await;

    // Valid status should work
    let result = sqlx::query(
        "INSERT INTO classification_jobs (started_at, total_sessions, provider, model, status) VALUES ('2026-02-05T12:00:00Z', 100, 'claude-cli', 'haiku', 'running')"
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "Valid status 'running' should be accepted");

    // Invalid status should fail
    let result = sqlx::query(
        "INSERT INTO classification_jobs (started_at, total_sessions, provider, model, status) VALUES ('2026-02-05T12:00:00Z', 100, 'claude-cli', 'haiku', 'invalid')"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "Invalid status should be rejected by CHECK constraint"
    );
}

#[tokio::test]
async fn test_migration13_index_runs_check_constraints() {
    let pool = setup_db().await;

    // Valid type and status should work
    let result = sqlx::query(
        "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'running')"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_ok(),
        "Valid type 'full' and status 'running' should be accepted"
    );

    // Invalid type should fail
    let result = sqlx::query(
        "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'invalid', 'running')"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "Invalid type should be rejected by CHECK constraint"
    );

    // Invalid status should fail
    let result = sqlx::query(
        "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'invalid')"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "Invalid status should be rejected by CHECK constraint"
    );
}

#[tokio::test]
async fn test_migration13_indexes_created() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(&pool)
            .await
            .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    // CQRS Phase D.3 — migration 85 drops `idx_sessions_category_l1`
    // and `idx_sessions_classified` together with the underlying columns
    // on `sessions`. The assertions that used to live here have moved
    // into `test_migration85_category_and_classified_indexes_dropped`.
    assert!(
        index_names.contains(&"idx_classification_jobs_status"),
        "Missing idx_classification_jobs_status index"
    );
    assert!(
        index_names.contains(&"idx_classification_jobs_started"),
        "Missing idx_classification_jobs_started index"
    );
    assert!(
        index_names.contains(&"idx_index_runs_started"),
        "Missing idx_index_runs_started index"
    );
}

#[tokio::test]
async fn test_migration8_check_constraints_work() {
    let pool = setup_db().await;

    // Insert a valid session first (required for FK in session_commits)
    sqlx::query(
        r#"
        INSERT INTO sessions (id, project_id, file_path, preview)
        VALUES ('test-sess', 'test-proj', '/tmp/test.jsonl', 'Test')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Test that negative values are rejected for user_prompt_count
    let result = sqlx::query("UPDATE sessions SET user_prompt_count = -1 WHERE id = 'test-sess'")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "Negative user_prompt_count should be rejected"
    );

    // Test that negative values are rejected for duration_seconds
    let result = sqlx::query("UPDATE sessions SET duration_seconds = -1 WHERE id = 'test-sess'")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "Negative duration_seconds should be rejected"
    );

    // Test that valid tier values work (1 and 2)
    sqlx::query(
        "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('abc123', '/repo', 'test', 1000)"
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = sqlx::query(
        "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('test-sess', 'abc123', 1)"
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "tier=1 should be valid");

    // Test that invalid tier value is rejected
    let result = sqlx::query(
        "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('test-sess', 'abc123', 3)"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "tier=3 should be rejected (only 1 or 2 allowed)"
    );

    // Test index_metadata singleton constraint
    let result = sqlx::query("INSERT INTO index_metadata (id) VALUES (2)")
        .execute(&pool)
        .await;
    assert!(result.is_err(), "index_metadata should only allow id=1");
}

#[tokio::test]
async fn test_migration8_default_values() {
    let pool = setup_db().await;

    // Insert a minimal session
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('test-sess', 'proj', '/tmp/t.jsonl', 'Preview')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify default values
    let row: (i64, i64, i64, String, String, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT user_prompt_count, api_call_count, tool_call_count,
               files_read, files_edited, files_read_count, files_edited_count,
               reedited_files_count, duration_seconds, commit_count
        FROM sessions WHERE id = 'test-sess'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0, "user_prompt_count default should be 0");
    assert_eq!(row.1, 0, "api_call_count default should be 0");
    assert_eq!(row.2, 0, "tool_call_count default should be 0");
    assert_eq!(row.3, "[]", "files_read default should be '[]'");
    assert_eq!(row.4, "[]", "files_edited default should be '[]'");
    assert_eq!(row.5, 0, "files_read_count default should be 0");
    assert_eq!(row.6, 0, "files_edited_count default should be 0");
    assert_eq!(row.7, 0, "reedited_files_count default should be 0");
    assert_eq!(row.8, 0, "duration_seconds default should be 0");
    assert_eq!(row.9, 0, "commit_count default should be 0");
}

#[tokio::test]
async fn test_migration9_full_parser_columns_exist() {
    let pool = setup_db().await;
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(names.contains(&"parse_version"), "Missing parse_version");
    assert!(
        names.contains(&"turn_duration_avg_ms"),
        "Missing turn_duration_avg_ms"
    );
    assert!(
        names.contains(&"turn_duration_max_ms"),
        "Missing turn_duration_max_ms"
    );
    assert!(
        names.contains(&"turn_duration_total_ms"),
        "Missing turn_duration_total_ms"
    );
    assert!(
        names.contains(&"api_error_count"),
        "Missing api_error_count"
    );
    assert!(
        names.contains(&"api_retry_count"),
        "Missing api_retry_count"
    );
    assert!(
        names.contains(&"compaction_count"),
        "Missing compaction_count"
    );
    assert!(
        names.contains(&"hook_blocked_count"),
        "Missing hook_blocked_count"
    );
    assert!(
        names.contains(&"agent_spawn_count"),
        "Missing agent_spawn_count"
    );
    assert!(
        names.contains(&"bash_progress_count"),
        "Missing bash_progress_count"
    );
    assert!(
        names.contains(&"hook_progress_count"),
        "Missing hook_progress_count"
    );
    assert!(
        names.contains(&"mcp_progress_count"),
        "Missing mcp_progress_count"
    );
    assert!(names.contains(&"summary_text"), "Missing summary_text");
    assert!(
        names.contains(&"total_input_tokens"),
        "Missing total_input_tokens"
    );
    assert!(
        names.contains(&"total_output_tokens"),
        "Missing total_output_tokens"
    );
    assert!(
        names.contains(&"cache_read_tokens"),
        "Missing cache_read_tokens"
    );
    assert!(
        names.contains(&"cache_creation_tokens"),
        "Missing cache_creation_tokens"
    );
    assert!(
        names.contains(&"thinking_block_count"),
        "Missing thinking_block_count"
    );
}

#[tokio::test]
async fn test_migration9_detail_tables_dropped() {
    // Migration 9 created turn_metrics + api_errors. Migration 63 (CQRS
    // Phase 0 Step 5) dropped them — zero writers + zero readers, dead
    // weight. This test asserts the drops landed.
    let pool = setup_db().await;

    let turn_metrics_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('turn_metrics')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        turn_metrics_cols.is_empty(),
        "turn_metrics table should be dropped by Migration 63"
    );

    let api_errors_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_errors')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        api_errors_cols.is_empty(),
        "api_errors table should be dropped by Migration 63"
    );
}

#[tokio::test]
async fn test_migration9_parse_version_default() {
    let pool = setup_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('pv-test', 'proj', '/tmp/pv.jsonl', 'Test')"
    ).execute(&pool).await.unwrap();

    let row: (i64,) = sqlx::query_as("SELECT parse_version FROM sessions WHERE id = 'pv-test'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, 0, "parse_version default should be 0");
}

#[tokio::test]
async fn test_migration8_cascade_delete() {
    let pool = setup_db().await;

    // Insert session and commit
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('sess-1', 'proj', '/tmp/s.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('hash1', '/repo', 'msg', 1000)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('sess-1', 'hash1', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify link exists
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);

    // Delete session - should cascade to session_commits
    sqlx::query("DELETE FROM sessions WHERE id = 'sess-1'")
        .execute(&pool)
        .await
        .unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "session_commits should be deleted via CASCADE");
}

// ========================================================================
// Migration 13: LOC estimation columns (from main)
// ========================================================================

#[tokio::test]
async fn test_migration13_loc_columns_exist() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        column_names.contains(&"lines_added"),
        "Missing lines_added column"
    );
    assert!(
        column_names.contains(&"lines_removed"),
        "Missing lines_removed column"
    );
    assert!(
        column_names.contains(&"loc_source"),
        "Missing loc_source column"
    );
}

#[tokio::test]
async fn test_migration13_loc_defaults() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('loc-test', 'proj', '/tmp/loc.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'loc-test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0, "lines_added default should be 0");
    assert_eq!(row.1, 0, "lines_removed default should be 0");
    assert_eq!(row.2, 0, "loc_source default should be 0");
}

#[tokio::test]
async fn test_migration13_loc_check_constraints() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('loc-check', 'proj', '/tmp/c.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Test that negative values are rejected for lines_added
    let result = sqlx::query("UPDATE sessions SET lines_added = -1 WHERE id = 'loc-check'")
        .execute(&pool)
        .await;
    assert!(result.is_err(), "Negative lines_added should be rejected");

    // Test that negative values are rejected for lines_removed
    let result = sqlx::query("UPDATE sessions SET lines_removed = -1 WHERE id = 'loc-check'")
        .execute(&pool)
        .await;
    assert!(result.is_err(), "Negative lines_removed should be rejected");

    // Test that valid loc_source values work (0, 1, 2)
    let result = sqlx::query("UPDATE sessions SET loc_source = 1 WHERE id = 'loc-check'")
        .execute(&pool)
        .await;
    assert!(result.is_ok(), "loc_source=1 should be valid");

    let result = sqlx::query("UPDATE sessions SET loc_source = 2 WHERE id = 'loc-check'")
        .execute(&pool)
        .await;
    assert!(result.is_ok(), "loc_source=2 should be valid");

    // Test that invalid loc_source value is rejected
    let result = sqlx::query("UPDATE sessions SET loc_source = 3 WHERE id = 'loc-check'")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "loc_source=3 should be rejected (only 0, 1, 2 allowed)"
    );
}

// ========================================================================
// Migration 14: Theme 3 contribution tracking
// ========================================================================

#[tokio::test]
async fn test_migration14_sessions_contribution_columns_exist() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        column_names.contains(&"ai_lines_added"),
        "Missing ai_lines_added column"
    );
    assert!(
        column_names.contains(&"ai_lines_removed"),
        "Missing ai_lines_removed column"
    );
    assert!(
        column_names.contains(&"work_type"),
        "Missing work_type column"
    );
}

#[tokio::test]
async fn test_migration14_commits_diff_stats_columns_exist() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('commits')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        column_names.contains(&"files_changed"),
        "Missing files_changed column"
    );
    assert!(
        column_names.contains(&"insertions"),
        "Missing insertions column"
    );
    assert!(
        column_names.contains(&"deletions"),
        "Missing deletions column"
    );
}

#[tokio::test]
async fn test_migration14_contribution_snapshots_table_exists() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('contribution_snapshots')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(column_names.contains(&"date"), "Missing date column");
    assert!(
        column_names.contains(&"project_id"),
        "Missing project_id column"
    );
    assert!(column_names.contains(&"branch"), "Missing branch column");
    assert!(
        column_names.contains(&"sessions_count"),
        "Missing sessions_count column"
    );
    assert!(
        column_names.contains(&"ai_lines_added"),
        "Missing ai_lines_added column"
    );
    assert!(
        column_names.contains(&"ai_lines_removed"),
        "Missing ai_lines_removed column"
    );
    assert!(
        column_names.contains(&"commits_count"),
        "Missing commits_count column"
    );
    assert!(
        column_names.contains(&"commit_insertions"),
        "Missing commit_insertions column"
    );
    assert!(
        column_names.contains(&"commit_deletions"),
        "Missing commit_deletions column"
    );
    assert!(
        column_names.contains(&"tokens_used"),
        "Missing tokens_used column"
    );
    assert!(
        column_names.contains(&"cost_cents"),
        "Missing cost_cents column"
    );
}

#[tokio::test]
async fn test_migration14_contribution_snapshots_unique_constraint() {
    let pool = setup_db().await;

    // Insert first snapshot
    sqlx::query(
        "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-05', 'proj1', 'main', 5)"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert second snapshot - same date+project+branch should fail
    let result = sqlx::query(
        "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-05', 'proj1', 'main', 10)"
    )
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "Should reject duplicate date+project_id+branch combination"
    );

    // Insert different date - should succeed
    let result = sqlx::query(
        "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-06', 'proj1', 'main', 10)"
    )
    .execute(&pool)
    .await;

    assert!(result.is_ok(), "Should allow different date");
}

#[tokio::test]
async fn test_migration14_sessions_default_values() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('contrib-test', 'proj', '/tmp/c.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64, i64, Option<String>) = sqlx::query_as(
        "SELECT ai_lines_added, ai_lines_removed, work_type FROM sessions WHERE id = 'contrib-test'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0, "ai_lines_added default should be 0");
    assert_eq!(row.1, 0, "ai_lines_removed default should be 0");
    assert!(row.2.is_none(), "work_type default should be NULL");
}

#[tokio::test]
async fn test_migration14_commits_default_values() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('abc123def456789012345678901234567890abcd', '/repo', 'test', 1000)"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT files_changed, insertions, deletions FROM commits WHERE hash = 'abc123def456789012345678901234567890abcd'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, 0, "files_changed default should be 0");
    assert_eq!(row.1, 0, "insertions default should be 0");
    assert_eq!(row.2, 0, "deletions default should be 0");
}

#[tokio::test]
async fn test_migration14_indexes_created() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_snapshots%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        index_names.contains(&"idx_snapshots_date"),
        "Missing idx_snapshots_date index"
    );
    assert!(
        index_names.contains(&"idx_snapshots_project_date"),
        "Missing idx_snapshots_project_date index"
    );
    assert!(
        index_names.contains(&"idx_snapshots_branch_date"),
        "Missing idx_snapshots_branch_date index"
    );
}

// ========================================================================
// Migration 16: Dashboard analytics indexes (renumbered from branch's 13)
// ========================================================================

#[tokio::test]
async fn test_migration16_dashboard_analytics_indexes() {
    let pool = setup_db().await;

    // Verify primary_model column was added
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        column_names.contains(&"primary_model"),
        "Missing primary_model column"
    );

    // Verify new indexes were created
    let indexes: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(&pool)
            .await
            .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        index_names.contains(&"idx_sessions_first_message"),
        "Missing idx_sessions_first_message index"
    );
    assert!(
        index_names.contains(&"idx_sessions_project_first_message"),
        "Missing idx_sessions_project_first_message index"
    );
    assert!(
        index_names.contains(&"idx_sessions_primary_model"),
        "Missing idx_sessions_primary_model index"
    );
}

#[tokio::test]
async fn test_migration16_primary_model_can_be_set() {
    let pool = setup_db().await;

    // Insert a session with primary_model
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, primary_model) VALUES ('pm-test', 'proj', '/tmp/pm.jsonl', 'Test', 'claude-sonnet-4')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify the value
    let row: (Option<String>,) =
        sqlx::query_as("SELECT primary_model FROM sessions WHERE id = 'pm-test'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(row.0, Some("claude-sonnet-4".to_string()));
}

// ========================================================================
// Migration 18: Drop file_hash, verify schema cleanup
// ========================================================================

#[tokio::test]
async fn test_migration18_file_hash_column_dropped() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    // file_hash should be gone
    assert!(
        !column_names.contains(&"file_hash"),
        "file_hash column should be dropped by migration 18"
    );

    // All other essential columns should still exist
    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(
        column_names.contains(&"project_id"),
        "Missing project_id column"
    );
    assert!(
        column_names.contains(&"file_path"),
        "Missing file_path column"
    );
    assert!(column_names.contains(&"summary"), "Missing summary column");
    assert!(
        column_names.contains(&"summary_text"),
        "Missing summary_text column"
    );
    assert!(
        column_names.contains(&"primary_model"),
        "Missing primary_model column"
    );
    assert!(
        column_names.contains(&"work_type"),
        "Missing work_type column"
    );
}

#[tokio::test]
async fn test_migration18_indexes_preserved() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_sessions%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    // All session indexes should be recreated
    assert!(
        index_names.contains(&"idx_sessions_project"),
        "Missing idx_sessions_project"
    );
    assert!(
        index_names.contains(&"idx_sessions_last_message"),
        "Missing idx_sessions_last_message"
    );
    assert!(
        index_names.contains(&"idx_sessions_project_branch"),
        "Missing idx_sessions_project_branch"
    );
    assert!(
        index_names.contains(&"idx_sessions_sidechain"),
        "Missing idx_sessions_sidechain"
    );
    assert!(
        index_names.contains(&"idx_sessions_commit_count"),
        "Missing idx_sessions_commit_count"
    );
    assert!(
        index_names.contains(&"idx_sessions_reedit"),
        "Missing idx_sessions_reedit"
    );
    assert!(
        index_names.contains(&"idx_sessions_duration"),
        "Missing idx_sessions_duration"
    );
    assert!(
        index_names.contains(&"idx_sessions_needs_reindex"),
        "Missing idx_sessions_needs_reindex"
    );
    assert!(
        index_names.contains(&"idx_sessions_first_message"),
        "Missing idx_sessions_first_message"
    );
    assert!(
        index_names.contains(&"idx_sessions_project_first_message"),
        "Missing idx_sessions_project_first_message"
    );
    assert!(
        index_names.contains(&"idx_sessions_primary_model"),
        "Missing idx_sessions_primary_model"
    );
}

#[tokio::test]
async fn test_migration18_data_preserved() {
    let pool = setup_db().await;

    // Insert a session before migration runs (it already ran via setup_db)
    // Instead, insert and verify data round-trips correctly
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, summary, summary_text, primary_model) VALUES ('m18-test', 'proj', '/tmp/m18.jsonl', 'Test', 'index summary', 'deep summary', 'claude-sonnet-4')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT summary, summary_text, primary_model FROM sessions WHERE id = 'm18-test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0.as_deref(), Some("index summary"));
    assert_eq!(row.1.as_deref(), Some("deep summary"));
    assert_eq!(row.2.as_deref(), Some("claude-sonnet-4"));
}

#[tokio::test]
async fn test_migration18_coalesce_summary_behavior() {
    let pool = setup_db().await;

    // Session with both summaries: summary_text wins
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, summary, summary_text) VALUES ('coal-1', 'proj', '/tmp/c1.jsonl', 'Test', 'from index', 'from deep')"
    ).execute(&pool).await.unwrap();

    // Session with only index summary: summary as fallback
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, summary) VALUES ('coal-2', 'proj', '/tmp/c2.jsonl', 'Test', 'from index only')"
    ).execute(&pool).await.unwrap();

    // Session with neither
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('coal-3', 'proj', '/tmp/c3.jsonl', 'Test')"
    ).execute(&pool).await.unwrap();

    let row: (Option<String>,) = sqlx::query_as(
        "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("from deep"),
        "summary_text should win when both present"
    );

    let row: (Option<String>,) = sqlx::query_as(
        "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("from index only"),
        "summary should be fallback"
    );

    let row: (Option<String>,) = sqlx::query_as(
        "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-3'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0.is_none(), "Both NULL should yield NULL");
}

#[tokio::test]
async fn test_migration_task_time_columns_exist() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(
        column_names.contains(&"total_task_time_seconds"),
        "Missing total_task_time_seconds column"
    );
    assert!(
        column_names.contains(&"longest_task_seconds"),
        "Missing longest_task_seconds column"
    );
    assert!(
        column_names.contains(&"longest_task_preview"),
        "Missing longest_task_preview column"
    );
}

#[tokio::test]
async fn test_migration_task_time_defaults() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('task-time-test', 'proj', '/tmp/tt.jsonl', 'Test')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (Option<i64>, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT total_task_time_seconds, longest_task_seconds, longest_task_preview FROM sessions WHERE id = 'task-time-test'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        row.0.is_none(),
        "total_task_time_seconds default should be NULL"
    );
    assert!(
        row.1.is_none(),
        "longest_task_seconds default should be NULL"
    );
    assert!(
        row.2.is_none(),
        "longest_task_preview default should be NULL"
    );
}

#[tokio::test]
async fn test_migration18_check_constraints_preserved() {
    let pool = setup_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('m18-chk', 'proj', '/tmp/chk.jsonl', 'Test')"
    ).execute(&pool).await.unwrap();

    // Verify CHECK constraints survived the table recreation
    let result = sqlx::query("UPDATE sessions SET lines_added = -1 WHERE id = 'm18-chk'")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "CHECK constraint on lines_added should survive migration 18"
    );

    let result = sqlx::query("UPDATE sessions SET loc_source = 3 WHERE id = 'm18-chk'")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "CHECK constraint on loc_source should survive migration 18"
    );
}

// ========================================================================
// Migration 25: Work Reports table
// ========================================================================

#[tokio::test]
async fn test_migration25_reports_table_exists() {
    let pool = setup_db().await;

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('reports')")
        .fetch_all(&pool)
        .await
        .unwrap();

    let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

    assert!(column_names.contains(&"id"), "Missing id column");
    assert!(
        column_names.contains(&"report_type"),
        "Missing report_type column"
    );
    assert!(
        column_names.contains(&"date_start"),
        "Missing date_start column"
    );
    assert!(
        column_names.contains(&"date_end"),
        "Missing date_end column"
    );
    assert!(
        column_names.contains(&"content_md"),
        "Missing content_md column"
    );
    assert!(
        column_names.contains(&"context_digest"),
        "Missing context_digest column"
    );
    assert!(
        column_names.contains(&"session_count"),
        "Missing session_count column"
    );
    assert!(
        column_names.contains(&"project_count"),
        "Missing project_count column"
    );
    assert!(
        column_names.contains(&"total_duration_secs"),
        "Missing total_duration_secs column"
    );
    assert!(
        column_names.contains(&"total_cost_cents"),
        "Missing total_cost_cents column"
    );
    assert!(
        column_names.contains(&"generation_ms"),
        "Missing generation_ms column"
    );
    assert!(
        column_names.contains(&"created_at"),
        "Missing created_at column"
    );
}

#[tokio::test]
async fn test_migration25_reports_check_constraints() {
    let pool = setup_db().await;

    // Valid report_type should work
    let result = sqlx::query(
        "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('daily', '2026-02-21', '2026-02-21', '- Shipped search', 8, 3, 15120, 680)"
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_ok(),
        "Valid report_type 'daily' should be accepted"
    );

    // Invalid report_type should fail
    let result = sqlx::query(
        "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('invalid', '2026-02-21', '2026-02-21', 'test', 0, 0, 0, 0)"
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "Invalid report_type should be rejected");
}

#[tokio::test]
async fn test_migration25_reports_indexes() {
    let pool = setup_db().await;

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_reports%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        index_names.contains(&"idx_reports_date"),
        "Missing idx_reports_date index"
    );
    assert!(
        index_names.contains(&"idx_reports_type"),
        "Missing idx_reports_type index"
    );
}

// ========================================================================
// Migration 27: Unified LLM settings (app_settings table)
// ========================================================================

#[tokio::test]
async fn test_migration27_app_settings_table() {
    let pool = setup_db().await;

    // Table should exist with exactly one default row
    let row: (String, i64) =
        sqlx::query_as("SELECT llm_model, llm_timeout_secs FROM app_settings WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(row.0, "haiku");
    assert_eq!(row.1, 120);

    // CHECK constraint: inserting id != 1 should fail
    let result = sqlx::query("INSERT INTO app_settings (id, llm_model) VALUES (2, 'sonnet')")
        .execute(&pool)
        .await;
    assert!(result.is_err());
}

// ========================================================================
// Migration 63: CQRS Phase 0 Step 5 — IRREVERSIBLE drops
// ========================================================================

#[tokio::test]
async fn test_migration63_dead_tables_dropped() {
    let pool = setup_db().await;
    for tbl in [
        "turn_metrics",
        "api_errors",
        "fluency_scores",
        "pricing_cache",
    ] {
        let cols: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{}')", tbl))
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(
            cols.is_empty(),
            "Migration 63 should have dropped table `{}`",
            tbl
        );
    }
}

#[tokio::test]
async fn test_migration63_dead_sessions_columns_dropped() {
    // `closed_at` + `dismissed_at` in-memory fields on ActiveSession stay
    // (see reaper.rs, server-live-state/core.rs). Only the DB columns drop.
    let pool = setup_db().await;
    let cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();
    for col in [
        "closed_at",
        "dismissed_at",
        "session_kind",
        "start_type",
        "prompt_word_count",
        "correction_count",
        "same_file_edit_count",
    ] {
        assert!(
            !names.contains(&col),
            "Migration 63 should have dropped sessions.{}",
            col
        );
    }
}

#[tokio::test]
async fn test_migration63_sdk_supported_dropped() {
    let pool = setup_db().await;
    let cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('models')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        !names.contains(&"sdk_supported"),
        "Migration 63 should have dropped models.sdk_supported"
    );
}

// ========================================================================
// Migration 64: session_stats — Phase 2 read-side mirror table
// ========================================================================

#[tokio::test]
async fn test_migration64_session_stats_columns_exist() {
    let pool = setup_db().await;
    let cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('session_stats')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();

    // 9 staleness/header
    for col in [
        "session_id",
        "source_content_hash",
        "source_size",
        "source_inode",
        "source_mid_hash",
        "parser_version",
        "stats_version",
        "indexed_at",
    ] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 6 token columns
    for col in [
        "total_input_tokens",
        "total_output_tokens",
        "cache_read_tokens",
        "cache_creation_tokens",
        "cache_creation_5m_tokens",
        "cache_creation_1hr_tokens",
    ] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 6 count columns
    for col in [
        "turn_count",
        "user_prompt_count",
        "line_count",
        "tool_call_count",
        "thinking_block_count",
        "api_error_count",
    ] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 4 tool counts
    for col in [
        "files_read_count",
        "files_edited_count",
        "bash_count",
        "agent_spawn_count",
    ] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 3 time columns
    for col in ["first_message_at", "last_message_at", "duration_seconds"] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 4 string columns
    for col in ["primary_model", "git_branch", "preview", "last_message"] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // 2 JSON columns
    assert!(
        names.contains(&"per_model_tokens_json"),
        "missing session_stats.per_model_tokens_json"
    );
    assert!(
        names.contains(&"invocation_counts"),
        "missing session_stats.invocation_counts"
    );

    // Phase 3 PR 3.a (migration 66) added 4 filesystem-mirror columns:
    // project_id, file_path, is_compressed, source_mtime.
    for col in ["project_id", "file_path", "is_compressed", "source_mtime"] {
        assert!(names.contains(&col), "missing session_stats.{}", col);
    }

    // Total = 8 header + 24 stats (Phase 2 PR 2.1) + 4 filesystem-mirror
    // (Phase 3 PR 3.a) + 1 invocation_counts (Phase 6.2 migration 86) = 37.
    // The design doc (§3.1) said "9 header + 25 stats = 34" but miscounted;
    // our schema and this assertion reflect what actually exists. If this
    // drifts, the writer ownership registry rule (§10.2) likely needs an
    // update.
    assert_eq!(
        names.len(),
        37,
        "session_stats column count drifted from the 37 documented in features.rs (got {})",
        names.len()
    );
}

#[tokio::test]
async fn test_migration64_session_stats_strict_mode_rejects_text_in_int() {
    // STRICT mode is the headline change vs the legacy `sessions` table —
    // catches `"123"` (TEXT) being stored where INTEGER is declared.
    let pool = setup_db().await;
    let result = sqlx::query(
        r#"INSERT INTO session_stats (session_id, source_content_hash, source_size,
                                       parser_version, stats_version, indexed_at,
                                       total_input_tokens)
           VALUES ('strict-test', X'00', 1, 1, 1, 0, 'not-an-int')"#,
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "STRICT mode must reject TEXT into total_input_tokens (INTEGER)"
    );
}

#[tokio::test]
async fn test_migration64_session_stats_indexes_created() {
    let pool = setup_db().await;
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='session_stats'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    for ix in [
        "idx_session_stats_last_ts",
        "idx_session_stats_indexed_at",
        "idx_session_stats_total_tokens",
        "idx_session_stats_primary_model",
        "idx_session_stats_git_branch",
    ] {
        assert!(index_names.contains(&ix), "missing index {}", ix);
    }
}

#[tokio::test]
async fn test_migration64_session_stats_default_row_inserts() {
    // All-defaults insert with the 8 NOT NULL no-default columns supplied —
    // the rest fall back to the migration's DEFAULT 0 / DEFAULT '' clauses.
    let pool = setup_db().await;
    sqlx::query(
        r#"INSERT INTO session_stats (session_id, source_content_hash, source_size,
                                       parser_version, stats_version, indexed_at)
           VALUES ('default-row', X'00', 1, 1, 1, 0)"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let row: (i64, i64, i64, String, String) = sqlx::query_as(
        r#"SELECT total_input_tokens, turn_count, files_read_count,
                  preview, per_model_tokens_json
             FROM session_stats WHERE session_id = 'default-row'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 0);
    assert_eq!(row.1, 0);
    assert_eq!(row.2, 0);
    assert_eq!(row.3, "");
    assert_eq!(row.4, "{}");
}

// ========================================================================
// Migration 65: session_flags — Phase 5 fold target (table only in Phase 2)
// ========================================================================

#[tokio::test]
async fn test_migration65_session_flags_columns_exist() {
    let pool = setup_db().await;
    let cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('session_flags')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();

    for col in [
        "session_id",
        "archived_at",
        "dismissed_at",
        "category_l1",
        "category_l2",
        "category_l3",
        "category_confidence",
        "category_source",
        "classified_at",
        "applied_seq",
    ] {
        assert!(names.contains(&col), "missing session_flags.{}", col);
    }
    assert_eq!(names.len(), 10, "session_flags column count drifted");
}

#[tokio::test]
async fn test_migration65_session_flags_partial_indexes_created() {
    // Both indexes are partial (WHERE x IS NOT NULL) — verify they exist
    // AND have the partial filter so they stay sparse on the prod DB.
    let pool = setup_db().await;
    let indexes: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name='session_flags'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let by_name: std::collections::HashMap<&str, &Option<String>> =
        indexes.iter().map(|(n, s)| (n.as_str(), s)).collect();

    let archived = by_name
        .get("idx_session_flags_archived")
        .expect("missing idx_session_flags_archived");
    assert!(
        archived
            .as_ref()
            .map(|s| s.contains("WHERE archived_at IS NOT NULL"))
            .unwrap_or(false),
        "archived index lost its WHERE clause: {:?}",
        archived
    );

    let category = by_name
        .get("idx_session_flags_category")
        .expect("missing idx_session_flags_category");
    assert!(
        category
            .as_ref()
            .map(|s| s.contains("WHERE category_l1 IS NOT NULL"))
            .unwrap_or(false),
        "category index lost its WHERE clause: {:?}",
        category
    );
}

#[tokio::test]
async fn test_migration65_session_flags_applied_seq_default_zero() {
    let pool = setup_db().await;
    sqlx::query("INSERT INTO session_flags (session_id) VALUES ('seq-default')")
        .execute(&pool)
        .await
        .unwrap();

    let row: (i64,) =
        sqlx::query_as("SELECT applied_seq FROM session_flags WHERE session_id = 'seq-default'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row.0, 0,
        "applied_seq must default to 0 (Phase 5 fold start)"
    );
}

// ──────────────────────────────────────────────────────────────────
// CQRS Phase 4 PR 4.1 — rollup tables 66–80
//
// The 15 rollup table CREATE statements are generated by the
// `#[derive(RollupTable)]` proc-macro on `StatsCore`; this test
// asserts the end-to-end chain (macro output → rollups.rs → migrations
// runner → sqlite master table) is intact. If any link breaks this is
// the first gate that fails.
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_phase4_rollup_tables_all_created() {
    let pool = setup_db().await;

    let expected_tables: &[&str] = &[
        "daily_global_stats",
        "daily_project_stats",
        "daily_branch_stats",
        "daily_model_stats",
        "daily_category_stats",
        "weekly_global_stats",
        "weekly_project_stats",
        "weekly_branch_stats",
        "weekly_model_stats",
        "weekly_category_stats",
        "monthly_global_stats",
        "monthly_project_stats",
        "monthly_branch_stats",
        "monthly_model_stats",
        "monthly_category_stats",
    ];

    for name in expected_tables {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name = ?1")
                .bind(name)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(
            row.is_some(),
            "Phase 4 rollup table `{name}` missing after migrations"
        );
    }
}

#[tokio::test]
async fn test_phase4_rollup_tables_are_strict_mode() {
    // STRICT tables enforce column-type contracts at INSERT time;
    // losing this would let Stage C's rollup writes silently widen into
    // ANY-typed columns. Catch it here rather than in integration.
    let pool = setup_db().await;
    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type='table'
           AND name LIKE '%_stats'
           AND name NOT IN ('session_stats')
           AND sql NOT LIKE '%STRICT%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        cnt, 0,
        "every Phase 4 rollup table must be STRICT — {cnt} non-STRICT table(s) found"
    );
}

#[tokio::test]
async fn test_phase4_daily_branch_stats_composite_pk() {
    // Branch has the hairiest composite PK (period_start, project_id,
    // branch). Confirms the macro emitted the PK in the right order —
    // the sqlite_master SQL is parseable as the CREATE TABLE string.
    let pool = setup_db().await;
    let (sql,): (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='daily_branch_stats'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        sql.contains("PRIMARY KEY (period_start, project_id, branch)"),
        "daily_branch_stats missing expected composite PK; got:\n{sql}"
    );
}

// ──────────────────────────────────────────────────────────────────
// CQRS Phase 5 PR 5.1 — Migration 82: session_action_log.
//
// Append-only event log for session mutations (archive / unarchive /
// classify / dismiss / reclassify). Consumed by the Phase 5 fold task
// (PR 5.3) into `session_flags`. These tests assert the schema shape
// the Rust fold writer + handlers rely on. A drift here breaks PR 5.2
// dual-write + PR 5.3 fold advance.
//
// See design doc §7.1 and `crates/db/src/migrations/events.rs` for the
// per-column rationale.
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_migration82_session_action_log_columns_exist() {
    let pool = setup_db().await;

    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk
         FROM pragma_table_info('session_action_log')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let by_name: std::collections::HashMap<&str, (&str, i64, i64)> = cols
        .iter()
        .map(|(_, name, ty, notnull, _, pk)| (name.as_str(), (ty.as_str(), *notnull, *pk)))
        .collect();

    // Column set + types + NOT NULL must match the §7.1 schema byte-for-byte.
    for (col, expected_type, expected_notnull, expected_pk) in [
        ("seq", "INTEGER", 0_i64, 1_i64), // PK column; NOT NULL is implicit via PK
        ("session_id", "TEXT", 1, 0),
        ("action", "TEXT", 1, 0),
        ("payload", "TEXT", 1, 0),
        ("actor", "TEXT", 1, 0),
        ("at", "INTEGER", 1, 0),
    ] {
        let (ty, notnull, pk) = by_name
            .get(col)
            .copied()
            .unwrap_or_else(|| panic!("missing session_action_log.{col}"));
        assert_eq!(ty, expected_type, "session_action_log.{col} type drift");
        assert_eq!(
            notnull, expected_notnull,
            "session_action_log.{col} NOT NULL drift"
        );
        assert_eq!(
            pk, expected_pk,
            "session_action_log.{col} primary-key drift"
        );
    }

    assert_eq!(
        cols.len(),
        6,
        "session_action_log column count drifted — fold writer + dual-write handlers will break silently"
    );
}

#[tokio::test]
async fn test_migration82_session_action_log_is_strict() {
    // STRICT protects the INTEGER `at` + `seq` columns from silent type
    // widening. Without STRICT a buggy handler that bound a string to
    // `at` would land, and the fold task's `ORDER BY at` would mix
    // string-sort with integer-sort. Loud failure at INSERT is correct.
    let pool = setup_db().await;
    let (sql,): (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='session_action_log'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        sql.contains("STRICT"),
        "session_action_log must be STRICT; got:\n{sql}"
    );
}

#[tokio::test]
async fn test_migration82_session_action_log_seq_autoincrements() {
    // `seq` must strictly increase across inserts so the fold task can
    // treat it as a watermark and `applied_seq < seq` is monotonic.
    // AUTOINCREMENT in SQLite is stricter than ROWID: deleted rows'
    // sequence numbers are not reused. The fold writer relies on that.
    let pool = setup_db().await;

    for (sid, action, at) in [
        ("sess-a", "archive", 1_700_000_000_000_i64),
        ("sess-b", "classify", 1_700_000_000_001),
        ("sess-a", "unarchive", 1_700_000_000_002),
    ] {
        sqlx::query(
            "INSERT INTO session_action_log (session_id, action, payload, actor, at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(sid)
        .bind(action)
        .bind("{}")
        .bind("user")
        .bind(at)
        .execute(&pool)
        .await
        .unwrap();
    }

    let seqs: Vec<(i64,)> = sqlx::query_as("SELECT seq FROM session_action_log ORDER BY rowid ASC")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(seqs.len(), 3);

    // Strictly increasing under AUTOINCREMENT.
    let flat: Vec<i64> = seqs.iter().map(|(s,)| *s).collect();
    assert!(
        flat.windows(2).all(|w| w[0] < w[1]),
        "seq must be strictly increasing under AUTOINCREMENT; got {flat:?}"
    );

    // Seq starts at 1 on an empty table (sqlite_sequence contract).
    assert_eq!(flat[0], 1, "seq must start at 1 on a fresh DB");
}

#[tokio::test]
async fn test_migration82_session_action_log_indexes_created() {
    // Both covering indexes must exist with the expected column
    // ordering — the fold task's `WHERE seq > ? ORDER BY seq` scan and
    // the UI's per-session audit query both depend on them. A rename
    // here will silently degrade both queries to full-table scans.
    let pool = setup_db().await;

    let indexes: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, sql FROM sqlite_master
         WHERE type='index' AND tbl_name='session_action_log'
           AND name NOT LIKE 'sqlite_autoindex_%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let by_name: std::collections::HashMap<&str, Option<&str>> = indexes
        .iter()
        .map(|(n, s)| (n.as_str(), s.as_deref()))
        .collect();

    let session = by_name
        .get("idx_action_session")
        .copied()
        .flatten()
        .expect("missing idx_action_session");
    assert!(
        session.contains("(session_id, at)") || session.contains("(session_id,at)"),
        "idx_action_session must index (session_id, at) in that order; got:\n{session}"
    );

    let actor = by_name
        .get("idx_action_actor_at")
        .copied()
        .flatten()
        .expect("missing idx_action_actor_at");
    assert!(
        actor.contains("(actor, at)") || actor.contains("(actor,at)"),
        "idx_action_actor_at must index (actor, at) in that order; got:\n{actor}"
    );
}

// ──────────────────────────────────────────────────────────────────
// CQRS Phase 5 PR 5.3 — Migration 83: fold_state watermark.
//
// Single-row table driving the `session_action_log` fold task (see
// `crates/db/src/fold/mod.rs`). Tests enforce the contract the fold
// driver relies on: `id=0` PK + CHECK, `applied_seq` seeded at 0,
// single-row-ness by schema. A drift here breaks the fold's advance
// step, which would silently stall the entire Phase 5 pipeline.
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_migration83_fold_state_seeded_with_single_zero_row() {
    let pool = setup_db().await;

    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT id, applied_seq FROM fold_state ORDER BY id ASC")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "fold_state must be seeded with exactly one row"
    );
    assert_eq!(rows[0], (0, 0), "seed row must be (id=0, applied_seq=0)");
}

#[tokio::test]
async fn test_migration83_fold_state_rejects_non_zero_id() {
    // CHECK (id = 0) enforces single-row-ness at the schema layer — a
    // future migration or a buggy script cannot accidentally split the
    // watermark by inserting id=1.
    let pool = setup_db().await;

    let result = sqlx::query("INSERT INTO fold_state (id, applied_seq) VALUES (1, 0)")
        .execute(&pool)
        .await;
    assert!(
        result.is_err(),
        "id != 0 must be rejected by CHECK constraint"
    );
}

#[tokio::test]
async fn test_migration83_fold_state_applied_seq_update_roundtrips() {
    let pool = setup_db().await;

    sqlx::query("UPDATE fold_state SET applied_seq = ?1 WHERE id = 0")
        .bind(123_456_i64)
        .execute(&pool)
        .await
        .unwrap();

    let seq: (i64,) = sqlx::query_as("SELECT applied_seq FROM fold_state WHERE id = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(seq.0, 123_456);
}

#[tokio::test]
async fn test_migration83_fold_state_is_strict() {
    let pool = setup_db().await;
    let (sql,): (String,) =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE type='table' AND name='fold_state'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        sql.contains("STRICT"),
        "fold_state must be STRICT; got:\n{sql}"
    );
}

#[tokio::test]
async fn test_migration82_session_action_log_rejects_null_required_fields() {
    // Every NOT NULL column must be enforced. Bind a NULL for each and
    // confirm the INSERT fails. Guards against a future migration
    // accidentally dropping NOT NULL on the audit trail (which would let
    // a bad handler silently land a zero-action row).
    let pool = setup_db().await;

    let each_null: &[(&str, &[&str])] = &[
        ("session_id", &["session_id"]),
        ("action", &["action"]),
        ("payload", &["payload"]),
        ("actor", &["actor"]),
        ("at", &["at"]),
    ];

    for (label, null_cols) in each_null {
        let sid = if null_cols.contains(&"session_id") {
            None
        } else {
            Some("s")
        };
        let act = if null_cols.contains(&"action") {
            None
        } else {
            Some("archive")
        };
        let pay = if null_cols.contains(&"payload") {
            None
        } else {
            Some("{}")
        };
        let who = if null_cols.contains(&"actor") {
            None
        } else {
            Some("user")
        };
        let at: Option<i64> = if null_cols.contains(&"at") {
            None
        } else {
            Some(1)
        };

        let result = sqlx::query(
            "INSERT INTO session_action_log (session_id, action, payload, actor, at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(sid)
        .bind(act)
        .bind(pay)
        .bind(who)
        .bind(at)
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "NULL {label} must be rejected; INSERT unexpectedly succeeded"
        );
    }
}

#[tokio::test]
async fn test_migration84_stage_c_outbox_columns_exist() {
    let pool = setup_db().await;
    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk
         FROM pragma_table_info('stage_c_outbox')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let by_name: std::collections::HashMap<&str, &str> = cols
        .iter()
        .map(|(_, n, t, _, _, _)| (n.as_str(), t.as_str()))
        .collect();
    assert_eq!(by_name.get("seq"), Some(&"INTEGER"));
    assert_eq!(by_name.get("delta_type"), Some(&"TEXT"));
    assert_eq!(by_name.get("payload_json"), Some(&"TEXT"));
    assert_eq!(by_name.get("applied_at"), Some(&"INTEGER"));
    assert_eq!(cols.len(), 4);
}

#[tokio::test]
async fn test_migration84_stage_c_outbox_partial_index() {
    let pool = setup_db().await;
    let (sql,): (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master
         WHERE type='index' AND name='idx_stage_c_outbox_pending'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        sql.contains("WHERE applied_at IS NULL"),
        "pending-index must be partial; got:\n{sql}"
    );
}

#[tokio::test]
async fn test_migration84_stage_c_outbox_is_strict() {
    let pool = setup_db().await;
    let (sql,): (String,) = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='stage_c_outbox'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        sql.contains("STRICT"),
        "stage_c_outbox must be STRICT; got:\n{sql}"
    );
}

#[tokio::test]
async fn test_migration85_legacy_columns_dropped() {
    let pool = setup_db().await;
    let cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let names: Vec<&str> = cols.iter().map(|(n,)| n.as_str()).collect();
    for dropped in [
        "archived_at",
        "category_l1",
        "category_l2",
        "category_l3",
        "category_confidence",
        "category_source",
        "classified_at",
    ] {
        assert!(
            !names.contains(&dropped),
            "column {dropped} must be dropped by migration 85"
        );
    }
}

#[tokio::test]
async fn test_migration85_valid_sessions_view_joins_session_flags() {
    let pool = setup_db().await;
    let (sql,): (String,) =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE type='view' AND name='valid_sessions'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        sql.contains("JOIN session_flags"),
        "valid_sessions must JOIN session_flags after migration 85; got:\n{sql}"
    );
    assert!(
        sql.contains("sf.archived_at IS NULL"),
        "valid_sessions must filter on sf.archived_at IS NULL; got:\n{sql}"
    );
}
