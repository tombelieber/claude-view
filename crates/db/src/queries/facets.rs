// crates/db/src/queries/facets.rs
// Facet CRUD operations: upsert, lookup, aggregate stats, quality badges.

use crate::Database;
use sqlx::Row;

/// Maximum number of SQLite bind variables per query.
/// SQLite has a hard limit of 999; we stay below it.
const SQLITE_VARIABLE_LIMIT: usize = 900;

#[derive(Debug, Clone)]
pub struct FacetRow {
    pub session_id: String,
    pub source: String,
    pub underlying_goal: Option<String>,
    pub goal_categories: String,
    pub outcome: Option<String>,
    pub satisfaction: Option<String>,
    pub user_satisfaction_counts: String,
    pub claude_helpfulness: Option<String>,
    pub session_type: Option<String>,
    pub friction_counts: String,
    pub friction_detail: Option<String>,
    pub primary_success: Option<String>,
    pub brief_summary: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for FacetRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            session_id: row.try_get("session_id")?,
            source: row.try_get("source")?,
            underlying_goal: row.try_get("underlying_goal")?,
            goal_categories: row.try_get("goal_categories")?,
            outcome: row.try_get("outcome")?,
            satisfaction: row.try_get("satisfaction")?,
            user_satisfaction_counts: row.try_get("user_satisfaction_counts")?,
            claude_helpfulness: row.try_get("claude_helpfulness")?,
            session_type: row.try_get("session_type")?,
            friction_counts: row.try_get("friction_counts")?,
            friction_detail: row.try_get("friction_detail")?,
            primary_success: row.try_get("primary_success")?,
            brief_summary: row.try_get("brief_summary")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FacetAggregateStats {
    pub total_with_facets: i64,
    pub total_without_facets: i64,
    pub achievement_rate: f64,
    pub frustrated_count: i64,
    pub satisfied_or_above_count: i64,
    pub friction_session_count: i64,
}

impl Database {
    /// Batch upsert facets into session_facets using INSERT OR REPLACE.
    /// All rows are written in a single transaction.
    /// Returns the count of rows processed.
    pub async fn batch_upsert_facets(&self, facets: &[FacetRow]) -> sqlx::Result<usize> {
        let mut tx = self.pool.begin().await?;
        for facet in facets {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO session_facets (
                    session_id, source, underlying_goal, goal_categories,
                    outcome, satisfaction, user_satisfaction_counts,
                    claude_helpfulness, session_type, friction_counts,
                    friction_detail, primary_success, brief_summary
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
            )
            .bind(&facet.session_id)
            .bind(&facet.source)
            .bind(&facet.underlying_goal)
            .bind(&facet.goal_categories)
            .bind(&facet.outcome)
            .bind(&facet.satisfaction)
            .bind(&facet.user_satisfaction_counts)
            .bind(&facet.claude_helpfulness)
            .bind(&facet.session_type)
            .bind(&facet.friction_counts)
            .bind(&facet.friction_detail)
            .bind(&facet.primary_success)
            .bind(&facet.brief_summary)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(facets.len())
    }

    /// Get the facet row for a single session, if it exists.
    pub async fn get_session_facet(&self, session_id: &str) -> sqlx::Result<Option<FacetRow>> {
        let row: Option<FacetRow> = sqlx::query_as(
            r#"
            SELECT session_id, source, underlying_goal, goal_categories,
                   outcome, satisfaction, user_satisfaction_counts,
                   claude_helpfulness, session_type, friction_counts,
                   friction_detail, primary_success, brief_summary
            FROM session_facets
            WHERE session_id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row)
    }

    /// Get all session IDs that have facets stored.
    pub async fn get_all_facet_session_ids(&self) -> sqlx::Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT session_id FROM session_facets")
                .fetch_all(self.pool())
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Get session IDs that do NOT have facets yet.
    /// Returns sessions ordered by last_message_at DESC (most recent first).
    pub async fn get_session_ids_without_facets(&self) -> sqlx::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT s.id
            FROM sessions s
            LEFT JOIN session_facets f ON s.id = f.session_id
            WHERE f.session_id IS NULL
            ORDER BY s.last_message_at DESC
            "#,
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Compute aggregate statistics across all facets.
    pub async fn get_facet_aggregate_stats(&self) -> sqlx::Result<FacetAggregateStats> {
        let (total_with_facets,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_facets")
                .fetch_one(self.pool())
                .await?;

        let (total_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions")
                .fetch_one(self.pool())
                .await?;

        let total_without_facets = total_sessions - total_with_facets;

        // Achievement rate: % of facets with outcome = 'fully_achieved' or 'mostly_achieved'
        let (achieved_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_facets WHERE outcome IN ('fully_achieved', 'mostly_achieved')",
        )
        .fetch_one(self.pool())
        .await?;

        let achievement_rate = if total_with_facets > 0 {
            (achieved_count as f64 / total_with_facets as f64) * 100.0
        } else {
            0.0
        };

        // Frustrated count
        let (frustrated_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_facets WHERE satisfaction = 'frustrated'",
        )
        .fetch_one(self.pool())
        .await?;

        // Satisfied or above: satisfied, very_satisfied, delighted
        let (satisfied_or_above_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_facets WHERE satisfaction IN ('satisfied', 'very_satisfied', 'delighted')",
        )
        .fetch_one(self.pool())
        .await?;

        // Friction: sessions where friction_counts is not empty/default
        let (friction_session_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_facets WHERE friction_counts != '{}' AND friction_counts != ''",
        )
        .fetch_one(self.pool())
        .await?;

        Ok(FacetAggregateStats {
            total_with_facets,
            total_without_facets,
            achievement_rate,
            frustrated_count,
            satisfied_or_above_count,
            friction_session_count,
        })
    }

    /// Get quality badges (outcome, satisfaction) for a batch of session IDs.
    /// Returns tuples of (session_id, outcome, satisfaction).
    /// Chunks the query to stay under the SQLite 999 variable limit.
    pub async fn get_session_quality_badges(
        &self,
        session_ids: &[String],
    ) -> sqlx::Result<Vec<(String, Option<String>, Option<String>)>> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for chunk in session_ids.chunks(SQLITE_VARIABLE_LIMIT) {
            let placeholders: Vec<String> =
                (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(", ");

            let sql = format!(
                "SELECT session_id, outcome, satisfaction FROM session_facets WHERE session_id IN ({})",
                in_clause
            );

            let mut query = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(&sql);
            for id in chunk {
                query = query.bind(id);
            }

            let rows = query.fetch_all(self.pool()).await?;
            results.extend(rows);
        }

        Ok(results)
    }

    /// Check the last 7 sessions for a negative satisfaction pattern.
    /// If 3+ of the last 7 are "frustrated" or "dissatisfied", returns
    /// (pattern_name, count, tip_text).
    pub async fn get_pattern_alert(
        &self,
    ) -> sqlx::Result<Option<(String, i64, String)>> {
        // Get last 7 sessions ordered by ingested_at DESC
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            r#"
            SELECT f.satisfaction
            FROM session_facets f
            JOIN sessions s ON s.id = f.session_id
            ORDER BY s.last_message_at DESC
            LIMIT 7
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        let mut frustrated_count: i64 = 0;
        let mut dissatisfied_count: i64 = 0;

        for (satisfaction,) in &rows {
            match satisfaction.as_deref() {
                Some("frustrated") => frustrated_count += 1,
                Some("dissatisfied") => dissatisfied_count += 1,
                _ => {}
            }
        }

        if frustrated_count >= 3 {
            return Ok(Some((
                "frustrated".to_string(),
                frustrated_count,
                "Try starting with a 1-sentence summary of what you want before diving into details.".to_string(),
            )));
        }

        if dissatisfied_count >= 3 {
            return Ok(Some((
                "dissatisfied".to_string(),
                dissatisfied_count,
                "Consider breaking complex tasks into smaller sessions.".to_string(),
            )));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB should open")
    }

    async fn insert_test_session(db: &Database, id: &str) {
        sqlx::query(
            r#"INSERT INTO sessions (
                id, project_id, file_path, preview, project_path,
                duration_seconds, files_edited_count, reedited_files_count,
                files_read_count, user_prompt_count, api_call_count,
                tool_call_count, commit_count, turn_count,
                last_message_at, size_bytes, last_message,
                files_touched, skills_used, files_read, files_edited
            )
            VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                1800, 5, 1, 5, 5, 10, 20, 1, 10,
                strftime('%s', 'now'), 1024, '', '[]', '[]', '[]', '[]')"#,
        )
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    }

    fn make_facet(session_id: &str) -> FacetRow {
        FacetRow {
            session_id: session_id.to_string(),
            source: "insights_cache".to_string(),
            underlying_goal: Some("Build a feature".to_string()),
            goal_categories: r#"{"coding":1}"#.to_string(),
            outcome: Some("fully_achieved".to_string()),
            satisfaction: Some("satisfied".to_string()),
            user_satisfaction_counts: r#"{"satisfied":3}"#.to_string(),
            claude_helpfulness: Some("very_helpful".to_string()),
            session_type: Some("single_task".to_string()),
            friction_counts: "{}".to_string(),
            friction_detail: None,
            primary_success: Some("Code works".to_string()),
            brief_summary: Some("Built a feature successfully".to_string()),
        }
    }

    #[tokio::test]
    async fn test_batch_upsert_facets() {
        let db = setup_db().await;
        let facet = make_facet("sess-1");
        let count = db.batch_upsert_facets(&[facet]).await.unwrap();
        assert_eq!(count, 1);

        // Verify it's actually in the DB
        let (row_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_facets")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row_count, 1);
    }

    #[tokio::test]
    async fn test_upsert_is_idempotent() {
        let db = setup_db().await;
        let facet = make_facet("sess-1");

        // Insert once
        db.batch_upsert_facets(&[facet.clone()]).await.unwrap();

        // Insert again — should not error (INSERT OR REPLACE)
        let mut facet2 = facet;
        facet2.brief_summary = Some("Updated summary".to_string());
        db.batch_upsert_facets(&[facet2]).await.unwrap();

        // Still just 1 row
        let (row_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_facets")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row_count, 1);

        // Verify updated value
        let row = db.get_session_facet("sess-1").await.unwrap().unwrap();
        assert_eq!(row.brief_summary.as_deref(), Some("Updated summary"));
    }

    #[tokio::test]
    async fn test_get_facet_for_session() {
        let db = setup_db().await;
        let facet = make_facet("sess-abc");
        db.batch_upsert_facets(&[facet]).await.unwrap();

        let row = db.get_session_facet("sess-abc").await.unwrap().unwrap();
        assert_eq!(row.session_id, "sess-abc");
        assert_eq!(row.source, "insights_cache");
        assert_eq!(row.underlying_goal.as_deref(), Some("Build a feature"));
        assert_eq!(row.outcome.as_deref(), Some("fully_achieved"));
        assert_eq!(row.satisfaction.as_deref(), Some("satisfied"));
        assert_eq!(row.claude_helpfulness.as_deref(), Some("very_helpful"));
        assert_eq!(row.session_type.as_deref(), Some("single_task"));
        assert_eq!(row.friction_counts, "{}");
        assert!(row.friction_detail.is_none());
        assert_eq!(row.primary_success.as_deref(), Some("Code works"));
        assert_eq!(
            row.brief_summary.as_deref(),
            Some("Built a feature successfully")
        );

        // Non-existent session returns None
        let missing = db.get_session_facet("no-such-id").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_get_sessions_without_facets() {
        let db = setup_db().await;

        // Create 2 sessions in the sessions table
        insert_test_session(&db, "sess-with").await;
        insert_test_session(&db, "sess-without").await;

        // Add facet for only one session
        let facet = make_facet("sess-with");
        db.batch_upsert_facets(&[facet]).await.unwrap();

        let missing = db.get_session_ids_without_facets().await.unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "sess-without");
    }

    #[tokio::test]
    async fn test_facet_aggregate_stats() {
        let db = setup_db().await;

        // Create sessions in the sessions table first
        insert_test_session(&db, "s1").await;
        insert_test_session(&db, "s2").await;
        insert_test_session(&db, "s3").await;

        // 3 sessions with facets: 2 achieved, 1 frustrated
        let mut f1 = make_facet("s1");
        f1.outcome = Some("fully_achieved".to_string());
        f1.satisfaction = Some("satisfied".to_string());

        let mut f2 = make_facet("s2");
        f2.outcome = Some("mostly_achieved".to_string());
        f2.satisfaction = Some("satisfied".to_string());

        let mut f3 = make_facet("s3");
        f3.outcome = Some("not_achieved".to_string());
        f3.satisfaction = Some("frustrated".to_string());
        f3.friction_counts = r#"{"context_loss":2}"#.to_string();

        db.batch_upsert_facets(&[f1, f2, f3]).await.unwrap();

        // Also insert 1 session without a facet (for total_without_facets)
        insert_test_session(&db, "s4-no-facet").await;

        let stats = db.get_facet_aggregate_stats().await.unwrap();
        assert_eq!(stats.total_with_facets, 3);
        assert_eq!(stats.total_without_facets, 1);
        // 2 out of 3 achieved => 66.666...%
        assert!((stats.achievement_rate - 66.666_666_666_666_66).abs() < 1e-10);
        assert_eq!(stats.frustrated_count, 1);
        assert_eq!(stats.satisfied_or_above_count, 2);
        assert_eq!(stats.friction_session_count, 1);
    }

    #[tokio::test]
    async fn test_get_session_quality_badges_chunked() {
        let db = setup_db().await;

        // Insert 3 facets
        let f1 = make_facet("badge-1");
        let mut f2 = make_facet("badge-2");
        f2.outcome = Some("not_achieved".to_string());
        f2.satisfaction = Some("frustrated".to_string());
        let f3 = make_facet("badge-3");

        db.batch_upsert_facets(&[f1, f2, f3]).await.unwrap();

        // Query for 2 of the 3
        let ids = vec!["badge-1".to_string(), "badge-3".to_string()];
        let badges = db.get_session_quality_badges(&ids).await.unwrap();
        assert_eq!(badges.len(), 2);

        // All returned should be in our requested set
        for (sid, _outcome, _satisfaction) in &badges {
            assert!(ids.contains(sid));
        }

        // Empty input returns empty output
        let empty = db.get_session_quality_badges(&[]).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_pattern_alert_frustrated() {
        let db = setup_db().await;

        // Create 7 sessions with staggered timestamps so ordering works
        for i in 1..=7 {
            let id = format!("pa-{}", i);
            sqlx::query(
                r#"INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    1800, 5, 1, 5, 5, 10, 20, 1, 10,
                    ?2, 1024, '', '[]', '[]', '[]', '[]')"#,
            )
            .bind(&id)
            .bind(1000 + i as i64) // stagger timestamps
            .execute(db.pool())
            .await
            .unwrap();
        }

        // 4 of 7 are frustrated
        let facets: Vec<FacetRow> = (1..=7)
            .map(|i| {
                let mut f = make_facet(&format!("pa-{}", i));
                if i >= 4 {
                    f.satisfaction = Some("frustrated".to_string());
                } else {
                    f.satisfaction = Some("satisfied".to_string());
                }
                f
            })
            .collect();
        db.batch_upsert_facets(&facets).await.unwrap();

        let alert = db.get_pattern_alert().await.unwrap();
        assert!(alert.is_some());
        let (pattern, count, tip) = alert.unwrap();
        assert_eq!(pattern, "frustrated");
        assert!(count >= 3);
        assert!(tip.contains("1-sentence summary"));
    }

    #[tokio::test]
    async fn test_pattern_alert_none_when_happy() {
        let db = setup_db().await;

        // Create 7 happy sessions
        for i in 1..=7 {
            let id = format!("happy-{}", i);
            insert_test_session(&db, &id).await;
        }

        let facets: Vec<FacetRow> = (1..=7)
            .map(|i| make_facet(&format!("happy-{}", i)))
            .collect();
        db.batch_upsert_facets(&facets).await.unwrap();

        let alert = db.get_pattern_alert().await.unwrap();
        assert!(alert.is_none());
    }

    #[tokio::test]
    async fn test_get_all_facet_session_ids() {
        let db = setup_db().await;

        let f1 = make_facet("ids-1");
        let f2 = make_facet("ids-2");
        db.batch_upsert_facets(&[f1, f2]).await.unwrap();

        let ids = db.get_all_facet_session_ids().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"ids-1".to_string()));
        assert!(ids.contains(&"ids-2".to_string()));
    }
}
