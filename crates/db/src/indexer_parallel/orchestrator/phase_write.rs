// crates/db/src/indexer_parallel/orchestrator/phase_write.rs
// Phase 2: sequential chunked SQLite writes (single writer, no parallelism).

use std::collections::HashMap;
use std::sync::Arc;

use crate::Database;

use super::super::types::IndexedSession;
use super::super::writer::check_token_reconciliation;

/// Write parsed sessions to SQLite in chunked transactions of 200.
///
/// Writes session upserts, topology, turns, invocations, and hook events.
/// Fires `on_file_done` after each chunk is committed.
/// Returns the number of sessions written.
#[tracing::instrument(skip_all)]
pub(crate) async fn run_phase_write<F>(
    db: &Database,
    indexed_sessions: &[IndexedSession],
    seen_at: i64,
    on_file_done: &Arc<F>,
) -> Result<usize, String>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    let mut indexed_count: usize = 0;

    for chunk in indexed_sessions.chunks(200) {
        let mut tx = db
            .pool()
            .begin()
            .await
            .map_err(|e| format!("Failed to begin write transaction: {}", e))?;

        sqlx::query("PRAGMA busy_timeout = 30000")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;

        // Dedup and UPSERT models FIRST -- turns.model_id has FK to models.id
        let mut chunk_models: HashMap<String, ()> = HashMap::new();
        for session in chunk {
            for model in &session.models_seen {
                chunk_models.entry(model.clone()).or_insert(());
            }
        }
        if !chunk_models.is_empty() {
            let model_ids: Vec<String> = chunk_models.into_keys().collect();
            crate::queries::batch_upsert_models_tx(&mut tx, &model_ids, seen_at)
                .await
                .map_err(|e| format!("Failed to upsert models: {}", e))?;
        }

        // Per-session writes: session upsert, topology, turns, invocations
        for session in chunk {
            write_single_session(&mut tx, session).await?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit write transaction: {}", e))?;

        let chunk_session_ids: Vec<String> = chunk.iter().map(|s| s.parsed.id.clone()).collect();
        check_token_reconciliation(db, &chunk_session_ids).await;

        for session in chunk {
            on_file_done(&session.parsed.id);
        }

        indexed_count += chunk.len();

        tokio::task::yield_now().await;
    }

    Ok(indexed_count)
}

/// Write a single session's data within an existing transaction.
async fn write_single_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session: &IndexedSession,
) -> Result<(), String> {
    sqlx::query("DELETE FROM turns WHERE session_id = ?1")
        .bind(&session.parsed.id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("DELETE turns for {}: {}", session.parsed.id, e))?;

    sqlx::query("DELETE FROM invocations WHERE session_id = ?1")
        .bind(&session.parsed.id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("DELETE invocations for {}: {}", session.parsed.id, e))?;

    crate::queries::sessions::execute_upsert_parsed_session(&mut **tx, &session.parsed)
        .await
        .map_err(|e| format!("Failed to upsert session {}: {}", session.parsed.id, e))?;

    if session.cwd.is_some() {
        sqlx::query(
            "UPDATE sessions SET \
             session_cwd = COALESCE(?1, session_cwd), \
             git_root = COALESCE(?2, git_root) \
             WHERE id = ?3",
        )
        .bind(session.cwd.as_deref())
        .bind(session.git_root.as_deref())
        .bind(&session.parsed.id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("Failed to update topology {}: {}", session.parsed.id, e))?;
    }

    if !session.turns.is_empty() {
        crate::queries::batch_insert_turns_tx(tx, &session.parsed.id, &session.turns)
            .await
            .map_err(|e| format!("Failed to insert turns for {}: {}", session.parsed.id, e))?;
    }

    if !session.classified_invocations.is_empty() {
        crate::queries::batch_insert_invocations_tx(tx, &session.classified_invocations)
            .await
            .map_err(|e| {
                format!(
                    "Failed to insert invocations for {}: {}",
                    session.parsed.id, e
                )
            })?;
    }

    if !session.hook_progress_events.is_empty() {
        let mut events = session.hook_progress_events.clone();
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.event_name.cmp(&b.event_name))
                .then(a.tool_name.cmp(&b.tool_name))
                .then(a.source.cmp(&b.source))
        });
        events.dedup_by(|a, b| {
            a.timestamp == b.timestamp
                && a.event_name == b.event_name
                && a.tool_name == b.tool_name
                && a.source == b.source
        });

        crate::queries::hook_events::insert_hook_events_tx(tx, &session.parsed.id, &events)
            .await
            .map_err(|e| {
                format!(
                    "Failed to insert hook events for {}: {}",
                    session.parsed.id, e
                )
            })?;
    }

    Ok(())
}
