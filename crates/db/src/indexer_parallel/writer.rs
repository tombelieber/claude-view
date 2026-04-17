// crates/db/src/indexer_parallel/writer.rs
// Token reconciliation check used after chunked writes in the orchestrator.

use crate::Database;

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
