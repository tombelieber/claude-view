#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_core::{SessionInfo, ToolCounts};
    use claude_view_db::{AggregateCostBreakdown, Database};
    use sqlx::Executor;
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

    fn session_fixture(id: &str, modified_at: i64, is_sidechain: bool) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            project: "project-meta".to_string(),
            project_path: "/home/user/project-meta".to_string(),
            display_name: "project-meta".to_string(),
            git_root: None,
            file_path: format!("/path/{}.jsonl", id),
            modified_at,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain,
            deep_indexed: false,
            total_input_tokens: Some(100),
            total_output_tokens: Some(200),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 2,
            api_call_count: 4,
            tool_call_count: 6,
            files_read: vec![],
            files_edited: vec!["src/main.rs".to_string()],
            files_read_count: 1,
            files_edited_count: 1,
            reedited_files_count: 0,
            duration_seconds: 120,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        }
    }

    #[test]
    fn test_aggregate_cost_breakdown_defaults_are_explicit() {
        let cost = AggregateCostBreakdown::default();
        assert_eq!(cost.total_cost_usd, 0.0);
        assert_eq!(cost.computed_priced_total_cost_usd, 0.0);
        assert_eq!(cost.total_cost_source, "");
        assert!(!cost.has_unpriced_usage);
        assert_eq!(cost.priced_token_coverage, 0.0);
    }

    #[tokio::test]
    async fn test_dashboard_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);
        assert_eq!(json["totalProjects"], 0);
        assert!(json["heatmap"].is_array());
        assert!(json["topSkills"].is_array());
        assert!(json["topProjects"].is_array());
        assert!(json["toolTotals"].is_object());

        // Check extended fields
        assert!(json["currentWeek"].is_object());
        assert_eq!(json["currentWeek"]["sessionCount"], 0);
        // All-time view (no time range params) should NOT include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());
        // dataStartDate should be null for empty DB
        assert!(json["dataStartDate"].is_null());
        assert!(json["meta"]["ranges"]["currentPeriod"]["from"].is_number());
        assert!(json["meta"]["ranges"]["currentPeriod"]["to"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "default_all_time"
        );
        assert!(json["meta"]["ranges"]["heatmap"]["from"].is_number());
        assert!(json["meta"]["ranges"]["heatmap"]["to"].is_number());
        // No time range params → heatmap uses all-time default
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "default_all_time"
        );
    }

    #[tokio::test]
    async fn test_dashboard_stats_includes_data_scope_meta() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

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
    async fn test_dashboard_stats_includes_session_breakdown_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let primary = session_fixture("dash-primary", now - 120, false);
        let sidechain = session_fixture("dash-sidechain", now - 60, true);
        db.insert_session(&primary, "project-meta", "Project Meta")
            .await
            .unwrap();
        db.insert_session(&sidechain, "project-meta", "Project Meta")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["meta"]["sessionBreakdown"]["primarySessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["sidechainSessions"], 1);
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
        assert_eq!(json["meta"]["sessionBreakdown"]["totalObservedSessions"], 2);
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-range-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-range-1.jsonl".to_string(),
            modified_at: now - 86400, // 1 day ago
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);

        // Query with time range (7 days)
        let seven_days_ago = now - (7 * 86400);
        let uri = format!("/api/stats/dashboard?from={}&to={}", seven_days_ago, now);
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // With time range params, trends should be present
        assert!(json["trends"].is_object());
        assert!(json["trends"]["sessions"].is_object());
        assert!(json["trends"]["sessions"]["current"].is_number());
        assert!(json["trends"]["sessions"]["previous"].is_number());

        // Period bounds should be present
        assert!(json["periodStart"].is_number());
        assert!(json["periodEnd"].is_number());
        assert!(json["comparisonPeriodStart"].is_number());
        assert!(json["comparisonPeriodEnd"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "explicit_from_to"
        );
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "explicit_range_param"
        );

        // dataStartDate should be set
        assert!(json["dataStartDate"].is_number());
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec!["/commit".to_string()],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 20,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,

            parse_version: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalProjects"], 1);
        assert!(!json["heatmap"].as_array().unwrap().is_empty());

        // Check current week metrics (all-time view)
        assert!(json["currentWeek"]["sessionCount"].is_number());

        // All-time view should not include trends
        assert!(json.get("trends").is_none() || json["trends"].is_null());

        // dataStartDate should be set when there's data
        assert!(json["dataStartDate"].is_number());
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "default_all_time"
        );
        // No time range params → heatmap uses all-time default
        assert_eq!(
            json["meta"]["ranges"]["heatmap"]["source"],
            "default_all_time"
        );
    }

    #[tokio::test]
    async fn test_dashboard_stats_rejects_one_sided_ranges() {
        let db = test_db().await;
        let app = build_app(db);

        let (from_status, from_body) =
            do_get(app.clone(), "/api/stats/dashboard?from=1700000000").await;
        assert_eq!(from_status, StatusCode::BAD_REQUEST);
        let from_json: serde_json::Value = serde_json::from_str(&from_body).unwrap();
        assert!(from_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));

        let (to_status, to_body) = do_get(app, "/api/stats/dashboard?to=1700000000").await;
        assert_eq!(to_status, StatusCode::BAD_REQUEST);
        let to_json: serde_json::Value = serde_json::from_str(&to_body).unwrap();
        assert!(to_json["details"]
            .as_str()
            .unwrap()
            .contains("Both 'from' and 'to' must be provided together"));
    }

    #[tokio::test]
    async fn test_dashboard_stats_rejects_inverted_range() {
        let db = test_db().await;
        let app = build_app(db);

        let (status, body) =
            do_get(app, "/api/stats/dashboard?from=1700100000&to=1700000000").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["details"]
            .as_str()
            .unwrap()
            .contains("'from' must be <= 'to'"));
    }

    #[tokio::test]
    async fn test_dashboard_stats_accepts_equal_bounds() {
        let db = test_db().await;
        let app = build_app(db);

        let ts = chrono::Utc::now().timestamp();
        let (status, body) =
            do_get(app, &format!("/api/stats/dashboard?from={}&to={}", ts, ts)).await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["periodStart"], ts);
        assert_eq!(json["periodEnd"], ts);
        assert_eq!(
            json["meta"]["ranges"]["currentPeriod"]["source"],
            "explicit_from_to"
        );
    }

    #[tokio::test]
    async fn test_storage_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All counts should be 0
        assert_eq!(json["sessionCount"], 0);
        assert_eq!(json["projectCount"], 0);
        assert_eq!(json["commitCount"], 0);

        // Storage sizes should be present (even if 0)
        assert!(json["jsonlBytes"].is_number());
        assert!(json["sqliteBytes"].is_number());
        assert!(json["indexBytes"].is_number());

        // Oldest session should be null for empty DB
        assert!(json["oldestSessionDate"].is_null());

        // Last index/sync should be null for fresh DB
        assert!(json["lastIndexAt"].is_null());
        assert!(json["lastGitSyncAt"].is_null());
    }

    #[tokio::test]
    async fn test_storage_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session
        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            display_name: "project-a".to_string(),
            git_root: None,
            file_path: "/path/sess-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 0,
            api_call_count: 0,
            tool_call_count: 0,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 0,
            files_edited_count: 0,
            reedited_files_count: 0,
            duration_seconds: 0,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        // Update index metadata
        db.update_index_metadata_on_success(1500, 1, 1)
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/storage").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Should have 1 session and 1 project
        assert_eq!(json["sessionCount"], 1);
        assert_eq!(json["projectCount"], 1);

        // Oldest session should be set
        assert!(json["oldestSessionDate"].is_number());

        // Last index info should be present
        assert!(json["lastIndexAt"].is_number());
        assert_eq!(json["lastIndexDurationMs"], 1500);
        assert_eq!(json["lastIndexSessionCount"], 1);

        // SQLite size should be > 0 for non-empty db
        assert!(json["sqliteBytes"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All values should be 0 for empty DB
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);
        assert_eq!(json["filesCreated"], 0);
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);

        // Arrays should be empty
        assert!(json["tokensByModel"].as_array().unwrap().is_empty());
        assert!(json["tokensByProject"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_ai_generation_includes_data_scope_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let primary = session_fixture("ai-primary", now - 120, false);
        let sidechain = session_fixture("ai-sidechain", now - 60, true);
        db.insert_session(&primary, "project-meta", "Project Meta")
            .await
            .unwrap();
        db.insert_session(&sidechain, "project-meta", "Project Meta")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

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
    async fn test_ai_generation_stats_marks_partial_when_unpriced_model_present() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();

        let session = session_fixture("sess-aigen-unpriced", now - 60, false);
        db.insert_session(&session, "project-meta", "Project Meta")
            .await
            .unwrap();

        // Unknown model exists in DB but has no pricing entry.
        db.pool()
            .execute(sqlx::query(
                r#"
                INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                VALUES ('unknown-model-without-pricing', 'unknown', 'unknown', 0, 0)
                "#,
            ))
            .await
            .unwrap();
        db.pool()
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO turns (
                        session_id, uuid, seq, model_id, input_tokens, output_tokens,
                        cache_read_tokens, cache_creation_tokens, timestamp
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind("sess-aigen-unpriced")
                .bind("turn-unpriced-1")
                .bind(1)
                .bind("unknown-model-without-pricing")
                .bind(5_000)
                .bind(1_000)
                .bind(0)
                .bind(0)
                .bind(now - 60),
            )
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;
        assert_eq!(status, StatusCode::OK);

        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["cost"]["hasUnpricedUsage"], true);
        assert_eq!(
            json["cost"]["totalCostSource"],
            "computed_priced_tokens_partial"
        );
        assert_eq!(json["cost"]["unpricedModelCount"], 1);
        assert_eq!(json["cost"]["pricedTokenCoverage"], 0.0);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_data() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with token data
        // Use update_session_deep_fields to set token data since insert_session doesn't handle tokens
        let session = SessionInfo {
            id: "sess-ai-1".to_string(),
            project: "project-ai".to_string(),
            project_path: "/home/user/project-ai".to_string(),
            display_name: "project-ai".to_string(),
            git_root: None,
            file_path: "/path/sess-ai-1.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "AI Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec!["src/main.rs".to_string()],
            skills_used: vec![],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec!["src/main.rs".to_string()],
            files_read_count: 15,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-ai", "Project AI")
            .await
            .unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-ai-1",
            "Test msg",
            8,                    // turn_count
            5,                    // tool_edit
            10,                   // tool_read
            3,                    // tool_bash
            2,                    // tool_write
            r#"["src/main.rs"]"#, // files_touched
            "[]",                 // skills_used
            10,                   // user_prompt_count
            20,                   // api_call_count
            50,                   // tool_call_count
            "[]",                 // files_read
            r#"["src/main.rs"]"#, // files_edited
            15,                   // files_read_count
            5,                    // files_edited_count
            2,                    // reedited_files_count
            600,                  // duration_seconds
            3,                    // commit_count
            Some(now - 86400),    // first_message_at
            150000,               // total_input_tokens
            250000,               // total_output_tokens
            10000,                // cache_read_tokens
            5000,                 // cache_creation_tokens
            2,                    // thinking_block_count
            Some(500),            // turn_duration_avg_ms
            Some(2000),           // turn_duration_max_ms
            Some(4000),           // turn_duration_total_ms
            0,                    // api_error_count
            0,                    // api_retry_count
            0,                    // compaction_count
            0,                    // hook_blocked_count
            0,                    // agent_spawn_count
            0,                    // bash_progress_count
            0,                    // hook_progress_count
            0,                    // mcp_progress_count
            None,                 // summary_text
            1,                    // parse_version
            2048,                 // file_size
            now - 86400,          // file_mtime
            0,
            0,
            0, // lines_added, lines_removed, loc_source
            0,
            0,         // ai_lines_added, ai_lines_removed
            None,      // work_type
            None,      // git_branch
            None,      // primary_model
            None,      // last_message_at
            None,      // first_user_prompt
            0,         // total_task_time_seconds
            None,      // longest_task_seconds
            None,      // longest_task_preview
            Some(0.0), // total_cost_usd
        )
        .await
        .unwrap();

        // Update the primary_model column using the db pool directly
        db.set_session_primary_model("sess-ai-1", "claude-3-5-sonnet-20241022")
            .await
            .unwrap();

        // Ground-truth model rollups are sourced from turns.model_id.
        db.pool()
            .execute(sqlx::query(
                r#"
                    INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen)
                    VALUES ('claude-3-5-sonnet-20241022', 'anthropic', 'sonnet', 0, 0)
                    "#,
            ))
            .await
            .unwrap();
        db.pool()
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO turns (
                        session_id, uuid, seq, model_id, input_tokens, output_tokens,
                        cache_read_tokens, cache_creation_tokens, timestamp
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind("sess-ai-1")
                .bind("turn-ai-1")
                .bind(1)
                .bind("claude-3-5-sonnet-20241022")
                .bind(150000)
                .bind(250000)
                .bind(10000)
                .bind(5000)
                .bind(now - 86400),
            )
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/stats/ai-generation").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Lines are not tracked yet, should be 0
        assert_eq!(json["linesAdded"], 0);
        assert_eq!(json["linesRemoved"], 0);

        // Files created should match files_edited_count
        assert_eq!(json["filesCreated"], 5);

        // Token totals
        assert_eq!(json["totalInputTokens"], 150000);
        assert_eq!(json["totalOutputTokens"], 250000);

        // Token by model should have our model
        let models = json["tokensByModel"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(models[0]["inputTokens"], 150000);
        assert_eq!(models[0]["outputTokens"], 250000);

        // Token by project should have our project
        let projects = json["tokensByProject"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["project"], "Project AI");
        assert_eq!(projects[0]["inputTokens"], 150000);
        assert_eq!(projects[0]["outputTokens"], 250000);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_time_range() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert a session with a known first_message_at
        let session = SessionInfo {
            id: "sess-range".to_string(),
            project: "project-range".to_string(),
            project_path: "/home/user/project-range".to_string(),
            display_name: "project-range".to_string(),
            git_root: None,
            file_path: "/path/sess-range.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Range Test".to_string(),
            last_message: "Test msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: None,
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 10,
            files_edited_count: 3,
            reedited_files_count: 1,
            duration_seconds: 300,
            commit_count: 1,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session, "project-range", "Project Range")
            .await
            .unwrap();

        // Update with token data and first_message_at
        db.update_session_deep_fields(
            "sess-range",
            "Test msg",
            5,
            0,
            0,
            0,
            0,
            "[]",
            "[]",
            5,
            10,
            20,
            "[]",
            "[]",
            10,
            3,
            1,
            300,
            1,
            Some(now - 86400), // first_message_at: 1 day ago
            100000,
            200000,
            0,
            0,
            0,
            None,
            None,
            None,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            None,
            1,
            2048,
            now - 86400,
            0,
            0,
            0, // lines_added, lines_removed, loc_source
            0,
            0,         // ai_lines_added, ai_lines_removed
            None,      // work_type
            None,      // git_branch
            None,      // primary_model
            None,      // last_message_at
            None,      // first_user_prompt
            0,         // total_task_time_seconds
            None,      // longest_task_seconds
            None,      // longest_task_preview
            Some(0.0), // total_cost_usd
        )
        .await
        .unwrap();

        let app = build_app(db);

        // Query with time range that includes the session
        let seven_days_ago = now - (7 * 86400);
        let uri = format!(
            "/api/stats/ai-generation?from={}&to={}",
            seven_days_ago, now
        );
        let (status, body) = do_get(app.clone(), &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 100000);
        assert_eq!(json["totalOutputTokens"], 200000);

        // Query with time range that excludes the session (future)
        let uri = format!(
            "/api/stats/ai-generation?from={}&to={}",
            now + 86400,
            now + 172800
        );
        let (status, body) = do_get(app, &uri).await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);
    }

    #[tokio::test]
    async fn test_dashboard_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-proj-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            display_name: "project-alpha".to_string(),
            git_root: None,
            file_path: "/path/sess-proj-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha session".to_string(),
            last_message: "Test msg A".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts {
                edit: 5,
                read: 10,
                bash: 3,
                write: 2,
            },
            message_count: 20,
            turn_count: 8,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: Some(10000),
            total_output_tokens: Some(5000),
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 10,
            api_call_count: 20,
            tool_call_count: 50,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 15,
            files_edited_count: 5,
            reedited_files_count: 2,
            duration_seconds: 600,
            commit_count: 3,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha")
            .await
            .unwrap();

        let mut session_b = session_a.clone();
        session_b.id = "sess-proj-b".to_string();
        session_b.project = "project-beta".to_string();
        session_b.project_path = "/home/user/project-beta".to_string();
        session_b.file_path = "/path/sess-proj-b.jsonl".to_string();
        session_b.preview = "Beta session".to_string();
        session_b.git_branch = Some("develop".to_string());
        db.insert_session(&session_b, "project-beta", "Project Beta")
            .await
            .unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) =
            do_get(app.clone(), "/api/stats/dashboard?project=project-alpha").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            json["totalSessions"], 1,
            "should only count project-alpha sessions"
        );
        assert_eq!(json["totalProjects"], 1);

        // Filter by project + branch
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/dashboard?project=project-alpha&branch=main",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);

        // Filter by project + wrong branch = 0 sessions
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/dashboard?project=project-alpha&branch=develop",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 0);

        // No filter — both sessions
        let (status, body) = do_get(app, "/api/stats/dashboard").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 2);
    }

    #[tokio::test]
    async fn test_ai_generation_stats_with_project_filter() {
        let db = test_db().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let session_a = SessionInfo {
            id: "sess-aigen-a".to_string(),
            project: "project-alpha".to_string(),
            project_path: "/home/user/project-alpha".to_string(),
            display_name: "project-alpha".to_string(),
            git_root: None,
            file_path: "/path/sess-aigen-a.jsonl".to_string(),
            modified_at: now - 86400,
            size_bytes: 2048,
            preview: "Alpha AI".to_string(),
            last_message: "msg".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts::default(),
            message_count: 10,
            turn_count: 5,
            summary: None,
            git_branch: Some("main".to_string()),
            is_sidechain: false,
            deep_indexed: false,
            total_input_tokens: None,
            total_output_tokens: None,
            total_cache_read_tokens: None,
            total_cache_creation_tokens: None,
            turn_count_api: None,
            primary_model: None,
            user_prompt_count: 5,
            api_call_count: 10,
            tool_call_count: 20,
            files_read: vec![],
            files_edited: vec![],
            files_read_count: 5,
            files_edited_count: 3,
            reedited_files_count: 0,
            duration_seconds: 300,
            commit_count: 0,
            thinking_block_count: 0,
            turn_duration_avg_ms: None,
            turn_duration_max_ms: None,
            api_error_count: 0,
            compaction_count: 0,
            agent_spawn_count: 0,
            bash_progress_count: 0,
            hook_progress_count: 0,
            mcp_progress_count: 0,
            lines_added: 0,
            lines_removed: 0,
            loc_source: 0,

            parse_version: 0,
            category_l1: None,
            category_l2: None,
            category_l3: None,
            category_confidence: None,
            category_source: None,
            classified_at: None,
            prompt_word_count: None,
            correction_count: 0,
            same_file_edit_count: 0,
            total_task_time_seconds: None,
            longest_task_seconds: None,
            longest_task_preview: None,
            first_message_at: None,
            total_cost_usd: None,
            slug: None,
            entrypoint: None,
        };
        db.insert_session(&session_a, "project-alpha", "Project Alpha")
            .await
            .unwrap();

        let app = build_app(db);

        // Filter by project
        let (status, body) = do_get(
            app.clone(),
            "/api/stats/ai-generation?project=project-alpha",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 3);

        // Filter by non-existent project = 0
        let (status, body) = do_get(app, "/api/stats/ai-generation?project=project-nope").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["filesCreated"], 0);
    }
}
