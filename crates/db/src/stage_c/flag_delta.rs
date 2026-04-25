//! CQRS Phase 4.9 — FlagDelta type + Stage C apply path.
//!
//! Emitted by the Phase 5.3 fold writer in the SAME TX as each
//! `session_flags` UPSERT. Stage C's drainer deserialises and applies
//! compensating rollup UPDATEs on the affected `daily/weekly/monthly
//! × project/branch/category` buckets.
//!
//! ## Kinds
//!
//! - `Classify { after_category_l1 }` — session's category changed.
//!   Stage C must ADD the session's stats to `*_category_stats`
//!   buckets under the new category, and SUBTRACT from the old
//!   category (if `before_category_l1` was Some).
//! - `Archive` / `Unarchive` — archive state changed. Archived sessions
//!   are excluded from `valid_sessions`, so Stage C must subtract (on
//!   archive) / re-add (on unarchive) the session's stats across ALL
//!   rollup dimensions the session contributed to.
//! - `Dismiss` — no rollup impact (dismiss is audit-only, no
//!   rollup dimension tracks dismissal state).

use chrono::Datelike;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};

use crate::DbResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FlagDelta {
    Classify {
        session_id: String,
        before_category_l1: Option<String>,
        after_category_l1: Option<String>,
        at_ms: i64,
    },
    Archive {
        session_id: String,
        at_ms: i64,
    },
    Unarchive {
        session_id: String,
        at_ms: i64,
    },
    Dismiss {
        session_id: String,
        at_ms: i64,
    },
}

impl FlagDelta {
    pub fn delta_type(&self) -> &'static str {
        "flag_delta"
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::Classify { session_id, .. }
            | Self::Archive { session_id, .. }
            | Self::Unarchive { session_id, .. }
            | Self::Dismiss { session_id, .. } => session_id,
        }
    }
}

/// Apply one FlagDelta. Called by the drainer task under its own TX.
///
/// Classify: looks up the session's per-bucket contribution in
/// `session_stats`, ADDs to the new category buckets in
/// `*_category_stats`, SUBTRACTs from the old category buckets if
/// `before_category_l1` was Some.
///
/// Archive / Unarchive: `valid_sessions` excludes archived — so the
/// rollup tables must subtract (on archive) / re-add (on unarchive)
/// the session's stats across ALL bucket × dimension combos
/// (global / project / branch / model / category).
///
/// Dismiss: no-op (no dismissal dimension in rollups).
pub(crate) async fn apply_flag_delta_tx(
    tx: &mut Transaction<'_, Sqlite>,
    delta: &FlagDelta,
) -> DbResult<()> {
    match delta {
        FlagDelta::Classify {
            session_id,
            before_category_l1,
            after_category_l1,
            ..
        } => {
            if before_category_l1 != after_category_l1 {
                if let Some(before) = before_category_l1 {
                    fanout_category_delta(tx, session_id, before, -1).await?;
                }
                if let Some(after) = after_category_l1 {
                    fanout_category_delta(tx, session_id, after, 1).await?;
                }
            }
            Ok(())
        }
        FlagDelta::Archive { session_id, .. } => fanout_full_delta(tx, session_id, -1).await,
        FlagDelta::Unarchive { session_id, .. } => fanout_full_delta(tx, session_id, 1).await,
        FlagDelta::Dismiss { .. } => Ok(()),
    }
}

fn bucket_starts(last_ms: i64) -> (i64, i64, i64) {
    let day_start = last_ms.div_euclid(86_400_000) * 86_400_000;
    // ISO-week anchoring: Monday = 0. SQLite convention across the
    // rollup schema uses UTC day / Monday-anchored week / 1st-of-month.
    let days_since_epoch = day_start.div_euclid(86_400_000);
    // Unix epoch (1970-01-01) is a Thursday → day index 3 in Monday-0 week.
    let weekday_mon0 = (days_since_epoch + 3).rem_euclid(7);
    let week_start = day_start - weekday_mon0 * 86_400_000;
    let month_start = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(last_ms)
        .and_then(|dt| dt.date_naive().with_day(1))
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or(day_start);
    (day_start, week_start, month_start)
}

/// Add or subtract (`sign` = ±1) the session's stats to *_category_stats
/// buckets under the given category.
async fn fanout_category_delta(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
    category_l1: &str,
    sign: i64,
) -> DbResult<()> {
    let row: Option<(Option<i64>, i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT last_message_at, total_input_tokens, total_output_tokens,
                  user_prompt_count, duration_seconds
           FROM session_stats WHERE session_id = ?1"#,
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((Some(last_ms), in_tokens, out_tokens, prompts, duration_s)) = row else {
        return Ok(()); // no stats = nothing to fan out
    };

    let total_tokens = (in_tokens + out_tokens) * sign;
    let prompt_delta = prompts * sign;
    let duration_ms_delta = duration_s * 1000 * sign;
    let session_delta = sign;
    let duration_count_delta = if duration_s > 0 { sign } else { 0 };

    let (day_start, week_start, month_start) = bucket_starts(last_ms);

    for (table, period) in [
        ("daily_category_stats", day_start),
        ("weekly_category_stats", week_start),
        ("monthly_category_stats", month_start),
    ] {
        let sql = format!(
            "INSERT INTO {table}
               (period_start, category_l1, session_count, total_tokens,
                prompt_count, duration_sum_ms, duration_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(period_start, category_l1) DO UPDATE SET
               session_count  = session_count  + excluded.session_count,
               total_tokens   = total_tokens   + excluded.total_tokens,
               prompt_count   = prompt_count   + excluded.prompt_count,
               duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
               duration_count = duration_count + excluded.duration_count"
        );
        sqlx::query(&sql)
            .bind(period)
            .bind(category_l1)
            .bind(session_delta)
            .bind(total_tokens)
            .bind(prompt_delta)
            .bind(duration_ms_delta)
            .bind(duration_count_delta)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Archive/unarchive fanout — subtracts/adds across ALL dimensions
/// (global, project, branch, model, category if classified).
async fn fanout_full_delta(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
    sign: i64,
) -> DbResult<()> {
    let row: Option<(
        Option<i64>,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"SELECT ss.last_message_at, ss.total_input_tokens, ss.total_output_tokens,
                  ss.user_prompt_count, ss.duration_seconds,
                  ss.project_id, ss.git_branch, ss.primary_model
           FROM session_stats ss
           WHERE ss.session_id = ?1"#,
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((
        Some(last_ms),
        in_tokens,
        out_tokens,
        prompts,
        duration_s,
        project_id,
        branch,
        model,
    )) = row
    else {
        return Ok(());
    };

    let category_row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT category_l1 FROM session_flags WHERE session_id = ?1")
            .bind(session_id)
            .fetch_optional(&mut **tx)
            .await?;
    let category_l1 = category_row.and_then(|(c,)| c);

    let total_tokens = (in_tokens + out_tokens) * sign;
    let prompt_delta = prompts * sign;
    let duration_ms_delta = duration_s * 1000 * sign;
    let duration_count_delta = if duration_s > 0 { sign } else { 0 };

    let (day_start, week_start, month_start) = bucket_starts(last_ms);

    // Global dimension
    for (table, period) in [
        ("daily_global_stats", day_start),
        ("weekly_global_stats", week_start),
        ("monthly_global_stats", month_start),
    ] {
        let sql = format!(
            "INSERT INTO {table} (period_start, session_count, total_tokens,
                prompt_count, duration_sum_ms, duration_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(period_start) DO UPDATE SET
               session_count  = session_count  + excluded.session_count,
               total_tokens   = total_tokens   + excluded.total_tokens,
               prompt_count   = prompt_count   + excluded.prompt_count,
               duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
               duration_count = duration_count + excluded.duration_count"
        );
        sqlx::query(&sql)
            .bind(period)
            .bind(sign)
            .bind(total_tokens)
            .bind(prompt_delta)
            .bind(duration_ms_delta)
            .bind(duration_count_delta)
            .execute(&mut **tx)
            .await?;
    }

    // Project dimension
    if let Some(pid) = &project_id {
        for (table, period) in [
            ("daily_project_stats", day_start),
            ("weekly_project_stats", week_start),
            ("monthly_project_stats", month_start),
        ] {
            let sql = format!(
                "INSERT INTO {table} (period_start, project_id, session_count, total_tokens,
                    prompt_count, duration_sum_ms, duration_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(period_start, project_id) DO UPDATE SET
                   session_count  = session_count  + excluded.session_count,
                   total_tokens   = total_tokens   + excluded.total_tokens,
                   prompt_count   = prompt_count   + excluded.prompt_count,
                   duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
                   duration_count = duration_count + excluded.duration_count"
            );
            sqlx::query(&sql)
                .bind(period)
                .bind(pid)
                .bind(sign)
                .bind(total_tokens)
                .bind(prompt_delta)
                .bind(duration_ms_delta)
                .bind(duration_count_delta)
                .execute(&mut **tx)
                .await?;
        }
    }

    // Branch dimension
    if let (Some(pid), Some(br)) = (&project_id, &branch) {
        for (table, period) in [
            ("daily_branch_stats", day_start),
            ("weekly_branch_stats", week_start),
            ("monthly_branch_stats", month_start),
        ] {
            let sql = format!(
                "INSERT INTO {table} (period_start, project_id, branch,
                    session_count, total_tokens, prompt_count,
                    duration_sum_ms, duration_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(period_start, project_id, branch) DO UPDATE SET
                   session_count  = session_count  + excluded.session_count,
                   total_tokens   = total_tokens   + excluded.total_tokens,
                   prompt_count   = prompt_count   + excluded.prompt_count,
                   duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
                   duration_count = duration_count + excluded.duration_count"
            );
            sqlx::query(&sql)
                .bind(period)
                .bind(pid)
                .bind(br)
                .bind(sign)
                .bind(total_tokens)
                .bind(prompt_delta)
                .bind(duration_ms_delta)
                .bind(duration_count_delta)
                .execute(&mut **tx)
                .await?;
        }
    }

    // Model dimension
    if let Some(m) = &model {
        for (table, period) in [
            ("daily_model_stats", day_start),
            ("weekly_model_stats", week_start),
            ("monthly_model_stats", month_start),
        ] {
            let sql = format!(
                "INSERT INTO {table} (period_start, model_id, session_count,
                    total_tokens, prompt_count, duration_sum_ms, duration_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(period_start, model_id) DO UPDATE SET
                   session_count  = session_count  + excluded.session_count,
                   total_tokens   = total_tokens   + excluded.total_tokens,
                   prompt_count   = prompt_count   + excluded.prompt_count,
                   duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
                   duration_count = duration_count + excluded.duration_count"
            );
            sqlx::query(&sql)
                .bind(period)
                .bind(m)
                .bind(sign)
                .bind(total_tokens)
                .bind(prompt_delta)
                .bind(duration_ms_delta)
                .bind(duration_count_delta)
                .execute(&mut **tx)
                .await?;
        }
    }

    // Category dimension — only if session is currently classified.
    if let Some(cat) = &category_l1 {
        for (table, period) in [
            ("daily_category_stats", day_start),
            ("weekly_category_stats", week_start),
            ("monthly_category_stats", month_start),
        ] {
            let sql = format!(
                "INSERT INTO {table} (period_start, category_l1, session_count,
                    total_tokens, prompt_count, duration_sum_ms, duration_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(period_start, category_l1) DO UPDATE SET
                   session_count  = session_count  + excluded.session_count,
                   total_tokens   = total_tokens   + excluded.total_tokens,
                   prompt_count   = prompt_count   + excluded.prompt_count,
                   duration_sum_ms = duration_sum_ms + excluded.duration_sum_ms,
                   duration_count = duration_count + excluded.duration_count"
            );
            sqlx::query(&sql)
                .bind(period)
                .bind(cat)
                .bind(sign)
                .bind(total_tokens)
                .bind(prompt_delta)
                .bind(duration_ms_delta)
                .bind(duration_count_delta)
                .execute(&mut **tx)
                .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    async fn seed_session_with_stats(db: &Database, sid: &str, category_l1: Option<&str>) {
        sqlx::query(
            "INSERT INTO session_stats (session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, total_output_tokens, user_prompt_count,
                duration_seconds, last_message_at, project_id)
             VALUES (?1, X'00', 0, 1, 1, 0,
                     100, 200, 5, 60, 1700000000000, 'p1')",
        )
        .bind(sid)
        .execute(db.pool())
        .await
        .unwrap();
        if let Some(cat) = category_l1 {
            sqlx::query(
                "INSERT INTO session_flags (session_id, applied_seq, category_l1)
                 VALUES (?1, 0, ?2)",
            )
            .bind(sid)
            .bind(cat)
            .execute(db.pool())
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn classify_delta_adds_to_new_category() {
        let db = Database::new_in_memory().await.unwrap();
        seed_session_with_stats(&db, "s-fd-1", None).await;

        let delta = FlagDelta::Classify {
            session_id: "s-fd-1".to_string(),
            before_category_l1: None,
            after_category_l1: Some("engineering".to_string()),
            at_ms: 1_700_000_000_000,
        };

        let mut tx = db.pool().begin().await.unwrap();
        apply_flag_delta_tx(&mut tx, &delta).await.unwrap();
        tx.commit().await.unwrap();

        let (session_count, total_tokens): (i64, i64) = sqlx::query_as(
            "SELECT session_count, total_tokens FROM daily_category_stats
             WHERE category_l1 = 'engineering'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(session_count, 1);
        assert_eq!(total_tokens, 300);
    }

    #[tokio::test]
    async fn classify_delta_subtracts_before_and_adds_after() {
        let db = Database::new_in_memory().await.unwrap();
        seed_session_with_stats(&db, "s-fd-2", None).await;

        // Seed existing "old" bucket as if a prior classify already fanned out.
        let mut tx = db.pool().begin().await.unwrap();
        apply_flag_delta_tx(
            &mut tx,
            &FlagDelta::Classify {
                session_id: "s-fd-2".to_string(),
                before_category_l1: None,
                after_category_l1: Some("old".to_string()),
                at_ms: 1_700_000_000_000,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Reclassify: old → new. Old should be -1/-300, new should be +1/+300.
        let mut tx = db.pool().begin().await.unwrap();
        apply_flag_delta_tx(
            &mut tx,
            &FlagDelta::Classify {
                session_id: "s-fd-2".to_string(),
                before_category_l1: Some("old".to_string()),
                after_category_l1: Some("new".to_string()),
                at_ms: 1_700_000_000_000,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let (old_count, old_tokens): (i64, i64) = sqlx::query_as(
            "SELECT session_count, total_tokens FROM daily_category_stats
             WHERE category_l1 = 'old'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(old_count, 0);
        assert_eq!(old_tokens, 0);

        let (new_count, new_tokens): (i64, i64) = sqlx::query_as(
            "SELECT session_count, total_tokens FROM daily_category_stats
             WHERE category_l1 = 'new'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(new_count, 1);
        assert_eq!(new_tokens, 300);
    }

    #[tokio::test]
    async fn archive_delta_subtracts_across_all_dimensions() {
        let db = Database::new_in_memory().await.unwrap();
        seed_session_with_stats(&db, "s-fd-3", Some("engineering")).await;

        // Prime the global/project/category buckets with +1 so we can observe -1 land.
        let mut tx = db.pool().begin().await.unwrap();
        apply_flag_delta_tx(
            &mut tx,
            &FlagDelta::Unarchive {
                session_id: "s-fd-3".to_string(),
                at_ms: 1_700_000_000_000,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let (global_count,): (i64,) =
            sqlx::query_as("SELECT session_count FROM daily_global_stats WHERE period_start > 0")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(global_count, 1);

        let mut tx = db.pool().begin().await.unwrap();
        apply_flag_delta_tx(
            &mut tx,
            &FlagDelta::Archive {
                session_id: "s-fd-3".to_string(),
                at_ms: 1_700_000_000_000,
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let (global_after,): (i64,) =
            sqlx::query_as("SELECT session_count FROM daily_global_stats WHERE period_start > 0")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(global_after, 0);
    }
}
