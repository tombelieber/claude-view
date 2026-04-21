//! Analytics scope metadata queries for contributions endpoints.

use std::sync::Arc;

use chrono::Local;
use claude_view_core::{AnalyticsScopeMeta, AnalyticsSessionBreakdown};
use claude_view_db::TimeRange;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub(crate) fn build_session_breakdown(
    primary_sessions: i64,
    sidechain_sessions: i64,
) -> AnalyticsScopeMeta {
    AnalyticsScopeMeta::new(AnalyticsSessionBreakdown::new(
        primary_sessions,
        sidechain_sessions,
    ))
}

pub(crate) fn contributions_date_range(
    range: TimeRange,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> (String, String) {
    match range {
        TimeRange::Today => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            (today.clone(), today)
        }
        TimeRange::Custom => {
            let from = from_date.unwrap_or("1970-01-01").to_string();
            let to = to_date
                .map(ToString::to_string)
                .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
            (from, to)
        }
        TimeRange::All => (
            "1970-01-01".to_string(),
            Local::now().format("%Y-%m-%d").to_string(),
        ),
        _ => {
            let days = range.days_back().unwrap_or(7);
            let from = (Local::now() - chrono::Duration::days(days))
                .format("%Y-%m-%d")
                .to_string();
            let to = Local::now().format("%Y-%m-%d").to_string();
            (from, to)
        }
    }
}

pub(super) async fn fetch_contributions_scope_meta(
    state: &Arc<AppState>,
    range: TimeRange,
    from_date: Option<&str>,
    to_date: Option<&str>,
    project_id: Option<&str>,
    branch: Option<&str>,
) -> ApiResult<AnalyticsScopeMeta> {
    let (primary_sessions, sidechain_sessions): (i64, i64) = match range {
        TimeRange::Today => {
            let today_start = format!("{} 00:00:00", Local::now().format("%Y-%m-%d"));
            // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
                FROM session_stats ss
                LEFT JOIN sessions s ON s.id = ss.session_id
                LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
                WHERE sf.archived_at IS NULL
                  AND datetime(ss.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND (?2 IS NULL OR s.project_id = ?2
                       OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?2)
                       OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?2))
                  AND (?3 IS NULL OR ss.git_branch = ?3)
                "#,
            )
            .bind(&today_start)
            .bind(project_id)
            .bind(branch)
            .fetch_one(state.db.pool())
            .await
        }
        TimeRange::All => {
            // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
                FROM session_stats ss
                LEFT JOIN sessions s ON s.id = ss.session_id
                LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
                WHERE sf.archived_at IS NULL
                  AND (?1 IS NULL OR s.project_id = ?1
                       OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?1)
                       OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?1))
                  AND (?2 IS NULL OR ss.git_branch = ?2)
                "#,
            )
            .bind(project_id)
            .bind(branch)
            .fetch_one(state.db.pool())
            .await
        }
        _ => {
            let (from, to) = contributions_date_range(range, from_date, to_date);
            // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
            sqlx::query_as(
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
                FROM session_stats ss
                LEFT JOIN sessions s ON s.id = ss.session_id
                LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
                WHERE sf.archived_at IS NULL
                  AND date(ss.last_message_at, 'unixepoch', 'localtime') >= ?1
                  AND date(ss.last_message_at, 'unixepoch', 'localtime') <= ?2
                  AND (?3 IS NULL OR s.project_id = ?3
                       OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3)
                       OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
                  AND (?4 IS NULL OR ss.git_branch = ?4)
                "#,
            )
            .bind(&from)
            .bind(&to)
            .bind(project_id)
            .bind(branch)
            .fetch_one(state.db.pool())
            .await
        }
    }
    .map_err(|e| {
        ApiError::Internal(format!(
            "Failed to fetch contributions session breakdown: {e}"
        ))
    })?;

    Ok(build_session_breakdown(
        primary_sessions,
        sidechain_sessions,
    ))
}

pub(super) async fn fetch_session_contribution_scope_meta(
    state: &Arc<AppState>,
    session_id: &str,
) -> ApiResult<AnalyticsScopeMeta> {
    // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
        FROM session_stats ss
        LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
        WHERE sf.archived_at IS NULL
          AND ss.session_id = ?1
        "#,
    )
    .bind(session_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        ApiError::Internal(format!(
            "Failed to fetch session contribution breakdown for {session_id}: {e}"
        ))
    })?;

    Ok(build_session_breakdown(
        primary_sessions,
        sidechain_sessions,
    ))
}

pub(super) async fn fetch_branch_sessions_scope_meta(
    state: &Arc<AppState>,
    branch_filter: Option<&str>,
    range: TimeRange,
    from_date: Option<&str>,
    to_date: Option<&str>,
    project_id: Option<&str>,
) -> ApiResult<AnalyticsScopeMeta> {
    let (from, to) = contributions_date_range(range, from_date, to_date);
    // CQRS Phase 7.c — is_sidechain now reads from session_stats; archived_at from session_flags.
    let (primary_sessions, sidechain_sessions): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 0 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ss.is_sidechain = 1 THEN 1 ELSE 0 END), 0)
        FROM session_stats ss
        LEFT JOIN sessions s ON s.id = ss.session_id
        LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
        WHERE sf.archived_at IS NULL
          AND date(ss.last_message_at, 'unixepoch', 'localtime') >= ?1
          AND date(ss.last_message_at, 'unixepoch', 'localtime') <= ?2
          AND (?3 IS NULL OR s.project_id = ?3
               OR (s.git_root IS NOT NULL AND s.git_root <> '' AND s.git_root = ?3)
               OR (s.project_path IS NOT NULL AND s.project_path <> '' AND s.project_path = ?3))
          AND (
                (?4 IS NULL AND ss.git_branch IS NULL)
             OR (?4 IS NOT NULL AND ss.git_branch = ?4)
          )
        "#,
    )
    .bind(&from)
    .bind(&to)
    .bind(project_id)
    .bind(branch_filter)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        ApiError::Internal(format!(
            "Failed to fetch branch sessions breakdown for branch filter {:?}: {e}",
            branch_filter
        ))
    })?;

    Ok(build_session_breakdown(
        primary_sessions,
        sidechain_sessions,
    ))
}
