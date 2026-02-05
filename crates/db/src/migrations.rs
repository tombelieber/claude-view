/// Inline SQL migrations for vibe-recall database schema.
///
/// We use simple inline migrations rather than sqlx migration files
/// because the schema is small and self-contained.

pub const MIGRATIONS: &[&str] = &[
    // Migration 1: sessions table
    r#"
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT,
    preview TEXT NOT NULL DEFAULT '',
    turn_count INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    first_message_at INTEGER,
    last_message_at INTEGER,
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,
    indexed_at INTEGER,
    project_path TEXT NOT NULL DEFAULT '',
    project_display_name TEXT NOT NULL DEFAULT '',
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_message TEXT NOT NULL DEFAULT '',
    files_touched TEXT NOT NULL DEFAULT '[]',
    skills_used TEXT NOT NULL DEFAULT '[]',
    tool_counts_edit INTEGER NOT NULL DEFAULT 0,
    tool_counts_read INTEGER NOT NULL DEFAULT 0,
    tool_counts_bash INTEGER NOT NULL DEFAULT 0,
    tool_counts_write INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0
);
"#,
    // Migration 2: sessions indexes
    r#"
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
"#,
    r#"
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);
"#,
    // Migration 3: indexer_state table
    r#"
CREATE TABLE IF NOT EXISTS indexer_state (
    file_path TEXT PRIMARY KEY,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL
);
"#,
    // Migration 4: add session index fields
    r#"ALTER TABLE sessions ADD COLUMN summary TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN git_branch TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN is_sidechain BOOLEAN NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN deep_indexed_at INTEGER;"#,
    // Migration 5: invocables + invocations tables
    r#"
CREATE TABLE IF NOT EXISTS invocables (
    id          TEXT PRIMARY KEY,
    plugin_name TEXT,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    description TEXT DEFAULT '',
    status      TEXT DEFAULT 'enabled'
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS invocations (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    invocable_id TEXT NOT NULL REFERENCES invocables(id),
    session_id   TEXT NOT NULL,
    project      TEXT NOT NULL,
    timestamp    INTEGER NOT NULL,
    PRIMARY KEY (source_file, byte_offset)
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_invocable ON invocations(invocable_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_session   ON invocations(session_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_timestamp ON invocations(timestamp);"#,
    // Migration 6: models + turns tables (Phase 2B: token & model tracking)
    r#"
CREATE TABLE IF NOT EXISTS models (
    id         TEXT PRIMARY KEY,
    provider   TEXT,
    family     TEXT,
    first_seen INTEGER,
    last_seen  INTEGER
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS turns (
    session_id            TEXT NOT NULL,
    uuid                  TEXT NOT NULL,
    seq                   INTEGER NOT NULL,
    model_id              TEXT REFERENCES models(id),
    parent_uuid           TEXT,
    content_type          TEXT,
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cache_read_tokens     INTEGER,
    cache_creation_tokens INTEGER,
    service_tier          TEXT,
    timestamp             INTEGER,
    PRIMARY KEY (session_id, uuid)
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id, seq);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_turns_model   ON turns(model_id);"#,
    // Migration 7: Phase 2C indexes for branch/sidechain filtering
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_project_branch ON sessions(project_id, git_branch);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_sidechain ON sessions(is_sidechain);"#,
    // Migration 8: Phase 3 metrics engine schema
    // 8a: Add new session columns
    r#"ALTER TABLE sessions ADD COLUMN user_prompt_count INTEGER NOT NULL DEFAULT 0 CHECK (user_prompt_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN api_call_count INTEGER NOT NULL DEFAULT 0 CHECK (api_call_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN files_read TEXT NOT NULL DEFAULT '[]';"#,
    r#"ALTER TABLE sessions ADD COLUMN files_edited TEXT NOT NULL DEFAULT '[]';"#,
    r#"ALTER TABLE sessions ADD COLUMN files_read_count INTEGER NOT NULL DEFAULT 0 CHECK (files_read_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN files_edited_count INTEGER NOT NULL DEFAULT 0 CHECK (files_edited_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN reedited_files_count INTEGER NOT NULL DEFAULT 0 CHECK (reedited_files_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN commit_count INTEGER NOT NULL DEFAULT 0 CHECK (commit_count >= 0);"#,
    // 8b: Create commits table
    r#"
CREATE TABLE IF NOT EXISTS commits (
    hash            TEXT PRIMARY KEY,
    repo_path       TEXT NOT NULL,
    message         TEXT NOT NULL,
    author          TEXT,
    timestamp       INTEGER NOT NULL,
    branch          TEXT,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
"#,
    // 8c: Create session_commits junction table
    r#"
CREATE TABLE IF NOT EXISTS session_commits (
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL REFERENCES commits(hash) ON DELETE CASCADE,
    tier            INTEGER NOT NULL CHECK (tier IN (1, 2)),
    evidence        TEXT NOT NULL DEFAULT '{}',
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (session_id, commit_hash)
);
"#,
    // 8d: Create index_metadata singleton table
    r#"
CREATE TABLE IF NOT EXISTS index_metadata (
    id                      INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_indexed_at         INTEGER,
    last_index_duration_ms  INTEGER,
    sessions_indexed        INTEGER NOT NULL DEFAULT 0,
    projects_indexed        INTEGER NOT NULL DEFAULT 0,
    last_git_sync_at        INTEGER,
    commits_found           INTEGER NOT NULL DEFAULT 0,
    links_created           INTEGER NOT NULL DEFAULT 0,
    updated_at              INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
"#,
    r#"INSERT OR IGNORE INTO index_metadata (id) VALUES (1);"#,
    // 8e: Create indexes for commits and session_commits
    r#"CREATE INDEX IF NOT EXISTS idx_commits_repo_ts ON commits(repo_path, timestamp DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_commits_timestamp ON commits(timestamp DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_commits_session ON session_commits(session_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_commits_commit ON session_commits(commit_hash);"#,
    // 8f: Create indexes for new session columns
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_commit_count ON sessions(commit_count) WHERE commit_count > 0;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_reedit ON sessions(reedited_files_count) WHERE reedited_files_count > 0;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_duration ON sessions(duration_seconds);"#,
    // Migration 9: Add user-configurable git sync interval to settings
    r#"ALTER TABLE index_metadata ADD COLUMN git_sync_interval_secs INTEGER NOT NULL DEFAULT 60;"#,
    // Migration 10: Full JSONL parser schema (Phase 3.5)
    // 10a: Token aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN total_input_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN total_output_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN cache_creation_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN thinking_block_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10b: System line aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_avg_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_max_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_total_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN api_error_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN api_retry_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN compaction_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN hook_blocked_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10c: Progress line aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN agent_spawn_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN bash_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN hook_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN mcp_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10d: Summary + parse_version
    r#"ALTER TABLE sessions ADD COLUMN summary_text TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;"#,
    // 10e: Detail tables
    r#"
CREATE TABLE IF NOT EXISTS turn_metrics (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_seq INTEGER NOT NULL,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    model TEXT,
    PRIMARY KEY (session_id, turn_seq)
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS api_errors (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,
    retry_attempt INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    retry_in_ms REAL NOT NULL DEFAULT 0.0
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_api_errors_session_id ON api_errors(session_id);"#,
    // 10f: Partial index for re-indexing
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_needs_reindex ON sessions(id, file_path) WHERE parse_version < 1;"#,
    // Migration 11: File modification detection for re-indexing
    // Stores file size and mtime at last deep-index time so we can detect
    // when a JSONL file has grown (user continued a conversation).
    r#"ALTER TABLE sessions ADD COLUMN file_size_at_index INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN file_mtime_at_index INTEGER;"#,
    // Migration 12: Drop unused indexes on invocations table.
    // idx_invocations_session and idx_invocations_timestamp are never queried —
    // they only slow down bulk writes (~24k unnecessary B-tree ops per reindex).
    r#"DROP INDEX IF EXISTS idx_invocations_session;"#,
    r#"DROP INDEX IF EXISTS idx_invocations_timestamp;"#,
    // Migration 13: Theme 4 Foundation
    // 13a: Classification hierarchy columns on sessions
    r#"ALTER TABLE sessions ADD COLUMN category_l1 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_l2 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_l3 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_confidence REAL;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_source TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN classified_at TEXT;"#,
    // 13b: Behavioral metrics columns on sessions
    r#"ALTER TABLE sessions ADD COLUMN prompt_word_count INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN correction_count INTEGER DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN same_file_edit_count INTEGER DEFAULT 0;"#,
    // 13c: classification_jobs table
    r#"
CREATE TABLE IF NOT EXISTS classification_jobs (
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
);
"#,
    // 13d: index_runs table
    r#"
CREATE TABLE IF NOT EXISTS index_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    type TEXT NOT NULL,
    sessions_before INTEGER,
    sessions_after INTEGER,
    duration_ms INTEGER,
    throughput_mb_per_sec REAL,
    status TEXT DEFAULT 'running',
    error_message TEXT,
    CONSTRAINT valid_type CHECK (type IN ('full', 'incremental', 'deep')),
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'failed'))
);
"#,
    // 13e: Indexes for classification and index_runs
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_category_l1 ON sessions(category_l1) WHERE category_l1 IS NOT NULL;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_classified ON sessions(classified_at) WHERE classified_at IS NOT NULL;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_classification_jobs_status ON classification_jobs(status);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_classification_jobs_started ON classification_jobs(started_at DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_index_runs_started ON index_runs(started_at DESC);"#,
];

// ============================================================================
// Tests for migrations
// ============================================================================

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    /// Helper to create an in-memory database and run migrations.
    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        // Create migration tracking table
        sqlx::query("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();

        // Run all migrations
        for (i, migration) in super::MIGRATIONS.iter().enumerate() {
            let version = i + 1;
            match sqlx::query(migration).execute(&pool).await {
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

    #[tokio::test]
    async fn test_migration8_sessions_new_columns_exist() {
        let pool = setup_db().await;

        // Query the sessions table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        // Verify all new Phase 3 columns exist
        assert!(column_names.contains(&"user_prompt_count"), "Missing user_prompt_count column");
        assert!(column_names.contains(&"api_call_count"), "Missing api_call_count column");
        assert!(column_names.contains(&"tool_call_count"), "Missing tool_call_count column");
        assert!(column_names.contains(&"files_read"), "Missing files_read column");
        assert!(column_names.contains(&"files_edited"), "Missing files_edited column");
        assert!(column_names.contains(&"files_read_count"), "Missing files_read_count column");
        assert!(column_names.contains(&"files_edited_count"), "Missing files_edited_count column");
        assert!(column_names.contains(&"reedited_files_count"), "Missing reedited_files_count column");
        assert!(column_names.contains(&"duration_seconds"), "Missing duration_seconds column");
        assert!(column_names.contains(&"commit_count"), "Missing commit_count column");
    }

    #[tokio::test]
    async fn test_migration8_commits_table_exists() {
        let pool = setup_db().await;

        // Query commits table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('commits')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"hash"), "Missing hash column");
        assert!(column_names.contains(&"repo_path"), "Missing repo_path column");
        assert!(column_names.contains(&"message"), "Missing message column");
        assert!(column_names.contains(&"author"), "Missing author column");
        assert!(column_names.contains(&"timestamp"), "Missing timestamp column");
        assert!(column_names.contains(&"branch"), "Missing branch column");
        assert!(column_names.contains(&"created_at"), "Missing created_at column");
    }

    #[tokio::test]
    async fn test_migration8_session_commits_table_exists() {
        let pool = setup_db().await;

        // Query session_commits table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('session_commits')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"session_id"), "Missing session_id column");
        assert!(column_names.contains(&"commit_hash"), "Missing commit_hash column");
        assert!(column_names.contains(&"tier"), "Missing tier column");
        assert!(column_names.contains(&"evidence"), "Missing evidence column");
        assert!(column_names.contains(&"created_at"), "Missing created_at column");
    }

    #[tokio::test]
    async fn test_migration8_index_metadata_table_exists() {
        let pool = setup_db().await;

        // Query index_metadata table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('index_metadata')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"last_indexed_at"), "Missing last_indexed_at column");
        assert!(column_names.contains(&"last_index_duration_ms"), "Missing last_index_duration_ms column");
        assert!(column_names.contains(&"sessions_indexed"), "Missing sessions_indexed column");
        assert!(column_names.contains(&"projects_indexed"), "Missing projects_indexed column");
        assert!(column_names.contains(&"last_git_sync_at"), "Missing last_git_sync_at column");
        assert!(column_names.contains(&"commits_found"), "Missing commits_found column");
        assert!(column_names.contains(&"links_created"), "Missing links_created column");
        assert!(column_names.contains(&"updated_at"), "Missing updated_at column");

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
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        // Verify new Phase 3 indexes
        assert!(index_names.contains(&"idx_commits_repo_ts"), "Missing idx_commits_repo_ts index");
        assert!(index_names.contains(&"idx_commits_timestamp"), "Missing idx_commits_timestamp index");
        assert!(index_names.contains(&"idx_session_commits_session"), "Missing idx_session_commits_session index");
        assert!(index_names.contains(&"idx_session_commits_commit"), "Missing idx_session_commits_commit index");
        assert!(index_names.contains(&"idx_sessions_commit_count"), "Missing idx_sessions_commit_count index");
        assert!(index_names.contains(&"idx_sessions_reedit"), "Missing idx_sessions_reedit index");
        assert!(index_names.contains(&"idx_sessions_duration"), "Missing idx_sessions_duration index");
    }

    #[tokio::test]
    async fn test_migration12_unused_indexes_dropped() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        // These unused indexes should be dropped by migration 12
        assert!(!index_names.contains(&"idx_invocations_session"),
            "idx_invocations_session should be dropped (unused)");
        assert!(!index_names.contains(&"idx_invocations_timestamp"),
            "idx_invocations_timestamp should be dropped (unused)");

        // These used indexes must still exist
        assert!(index_names.contains(&"idx_invocations_invocable"),
            "idx_invocations_invocable must still exist (used by skills dashboard)");
        assert!(index_names.contains(&"idx_turns_session"),
            "idx_turns_session must still exist (used by session listing)");
        assert!(index_names.contains(&"idx_turns_model"),
            "idx_turns_model must still exist (used by models API)");
    }

    // ========================================================================
    // Migration 13 tests (Theme 4: Classification + Index Runs)
    // ========================================================================

    #[tokio::test]
    async fn test_migration13_classification_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"category_l1"), "Missing category_l1 column");
        assert!(column_names.contains(&"category_l2"), "Missing category_l2 column");
        assert!(column_names.contains(&"category_l3"), "Missing category_l3 column");
        assert!(column_names.contains(&"category_confidence"), "Missing category_confidence column");
        assert!(column_names.contains(&"category_source"), "Missing category_source column");
        assert!(column_names.contains(&"classified_at"), "Missing classified_at column");
        assert!(column_names.contains(&"prompt_word_count"), "Missing prompt_word_count column");
        assert!(column_names.contains(&"correction_count"), "Missing correction_count column");
        assert!(column_names.contains(&"same_file_edit_count"), "Missing same_file_edit_count column");
    }

    #[tokio::test]
    async fn test_migration13_classification_jobs_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('classification_jobs')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"started_at"), "Missing started_at column");
        assert!(column_names.contains(&"completed_at"), "Missing completed_at column");
        assert!(column_names.contains(&"total_sessions"), "Missing total_sessions column");
        assert!(column_names.contains(&"classified_count"), "Missing classified_count column");
        assert!(column_names.contains(&"skipped_count"), "Missing skipped_count column");
        assert!(column_names.contains(&"failed_count"), "Missing failed_count column");
        assert!(column_names.contains(&"provider"), "Missing provider column");
        assert!(column_names.contains(&"model"), "Missing model column");
        assert!(column_names.contains(&"status"), "Missing status column");
        assert!(column_names.contains(&"error_message"), "Missing error_message column");
        assert!(column_names.contains(&"cost_estimate_cents"), "Missing cost_estimate_cents column");
        assert!(column_names.contains(&"actual_cost_cents"), "Missing actual_cost_cents column");
        assert!(column_names.contains(&"tokens_used"), "Missing tokens_used column");
    }

    #[tokio::test]
    async fn test_migration13_index_runs_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('index_runs')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"started_at"), "Missing started_at column");
        assert!(column_names.contains(&"completed_at"), "Missing completed_at column");
        assert!(column_names.contains(&"type"), "Missing type column");
        assert!(column_names.contains(&"sessions_before"), "Missing sessions_before column");
        assert!(column_names.contains(&"sessions_after"), "Missing sessions_after column");
        assert!(column_names.contains(&"duration_ms"), "Missing duration_ms column");
        assert!(column_names.contains(&"throughput_mb_per_sec"), "Missing throughput_mb_per_sec column");
        assert!(column_names.contains(&"status"), "Missing status column");
        assert!(column_names.contains(&"error_message"), "Missing error_message column");
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
        assert!(result.is_err(), "Invalid status should be rejected by CHECK constraint");
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
        assert!(result.is_ok(), "Valid type 'full' and status 'running' should be accepted");

        // Invalid type should fail
        let result = sqlx::query(
            "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'invalid', 'running')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid type should be rejected by CHECK constraint");

        // Invalid status should fail
        let result = sqlx::query(
            "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'invalid')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid status should be rejected by CHECK constraint");
    }

    #[tokio::test]
    async fn test_migration13_indexes_created() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        assert!(index_names.contains(&"idx_sessions_category_l1"), "Missing idx_sessions_category_l1 index");
        assert!(index_names.contains(&"idx_sessions_classified"), "Missing idx_sessions_classified index");
        assert!(index_names.contains(&"idx_classification_jobs_status"), "Missing idx_classification_jobs_status index");
        assert!(index_names.contains(&"idx_classification_jobs_started"), "Missing idx_classification_jobs_started index");
        assert!(index_names.contains(&"idx_index_runs_started"), "Missing idx_index_runs_started index");
    }

    #[tokio::test]
    async fn test_migration8_check_constraints_work() {
        let pool = setup_db().await;

        // Insert a valid session first (required for FK in session_commits)
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview)
            VALUES ('test-sess', 'test-proj', '/tmp/test.jsonl', 'Test')
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        // Test that negative values are rejected for user_prompt_count
        let result = sqlx::query(
            "UPDATE sessions SET user_prompt_count = -1 WHERE id = 'test-sess'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative user_prompt_count should be rejected");

        // Test that negative values are rejected for duration_seconds
        let result = sqlx::query(
            "UPDATE sessions SET duration_seconds = -1 WHERE id = 'test-sess'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative duration_seconds should be rejected");

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
        assert!(result.is_err(), "tier=3 should be rejected (only 1 or 2 allowed)");

        // Test index_metadata singleton constraint
        let result = sqlx::query(
            "INSERT INTO index_metadata (id) VALUES (2)"
        )
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
            "#
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
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(names.contains(&"parse_version"), "Missing parse_version");
        assert!(names.contains(&"turn_duration_avg_ms"), "Missing turn_duration_avg_ms");
        assert!(names.contains(&"turn_duration_max_ms"), "Missing turn_duration_max_ms");
        assert!(names.contains(&"turn_duration_total_ms"), "Missing turn_duration_total_ms");
        assert!(names.contains(&"api_error_count"), "Missing api_error_count");
        assert!(names.contains(&"api_retry_count"), "Missing api_retry_count");
        assert!(names.contains(&"compaction_count"), "Missing compaction_count");
        assert!(names.contains(&"hook_blocked_count"), "Missing hook_blocked_count");
        assert!(names.contains(&"agent_spawn_count"), "Missing agent_spawn_count");
        assert!(names.contains(&"bash_progress_count"), "Missing bash_progress_count");
        assert!(names.contains(&"hook_progress_count"), "Missing hook_progress_count");
        assert!(names.contains(&"mcp_progress_count"), "Missing mcp_progress_count");
        assert!(names.contains(&"summary_text"), "Missing summary_text");
        assert!(names.contains(&"total_input_tokens"), "Missing total_input_tokens");
        assert!(names.contains(&"total_output_tokens"), "Missing total_output_tokens");
        assert!(names.contains(&"cache_read_tokens"), "Missing cache_read_tokens");
        assert!(names.contains(&"cache_creation_tokens"), "Missing cache_creation_tokens");
        assert!(names.contains(&"thinking_block_count"), "Missing thinking_block_count");
    }

    #[tokio::test]
    async fn test_migration9_detail_tables_exist() {
        let pool = setup_db().await;

        // Verify turn_metrics table
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('turn_metrics')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        assert!(names.contains(&"session_id"));
        assert!(names.contains(&"turn_seq"));
        assert!(names.contains(&"duration_ms"));
        assert!(names.contains(&"input_tokens"));
        assert!(names.contains(&"model"));

        // Verify api_errors table
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('api_errors')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        assert!(names.contains(&"session_id"));
        assert!(names.contains(&"timestamp_unix"));
        assert!(names.contains(&"retry_attempt"));
    }

    #[tokio::test]
    async fn test_migration9_parse_version_default() {
        let pool = setup_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('pv-test', 'proj', '/tmp/pv.jsonl', 'Test')"
        ).execute(&pool).await.unwrap();

        let row: (i64,) = sqlx::query_as(
            "SELECT parse_version FROM sessions WHERE id = 'pv-test'"
        ).fetch_one(&pool).await.unwrap();
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
            "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('sess-1', 'hash1', 1)"
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
}
