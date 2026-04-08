//! Tests for the insights API endpoints.

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn insert_session(db: &Database, id: &str, ts: i64, category_l1: Option<&str>) {
        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, file_path, preview, project_path,
                duration_seconds, files_edited_count, reedited_files_count,
                files_read_count, user_prompt_count, api_call_count,
                tool_call_count, commit_count, turn_count,
                last_message_at, size_bytes, last_message,
                files_touched, skills_used, files_read, files_edited,
                category_l1, category_l2, category_l3
            )
            VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                1800, 5, 1, 5, 10, 10, 20, 1, 10,
                ?2, 1024, '', '[]', '[]', '[]', '[]',
                ?3, 'feature', 'new-component')
            "#,
        )
        .bind(id)
        .bind(ts)
        .bind(category_l1)
        .execute(db.pool())
        .await
        .unwrap();
    }

    async fn mark_session_sidechain(db: &Database, id: &str) {
        sqlx::query("UPDATE sessions SET is_sidechain = 1 WHERE id = ?1")
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_insights_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have all expected top-level fields
        assert!(json.get("topInsight").is_some());
        assert!(json.get("overview").is_some());
        assert!(json.get("patterns").is_some());
        assert!(json.get("classificationStatus").is_some());
        assert!(json.get("meta").is_some());

        // top_insight should be null with no data
        assert!(json["topInsight"].is_null());

        // overview should have zero values
        assert_eq!(json["overview"]["workBreakdown"]["totalSessions"], 0);

        // patterns should be empty arrays
        assert_eq!(json["patterns"]["high"].as_array().unwrap().len(), 0);
        assert_eq!(json["patterns"]["medium"].as_array().unwrap().len(), 0);
        assert_eq!(
            json["patterns"]["observations"].as_array().unwrap().len(),
            0
        );

        // classification status
        assert_eq!(json["classificationStatus"]["total"], 0);
        assert_eq!(json["classificationStatus"]["classified"], 0);

        // meta
        assert!(json["meta"]["computedAt"].is_number());
        assert!(json["meta"]["patternsEvaluated"].is_number());
    }

    #[tokio::test]
    async fn test_insights_includes_data_scope_meta() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
    }

    #[tokio::test]
    async fn test_insights_includes_session_breakdown_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        insert_session(&db, "ins-meta-primary", now - 120, Some("code_work")).await;
        insert_session(&db, "ins-meta-sidechain", now - 60, Some("code_work")).await;
        mark_session_sidechain(&db, "ins-meta-sidechain").await;

        let app = build_app(db);
        let from = now - 3600;
        let to = now;
        let (status, body) = do_get(app, &format!("/api/insights?from={}&to={}", from, to)).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    #[tokio::test]
    async fn test_insights_with_custom_params() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(
            app,
            "/api/insights?from=1700000000&to=1700100000&min_impact=0.5&limit=10",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["timeRangeStart"], 1700000000);
        assert_eq!(json["meta"]["timeRangeEnd"], 1700100000);
    }

    #[tokio::test]
    async fn test_insights_default_all_time_includes_old_data() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        let old_ts = now - (120 * 86400);

        insert_session(&db, "ins-old", old_ts, None).await;
        insert_session(&db, "ins-new", now - 3600, None).await;

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["overview"]["workBreakdown"]["totalSessions"], 2);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "default_all_time");
        assert!(json["meta"]["timeRangeStart"].as_i64().unwrap() <= old_ts);
    }

    #[tokio::test]
    async fn test_insights_one_sided_range_rejected_in_strict_mode() {
        let db = test_db().await;
        let app = build_app(db);

        let (from_status, from_body) = do_get(app.clone(), "/api/insights?from=1700000000").await;
        assert_eq!(from_status, StatusCode::BAD_REQUEST);
        let from_json: serde_json::Value = serde_json::from_str(&from_body).unwrap();
        assert!(from_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));

        let (to_status, to_body) = do_get(app, "/api/insights?to=1700000000").await;
        assert_eq!(to_status, StatusCode::BAD_REQUEST);
        let to_json: serde_json::Value = serde_json::from_str(&to_body).unwrap();
        assert!(to_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));
    }

    #[tokio::test]
    async fn test_insights_inverted_range_rejected() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights?from=1700100000&to=1700000000").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("'from' must be <= 'to'"));
    }

    #[tokio::test]
    async fn test_insights_equality_range_valid() {
        let db = test_db().await;
        let ts = chrono::Utc::now().timestamp();
        insert_session(&db, "ins-eq", ts, None).await;

        let app = build_app(db);
        let (status, body) = do_get(app, &format!("/api/insights?from={}&to={}", ts, ts)).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["timeRangeStart"], ts);
        assert_eq!(json["meta"]["timeRangeEnd"], ts);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "explicit_from_to");
    }

    #[tokio::test]
    async fn test_insights_with_seeded_data() {
        let db = test_db().await;

        // Insert test sessions
        let now = chrono::Utc::now().timestamp();
        for i in 0..100 {
            let id = format!("test-{}", i);
            let duration = match i % 4 {
                0 => 600,
                1 => 1800,
                2 => 3600,
                _ => 7200,
            };
            let files_edited = if duration == 1800 { 10 } else { 3 };
            let reedited = if duration == 1800 { 1 } else { 2 };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    ?2, ?3, ?4, 5, 5, 10, 20, ?5, 10,
                    ?6, 1024, '', '[]', '[]', '[]', '[]')
                "#,
            )
            .bind(&id)
            .bind(duration)
            .bind(files_edited)
            .bind(reedited)
            .bind(if i % 3 == 0 { 1 } else { 0 })
            .bind(now - (i as i64 * 3600))
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have some sessions in overview
        assert!(
            json["overview"]["workBreakdown"]["totalSessions"]
                .as_u64()
                .unwrap()
                > 0,
            "Should have sessions: {}",
            body
        );

        // Meta should report patterns evaluated
        assert!(json["meta"]["patternsEvaluated"].is_number());
    }

    #[tokio::test]
    async fn test_insights_response_structure() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Verify the full response structure matches the API spec
        assert!(json["overview"]["workBreakdown"]["totalSessions"].is_number());
        assert!(json["overview"]["workBreakdown"]["withCommits"].is_number());
        assert!(json["overview"]["workBreakdown"]["exploration"].is_number());
        assert!(json["overview"]["workBreakdown"]["avgSessionMinutes"].is_number());

        assert!(json["overview"]["efficiency"]["avgReeditRate"].is_number());
        assert!(json["overview"]["efficiency"]["avgEditVelocity"].is_number());
        assert!(json["overview"]["efficiency"]["trend"].is_string());
        assert!(json["overview"]["efficiency"]["trendPct"].is_number());

        assert!(json["overview"]["bestTime"]["dayOfWeek"].is_string());
        assert!(json["overview"]["bestTime"]["timeSlot"].is_string());
        assert!(json["overview"]["bestTime"]["improvementPct"].is_number());

        assert!(json["classificationStatus"]["classified"].is_number());
        assert!(json["classificationStatus"]["total"].is_number());
        assert!(json["classificationStatus"]["pendingClassification"].is_number());
        assert!(json["classificationStatus"]["classificationPct"].is_number());
        assert!(json["meta"]["effectiveRange"]["from"].is_number());
        assert!(json["meta"]["effectiveRange"]["to"].is_number());
        assert!(json["meta"]["effectiveRange"]["source"].is_string());
    }

    // ========================================================================
    // GET /api/insights/categories tests (Phase 6)
    // ========================================================================

    #[tokio::test]
    async fn test_categories_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have all top-level fields
        assert!(json.get("breakdown").is_some());
        assert!(json.get("categories").is_some());
        assert!(json.get("overallAverages").is_some());

        // Breakdown should be zero
        assert_eq!(json["breakdown"]["codeWork"]["count"], 0);
        assert_eq!(json["breakdown"]["supportWork"]["count"], 0);
        assert_eq!(json["breakdown"]["thinkingWork"]["count"], 0);
        assert_eq!(json["breakdown"]["uncategorized"]["count"], 0);

        // Categories should be empty
        assert!(json["categories"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_categories_includes_data_scope_meta() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
    }

    #[tokio::test]
    async fn test_categories_with_data() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        // Insert sessions with categories
        for i in 0..20 {
            let id = format!("cat-{}", i);
            let (l1, l2, l3) = match i % 5 {
                0 => ("code_work", "feature", "new-component"),
                1 => ("code_work", "feature", "add-functionality"),
                2 => ("code_work", "bug_fix", "error-fix"),
                3 => ("support_work", "docs", "readme-guides"),
                _ => ("thinking_work", "planning", "brainstorming"),
            };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited,
                    category_l1, category_l2, category_l3
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    1800, 5, 1, 5, 10, 10, 20, ?2, 10,
                    ?3, 1024, '', '[]', '[]', '[]', '[]',
                    ?4, ?5, ?6)
                "#,
            )
            .bind(&id)
            .bind(if i % 2 == 0 { 1 } else { 0 })
            .bind(now - (i as i64 * 3600))
            .bind(l1)
            .bind(l2)
            .bind(l3)
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have categories
        let categories = json["categories"].as_array().unwrap();
        assert!(!categories.is_empty(), "Should have category nodes");

        // Code work should have the most sessions (12 out of 20)
        assert_eq!(json["breakdown"]["codeWork"]["count"], 12);
        assert_eq!(json["breakdown"]["supportWork"]["count"], 4);
        assert_eq!(json["breakdown"]["thinkingWork"]["count"], 4);
        assert_eq!(json["breakdown"]["uncategorized"]["count"], 0);

        // Overall averages should be present
        assert!(json["overallAverages"]["avgReeditRate"].is_number());
        assert!(json["overallAverages"]["avgDuration"].is_number());
        assert!(json["overallAverages"]["avgPrompts"].is_number());
        assert!(json["overallAverages"]["commitRate"].is_number());
        assert!(json["meta"]["effectiveRange"]["from"].is_number());
        assert!(json["meta"]["effectiveRange"]["to"].is_number());
        assert!(json["meta"]["effectiveRange"]["source"].is_string());
    }

    #[tokio::test]
    async fn test_categories_time_filter() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        // Insert sessions: some recent, some old
        for i in 0..10 {
            let id = format!("tf-{}", i);
            let ts = if i < 5 {
                now - 3600 // 1 hour ago (recent)
            } else {
                now - 30 * 86400 // 30 days ago (old)
            };

            sqlx::query(
                r#"
                INSERT INTO sessions (
                    id, project_id, file_path, preview, project_path,
                    duration_seconds, files_edited_count, reedited_files_count,
                    files_read_count, user_prompt_count, api_call_count,
                    tool_call_count, commit_count, turn_count,
                    last_message_at, size_bytes, last_message,
                    files_touched, skills_used, files_read, files_edited,
                    category_l1, category_l2, category_l3
                )
                VALUES (?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp',
                    1800, 5, 1, 5, 10, 10, 20, 1, 10,
                    ?2, 1024, '', '[]', '[]', '[]', '[]',
                    'code_work', 'feature', 'new-component')
                "#,
            )
            .bind(&id)
            .bind(ts)
            .execute(db.pool())
            .await
            .unwrap();
        }

        let app = build_app(db);
        // Filter to last 7 days
        let from = now - 7 * 86400;
        let (status, body) = do_get(
            app,
            &format!("/api/insights/categories?from={}&to={}", from, now),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // Only recent sessions should be counted
        assert_eq!(json["breakdown"]["codeWork"]["count"], 5);
    }

    #[tokio::test]
    async fn test_categories_default_all_time_includes_old_data() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        let old_ts = now - (180 * 86400);

        insert_session(&db, "cat-old", old_ts, Some("code_work")).await;
        insert_session(&db, "cat-new", now - 3600, Some("support_work")).await;

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["breakdown"]["codeWork"]["count"], 1);
        assert_eq!(json["breakdown"]["supportWork"]["count"], 1);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "default_all_time");
        assert!(json["meta"]["effectiveRange"]["from"].as_i64().unwrap() <= old_ts);
    }

    #[tokio::test]
    async fn test_categories_one_sided_range_rejected_in_strict_mode() {
        let db = test_db().await;
        let app = build_app(db);

        let (from_status, from_body) =
            do_get(app.clone(), "/api/insights/categories?from=1700000000").await;
        assert_eq!(from_status, StatusCode::BAD_REQUEST);
        let from_json: serde_json::Value = serde_json::from_str(&from_body).unwrap();
        assert!(from_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));

        let (to_status, to_body) = do_get(app, "/api/insights/categories?to=1700000000").await;
        assert_eq!(to_status, StatusCode::BAD_REQUEST);
        let to_json: serde_json::Value = serde_json::from_str(&to_body).unwrap();
        assert!(to_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));
    }

    #[tokio::test]
    async fn test_categories_invalid_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(
            app,
            "/api/insights/categories?from=1700100000&to=1700000000",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("'from' must be <= 'to'"));
    }

    #[tokio::test]
    async fn test_categories_equality_range_valid() {
        let db = test_db().await;
        let ts = chrono::Utc::now().timestamp();
        insert_session(&db, "cat-eq", ts, Some("code_work")).await;

        let app = build_app(db);
        let (status, body) = do_get(
            app,
            &format!("/api/insights/categories?from={}&to={}", ts, ts),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["breakdown"]["codeWork"]["count"], 1);
        assert_eq!(json["meta"]["effectiveRange"]["from"], ts);
        assert_eq!(json["meta"]["effectiveRange"]["to"], ts);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "explicit_from_to");
    }

    #[tokio::test]
    async fn test_categories_response_structure() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/categories").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Verify full response structure
        assert!(json["breakdown"]["codeWork"]["count"].is_number());
        assert!(json["breakdown"]["codeWork"]["percentage"].is_number());
        assert!(json["breakdown"]["supportWork"]["count"].is_number());
        assert!(json["breakdown"]["thinkingWork"]["count"].is_number());
        assert!(json["breakdown"]["uncategorized"]["count"].is_number());

        assert!(json["overallAverages"]["avgReeditRate"].is_number());
        assert!(json["overallAverages"]["avgDuration"].is_number());
        assert!(json["overallAverages"]["avgPrompts"].is_number());
        assert!(json["overallAverages"]["commitRate"].is_number());
    }

    #[tokio::test]
    async fn test_benchmarks_includes_data_scope_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        insert_session(&db, "bench-meta-primary", now - 3600, Some("code_work")).await;
        insert_session(&db, "bench-meta-sidechain", now - 1800, Some("code_work")).await;
        mark_session_sidechain(&db, "bench-meta-sidechain").await;

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/benchmarks?range=all").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    // ========================================================================
    // GET /api/insights/trends tests (Phase 7)
    // ========================================================================

    #[tokio::test]
    async fn test_trends_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/trends").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(json["metric"], "reedit_rate");
        assert!(json["dataPoints"].is_array());
        assert!(json["activityHeatmap"].is_array());
        assert!(json["average"].is_number());
        assert!(json["trend"].is_number());
        assert!(json["trendDirection"].is_string());
        assert!(json["insight"].is_string());
        assert!(json["heatmapInsight"].is_string());
        assert!(json["periodStart"].is_string());
        assert!(json["periodEnd"].is_string());
        assert!(json["totalSessions"].is_number());
        assert_eq!(json["meta"]["effectiveRange"]["source"], "default_all_time");
        assert_eq!(json["classificationRequired"], true);
        assert!(json["categoryEvolution"].is_null());
    }

    #[tokio::test]
    async fn test_insights_trends_includes_data_scope_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        insert_session(&db, "trend-meta-primary", now - 120, Some("code_work")).await;
        insert_session(&db, "trend-meta-sidechain", now - 60, Some("code_work")).await;
        mark_session_sidechain(&db, "trend-meta-sidechain").await;

        let app = build_app(db);
        let from = now - 3600;
        let to = now;
        let (status, body) = do_get(
            app,
            &format!(
                "/api/insights/trends?metric=sessions&from={}&to={}",
                from, to
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["dataScope"]["sessions"],
            "primary_sessions_only"
        );
        assert_eq!(
            json["meta"]["dataScope"]["workload"],
            "primary_plus_subagent_work"
        );
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    #[tokio::test]
    async fn test_trends_invalid_metric() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_get(app, "/api/insights/trends?metric=invalid").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_invalid_range() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_get(app, "/api/insights/trends?range=2yr").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_invalid_granularity() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) = do_get(app, "/api/insights/trends?granularity=quarter").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_from_greater_than_to() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, _body) =
            do_get(app, "/api/insights/trends?from=1700100000&to=1700000000").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_trends_one_sided_range_rejected_in_strict_mode() {
        let db = test_db().await;
        let app = build_app(db);

        let (from_status, from_body) =
            do_get(app.clone(), "/api/insights/trends?from=1700000000").await;
        assert_eq!(from_status, StatusCode::BAD_REQUEST);
        let from_json: serde_json::Value = serde_json::from_str(&from_body).unwrap();
        assert!(from_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));

        let (to_status, to_body) = do_get(app, "/api/insights/trends?to=1700000000").await;
        assert_eq!(to_status, StatusCode::BAD_REQUEST);
        let to_json: serde_json::Value = serde_json::from_str(&to_body).unwrap();
        assert!(to_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));
    }

    #[tokio::test]
    async fn test_trends_default_all_time_includes_old_data_beyond_six_months() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        let old_ts = now - (300 * 86400);
        insert_session(&db, "trend-old", old_ts, Some("code_work")).await;

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/trends?metric=sessions").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "default_all_time");
        assert!(json["meta"]["effectiveRange"]["from"].as_i64().unwrap() <= old_ts);
    }

    #[tokio::test]
    async fn test_trends_custom_range() {
        let db = test_db().await;
        let app = build_app(db);
        let now = chrono::Utc::now().timestamp();
        let from = now - 86400 * 30;

        let (status, body) = do_get(
            app,
            &format!("/api/insights/trends?from={}&to={}", from, now),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["periodStart"].is_string());
        assert!(json["periodEnd"].is_string());
        assert_eq!(json["meta"]["effectiveRange"]["source"], "explicit_from_to");
    }

    #[tokio::test]
    async fn test_trends_explicit_range_source() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, body) = do_get(app, "/api/insights/trends?range=3mo").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["meta"]["effectiveRange"]["source"],
            "explicit_range_param"
        );
    }

    #[tokio::test]
    async fn test_trends_explicit_from_to_takes_precedence_over_range() {
        let db = test_db().await;
        let app = build_app(db);
        let now = chrono::Utc::now().timestamp();
        let from = now - 86400;
        let to = now - 3600;

        let (status, body) = do_get(
            app,
            &format!("/api/insights/trends?from={}&to={}&range=2yr", from, to),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["effectiveRange"]["source"], "explicit_from_to");
        assert_eq!(json["meta"]["effectiveRange"]["from"], from);
        assert_eq!(json["meta"]["effectiveRange"]["to"], to);
    }

    #[tokio::test]
    async fn test_trends_equality_range_valid() {
        let db = test_db().await;
        let ts = chrono::Utc::now().timestamp();
        insert_session(&db, "trend-eq", ts, Some("code_work")).await;

        let app = build_app(db);
        let (status, body) = do_get(
            app,
            &format!("/api/insights/trends?from={}&to={}&metric=sessions", ts, ts),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["effectiveRange"]["from"], ts);
        assert_eq!(json["meta"]["effectiveRange"]["to"], ts);
        assert_eq!(json["meta"]["effectiveRange"]["source"], "explicit_from_to");
    }

    #[tokio::test]
    async fn test_trends_sessions_metric() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/trends?metric=sessions").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["metric"], "sessions");
    }

    #[tokio::test]
    async fn test_trends_day_granularity() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/insights/trends?granularity=day&range=3mo").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["dataPoints"].is_array());
    }
}
