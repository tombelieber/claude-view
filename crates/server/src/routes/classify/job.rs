//! Background classification job execution.

use std::sync::Arc;

use claude_view_core::classification::{ClassificationInput, BATCH_SIZE};
use claude_view_core::llm::{ClassificationRequest, LlmProvider};

use crate::state::AppState;

/// Run the classification loop in the background.
///
/// Sessions are classified individually via the LLM provider, grouped into
/// batches only for progress tracking and database writes.
pub(super) async fn run_classification(state: Arc<AppState>, db_job_id: i64, mode: &str) {
    let classify_state = &state.classify;
    let db = &state.db;

    // Fetch all sessions to classify
    let sessions = match mode {
        "all" => db.get_all_sessions_for_classification(100_000).await,
        _ => db.get_unclassified_sessions(100_000).await,
    };

    let sessions = match sessions {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Failed to fetch sessions: {}", e);
            tracing::error!("{}", msg);
            classify_state.set_failed(msg.clone());
            if let Err(e) = db.fail_classification_job(db_job_id, &msg).await {
                tracing::error!(error = %e, "Failed to record classification job failure");
            }
            return;
        }
    };

    let total = sessions.len();
    if total == 0 {
        classify_state.set_completed();
        if let Err(e) = db.complete_classification_job(db_job_id, None).await {
            tracing::error!(error = %e, "Failed to complete classification job with 0 sessions");
        }
        return;
    }

    // Build classification inputs
    let inputs: Vec<ClassificationInput> = sessions
        .iter()
        .map(|(id, preview, skills_json)| {
            let skills: Vec<String> = serde_json::from_str(skills_json).unwrap_or_else(|e| {
                tracing::warn!(session_id = %id, error = %e, "Malformed skills JSON, using empty array");
                vec![]
            });
            ClassificationInput {
                session_id: id.clone(),
                preview: preview.clone(),
                skills_used: skills,
            }
        })
        .collect();

    // Process in batches
    let mut classified_total = 0u64;
    let mut failed_total = 0u64;
    let mut batch_num = 0usize;
    let mut total_tokens_used: i64 = 0;
    let mut total_cost_usd: f64 = 0.0;
    let mut tokens_known = true;
    let mut cost_known = true;

    for batch in inputs.chunks(BATCH_SIZE) {
        // Check for cancellation
        if classify_state.is_cancel_requested() {
            classify_state.set_cancelled();
            if let Err(e) = db.cancel_classification_job(db_job_id).await {
                tracing::error!(error = %e, "Failed to cancel classification job");
            }
            if let Err(e) = db
                .update_classification_job_progress(
                    db_job_id,
                    classified_total as i64,
                    0,
                    failed_total as i64,
                    None,
                )
                .await
            {
                tracing::error!(error = %e, "Failed to update cancelled job progress");
            }
            return;
        }

        batch_num += 1;
        classify_state.set_current_batch(format!("Batch {} ({} sessions)", batch_num, batch.len()));

        tracing::debug!(batch_num, batch_size = batch.len(), "Processing batch");

        // For the MVP, classify each session individually using the existing provider
        let mut batch_updates: Vec<(String, String, String, String, f64, String)> = Vec::new();

        for input in batch {
            if classify_state.is_cancel_requested() {
                break;
            }

            let single_provider = match crate::routes::settings::create_llm_provider(db).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create LLM provider");
                    continue;
                }
            };
            let single_request = ClassificationRequest {
                session_id: input.session_id.clone(),
                first_prompt: input.preview.clone(),
                files_touched: vec![],
                skills_used: input.skills_used.clone(),
            };

            match single_provider.classify(single_request).await {
                Ok(resp) => {
                    batch_updates.push((
                        input.session_id.clone(),
                        resp.category_l1.clone(),
                        resp.category_l2.clone(),
                        resp.category_l3.clone(),
                        resp.confidence,
                        "claude-cli".to_string(),
                    ));
                    classified_total += 1;

                    match resp.total_tokens_used() {
                        Some(tokens) => {
                            let tokens_i64 = tokens.min(i64::MAX as u64) as i64;
                            total_tokens_used = total_tokens_used.saturating_add(tokens_i64);
                        }
                        None => {
                            tokens_known = false;
                        }
                    }

                    if let Some(cost_usd) = resp.total_cost_usd {
                        if !cost_usd.is_finite() || cost_usd < 0.0 {
                            cost_known = false;
                            tracing::warn!(
                                session_id = %input.session_id,
                                cost_usd,
                                "Invalid classification cost telemetry; falling back to NULL"
                            );
                            continue;
                        }
                        total_cost_usd += cost_usd;
                    } else {
                        cost_known = false;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %input.session_id,
                        error = %e,
                        "Single session classification failed"
                    );
                    failed_total += 1;
                    classify_state.increment_errors();
                    tokens_known = false;
                    cost_known = false;
                }
            }
        }

        // Batch write to database (single transaction)
        let batch_persisted = if !batch_updates.is_empty() {
            match db
                .batch_update_session_classifications(&batch_updates)
                .await
            {
                Ok(_) => true,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to persist batch classifications");
                    // Undo the per-item classified_total increments since nothing was persisted
                    let batch_classified = batch_updates.len() as u64;
                    if classified_total >= batch_classified {
                        classified_total -= batch_classified;
                    }
                    failed_total += batch_classified;
                    false
                }
            }
        } else {
            true
        };

        // Only report progress for successfully persisted batches
        if batch_persisted {
            classify_state.increment_classified(batch_updates.len() as u64);
        }

        let tokens_for_progress = if tokens_known {
            Some(total_tokens_used)
        } else {
            None
        };

        // Update job progress in database
        if let Err(e) = db
            .update_classification_job_progress(
                db_job_id,
                classified_total as i64,
                0,
                failed_total as i64,
                tokens_for_progress,
            )
            .await
        {
            tracing::error!(error = %e, "Failed to update classification progress");
        }
    }

    // Job completed
    classify_state.set_completed();
    let actual_cost_cents = actual_cost_cents_from_total(total_cost_usd, cost_known);
    if let Err(e) = db
        .complete_classification_job(db_job_id, actual_cost_cents)
        .await
    {
        tracing::error!(error = %e, "Failed to complete classification job");
    }
    let tokens_for_progress = if tokens_known {
        Some(total_tokens_used)
    } else {
        None
    };
    if let Err(e) = db
        .update_classification_job_progress(
            db_job_id,
            classified_total as i64,
            0,
            failed_total as i64,
            tokens_for_progress,
        )
        .await
    {
        tracing::error!(error = %e, "Failed to update final job progress");
    }

    tracing::info!(
        classified = classified_total,
        failed = failed_total,
        "Classification job completed"
    );
}

pub(super) fn actual_cost_cents_from_total(total_cost_usd: f64, cost_known: bool) -> Option<i64> {
    if !cost_known || !total_cost_usd.is_finite() {
        return None;
    }

    let cents = (total_cost_usd * 100.0).round();
    if cents > i64::MAX as f64 {
        tracing::warn!(
            total_cost_usd,
            "Classification cost overflow; storing NULL actual_cost_cents"
        );
        return None;
    }
    if cents < i64::MIN as f64 {
        tracing::warn!(
            total_cost_usd,
            "Classification cost underflow; storing NULL actual_cost_cents"
        );
        return None;
    }

    Some(cents as i64)
}
