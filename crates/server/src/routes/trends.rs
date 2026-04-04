//! Trends endpoint for week-over-week metrics.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use claude_view_core::{AnalyticsScopeMeta, AnalyticsSessionBreakdown};
use claude_view_db::trends::WeekTrends;
use claude_view_db::{current_week_bounds, previous_week_bounds};
use serde::Serialize;
use ts_rs::TS;

use crate::error::ApiResult;
use crate::state::AppState;

/// Legacy trends response wrapper with additive metadata.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WeekTrendsResponse {
    #[serde(flatten)]
    pub base: WeekTrends,
    pub meta: AnalyticsScopeMeta,
}

/// GET /api/trends - Get week-over-week trend metrics.
#[utoipa::path(get, path = "/api/trends", tag = "stats",
    responses(
        (status = 200, description = "Week-over-week trend metrics for sessions, tokens, files, commits", body = serde_json::Value),
    )
)]
///
/// Returns trends for:
/// - Session count
/// - Total tokens
/// - Avg tokens per prompt
/// - Total files edited
/// - Avg re-edit rate
/// - Commit link count
pub async fn get_trends(State(state): State<Arc<AppState>>) -> ApiResult<Json<WeekTrendsResponse>> {
    let trends = state.db.get_week_trends().await?;
    let (curr_start, curr_end) = current_week_bounds();
    let (prev_start, _) = previous_week_bounds();
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN is_sidechain = 0 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN is_sidechain = 1 THEN 1 ELSE 0 END), 0)
        FROM sessions
        WHERE archived_at IS NULL
          AND last_message_at >= ?1
          AND last_message_at <= ?2
        "#,
    )
    .bind(prev_start)
    .bind(curr_end)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        crate::error::ApiError::Internal(format!(
            "Failed to fetch trends session breakdown for [{prev_start}, {curr_end}] (current starts at {curr_start}): {e}"
        ))
    })?;

    Ok(Json(WeekTrendsResponse {
        base: trends,
        meta: AnalyticsScopeMeta::new(AnalyticsSessionBreakdown::new(
            primary_sessions,
            sidechain_sessions,
        )),
    }))
}

/// Create the trends routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/trends", get(get_trends))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
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

    async fn insert_trend_session(
        db: &Database,
        id: &str,
        last_message_at: i64,
        is_sidechain: bool,
    ) {
        db.pool()
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO sessions (
                        id, project_id, file_path, preview, project_path, project_display_name,
                        duration_seconds, files_edited_count, reedited_files_count, files_read_count,
                        user_prompt_count, api_call_count, tool_call_count, commit_count, turn_count,
                        last_message_at, size_bytes, last_message, files_touched, skills_used,
                        files_read, files_edited, ai_lines_added, ai_lines_removed, is_sidechain
                    )
                    VALUES (
                        ?1, 'proj', '/tmp/' || ?1 || '.jsonl', 'test', '/tmp', 'Project',
                        120, 2, 0, 1,
                        3, 5, 7, 0, 4,
                        ?2, 1024, 'last', '[]', '[]',
                        '[]', '[]', 10, 2, ?3
                    )
                    "#,
                )
                .bind(id)
                .bind(last_message_at)
                .bind(is_sidechain),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_trends_empty_db() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/trends").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // All metrics should be present with 0/0 values
        assert!(json["sessionCount"].is_object());
        assert!(json["totalTokens"].is_object());
        assert!(json["avgTokensPerPrompt"].is_object());
        assert!(json["totalFilesEdited"].is_object());
        assert!(json["avgReeditRate"].is_object());
        assert!(json["commitLinkCount"].is_object());

        // Verify structure of a metric
        assert_eq!(json["sessionCount"]["current"], 0);
        assert_eq!(json["sessionCount"]["previous"], 0);
        assert_eq!(json["sessionCount"]["delta"], 0);
    }

    #[tokio::test]
    async fn test_trends_includes_data_scope_meta() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp();
        insert_trend_session(&db, "trends-meta-primary", now - 120, false).await;
        insert_trend_session(&db, "trends-meta-sidechain", now - 60, true).await;

        let app = build_app(db);
        let (status, body) = do_get(app, "/api/trends").await;

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
        assert!(
            json["meta"]["sessionBreakdown"]["primarySessions"]
                .as_i64()
                .unwrap()
                >= 1
        );
        assert!(
            json["meta"]["sessionBreakdown"]["sidechainSessions"]
                .as_i64()
                .unwrap()
                >= 1
        );
        assert_eq!(json["meta"]["sessionBreakdown"]["otherSessions"], 0);
    }
}
