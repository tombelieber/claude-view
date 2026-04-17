// crates/db/src/indexer_parallel/writer.rs
// DB write operations: write_results_sqlx, check_token_reconciliation.

use crate::Database;

use super::cost::*;
use super::helpers::*;
use super::types::*;

/// Write deep index results using sqlx async transactions.
#[allow(deprecated)]
#[deprecated(
    note = "use test_support::seed_session_via_upsert — scheduled for removal in Phase 0 step 5"
)]
pub(crate) async fn write_results_sqlx(
    db: &Database,
    results: &[DeepIndexResult],
) -> Result<usize, String> {
    let mut tx = db
        .pool()
        .begin()
        .await
        .map_err(|e| format!("Failed to begin write transaction: {}", e))?;

    let seen_at = chrono::Utc::now().timestamp();
    let pricing = load_indexing_pricing();

    for result in results {
        let meta = &result.parse_result.deep;

        let files_touched =
            serde_json::to_string(&meta.files_touched).unwrap_or_else(|_| "[]".to_string());
        let skills_used =
            serde_json::to_string(&meta.skills_used).unwrap_or_else(|_| "[]".to_string());
        let files_read =
            serde_json::to_string(&meta.files_read).unwrap_or_else(|_| "[]".to_string());
        let files_edited =
            serde_json::to_string(&meta.files_edited).unwrap_or_else(|_| "[]".to_string());

        let commit_invocations =
            extract_commit_skill_invocations(&result.parse_result.raw_invocations);
        let commit_count = commit_invocations.len() as i32;

        let (dur_avg, dur_max, dur_total) = if meta.turn_durations_ms.is_empty() {
            (None, None, None)
        } else {
            let total: u64 = meta.turn_durations_ms.iter().sum();
            let max = *meta
                .turn_durations_ms
                .iter()
                .max()
                .expect("non-empty checked above");
            let count = meta.turn_durations_ms.len() as u64;
            let avg = (total + count / 2) / count;
            (Some(avg as i64), Some(max as i64), Some(total as i64))
        };

        let work_type_input = claude_view_core::ClassificationInput::new(
            meta.duration_seconds,
            meta.turn_count as u32,
            meta.files_edited_count,
            meta.ai_lines_added,
            meta.skills_used.clone(),
        );
        let work_type = claude_view_core::classify_work_type(&work_type_input);

        let primary_model = compute_primary_model(&result.parse_result.turns);

        crate::queries::update_session_deep_fields_tx(
            &mut tx,
            &result.session_id,
            &meta.last_message,
            meta.turn_count as i32,
            meta.tool_counts.edit as i32,
            meta.tool_counts.read as i32,
            meta.tool_counts.bash as i32,
            meta.tool_counts.write as i32,
            &files_touched,
            &skills_used,
            meta.user_prompt_count as i32,
            meta.api_call_count as i32,
            meta.tool_call_count as i32,
            &files_read,
            &files_edited,
            meta.files_read_count as i32,
            meta.files_edited_count as i32,
            meta.reedited_files_count as i32,
            meta.duration_seconds as i32,
            commit_count,
            meta.first_timestamp,
            meta.total_input_tokens as i64,
            meta.total_output_tokens as i64,
            meta.cache_read_tokens as i64,
            meta.cache_creation_tokens as i64,
            meta.thinking_block_count as i32,
            dur_avg,
            dur_max,
            dur_total,
            meta.api_error_count as i32,
            meta.api_retry_count as i32,
            meta.compaction_count as i32,
            meta.hook_blocked_count as i32,
            meta.agent_spawn_count as i32,
            meta.bash_progress_count as i32,
            meta.hook_progress_count as i32,
            meta.mcp_progress_count as i32,
            meta.summary_text.as_deref(),
            CURRENT_PARSE_VERSION,
            result.file_size,
            result.file_mtime,
            result.parse_result.lines_added as i32,
            result.parse_result.lines_removed as i32,
            1,
            meta.ai_lines_added as i32,
            meta.ai_lines_removed as i32,
            Some(work_type.as_str()),
            result.parse_result.git_branch.as_deref(),
            primary_model.as_deref(),
            meta.last_timestamp,
            meta.first_user_prompt.as_deref(),
            meta.total_task_time_seconds as i32,
            meta.longest_task_seconds
                .map(|v| v.min(i32::MAX as u32) as i32),
            meta.longest_task_preview.as_deref(),
            calculate_per_turn_cost(&result.parse_result.turns, &pricing),
        )
        .await
        .map_err(|e| {
            format!(
                "Failed to update deep fields for {}: {}",
                result.session_id, e
            )
        })?;

        sqlx::query("DELETE FROM turns WHERE session_id = ?1")
            .bind(&result.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("DELETE turns for {} error: {}", result.session_id, e))?;

        sqlx::query("DELETE FROM invocations WHERE session_id = ?1")
            .bind(&result.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("DELETE invocations for {} error: {}", result.session_id, e))?;

        if !result.classified_invocations.is_empty() {
            crate::queries::batch_insert_invocations_tx(&mut tx, &result.classified_invocations)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to insert invocations for {}: {}",
                        result.session_id, e
                    )
                })?;
        }

        if !result.parse_result.turns.is_empty() {
            crate::queries::batch_upsert_models_tx(
                &mut tx,
                &result.parse_result.models_seen,
                seen_at,
            )
            .await
            .map_err(|e| format!("Failed to upsert models for {}: {}", result.session_id, e))?;

            crate::queries::batch_insert_turns_tx(
                &mut tx,
                &result.session_id,
                &result.parse_result.turns,
            )
            .await
            .map_err(|e| format!("Failed to insert turns for {}: {}", result.session_id, e))?;
        }

        if !result.parse_result.deep.hook_progress_events.is_empty() {
            let mut events = result.parse_result.deep.hook_progress_events.clone();
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

            crate::queries::hook_events::insert_hook_events_tx(
                &mut tx,
                &result.session_id,
                &events,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to insert hook events for {}: {}",
                    result.session_id, e
                )
            })?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit write transaction: {}", e))?;

    let session_ids: Vec<String> = results.iter().map(|r| r.session_id.clone()).collect();
    check_token_reconciliation(db, &session_ids).await;

    Ok(results.len())
}

/// Check for token divergence between sessions and turns tables.
pub(crate) async fn check_token_reconciliation(db: &Database, session_ids: &[String]) {
    if session_ids.is_empty() {
        return;
    }

    let sample: Vec<&str> = session_ids.iter().take(10).map(|s| s.as_str()).collect();

    for session_id in sample {
        let result: Option<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT
                s.total_input_tokens,
                s.total_output_tokens,
                (SELECT COALESCE(SUM(input_tokens), 0) FROM turns WHERE session_id = s.id),
                (SELECT COALESCE(SUM(output_tokens), 0) FROM turns WHERE session_id = s.id)
            FROM sessions s WHERE s.id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(db.pool())
        .await
        .ok()
        .flatten();

        if let Some((Some(sess_input), Some(sess_output), Some(turn_input), Some(turn_output))) =
            result
        {
            let input_drift = (sess_input - turn_input).abs();
            let output_drift = (sess_output - turn_output).abs();

            if input_drift > 0 || output_drift > 0 {
                tracing::warn!(
                    session_id = %session_id,
                    sess_input_tokens = sess_input,
                    turn_sum_input_tokens = turn_input,
                    input_drift = input_drift,
                    sess_output_tokens = sess_output,
                    turn_sum_output_tokens = turn_output,
                    output_drift = output_drift,
                    "Token reconciliation: session vs turns divergence detected"
                );
            }
        }
    }
}
