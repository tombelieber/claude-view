// crates/db/src/snapshots/session_detail.rs
//! Session contribution detail, linked commits, and file impact queries.

use super::types::{FileImpact, LinkedCommit, SessionContribution};
use crate::{Database, DbResult};

impl Database {
    /// Get contribution detail for a single session.
    pub async fn get_session_contribution(
        &self,
        session_id: &str,
    ) -> DbResult<Option<SessionContribution>> {
        let row: Option<(String, Option<String>, i64, i64, i64, i64, i64, i64, i64)> =
            sqlx::query_as(
                r#"
            SELECT
                id,
                work_type,
                duration_seconds,
                user_prompt_count,
                ai_lines_added,
                ai_lines_removed,
                files_edited_count,
                reedited_files_count,
                commit_count
            FROM valid_sessions
            WHERE id = ?1
            "#,
            )
            .bind(session_id)
            .fetch_optional(self.pool())
            .await?;

        Ok(row.map(
            |(
                session_id,
                work_type,
                duration_seconds,
                prompt_count,
                ai_lines_added,
                ai_lines_removed,
                files_edited_count,
                reedited_files_count,
                commit_count,
            )| {
                SessionContribution {
                    session_id,
                    work_type,
                    duration_seconds,
                    prompt_count,
                    ai_lines_added,
                    ai_lines_removed,
                    files_edited_count,
                    reedited_files_count,
                    commit_count,
                }
            },
        ))
    }

    /// Get commits linked to a session.
    pub async fn get_session_commits(&self, session_id: &str) -> DbResult<Vec<LinkedCommit>> {
        let rows: Vec<(String, String, Option<i64>, Option<i64>, i64)> = sqlx::query_as(
            r#"
            SELECT
                c.hash,
                c.message,
                c.insertions,
                c.deletions,
                sc.tier
            FROM session_commits sc
            JOIN commits c ON sc.commit_hash = c.hash
            WHERE sc.session_id = ?1
            ORDER BY c.timestamp ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(hash, message, insertions, deletions, tier)| LinkedCommit {
                    hash,
                    message,
                    insertions,
                    deletions,
                    tier,
                },
            )
            .collect())
    }

    /// Get file impacts for a session.
    ///
    /// Parses files_edited JSON from the session.
    pub async fn get_session_file_impacts(&self, session_id: &str) -> DbResult<Vec<FileImpact>> {
        // Get files_edited JSON from session
        let row: Option<(String,)> =
            sqlx::query_as("SELECT files_edited FROM valid_sessions WHERE id = ?1")
                .bind(session_id)
                .fetch_optional(self.pool())
                .await?;

        let Some((files_json,)) = row else {
            return Ok(Vec::new());
        };

        // Parse the files_edited JSON
        // Expected format: array of file paths or objects with path info
        let files: Vec<String> = match serde_json::from_str(&files_json) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Failed to parse files_edited JSON for session {session_id}: {e}");
                Vec::new()
            }
        };

        // For now, we return basic file info without detailed line counts
        // (detailed line counts would require parsing the JSONL file)
        Ok(files
            .into_iter()
            .map(|path| FileImpact {
                path,
                lines_added: 0, // Would need JSONL parsing for actual counts
                lines_removed: 0,
                action: "modified".to_string(),
            })
            .collect())
    }
}
