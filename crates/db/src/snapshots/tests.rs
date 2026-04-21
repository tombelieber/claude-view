// crates/db/src/snapshots/tests.rs
//! Tests for contribution snapshot queries and aggregation.

#[cfg(test)]
mod tests {
    use crate::snapshots::helpers::{usd_opt_to_cents, usd_to_cents};
    use crate::snapshots::types::*;
    use crate::Database;
    use chrono::Local;

    #[test]
    fn test_time_range_from_str() {
        assert_eq!(TimeRange::parse_str("today"), Some(TimeRange::Today));
        assert_eq!(TimeRange::parse_str("week"), Some(TimeRange::Week));
        assert_eq!(TimeRange::parse_str("month"), Some(TimeRange::Month));
        assert_eq!(TimeRange::parse_str("90days"), Some(TimeRange::NinetyDays));
        assert_eq!(TimeRange::parse_str("all"), Some(TimeRange::All));
        assert_eq!(TimeRange::parse_str("custom"), Some(TimeRange::Custom));
        assert_eq!(TimeRange::parse_str("invalid"), None);
    }

    #[test]
    fn test_time_range_days_back() {
        assert_eq!(TimeRange::Today.days_back(), Some(0));
        assert_eq!(TimeRange::Week.days_back(), Some(7));
        assert_eq!(TimeRange::Month.days_back(), Some(30));
        assert_eq!(TimeRange::NinetyDays.days_back(), Some(90));
        assert_eq!(TimeRange::All.days_back(), None);
        assert_eq!(TimeRange::Custom.days_back(), None);
    }

    #[test]
    fn test_time_range_cache_seconds() {
        assert_eq!(TimeRange::Today.cache_seconds(), 60);
        assert_eq!(TimeRange::Week.cache_seconds(), 300);
        assert_eq!(TimeRange::Month.cache_seconds(), 900);
        assert_eq!(TimeRange::NinetyDays.cache_seconds(), 1800);
        assert_eq!(TimeRange::All.cache_seconds(), 1800);
    }

    #[test]
    fn test_usd_to_cents() {
        assert_eq!(usd_to_cents(2.50), 250);
        assert_eq!(usd_to_cents(0.0), 0);
        assert_eq!(usd_to_cents(0.026), 3); // rounded
    }

    #[test]
    fn test_usd_opt_to_cents() {
        assert_eq!(usd_opt_to_cents(Some(2.50), 1), 250);
        assert_eq!(usd_opt_to_cents(Some(0.0), 1), 0);
        assert_eq!(usd_opt_to_cents(None, 0), 0);
        assert_eq!(usd_opt_to_cents(None, 3), 0);
    }

    #[tokio::test]
    async fn test_aggregated_contributions_default() {
        let agg = AggregatedContributions::default();
        assert_eq!(agg.sessions_count, 0);
        assert_eq!(agg.ai_lines_added, 0);
        assert_eq!(agg.commits_count, 0);
    }

    #[tokio::test]
    async fn test_get_aggregated_contributions_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let agg = db
            .get_aggregated_contributions(TimeRange::Week, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(agg.sessions_count, 0);
        assert_eq!(agg.ai_lines_added, 0);
    }

    #[tokio::test]
    async fn test_upsert_snapshot() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a snapshot
        db.upsert_snapshot(
            "2026-02-05",
            None,
            None,
            10,
            500,
            100,
            5,
            450,
            80,
            100000,
            25,
            12,
        )
        .await
        .unwrap();

        // Query it back
        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT sessions_count, ai_lines_added, commits_count FROM contribution_snapshots WHERE date = '2026-02-05'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(row.0, 10);
        assert_eq!(row.1, 500);
        assert_eq!(row.2, 5);

        // Upsert with different values
        db.upsert_snapshot(
            "2026-02-05",
            None,
            None,
            15,
            600,
            150,
            7,
            500,
            100,
            150000,
            38,
            18,
        )
        .await
        .unwrap();

        // Should be updated, not duplicated
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM contribution_snapshots WHERE date = '2026-02-05'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 1);

        let row: (i64, i64) = sqlx::query_as(
            "SELECT sessions_count, ai_lines_added FROM contribution_snapshots WHERE date = '2026-02-05'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, 15);
        assert_eq!(row.1, 600);
    }

    #[tokio::test]
    async fn test_get_contribution_trend_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let trend = db
            .get_contribution_trend(TimeRange::Week, None, None, None, None)
            .await
            .unwrap();

        assert!(trend.is_empty());
    }

    #[tokio::test]
    async fn test_get_contribution_trend_with_data() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert some snapshots
        db.upsert_snapshot(
            "2026-02-03",
            None,
            None,
            5,
            200,
            50,
            2,
            180,
            40,
            50000,
            13,
            5,
        )
        .await
        .unwrap();
        db.upsert_snapshot(
            "2026-02-04",
            None,
            None,
            8,
            350,
            80,
            4,
            300,
            60,
            80000,
            20,
            10,
        )
        .await
        .unwrap();
        db.upsert_snapshot(
            "2026-02-05",
            None,
            None,
            10,
            500,
            100,
            5,
            450,
            80,
            100000,
            25,
            15,
        )
        .await
        .unwrap();

        let trend = db
            .get_contribution_trend(
                TimeRange::Custom,
                Some("2026-02-01"),
                Some("2026-02-10"),
                None,
                None,
            )
            .await
            .unwrap();

        // Gap-filled: Feb 1-10 = 10 days, with zero-value entries for days without data
        assert_eq!(trend.len(), 10);
        assert_eq!(trend[0].date, "2026-02-01");
        assert_eq!(trend[0].sessions, 0); // no data, gap-filled
        assert_eq!(trend[2].date, "2026-02-03");
        assert_eq!(trend[2].lines_added, 200); // real data
        assert_eq!(trend[4].date, "2026-02-05");
        assert_eq!(trend[4].sessions, 10); // real data
        assert_eq!(trend[9].date, "2026-02-10");
        assert_eq!(trend[9].sessions, 0); // no data, gap-filled
    }

    #[tokio::test]
    async fn test_get_session_contribution_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.get_session_contribution("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_session_contribution_excludes_sidechain() {
        let db = Database::new_in_memory().await.unwrap();

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, file_path, preview, is_sidechain,
                work_type, duration_seconds, user_prompt_count,
                ai_lines_added, ai_lines_removed, files_edited_count,
                reedited_files_count, commit_count
            )
            VALUES
                ('primary-sess', 'proj', '/tmp/primary.jsonl', 'Primary', 0, 'code', 120, 3, 20, 5, 2, 1, 1),
                ('sidechain-sess', 'proj', '/tmp/side.jsonl', 'Sidechain', 1, 'code', 120, 3, 20, 5, 2, 1, 1)
            "#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let primary = db.get_session_contribution("primary-sess").await.unwrap();
        assert!(primary.is_some(), "primary session should be returned");

        let sidechain = db.get_session_contribution("sidechain-sess").await.unwrap();
        assert!(
            sidechain.is_none(),
            "sidechain session must be excluded from contribution detail"
        );
    }

    #[tokio::test]
    async fn test_get_session_commits_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let commits = db.get_session_commits("nonexistent").await.unwrap();
        assert!(commits.is_empty());
    }

    // ========================================================================
    // New functionality tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_model_breakdown_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let breakdown = db
            .get_model_breakdown(TimeRange::Week, None, None, None, None)
            .await
            .unwrap();
        assert!(breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_get_learning_curve_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let curve = db.get_learning_curve(None, None).await.unwrap();
        assert!(curve.periods.is_empty());
        assert_eq!(curve.current_avg, 0.0);
        assert_eq!(curve.improvement, 0.0);
    }

    #[tokio::test]
    async fn test_get_skill_breakdown_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let breakdown = db
            .get_skill_breakdown(TimeRange::Week, None, None, None, None)
            .await
            .unwrap();
        assert!(breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_get_uncommitted_work_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let uncommitted = db.get_uncommitted_work().await.unwrap();
        assert!(uncommitted.is_empty());
    }

    #[tokio::test]
    async fn test_get_session_file_impacts_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let impacts = db.get_session_file_impacts("nonexistent").await.unwrap();
        assert!(impacts.is_empty());
    }

    #[tokio::test]
    async fn test_get_session_file_impacts_with_data() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert a session with files_edited
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview, files_edited)
            VALUES ('test-sess', 'proj', '/tmp/t.jsonl', 'Preview', '["src/main.rs", "src/lib.rs"]')
            "#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let impacts = db.get_session_file_impacts("test-sess").await.unwrap();
        assert_eq!(impacts.len(), 2);
        assert_eq!(impacts[0].path, "src/main.rs");
        assert_eq!(impacts[1].path, "src/lib.rs");
        assert_eq!(impacts[0].action, "modified");
    }

    #[tokio::test]
    async fn test_get_session_file_impacts_excludes_sidechain() {
        let db = Database::new_in_memory().await.unwrap();

        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview, files_edited, is_sidechain)
            VALUES
                ('primary-files', 'proj', '/tmp/primary-files.jsonl', 'Primary', '["src/main.rs"]', 0),
                ('sidechain-files', 'proj', '/tmp/sidechain-files.jsonl', 'Side', '["src/secret.rs"]', 1)
            "#,
        )
        .execute(db.pool())
        .await
        .unwrap();

        let primary_impacts = db.get_session_file_impacts("primary-files").await.unwrap();
        assert_eq!(primary_impacts.len(), 1);
        assert_eq!(primary_impacts[0].path, "src/main.rs");

        let sidechain_impacts = db
            .get_session_file_impacts("sidechain-files")
            .await
            .unwrap();
        assert!(
            sidechain_impacts.is_empty(),
            "sidechain session file impacts must not leak into detail response"
        );
    }

    #[tokio::test]
    async fn test_get_skill_breakdown_with_data() {
        let db = Database::new_in_memory().await.unwrap();
        let now = Local::now().timestamp();

        // Insert sessions
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview, ai_lines_added, ai_lines_removed, commit_count, files_edited_count, reedited_files_count, last_message_at)
            VALUES
                ('sess1', 'proj', '/tmp/1.jsonl', 'Preview', 200, 50, 1, 5, 1, ?1),
                ('sess2', 'proj', '/tmp/2.jsonl', 'Preview', 150, 30, 1, 3, 0, ?1),
                ('sess3', 'proj', '/tmp/3.jsonl', 'Preview', 100, 20, 0, 4, 2, ?1)
            "#,
        )
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        // Skill invocations now live in session_stats.invocation_counts.
        // sess1 uses tdd + commit, sess2 uses tdd, sess3 uses nothing.
        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   parser_version, stats_version, indexed_at,
                   last_message_at, invocation_counts
               ) VALUES
                   ('sess1', X'01', 0, 1, 1, 0, ?1, '{"Skill:tdd":1,"Skill:commit":1}'),
                   ('sess2', X'02', 0, 1, 1, 0, ?1, '{"Skill:tdd":1}'),
                   ('sess3', X'03', 0, 1, 1, 0, ?1, '{}')"#,
        )
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        let breakdown = db
            .get_skill_breakdown(TimeRange::All, None, None, None, None)
            .await
            .unwrap();

        // Should have 3 entries: tdd, commit, and (no skill)
        assert_eq!(breakdown.len(), 3);

        // Find the tdd skill
        let tdd = breakdown.iter().find(|s| s.skill == "tdd").unwrap();
        assert_eq!(tdd.sessions, 2);
        assert_eq!(tdd.commit_rate, 1.0); // Both sessions have commits

        // Find the no skill entry
        let no_skill = breakdown.iter().find(|s| s.skill == "(no skill)").unwrap();
        assert_eq!(no_skill.sessions, 1);
        assert_eq!(no_skill.commit_rate, 0.0); // No commit
    }

    #[tokio::test]
    async fn test_model_stats_serialization() {
        let stats = ModelStats {
            model: "claude-sonnet".to_string(),
            sessions: 10,
            lines: 500,
            input_tokens: 100_000,
            output_tokens: 50_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            reedit_rate: Some(0.15),
            cost_per_line: Some(0.003),
            insight: "Low re-edit rate".to_string(),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet\""));
        assert!(json.contains("\"reeditRate\":0.15"));
        assert!(json.contains("\"costPerLine\":0.003"));
    }

    #[tokio::test]
    async fn test_learning_curve_serialization() {
        let curve = LearningCurve {
            periods: vec![
                LearningCurvePeriod {
                    period: "2026-01".to_string(),
                    reedit_rate: 0.3,
                },
                LearningCurvePeriod {
                    period: "2026-02".to_string(),
                    reedit_rate: 0.2,
                },
            ],
            current_avg: 0.2,
            improvement: 33.3,
            insight: "Improving".to_string(),
        };

        let json = serde_json::to_string(&curve).unwrap();
        assert!(json.contains("\"currentAvg\":0.2"));
        assert!(json.contains("\"improvement\":33.3"));
        assert!(json.contains("\"reeditRate\":0.3"));
    }

    #[tokio::test]
    async fn test_skill_stats_serialization() {
        let stats = SkillStats {
            skill: "tdd".to_string(),
            sessions: 10,
            avg_loc: 200,
            commit_rate: 0.9,
            reedit_rate: 0.12,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"skill\":\"tdd\""));
        assert!(json.contains("\"avgLoc\":200"));
        assert!(json.contains("\"commitRate\":0.9"));
        assert!(json.contains("\"reeditRate\":0.12"));
    }

    #[tokio::test]
    async fn test_uncommitted_work_serialization() {
        let work = UncommittedWork {
            project_id: "proj1".to_string(),
            project_name: "My Project".to_string(),
            branch: Some("feature/test".to_string()),
            lines_added: 500,
            files_count: 5,
            last_session_id: "sess123".to_string(),
            last_session_preview: "Add feature".to_string(),
            last_activity_at: 1700000000,
            insight: "Recent work".to_string(),
        };

        let json = serde_json::to_string(&work).unwrap();
        assert!(json.contains("\"projectId\":\"proj1\""));
        assert!(json.contains("\"projectName\":\"My Project\""));
        assert!(json.contains("\"linesAdded\":500"));
        assert!(json.contains("\"lastSessionId\":\"sess123\""));
    }

    #[tokio::test]
    async fn test_file_impact_serialization() {
        let impact = FileImpact {
            path: "src/main.rs".to_string(),
            lines_added: 50,
            lines_removed: 10,
            action: "modified".to_string(),
        };

        let json = serde_json::to_string(&impact).unwrap();
        assert!(json.contains("\"path\":\"src/main.rs\""));
        assert!(json.contains("\"linesAdded\":50"));
        assert!(json.contains("\"linesRemoved\":10"));
        assert!(json.contains("\"action\":\"modified\""));
    }

    // ========================================================================
    // Weekly Rollup Tests
    // ========================================================================

    #[tokio::test]
    async fn test_rollup_weekly_snapshots_empty_db() {
        let db = Database::new_in_memory().await.unwrap();

        // Should return 0 when no snapshots exist
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_rollup_weekly_snapshots_creates_weekly() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert daily snapshots from 60 days ago (should be rolled up with retention=30)
        // Week of 2025-12-02 (Mon) to 2025-12-08 (Sun)
        db.upsert_snapshot("2025-12-02", None, None, 5, 100, 20, 2, 90, 15, 10000, 3, 4)
            .await
            .unwrap();
        db.upsert_snapshot(
            "2025-12-03",
            None,
            None,
            8,
            200,
            40,
            3,
            180,
            30,
            20000,
            5,
            7,
        )
        .await
        .unwrap();
        db.upsert_snapshot(
            "2025-12-04",
            None,
            None,
            6,
            150,
            30,
            2,
            130,
            25,
            15000,
            4,
            5,
        )
        .await
        .unwrap();

        // Perform rollup with 30 day retention
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 1); // One week rolled up

        // Check weekly snapshot was created
        let weekly: Option<(String, i64, i64)> = sqlx::query_as(
            "SELECT date, sessions_count, ai_lines_added FROM contribution_snapshots WHERE date LIKE 'W:%'",
        )
        .fetch_optional(db.pool())
        .await
        .unwrap();

        assert!(weekly.is_some());
        let (date, sessions, lines_added) = weekly.unwrap();
        assert!(date.starts_with("W:"));
        assert_eq!(sessions, 19); // 5 + 8 + 6
        assert_eq!(lines_added, 450); // 100 + 200 + 150

        // Daily snapshots should be deleted
        let daily_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contribution_snapshots WHERE length(date) = 10 AND project_id IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(daily_count.0, 0);
    }

    #[tokio::test]
    async fn test_rollup_preserves_recent_daily() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert recent snapshot (should NOT be rolled up)
        let today = Local::now().format("%Y-%m-%d").to_string();
        db.upsert_snapshot(&today, None, None, 10, 500, 100, 5, 450, 80, 50000, 13, 8)
            .await
            .unwrap();

        // Perform rollup
        let count = db.rollup_weekly_snapshots(30).await.unwrap();
        assert_eq!(count, 0); // Nothing rolled up

        // Recent snapshot should still exist
        let daily_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM contribution_snapshots WHERE date = ?1")
                .bind(&today)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(daily_count.0, 1);
    }

    #[tokio::test]
    async fn test_get_snapshot_stats() {
        let db = Database::new_in_memory().await.unwrap();

        // Insert some daily snapshots
        db.upsert_snapshot("2026-01-15", None, None, 5, 100, 20, 2, 90, 15, 10000, 3, 3)
            .await
            .unwrap();
        db.upsert_snapshot(
            "2026-01-16",
            None,
            None,
            8,
            200,
            40,
            3,
            180,
            30,
            20000,
            5,
            6,
        )
        .await
        .unwrap();

        // Insert a weekly snapshot manually
        sqlx::query(
            "INSERT INTO contribution_snapshots (date, sessions_count, ai_lines_added, ai_lines_removed, commits_count, commit_insertions, commit_deletions, tokens_used, cost_cents) VALUES ('W:2025-12-02', 50, 1000, 200, 10, 900, 150, 100000, 25)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let stats = db.get_snapshot_stats().await.unwrap();

        assert_eq!(stats.daily_count, 2);
        assert_eq!(stats.weekly_count, 1);
        assert_eq!(stats.oldest_daily, Some("2026-01-15".to_string()));
        assert_eq!(stats.oldest_weekly, Some("2025-12-02".to_string()));
    }

    #[tokio::test]
    async fn test_snapshot_stats_serialization() {
        let stats = SnapshotStats {
            daily_count: 30,
            weekly_count: 12,
            oldest_daily: Some("2026-01-15".to_string()),
            oldest_weekly: Some("2025-10-07".to_string()),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"dailyCount\":30"));
        assert!(json.contains("\"weeklyCount\":12"));
        assert!(json.contains("\"oldestDaily\":\"2026-01-15\""));
        assert!(json.contains("\"oldestWeekly\":\"2025-10-07\""));
    }
}
