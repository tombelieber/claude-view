//! stage_c_outbox durable-delivery queries.
//!
//! Writes in the fold TX (see `crates/db/src/fold/apply.rs`), reads by
//! the drainer task (see `super::drainer`).

use sqlx::{Executor, Sqlite};

use super::flag_delta::FlagDelta;
use crate::{DbError, DbResult};

/// Insert one FlagDelta into stage_c_outbox. Run inside the fold TX
/// so the outbox row commits atomically with the `session_flags`
/// UPSERT.
pub(crate) async fn insert_flag_delta_tx<'e, E>(executor: E, delta: &FlagDelta) -> DbResult<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let payload = serde_json::to_string(delta).map_err(|e| {
        DbError::Sqlx(sqlx::Error::Protocol(format!(
            "failed to serialise FlagDelta: {e}"
        )))
    })?;
    let seq = sqlx::query_scalar::<_, i64>(
        "INSERT INTO stage_c_outbox (delta_type, payload_json, applied_at)
         VALUES (?1, ?2, NULL) RETURNING seq",
    )
    .bind(delta.delta_type())
    .bind(&payload)
    .fetch_one(executor)
    .await?;
    Ok(seq)
}

/// Pending FlagDelta row. `seq` + `payload` are sufficient; drainer
/// deserialises the payload to decide the apply path.
#[derive(Debug)]
pub(crate) struct PendingOutboxRow {
    pub seq: i64,
    pub payload: String,
}

/// Select up to `limit` pending outbox rows ordered by seq ASC.
pub(crate) async fn select_pending(
    pool: &sqlx::SqlitePool,
    limit: i64,
) -> DbResult<Vec<PendingOutboxRow>> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT seq, payload_json FROM stage_c_outbox
         WHERE applied_at IS NULL
         ORDER BY seq ASC
         LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(seq, payload)| PendingOutboxRow { seq, payload })
        .collect())
}

/// Mark a set of outbox rows applied, in a single UPDATE statement.
/// Called by the drainer AFTER its compensating rollup writes commit.
pub(crate) async fn mark_applied(
    pool: &sqlx::SqlitePool,
    seqs: &[i64],
    at_ms: i64,
) -> DbResult<()> {
    if seqs.is_empty() {
        return Ok(());
    }
    let placeholders = seqs
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 2))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("UPDATE stage_c_outbox SET applied_at = ?1 WHERE seq IN ({placeholders})");
    let mut q = sqlx::query(&sql).bind(at_ms);
    for s in seqs {
        q = q.bind(s);
    }
    q.execute(pool).await?;
    Ok(())
}
