//! Lightweight SQLite pre-filter for search.
//!
//! Returns session IDs matching structured filters (project, branch, model, date).
//! Used by the search handler to narrow the file set before grep runs.

use crate::{Database, DbResult};
use std::collections::HashSet;

/// Structured filters for search pre-filtering.
/// All fields are optional — None means no filter on that dimension.
#[derive(Debug, Default)]
pub struct SearchPrefilter {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub after: Option<i64>,  // Unix timestamp — filter on last_message_at
    pub before: Option<i64>, // Unix timestamp — filter on last_message_at
}

impl SearchPrefilter {
    /// Returns true if no filters are set (caller can skip SQLite entirely).
    pub fn is_empty(&self) -> bool {
        self.project.is_none()
            && self.branch.is_none()
            && self.model.is_none()
            && self.after.is_none()
            && self.before.is_none()
    }
}

impl Database {
    /// Return session IDs matching structured filters.
    ///
    /// Uses the polymorphic project filter pattern (CLAUDE.md Hard Rule):
    /// checks BOTH `project_id` AND `git_root` for any project filter value.
    ///
    /// Column name mapping (actual schema):
    /// - branch  → `git_branch`
    /// - model   → `primary_model`
    /// - project → `project_id` OR `git_root`
    /// - after/before → `last_message_at`
    pub async fn search_prefilter_session_ids(
        &self,
        filter: &SearchPrefilter,
    ) -> DbResult<HashSet<String>> {
        let mut qb = sqlx::QueryBuilder::new("SELECT id FROM sessions WHERE 1=1");

        if let Some(ref project) = filter.project {
            // Polymorphic project filter — check BOTH project_id AND git_root.
            // The sidebar sends git_root paths (e.g. `/Users/TBGor/dev/project`)
            // for 98%+ of sessions; project_id is the encoded form (e.g. `-Users-...`).
            qb.push(" AND (project_id = ");
            qb.push_bind(project);
            qb.push(" OR (git_root IS NOT NULL AND git_root <> '' AND git_root = ");
            qb.push_bind(project);
            qb.push("))");
        }

        if let Some(ref branch) = filter.branch {
            // Column is git_branch (not branch)
            qb.push(" AND git_branch = ");
            qb.push_bind(branch);
        }

        if let Some(ref model) = filter.model {
            // Column is primary_model (not model)
            qb.push(" AND primary_model = ");
            qb.push_bind(model);
        }

        if let Some(after) = filter.after {
            qb.push(" AND last_message_at > ");
            qb.push_bind(after);
        }

        if let Some(before) = filter.before {
            qb.push(" AND last_message_at < ");
            qb.push_bind(before);
        }

        let rows: Vec<(String,)> = qb.build_query_as().fetch_all(self.pool()).await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    /// Insert three test sessions with different projects/branches/models.
    /// `file_path` must be NOT NULL UNIQUE so each row gets a distinct value.
    async fn setup_db() -> Database {
        let db = Database::new_in_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, git_root, git_branch, primary_model, file_path, last_message_at)
             VALUES
             ('s1', 'proj-a', '/dev/proj-a', 'main',   'claude-3-opus-20240229',   '/dev/proj-a/s1.jsonl', 1710000100),
             ('s2', 'proj-a', '/dev/proj-a', 'feat',   'claude-3-5-sonnet-20241022', '/dev/proj-a/s2.jsonl', 1710000200),
             ('s3', 'proj-b', '/dev/proj-b', 'main',   'claude-3-opus-20240229',   '/dev/proj-b/s3.jsonl', 1710000300)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn test_prefilter_no_filters_returns_all() {
        let db = setup_db().await;
        let ids = db
            .search_prefilter_session_ids(&SearchPrefilter::default())
            .await
            .unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[tokio::test]
    async fn test_prefilter_by_project_uses_git_root() {
        let db = setup_db().await;
        // Filter by git_root path — the polymorphic filter must match this
        let filter = SearchPrefilter {
            project: Some("/dev/proj-a".to_string()),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("s1"));
        assert!(ids.contains("s2"));
    }

    #[tokio::test]
    async fn test_prefilter_by_project_uses_project_id() {
        let db = setup_db().await;
        // Filter by project_id (encoded form) — should also match
        let filter = SearchPrefilter {
            project: Some("proj-b".to_string()),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("s3"));
    }

    #[tokio::test]
    async fn test_prefilter_by_branch() {
        let db = setup_db().await;
        let filter = SearchPrefilter {
            branch: Some("main".to_string()),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        // s1 (proj-a/main) and s3 (proj-b/main)
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("s1"));
        assert!(ids.contains("s3"));
    }

    #[tokio::test]
    async fn test_prefilter_by_model() {
        let db = setup_db().await;
        let filter = SearchPrefilter {
            model: Some("claude-3-5-sonnet-20241022".to_string()),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("s2"));
    }

    #[tokio::test]
    async fn test_prefilter_by_project_and_branch() {
        let db = setup_db().await;
        let filter = SearchPrefilter {
            project: Some("/dev/proj-a".to_string()),
            branch: Some("main".to_string()),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("s1"));
    }

    #[tokio::test]
    async fn test_prefilter_by_date_range_after() {
        let db = setup_db().await;
        // last_message_at > 1710000150 → s2 (200) and s3 (300)
        let filter = SearchPrefilter {
            after: Some(1710000150),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("s2"));
        assert!(ids.contains("s3"));
    }

    #[tokio::test]
    async fn test_prefilter_by_date_range_before() {
        let db = setup_db().await;
        // last_message_at < 1710000200 → s1 (100) only
        let filter = SearchPrefilter {
            before: Some(1710000200),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("s1"));
    }

    #[tokio::test]
    async fn test_prefilter_by_date_range_combined() {
        let db = setup_db().await;
        // 1710000100 < last_message_at < 1710000300 → s2 (200) only
        let filter = SearchPrefilter {
            after: Some(1710000100),
            before: Some(1710000300),
            ..Default::default()
        };
        let ids = db.search_prefilter_session_ids(&filter).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("s2"));
    }

    #[tokio::test]
    async fn test_prefilter_is_empty() {
        assert!(SearchPrefilter::default().is_empty());
        assert!(!SearchPrefilter {
            project: Some("x".to_string()),
            ..Default::default()
        }
        .is_empty());
        assert!(!SearchPrefilter {
            branch: Some("main".to_string()),
            ..Default::default()
        }
        .is_empty());
        assert!(!SearchPrefilter {
            model: Some("opus".to_string()),
            ..Default::default()
        }
        .is_empty());
        assert!(!SearchPrefilter {
            after: Some(1000),
            ..Default::default()
        }
        .is_empty());
        assert!(!SearchPrefilter {
            before: Some(2000),
            ..Default::default()
        }
        .is_empty());
    }
}
