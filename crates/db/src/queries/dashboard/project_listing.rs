// crates/db/src/queries/dashboard/project_listing.rs
// Branch listing for a project. (Project summaries + per-project session
// listing moved to SessionCatalog-backed handlers in the server crate;
// see `crates/server/src/routes/projects.rs`.)

use super::super::BranchCount;
use crate::{Database, DbResult};

impl Database {
    /// List distinct branches with session counts for a project identity.
    ///
    /// Returns branches sorted by session count DESC.
    /// Includes sessions with `git_branch = NULL` as a separate entry.
    ///
    /// `project_identity` may be either:
    /// - `project_id` (legacy per-worktree identity), or
    /// - `git_root` (effective sidebar identity for merged worktrees).
    pub async fn list_branches_for_project(
        &self,
        project_identity: &str,
    ) -> DbResult<Vec<BranchCount>> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT NULLIF(git_branch, '') as branch, COUNT(*) as count
            FROM valid_sessions
            WHERE (
                project_id = ?1
                OR (git_root IS NOT NULL AND git_root != '' AND git_root = ?1)
                OR (project_path IS NOT NULL AND project_path != '' AND project_path = ?1)
            )
            GROUP BY NULLIF(git_branch, '')
            ORDER BY count DESC
            "#,
        )
        .bind(project_identity)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(branch, count)| BranchCount { branch, count })
            .collect())
    }
}
