// crates/db/src/lib.rs
// Phase 2: SQLite database for vibe-recall session indexing

mod migrations;
mod queries;
pub mod indexer;
pub mod indexer_parallel;
pub mod git_correlation;
pub mod trends;

pub use queries::IndexerEntry;
pub use queries::InvocableWithCount;
pub use queries::ModelWithStats;
pub use queries::StatsOverview;
pub use queries::TokenStats;

// Re-export trends types
pub use trends::current_week_bounds;
pub use trends::previous_week_bounds;
pub use trends::IndexMetadata;
pub use trends::TrendMetric;
pub use trends::WeekTrends;

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

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .map_err(sqlx::Error::from)?
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

        let db = Self { pool, db_path: path.to_owned() };
        db.run_migrations().await?;

        info!("Database opened at {}", path.display());
        Ok(db)
    }

    /// Create an in-memory database (for testing).
    pub async fn new_in_memory() -> DbResult<Self> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        let db = Self { pool, db_path: PathBuf::new() };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Open the database at the default location: `~/.cache/vibe-recall/vibe-recall.db`
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
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY)"
        )
        .execute(&self.pool)
        .await?;

        // Find the highest version already applied (0 if none)
        let row: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations"
        )
        .fetch_one(&self.pool)
        .await?;
        let current_version = row.0 as usize;

        // Run only new migrations
        for (i, migration) in migrations::MIGRATIONS.iter().enumerate() {
            let version = i + 1; // 1-based
            if version > current_version {
                match sqlx::query(migration).execute(&self.pool).await {
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

/// Returns the default database path: `~/.cache/vibe-recall/vibe-recall.db`
pub fn default_db_path() -> DbResult<PathBuf> {
    let cache_dir = dirs::cache_dir().ok_or(DbError::NoCacheDir)?;
    Ok(cache_dir.join("vibe-recall").join("vibe-recall.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_database() {
        // Open in-memory DB, run migrations, verify no errors
        let db = Database::new_in_memory().await.expect("should create in-memory database");

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
        let db = Database::new_in_memory().await.expect("first open should succeed");

        // Run migrations again explicitly
        db.run_migrations().await.expect("second migration run should succeed");

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

        let db = Database::new(&db_path).await.expect("should create file-based database");

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
        assert!(path.to_string_lossy().contains("vibe-recall"));
        assert!(path.to_string_lossy().ends_with("vibe-recall.db"));
    }
}
