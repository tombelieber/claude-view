// crates/db/src/git_correlation/db_ops.rs
//! Database CRUD operations for commits and session-commit links.

use super::types::{CorrelationMatch, DiffStats, GitCommit, SessionSyncInfo};
use crate::{Database, DbResult};

impl Database {
    /// Batch upsert commits into the database.
    ///
    /// Uses `INSERT ... ON CONFLICT DO UPDATE` to upsert on hash.
    /// Returns the number of rows affected.
    pub async fn batch_upsert_commits(&self, commits: &[GitCommit]) -> DbResult<u64> {
        if commits.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool().begin().await?;
        let mut affected: u64 = 0;

        for commit in commits {
            let result = sqlx::query(
                r#"
                INSERT INTO commits (hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(hash) DO UPDATE SET
                    repo_path = excluded.repo_path,
                    message = excluded.message,
                    author = excluded.author,
                    timestamp = excluded.timestamp,
                    branch = excluded.branch,
                    files_changed = COALESCE(excluded.files_changed, commits.files_changed),
                    insertions = COALESCE(excluded.insertions, commits.insertions),
                    deletions = COALESCE(excluded.deletions, commits.deletions)
                "#,
            )
            .bind(&commit.hash)
            .bind(&commit.repo_path)
            .bind(&commit.message)
            .bind(&commit.author)
            .bind(commit.timestamp)
            .bind(&commit.branch)
            .bind(commit.files_changed.map(|v| v as i64))
            .bind(commit.insertions.map(|v| v as i64))
            .bind(commit.deletions.map(|v| v as i64))
            .execute(&mut *tx)
            .await?;

            affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    /// Update diff stats for a commit.
    ///
    /// Used to populate diff stats for commits that were initially created without them.
    pub async fn update_commit_diff_stats(
        &self,
        commit_hash: &str,
        stats: DiffStats,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE commits SET
                files_changed = ?2,
                insertions = ?3,
                deletions = ?4
            WHERE hash = ?1
            "#,
        )
        .bind(commit_hash)
        .bind(stats.files_changed as i64)
        .bind(stats.insertions as i64)
        .bind(stats.deletions as i64)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get commits missing diff stats (for backfill).
    pub async fn get_commits_without_diff_stats(&self, limit: usize) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT hash FROM commits
            WHERE files_changed IS NULL OR insertions IS NULL OR deletions IS NULL
            LIMIT ?1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(|(h,)| h).collect())
    }

    /// Insert session-commit links with tier and evidence.
    ///
    /// Uses `INSERT OR IGNORE` to skip duplicates (session_id + commit_hash).
    /// If a link already exists with a higher tier, it won't be overwritten.
    /// To prefer Tier 1 over Tier 2, call with Tier 1 matches first.
    ///
    /// Returns the number of rows inserted.
    pub async fn batch_insert_session_commits(
        &self,
        matches: &[CorrelationMatch],
    ) -> DbResult<u64> {
        if matches.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool().begin().await?;
        let mut inserted: u64 = 0;

        for m in matches {
            let evidence_json =
                serde_json::to_string(&m.evidence).unwrap_or_else(|_| "{}".to_string());

            // Use INSERT OR REPLACE to allow upgrading tier (lower tier number = higher priority)
            // First check if a link exists with a lower (better) tier
            let existing: Option<(i32,)> = sqlx::query_as(
                "SELECT tier FROM session_commits WHERE session_id = ?1 AND commit_hash = ?2",
            )
            .bind(&m.session_id)
            .bind(&m.commit_hash)
            .fetch_optional(&mut *tx)
            .await?;

            let should_insert = match existing {
                None => true,                                     // No existing link
                Some((existing_tier,)) => m.tier < existing_tier, // Only insert if new tier is better
            };

            if should_insert {
                let result = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO session_commits (session_id, commit_hash, tier, evidence)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                )
                .bind(&m.session_id)
                .bind(&m.commit_hash)
                .bind(m.tier)
                .bind(&evidence_json)
                .execute(&mut *tx)
                .await?;

                inserted += result.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(inserted)
    }

    /// Get all commits linked to a session with their tier and evidence.
    pub async fn get_commits_for_session(
        &self,
        session_id: &str,
    ) -> DbResult<Vec<(GitCommit, i32, String)>> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            i64,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            i32,
            String,
        )> = sqlx::query_as(
            r#"
            SELECT c.hash, c.repo_path, c.message, c.author, c.timestamp, c.branch,
                   c.files_changed, c.insertions, c.deletions,
                   sc.tier, sc.evidence
            FROM commits c
            INNER JOIN session_commits sc ON c.hash = sc.commit_hash
            WHERE sc.session_id = ?1
            ORDER BY c.timestamp DESC
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        let results = rows
            .into_iter()
            .map(
                |(
                    hash,
                    repo_path,
                    message,
                    author,
                    timestamp,
                    branch,
                    files_changed,
                    insertions,
                    deletions,
                    tier,
                    evidence,
                )| {
                    let commit = GitCommit {
                        hash,
                        repo_path,
                        message,
                        author,
                        timestamp,
                        branch,
                        files_changed: files_changed.map(|v| v as u32),
                        insertions: insertions.map(|v| v as u32),
                        deletions: deletions.map(|v| v as u32),
                    };
                    (commit, tier, evidence)
                },
            )
            .collect();

        Ok(results)
    }

    /// Get commits for a repository within a time range.
    ///
    /// Useful for finding commits that might correlate with sessions.
    pub async fn get_commits_in_range(
        &self,
        repo_path: &str,
        start_ts: i64,
        end_ts: i64,
    ) -> DbResult<Vec<GitCommit>> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, Option<String>, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>)> =
            sqlx::query_as(
                r#"
            SELECT hash, repo_path, message, author, timestamp, branch, files_changed, insertions, deletions
            FROM commits
            WHERE repo_path = ?1 AND timestamp >= ?2 AND timestamp <= ?3
            ORDER BY timestamp DESC
            "#,
            )
            .bind(repo_path)
            .bind(start_ts)
            .bind(end_ts)
            .fetch_all(self.pool())
            .await?;

        let commits = rows
            .into_iter()
            .map(
                |(
                    hash,
                    repo_path,
                    message,
                    author,
                    timestamp,
                    branch,
                    files_changed,
                    insertions,
                    deletions,
                )| GitCommit {
                    hash,
                    repo_path,
                    message,
                    author,
                    timestamp,
                    branch,
                    files_changed: files_changed.map(|v| v as u32),
                    insertions: insertions.map(|v| v as u32),
                    deletions: deletions.map(|v| v as u32),
                },
            )
            .collect();

        Ok(commits)
    }

    /// Count commits linked to a session (for updating session.commit_count).
    pub async fn count_commits_for_session(&self, session_id: &str) -> DbResult<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_commits WHERE session_id = ?1")
                .bind(session_id)
                .fetch_one(self.pool())
                .await?;

        Ok(count)
    }

    /// Update the commit_count field on a session.
    pub async fn update_session_commit_count(
        &self,
        session_id: &str,
        commit_count: i32,
    ) -> DbResult<()> {
        sqlx::query("UPDATE sessions SET commit_count = ?2 WHERE id = ?1")
            .bind(session_id)
            .bind(commit_count)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Update session LOC stats from git diff (Phase F: Git Diff Stats Overlay).
    ///
    /// Sets lines_added, lines_removed, and loc_source = 2 (git verified).
    /// Only updates if new stats are provided (not 0+0).
    pub async fn update_session_loc_from_git(
        &self,
        session_id: &str,
        stats: &DiffStats,
    ) -> DbResult<()> {
        // Only update if we have actual stats
        if stats.insertions == 0 && stats.deletions == 0 {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE sessions
            SET lines_added = ?2, lines_removed = ?3, loc_source = 2
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .bind(stats.insertions as i64)
        .bind(stats.deletions as i64)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Fetch all sessions eligible for git correlation.
    ///
    /// Filters:
    /// - `project_path` must be non-empty (sessions without a project can't have a repo)
    /// - `last_message_at` must be non-NULL (need at least one timestamp for time window)
    ///
    /// This is deliberately lightweight: a single-table SELECT with no JOINs.
    pub async fn get_sessions_for_git_sync(&self) -> DbResult<Vec<SessionSyncInfo>> {
        let rows: Vec<(String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT id, project_path, first_message_at, last_message_at
            FROM sessions
            WHERE project_path != '' AND last_message_at IS NOT NULL
            ORDER BY last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(session_id, project_path, first_message_at, last_message_at)| SessionSyncInfo {
                    session_id,
                    project_path,
                    first_message_at,
                    last_message_at,
                },
            )
            .collect())
    }
}

#[cfg(test)]
#[path = "db_ops_tests.rs"]
mod tests;
