//! Integration tests for classification job queries.

use claude_view_db::Database;

#[tokio::test]
async fn test_complete_classification_job_none_cost_persists_null() {
    let db = Database::new_in_memory().await.unwrap();

    let job_id = db
        .create_classification_job(5, "claude-cli", "claude-haiku-4-5-20251001")
        .await
        .unwrap();
    db.complete_classification_job(job_id, None).await.unwrap();

    let row: (Option<i64>,) =
        sqlx::query_as("SELECT actual_cost_cents FROM classification_jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row.0, None);

    let job = db.get_classification_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.actual_cost_cents, None);
}

#[tokio::test]
async fn test_get_classification_status_last_run_cost_is_none_when_db_null() {
    let db = Database::new_in_memory().await.unwrap();

    let job_id = db
        .create_classification_job(3, "claude-cli", "claude-sonnet-4-20250514")
        .await
        .unwrap();
    db.complete_classification_job(job_id, None).await.unwrap();

    let status = db.get_classification_status().await.unwrap();
    assert!(!status.is_running);
    assert_eq!(status.last_run_cost_cents, None);
}
