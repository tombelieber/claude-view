// crates/server/src/routes/invocables.rs
//! Invocables and stats endpoints.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use vibe_recall_db::{InvocableWithCount, StatsOverview, TokenStats};

use crate::error::ApiResult;
use crate::state::AppState;

/// GET /api/invocables - List all invocables with their usage counts.
///
/// Returns a list of all known invocables (tools, skills, MCPs) ordered by
/// invocation count descending, then name ascending.
pub async fn list_invocables(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<InvocableWithCount>>> {
    let invocables = state.db.list_invocables_with_counts().await?;
    Ok(Json(invocables))
}

/// GET /api/stats/overview - Aggregate usage statistics.
///
/// Returns total sessions, total invocations, unique invocables used,
/// and the top 10 invocables by usage count.
pub async fn stats_overview(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<StatsOverview>> {
    let stats = state.db.get_stats_overview().await?;
    Ok(Json(stats))
}

/// GET /api/stats/tokens - Aggregate token usage statistics.
pub async fn stats_tokens(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<TokenStats>> {
    let stats = state.db.get_token_stats().await?;
    Ok(Json(stats))
}

/// Create the invocables/stats routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/invocables", get(list_invocables))
        .route("/stats/overview", get(stats_overview))
        .route("/stats/tokens", get(stats_tokens))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use vibe_recall_db::Database;

    /// Helper: create an in-memory database for tests.
    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    /// Helper: build a full app Router with the given database.
    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    /// Helper: make a GET request and return status + body string.
    async fn get(app: axum::Router, uri: &str) -> (StatusCode, String) {
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

    #[tokio::test]
    async fn test_invocables_endpoint_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/invocables").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let items = json.as_array().expect("response should be an array");
        assert_eq!(items.len(), 0, "Empty DB should return empty array");
    }

    #[tokio::test]
    async fn test_invocables_endpoint_with_data() {
        let db = test_db().await;

        // Insert invocables
        db.upsert_invocable("tool::Read", Some("core"), "Read", "tool", "Read files")
            .await
            .unwrap();
        db.upsert_invocable("tool::Edit", None, "Edit", "tool", "Edit files")
            .await
            .unwrap();

        let app = build_app(db);
        let (status, body) = get(app, "/api/invocables").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let items = json.as_array().expect("response should be an array");
        assert_eq!(items.len(), 2);

        // Verify camelCase field names
        let item = &items[0];
        assert!(item.get("id").is_some());
        assert!(item.get("pluginName").is_some());
        assert!(item.get("name").is_some());
        assert!(item.get("kind").is_some());
        assert!(item.get("description").is_some());
        assert!(item.get("invocationCount").is_some());
        assert!(item.get("lastUsedAt").is_some());
    }

    #[tokio::test]
    async fn test_stats_overview_endpoint_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/stats/overview").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // Verify structure with camelCase fields
        assert_eq!(json["totalSessions"], 0);
        assert_eq!(json["totalInvocations"], 0);
        assert_eq!(json["uniqueInvocablesUsed"], 0);
        assert!(json["topInvocables"].is_array());
        assert_eq!(json["topInvocables"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_stats_tokens_endpoint_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = get(app, "/api/stats/tokens").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(json["totalInputTokens"], 0);
        assert_eq!(json["totalOutputTokens"], 0);
        assert_eq!(json["totalCacheReadTokens"], 0);
        assert_eq!(json["totalCacheCreationTokens"], 0);
        assert_eq!(json["cacheHitRatio"], 0.0);
        assert_eq!(json["turnsCount"], 0);
        assert_eq!(json["sessionsCount"], 0);
    }

    #[tokio::test]
    async fn test_stats_overview_endpoint_with_data() {
        let db = test_db().await;

        // Insert a session
        use vibe_recall_core::{SessionInfo, ToolCounts};
        let session = SessionInfo {
            id: "sess-1".to_string(),
            project: "project-a".to_string(),
            project_path: "/home/user/project-a".to_string(),
            file_path: "/home/user/.claude/projects/project-a/sess-1.jsonl".to_string(),
            modified_at: 1000,
            size_bytes: 2048,
            preview: "Preview".to_string(),
            last_message: "Last message".to_string(),
            files_touched: vec![],
            skills_used: vec![],
            tool_counts: ToolCounts {
                edit: 0,
                read: 0,
                bash: 0,
                write: 0,
            },
            message_count: 5,
            turn_count: 3,
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
            // Phase 3: Atomic unit metrics
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
            summary_text: None,
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
        };
        db.insert_session(&session, "project-a", "Project A")
            .await
            .unwrap();

        // Insert invocables and invocations
        db.upsert_invocable("tool::Read", None, "Read", "tool", "Read files")
            .await
            .unwrap();
        let invocations = vec![(
            "f1.jsonl".to_string(),
            10,
            "tool::Read".to_string(),
            "sess-1".to_string(),
            "p".to_string(),
            1000,
        )];
        db.batch_insert_invocations(&invocations).await.unwrap();

        let app = build_app(db);
        let (status, body) = get(app, "/api/stats/overview").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["totalSessions"], 1);
        assert_eq!(json["totalInvocations"], 1);
        assert_eq!(json["uniqueInvocablesUsed"], 1);
        assert_eq!(json["topInvocables"].as_array().unwrap().len(), 1);
        assert_eq!(json["topInvocables"][0]["id"], "tool::Read");
    }
}
