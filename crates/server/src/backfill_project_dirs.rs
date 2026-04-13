//! Backfill project directory existence status.
//!
//! Runs at startup after indexing. Checks each distinct effective project
//! path and stores whether the directory still exists on disk.

use claude_view_db::Database;
use std::sync::Arc;
use tracing::info;

/// Check directory existence for all effective project paths and store in DB.
///
/// Effective path = COALESCE(git_root, project_path, project_id) for each project group.
/// ~36 stat() calls for a typical install — sub-millisecond total.
pub async fn backfill_project_dir_status(db: Arc<Database>) {
    info!("project_dir_status backfill: starting");
    let now = chrono::Utc::now().timestamp();

    // Get all distinct effective paths from sessions
    let rows: Vec<(String,)> = match sqlx::query_as(
        r#"
        SELECT DISTINCT COALESCE(NULLIF(git_root, ''), NULLIF(project_path, ''), project_id) as effective_path
        FROM valid_sessions
        "#,
    )
    .fetch_all(db.pool())
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("project_dir_status backfill: query error: {e}");
            return;
        }
    };

    let mut exists_count = 0usize;
    let mut missing_count = 0usize;

    for (effective_path,) in &rows {
        let dir_exists = if effective_path.starts_with('/') {
            std::path::Path::new(effective_path).is_dir()
        } else {
            // Encoded project_id (not a real path) — can't check, assume exists
            true
        };

        if dir_exists {
            exists_count += 1;
        } else {
            missing_count += 1;
        }

        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO project_dir_status (effective_path, dir_exists, checked_at) VALUES (?1, ?2, ?3)",
        )
        .bind(effective_path)
        .bind(dir_exists as i32)
        .bind(now)
        .execute(db.pool())
        .await
        {
            tracing::warn!("project_dir_status backfill: insert error for {effective_path}: {e}");
        }
    }

    info!("project_dir_status backfill complete: {exists_count} exist, {missing_count} missing");
}
