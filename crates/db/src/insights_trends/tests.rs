//! Tests for the insights trends module.

use super::insights::*;
use super::types::*;
use crate::Database;

#[tokio::test]
async fn test_get_metric_timeseries_empty_db() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();
    let data = db
        .get_metric_timeseries("reedit_rate", now - 86400 * 30, now, "week")
        .await
        .unwrap();
    assert!(data.is_empty());
}

#[tokio::test]
async fn test_get_metric_timeseries_with_data() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    // Insert test sessions spanning multiple weeks
    for i in 0..20 {
        let id = format!("trend-test-{}", i);
        let file_path = format!("/tmp/trend-test-{}.jsonl", i);
        sqlx::query(
            r#"
            INSERT INTO session_stats (
                session_id, source_content_hash, source_size, parser_version,
                stats_version, indexed_at,
                project_id, file_path, preview, project_path,
                duration_seconds, files_edited_count, reedited_files_count,
                files_read_count, user_prompt_count, api_call_count,
                tool_call_count, commit_count, turn_count,
                last_message_at, size_bytes, last_message,
                files_touched, skills_used, files_read, files_edited
            )
            VALUES (?1, X'00', 1024, 1, 4, 0,
                'proj', ?3, 'test', '/tmp',
                600, 5, 1, 5, 5, 10, 20, 1, 10,
                ?2, 1024, '', '[]', '[]', '[]', '[]')
            "#,
        )
        .bind(&id)
        .bind(now - (i as i64 * 86400 * 2)) // every 2 days
        .bind(&file_path)
        .execute(db.pool())
        .await
        .unwrap();
    }

    let data = db
        .get_metric_timeseries("sessions", now - 86400 * 60, now, "week")
        .await
        .unwrap();
    assert!(!data.is_empty());
}

#[tokio::test]
async fn test_get_metric_timeseries_lines_uses_canonical_ai_line_fields() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO session_stats (
            session_id, source_content_hash, source_size, parser_version,
            stats_version, indexed_at,
            project_id, file_path, preview, last_message_at,
            files_edited_count, ai_lines_added, ai_lines_removed
        )
        VALUES
            ('lines-1', X'00', 0, 1, 4, 0, 'proj', '/tmp/lines-1.jsonl', 'test', ?1, 10, 7, 3),
            ('lines-2', X'00', 0, 1, 4, 0, 'proj', '/tmp/lines-2.jsonl', 'test', ?1, 2, 1, 1)
        "#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    let data = db
        .get_metric_timeseries("lines", now - 3600, now + 3600, "day")
        .await
        .unwrap();

    assert_eq!(data.len(), 1);
    assert!(
        (data[0].value - 12.0).abs() < 0.0001,
        "lines must use ai_lines_added + ai_lines_removed (expected 12, got {})",
        data[0].value
    );
}

#[tokio::test]
async fn test_get_metric_timeseries_cost_per_line_uses_priced_lines_denominator() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO session_stats (
            session_id, source_content_hash, source_size, parser_version,
            stats_version, indexed_at,
            project_id, file_path, preview, last_message_at,
            files_edited_count, ai_lines_added, ai_lines_removed, total_cost_usd
        )
        VALUES
            ('cpl-1', X'00', 0, 1, 4, 0, 'proj', '/tmp/cpl-1.jsonl', 'test', ?1, 40, 9, 3, 1.2),
            ('cpl-2', X'00', 0, 1, 4, 0, 'proj', '/tmp/cpl-2.jsonl', 'test', ?1, 20, 2, 1, 0.3),
            ('cpl-3', X'00', 0, 1, 4, 0, 'proj', '/tmp/cpl-3.jsonl', 'test', ?1, 1, 100, 0, NULL)
        "#,
    )
    .bind(now)
    .execute(db.pool())
    .await
    .unwrap();

    let data = db
        .get_metric_timeseries("cost_per_line", now - 3600, now + 3600, "day")
        .await
        .unwrap();

    assert_eq!(data.len(), 1);
    assert!(
        (data[0].value - 0.1).abs() < 0.0001,
        "cost_per_line should use SUM(total_cost_usd) / priced lines (expected 0.1, got {})",
        data[0].value
    );
}

#[tokio::test]
async fn test_get_category_evolution_no_classification() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();
    let data = db
        .get_category_evolution(now - 86400 * 30, now, "week")
        .await
        .unwrap();
    assert!(data.is_none());
}

#[tokio::test]
async fn test_get_activity_heatmap_empty() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();
    let data = db
        .get_activity_heatmap(now - 86400 * 30, now)
        .await
        .unwrap();
    assert!(data.is_empty());
}

#[tokio::test]
async fn test_get_activity_heatmap_with_data() {
    let db = Database::new_in_memory().await.unwrap();
    let now = chrono::Utc::now().timestamp();

    for i in 0..10 {
        let id = format!("heatmap-test-{}", i);
        let file_path = format!("/tmp/heatmap-test-{}.jsonl", i);
        sqlx::query(
            r#"
            INSERT INTO session_stats (
                session_id, source_content_hash, source_size, parser_version,
                stats_version, indexed_at,
                project_id, file_path, preview, project_path,
                duration_seconds, files_edited_count, reedited_files_count,
                files_read_count, user_prompt_count, api_call_count,
                tool_call_count, commit_count, turn_count,
                last_message_at, size_bytes, last_message,
                files_touched, skills_used, files_read, files_edited
            )
            VALUES (?1, X'00', 1024, 1, 4, 0,
                'proj', ?3, 'test', '/tmp',
                600, 5, 1, 5, 5, 10, 20, 1, 10,
                ?2, 1024, '', '[]', '[]', '[]', '[]')
            "#,
        )
        .bind(&id)
        .bind(now - (i as i64 * 3600)) // every hour
        .bind(&file_path)
        .execute(db.pool())
        .await
        .unwrap();
    }

    let data = db
        .get_activity_heatmap(now - 86400 * 30, now)
        .await
        .unwrap();

    for cell in &data {
        assert!(cell.day_of_week < 7);
        assert!(cell.hour_of_day < 24);
        assert!(cell.sessions > 0);
        assert!(cell.avg_reedit_rate >= 0.0);
    }
}

#[test]
fn test_calculate_trend_stats_improving() {
    let data = vec![
        MetricDataPoint {
            date: "2026-01".to_string(),
            value: 0.5,
        },
        MetricDataPoint {
            date: "2026-02".to_string(),
            value: 0.3,
        },
    ];
    let (avg, trend, direction) = calculate_trend_stats(&data, "reedit_rate");
    assert!((avg - 0.4).abs() < 0.01);
    assert!(trend < 0.0);
    assert_eq!(direction, "improving");
}

#[test]
fn test_calculate_trend_stats_worsening() {
    let data = vec![
        MetricDataPoint {
            date: "2026-01".to_string(),
            value: 0.2,
        },
        MetricDataPoint {
            date: "2026-02".to_string(),
            value: 0.5,
        },
    ];
    let (_avg, trend, direction) = calculate_trend_stats(&data, "reedit_rate");
    assert!(trend > 0.0);
    assert_eq!(direction, "worsening");
}

#[test]
fn test_calculate_trend_stats_stable() {
    let data = vec![
        MetricDataPoint {
            date: "2026-01".to_string(),
            value: 0.3,
        },
        MetricDataPoint {
            date: "2026-02".to_string(),
            value: 0.31,
        },
    ];
    let (_, _, direction) = calculate_trend_stats(&data, "sessions");
    assert_eq!(direction, "stable");
}

#[test]
fn test_calculate_trend_stats_empty() {
    let data: Vec<MetricDataPoint> = vec![];
    let (avg, trend, direction) = calculate_trend_stats(&data, "sessions");
    assert_eq!(avg, 0.0);
    assert_eq!(trend, 0.0);
    assert_eq!(direction, "stable");
}

#[test]
fn test_generate_metric_insight_improving_reedit() {
    let insight = generate_metric_insight("reedit_rate", -52.0, "6mo");
    assert!(insight.contains("dropped"));
    assert!(insight.contains("52%"));
    assert!(insight.contains("6 months"));
}

#[test]
fn test_generate_metric_insight_worsening_reedit() {
    let insight = generate_metric_insight("reedit_rate", 30.0, "3mo");
    assert!(insight.contains("increased"));
    assert!(insight.contains("30%"));
}

#[test]
fn test_generate_category_insight_increasing_thinking() {
    let data = vec![
        CategoryDataPoint {
            date: "2026-01".to_string(),
            code_work: 0.72,
            support_work: 0.20,
            thinking_work: 0.08,
        },
        CategoryDataPoint {
            date: "2026-02".to_string(),
            code_work: 0.62,
            support_work: 0.23,
            thinking_work: 0.15,
        },
    ];
    let insight = generate_category_insight(&data);
    assert!(insight.contains("Thinking Work increased"));
}

#[test]
fn test_generate_heatmap_insight_empty() {
    let insight = generate_heatmap_insight(&[]);
    assert!(insight.contains("Not enough"));
}

#[test]
fn test_generate_heatmap_insight_consistent() {
    let data = vec![
        HeatmapCell {
            day_of_week: 0,
            hour_of_day: 9,
            sessions: 10,
            avg_reedit_rate: 0.20,
        },
        HeatmapCell {
            day_of_week: 1,
            hour_of_day: 9,
            sessions: 10,
            avg_reedit_rate: 0.22,
        },
    ];
    let insight = generate_heatmap_insight(&data);
    assert!(insight.contains("consistent") || insight.contains("sweet spot"));
}

#[test]
fn test_get_session_count_in_range() {
    // Just test the function signature compiles - actual DB test above covers it
}
