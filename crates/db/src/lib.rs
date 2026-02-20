// crates/db/src/lib.rs
// Phase 2: SQLite database for claude-view session indexing
#![allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::derivable_impls
)]

pub mod git_correlation;
pub mod indexer;
pub mod indexer_parallel;
pub mod insights_trends;
mod migrations;
pub mod pricing;
mod queries;
pub mod snapshots;
pub mod trends;

pub use queries::facets::{FacetAggregateStats, FacetRow};
pub use queries::hook_events::{self as hook_events_queries, HookEventRow};
pub use queries::reports::{ProjectPreview, ReportPreview, ReportRow};
pub use queries::AggregateCostBreakdown;
pub use queries::AIGenerationStats;
pub use queries::ActivityPoint;
pub use queries::BranchCount;
pub use queries::ClassificationStatus;
pub use queries::HealthStats;
pub use queries::HealthStatus;
pub use queries::IndexerEntry;
pub use queries::InvocableWithCount;
pub use queries::ModelWithStats;
pub use queries::SessionFilterParams;
pub use queries::StatsOverview;
pub use queries::StorageStats;
pub use queries::TokenStats;
pub use queries::TokensByModel;
pub use queries::TokensByProject;

// Re-export trends types
pub use trends::current_week_bounds;
pub use trends::previous_week_bounds;
pub use trends::IndexMetadata;
pub use trends::TrendMetric;
pub use trends::WeekTrends;

// Re-export unified pricing types (owned by claude_view_core::pricing)
pub use claude_view_core::pricing::{
    calculate_cost, calculate_cost_usd, default_pricing, lookup_pricing, CacheStatus,
    CostBreakdown, ModelPricing, TokenBreakdown, TokenUsage, FALLBACK_INPUT_COST_PER_TOKEN,
    FALLBACK_OUTPUT_COST_PER_TOKEN,
};
// Re-export DB-owned pricing refresh helpers.
pub use pricing::{fetch_litellm_pricing, load_pricing_cache, merge_pricing, save_pricing_cache};

// Re-export snapshots types
pub use snapshots::AggregatedContributions;
pub use snapshots::BranchBreakdown;
pub use snapshots::BranchSession;
pub use snapshots::ContributionSnapshot;
pub use snapshots::DailyTrendPoint;
pub use snapshots::FileImpact;
pub use snapshots::LearningCurve;
pub use snapshots::LearningCurvePeriod;
pub use snapshots::LinkedCommit;
pub use snapshots::ModelBreakdown;
pub use snapshots::ModelStats;
pub use snapshots::SessionContribution;
pub use snapshots::SkillStats;
pub use snapshots::TimeRange;
pub use snapshots::UncommittedWork;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Failed to determine cache directory")]
    NoCacheDir,

    #[error("Failed to create database directory: {0}")]
    CreateDir(#[from] std::io::Error),
}

pub type DbResult<T> = Result<T, DbError>;

/// Main database handle wrapping a SQLite connection pool.
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl Database {
    /// Open (or create) the database at the given path and run migrations.
    pub async fn new(path: &Path) -> DbResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(30))
            .log_slow_statements(
                tracing::log::LevelFilter::Warn,
                std::time::Duration::from_secs(5),
            );

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let db = Self {
            pool,
            db_path: path.to_owned(),
        };
        db.run_migrations().await?;

        info!("Database opened at {}", path.display());
        Ok(db)
    }

    /// Create an in-memory database (for testing).
    ///
    /// Uses `shared_cache(true)` so all pool connections share the same
    /// in-memory database. Without this, each connection gets its own
    /// separate database, breaking `tokio::try_join!` and concurrent queries.
    pub async fn new_in_memory() -> DbResult<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .shared_cache(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let db = Self {
            pool,
            db_path: PathBuf::new(),
        };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Open the database at the default location: `~/.cache/claude-view/claude-view.db`
    pub async fn open_default() -> DbResult<Self> {
        let path = default_db_path()?;
        Self::new(&path).await
    }

    /// Run all inline migrations.
    ///
    /// Uses a `_migrations` table to track which migrations have already been
    /// applied, so that non-idempotent statements (e.g. ALTER TABLE ADD COLUMN)
    /// are only executed once.
    async fn run_migrations(&self) -> DbResult<()> {
        // Ensure the migration-tracking table exists
        sqlx::query("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY)")
            .execute(&self.pool)
            .await?;

        // Find the highest version already applied (0 if none)
        let row: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(version), 0) FROM _migrations")
            .fetch_one(&self.pool)
            .await?;
        let current_version = row.0 as usize;

        // Run only new migrations
        for (i, migration) in migrations::MIGRATIONS.iter().enumerate() {
            let version = i + 1; // 1-based
            if version > current_version {
                // Multi-statement migrations (containing BEGIN/COMMIT) use raw_sql()
                // which supports executing multiple statements atomically.
                // Single-statement migrations use query() as before.
                let is_multi_statement =
                    migration.contains("BEGIN;") || migration.contains("BEGIN\n");
                let result = if is_multi_statement {
                    sqlx::raw_sql(migration)
                        .execute(&self.pool)
                        .await
                        .map(|_| ())
                } else {
                    sqlx::query(migration).execute(&self.pool).await.map(|_| ())
                };
                match result {
                    Ok(_) => {}
                    Err(e) if e.to_string().contains("duplicate column name") => {
                        // Column already exists from a previous run without tracking.
                        // Safe to skip.
                    }
                    Err(e) => return Err(e.into()),
                }
                sqlx::query("INSERT INTO _migrations (version) VALUES (?)")
                    .bind(version as i64)
                    .execute(&self.pool)
                    .await?;
            }
        }

        // Post-migration schema reconciliation: ensure critical columns exist
        // even if another branch's code occupied the same migration version slots.
        self.ensure_schema_columns().await?;

        Ok(())
    }

    /// Ensure critical columns exist regardless of migration version tracking.
    ///
    /// When multiple git branches add different migrations at the same version
    /// slots, the migration tracker may think a version is applied when the
    /// actual SQL was different. This catches that case by checking for expected
    /// columns and adding them if missing.
    async fn ensure_schema_columns(&self) -> DbResult<()> {
        let expected_session_cols = &[
            // Main LOC estimation columns
            ("lines_added", "INTEGER NOT NULL DEFAULT 0"),
            ("lines_removed", "INTEGER NOT NULL DEFAULT 0"),
            ("loc_source", "INTEGER NOT NULL DEFAULT 0"),
            // Theme 3 contribution columns
            ("ai_lines_added", "INTEGER NOT NULL DEFAULT 0"),
            ("ai_lines_removed", "INTEGER NOT NULL DEFAULT 0"),
            ("work_type", "TEXT"),
        ];
        let expected_commit_cols = &[
            ("files_changed", "INTEGER"),
            ("insertions", "INTEGER"),
            ("deletions", "INTEGER"),
        ];

        for (col, typedef) in expected_session_cols {
            self.add_column_if_missing("sessions", col, typedef).await?;
        }
        for (col, typedef) in expected_commit_cols {
            self.add_column_if_missing("commits", col, typedef).await?;
        }

        // Ensure contribution_snapshots table exists
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS contribution_snapshots (
                id INTEGER PRIMARY KEY,
                date TEXT NOT NULL,
                project_id TEXT,
                branch TEXT,
                sessions_count INTEGER DEFAULT 0,
                ai_lines_added INTEGER DEFAULT 0,
                ai_lines_removed INTEGER DEFAULT 0,
                commits_count INTEGER DEFAULT 0,
                commit_insertions INTEGER DEFAULT 0,
                commit_deletions INTEGER DEFAULT 0,
                tokens_used INTEGER DEFAULT 0,
                cost_cents INTEGER DEFAULT 0,
                UNIQUE(date, project_id, branch)
            )"#,
        )
        .execute(&self.pool)
        .await?;

        // Ensure indexes exist
        for idx_sql in &[
            "CREATE INDEX IF NOT EXISTS idx_snapshots_date ON contribution_snapshots(date)",
            "CREATE INDEX IF NOT EXISTS idx_snapshots_project_date ON contribution_snapshots(project_id, date)",
            "CREATE INDEX IF NOT EXISTS idx_snapshots_branch_date ON contribution_snapshots(project_id, branch, date)",
        ] {
            sqlx::query(idx_sql).execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Add a column to a table if it doesn't already exist.
    async fn add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        typedef: &str,
    ) -> DbResult<()> {
        let columns: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{}')", table))
                .fetch_all(&self.pool)
                .await?;

        let has_column = columns.iter().any(|(name,)| name == column);
        if !has_column {
            let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, typedef);
            sqlx::query(&sql).execute(&self.pool).await?;
            info!("Schema reconciliation: added {}.{}", table, column);
        }

        Ok(())
    }

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get the path to the database file.
    /// Returns an empty path for in-memory databases.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// Returns the default database path: `~/.cache/claude-view/claude-view.db`
pub fn default_db_path() -> DbResult<PathBuf> {
    claude_view_core::paths::db_path().ok_or(DbError::NoCacheDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_database() {
        // Open in-memory DB, run migrations, verify no errors
        let db = Database::new_in_memory()
            .await
            .expect("should create in-memory database");

        // Verify sessions table exists by querying it
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .expect("sessions table should exist");
        assert_eq!(count.0, 0);

        // Verify indexer_state table exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM indexer_state")
            .fetch_one(db.pool())
            .await
            .expect("indexer_state table should exist");
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        // Run migrations twice — should not error
        let db = Database::new_in_memory()
            .await
            .expect("first open should succeed");

        // Run migrations again explicitly
        db.run_migrations()
            .await
            .expect("second migration run should succeed");

        // Still works
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .expect("sessions table should still exist");
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn test_file_based_database() {
        let tmp = tempfile::tempdir().expect("should create temp dir");
        let db_path = tmp.path().join("test.db");

        let db = Database::new(&db_path)
            .await
            .expect("should create file-based database");

        // Verify table exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .expect("sessions table should exist");
        assert_eq!(count.0, 0);

        assert!(db_path.exists(), "database file should be created on disk");
    }

    #[tokio::test]
    async fn test_default_db_path() {
        let path = default_db_path().expect("should resolve default path");
        assert!(path.to_string_lossy().contains("claude-view"));
        assert!(path.to_string_lossy().ends_with("claude-view.db"));
    }
}
