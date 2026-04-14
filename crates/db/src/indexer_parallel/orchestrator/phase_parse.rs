// crates/db/src/indexer_parallel/orchestrator/phase_parse.rs
// Phase 1: parallel CPU-bound JSONL parsing with zero DB writes.

use claude_view_core::{
    classify_work_type, resolve_worktree_parent, ClassificationInput, ClassifyResult, Registry,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use claude_view_core::pricing::ModelPricing;

use super::super::cost::*;
use super::super::helpers::*;
use super::super::parser::*;
use super::super::types::*;

/// Spawn parallel parse tasks for all discovered `.jsonl` files.
///
/// Returns `(indexed_sessions, skipped_count)`. Fires `on_file_done` for each
/// skipped or errored file; parsed sessions are returned for Phase 2.
#[tracing::instrument(skip_all)]
pub(crate) async fn run_phase_parse<F>(
    files: Vec<(PathBuf, String, String)>,
    hints: &HashMap<String, IndexHints>,
    existing_map: &HashMap<String, (Option<i64>, Option<i64>, i32, Option<String>)>,
    registry: Option<Arc<Registry>>,
    force_search_reindex: bool,
    source_docs_validation_enabled: bool,
    on_file_done: Arc<F>,
) -> Result<(Vec<IndexedSession>, usize), String>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    let pricing = Arc::new(load_indexing_pricing());

    let semaphore = Arc::new(tokio::sync::Semaphore::new(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    ));

    let skipped = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(files.len());
    for (path, project_encoded, session_id) in files {
        let sem = semaphore.clone();
        let hints = hints.clone();
        let existing_map = existing_map.clone();
        let skipped = skipped.clone();
        let registry = registry.clone();
        let force_reindex = force_search_reindex;
        let pricing = pricing.clone();
        let validate_source_docs = source_docs_validation_enabled;

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("Semaphore error: {}", e))?;

            // 1. stat() the file for current size + mtime
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    return Ok::<(Option<IndexedSession>, String), String>((None, session_id))
                }
            };
            let current_size = metadata.len() as i64;
            let current_mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // 2a. If session is archived, skip re-indexing entirely.
            if let Some((_, _, _, archived_at)) = existing_map.get(&session_id) {
                if archived_at.is_some() {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return Ok((None, session_id));
                }
            }

            // 2b. Check staleness against DB (force_search_reindex bypasses)
            if !force_reindex {
                if let Some((Some(stored_size), Some(stored_mtime), pv, _)) =
                    existing_map.get(&session_id)
                {
                    if *stored_size == current_size
                        && *stored_mtime == current_mtime
                        && *pv >= CURRENT_PARSE_VERSION
                    {
                        skipped.fetch_add(1, Ordering::Relaxed);
                        return Ok((None, session_id));
                    }
                }
            }

            // 3. Parse the JSONL file (blocking I/O in spawn_blocking)
            let path_for_parse = path.clone();
            let mut parse_result =
                tokio::task::spawn_blocking(move || parse_file_bytes(&path_for_parse))
                    .await
                    .map_err(|e| format!("spawn_blocking join error: {}", e))?;
            merge_subagent_workload(&path, &mut parse_result);

            let meta = &parse_result.deep;

            // Skip files with no parseable timestamps
            if meta.last_timestamp.is_none() {
                tracing::debug!(path = %path.display(), "Skipping file with no timestamps");
                skipped.fetch_add(1, Ordering::Relaxed);
                return Ok((None, session_id));
            }

            // 4. Resolve project info from cwd (authoritative) or hints
            let hint = hints.get(&session_id);
            let cwd = parse_result.cwd.as_deref();

            let (effective_encoded, resolved) =
                if let Some(parent_encoded) = resolve_worktree_parent(&project_encoded) {
                    let r = claude_view_core::discovery::resolve_project_path_with_cwd(
                        &parent_encoded,
                        cwd,
                    );
                    (parent_encoded, r)
                } else {
                    let r = claude_view_core::discovery::resolve_project_path_with_cwd(
                        &project_encoded,
                        cwd,
                    );
                    (project_encoded.clone(), r)
                };

            let project_display_name = hint
                .and_then(|h| h.project_display_name.clone())
                .unwrap_or_else(|| resolved.display_name.clone());
            let project_path = hint
                .and_then(|h| h.project_path.clone())
                .unwrap_or_else(|| resolved.full_path.clone());
            let is_sidechain = hint.and_then(|h| h.is_sidechain).unwrap_or(false);
            let git_branch_hint = hint.and_then(|h| h.git_branch.clone());
            let summary_hint = hint.and_then(|h| h.summary.clone());

            let session = build_indexed_session(
                &path,
                &session_id,
                &effective_encoded,
                project_display_name,
                project_path,
                is_sidechain,
                git_branch_hint,
                summary_hint,
                current_size,
                current_mtime,
                &mut parse_result,
                registry.as_ref(),
                validate_source_docs,
                &pricing,
            );

            let sid = session_id;
            Ok((Some(session), sid))
        });

        handles.push(handle);
    }

    // Collect all parse results.
    let mut indexed_sessions: Vec<IndexedSession> = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(Ok((Some(session), _id))) => {
                indexed_sessions.push(session);
            }
            Ok(Ok((None, id))) => {
                on_file_done(&id);
            }
            Ok(Err(e)) => {
                on_file_done("_error");
                tracing::warn!("scan_and_index_all parse error: {}", e);
            }
            Err(e) => {
                on_file_done("_error");
                tracing::warn!("scan_and_index_all join error: {}", e);
            }
        }
    }

    Ok((indexed_sessions, skipped.load(Ordering::Relaxed)))
}

/// Build the final `IndexedSession` from parse results and resolved metadata.
#[allow(clippy::too_many_arguments)]
fn build_indexed_session(
    path: &Path,
    session_id: &str,
    effective_encoded: &str,
    project_display_name: String,
    project_path: String,
    is_sidechain: bool,
    git_branch_hint: Option<String>,
    summary_hint: Option<String>,
    current_size: i64,
    current_mtime: i64,
    parse_result: &mut ParseResult,
    registry: Option<&Arc<Registry>>,
    validate_source_docs: bool,
    pricing: &HashMap<String, ModelPricing>,
) -> IndexedSession {
    let meta = &parse_result.deep;

    // Compute derived fields
    let commit_invocations = extract_commit_skill_invocations(&parse_result.raw_invocations);
    let commit_count = commit_invocations.len() as i32;

    let (dur_avg, dur_max, dur_total) = if meta.turn_durations_ms.is_empty() {
        (None, None, None)
    } else {
        let total: u64 = meta.turn_durations_ms.iter().sum();
        let max = *meta.turn_durations_ms.iter().max().unwrap();
        let count = meta.turn_durations_ms.len() as u64;
        let avg = (total + count / 2) / count;
        (Some(avg as i64), Some(max as i64), Some(total as i64))
    };

    let work_type_input = ClassificationInput::new(
        meta.duration_seconds,
        meta.turn_count as u32,
        meta.files_edited_count,
        meta.ai_lines_added,
        meta.skills_used.clone(),
    );
    let work_type = classify_work_type(&work_type_input);
    let primary_model = compute_primary_model(&parse_result.turns);

    let files_touched_json =
        serde_json::to_string(&meta.files_touched).unwrap_or_else(|_| "[]".to_string());
    let skills_used_json =
        serde_json::to_string(&meta.skills_used).unwrap_or_else(|_| "[]".to_string());
    let files_read_json =
        serde_json::to_string(&meta.files_read).unwrap_or_else(|_| "[]".to_string());
    let files_edited_json =
        serde_json::to_string(&meta.files_edited).unwrap_or_else(|_| "[]".to_string());

    let git_branch = parse_result.git_branch.clone().or(git_branch_hint);
    let summary = meta.summary_text.clone().or(summary_hint);
    let preview = meta.first_user_prompt.clone().unwrap_or_default();
    let message_count = (meta.user_prompt_count + meta.api_call_count) as i32;

    // Classify invocations (CPU work, no DB)
    let classified = if let Some(registry) = registry {
        parse_result
            .raw_invocations
            .iter()
            .filter_map(|raw| {
                match claude_view_core::classify_tool_use(&raw.name, &raw.input, registry) {
                    ClassifyResult::Valid { invocable_id, .. } => Some((
                        path.to_string_lossy().to_string(),
                        raw.byte_offset as i64,
                        invocable_id,
                        session_id.to_string(),
                        String::new(),
                        raw.timestamp,
                    )),
                    _ => None,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Resolve cwd/git_root for topology
    let cwd_owned = parse_result.cwd.clone();
    let cwd = parse_result.cwd.as_deref();
    let git_root = cwd
        .and_then(claude_view_core::discovery::infer_git_root_from_worktree_path)
        .map(|s| s.to_string());

    // Compute cost per-turn (each turn = one API call)
    let total_cost_usd = calculate_per_turn_cost(&parse_result.turns, pricing);

    // Build ParsedSession
    let parsed = ParsedSession {
        id: session_id.to_string(),
        project_id: effective_encoded.to_string(),
        project_display_name,
        project_path,
        file_path: path.to_string_lossy().to_string(),
        preview,
        summary,
        message_count,
        last_message_at: meta.last_timestamp.unwrap_or(0),
        first_message_at: meta.first_timestamp.unwrap_or(0),
        git_branch,
        is_sidechain,
        size_bytes: current_size,
        last_message: meta.last_message.clone(),
        turn_count: meta.turn_count as i32,
        tool_counts_edit: meta.tool_counts.edit as i32,
        tool_counts_read: meta.tool_counts.read as i32,
        tool_counts_bash: meta.tool_counts.bash as i32,
        tool_counts_write: meta.tool_counts.write as i32,
        files_touched: files_touched_json,
        skills_used: skills_used_json,
        user_prompt_count: meta.user_prompt_count as i32,
        api_call_count: meta.api_call_count as i32,
        tool_call_count: meta.tool_call_count as i32,
        files_read: files_read_json,
        files_edited: files_edited_json,
        files_read_count: meta.files_read_count as i32,
        files_edited_count: meta.files_edited_count as i32,
        reedited_files_count: meta.reedited_files_count as i32,
        duration_seconds: meta.duration_seconds as i64,
        commit_count,
        total_input_tokens: meta.total_input_tokens as i64,
        total_output_tokens: meta.total_output_tokens as i64,
        cache_read_tokens: meta.cache_read_tokens as i64,
        cache_creation_tokens: meta.cache_creation_tokens as i64,
        thinking_block_count: meta.thinking_block_count as i32,
        turn_duration_avg_ms: dur_avg,
        turn_duration_max_ms: dur_max,
        turn_duration_total_ms: dur_total,
        api_error_count: meta.api_error_count as i32,
        api_retry_count: meta.api_retry_count as i32,
        compaction_count: meta.compaction_count as i32,
        hook_blocked_count: meta.hook_blocked_count as i32,
        agent_spawn_count: meta.agent_spawn_count as i32,
        bash_progress_count: meta.bash_progress_count as i32,
        hook_progress_count: meta.hook_progress_count as i32,
        mcp_progress_count: meta.mcp_progress_count as i32,
        summary_text: meta.summary_text.clone(),
        parse_version: CURRENT_PARSE_VERSION,
        file_size_at_index: current_size,
        file_mtime_at_index: current_mtime,
        lines_added: parse_result.lines_added as i64,
        lines_removed: parse_result.lines_removed as i64,
        loc_source: 1,
        ai_lines_added: meta.ai_lines_added as i64,
        ai_lines_removed: meta.ai_lines_removed as i64,
        work_type: Some(work_type.as_str().to_string()),
        primary_model,
        total_task_time_seconds: Some(meta.total_task_time_seconds as i64),
        longest_task_seconds: meta.longest_task_seconds.map(|v| v as i64),
        longest_task_preview: meta.longest_task_preview.clone(),
        total_cost_usd,
        slug: parse_result.slug.clone(),
        entrypoint: parse_result.entrypoint.clone(),
    };

    // Use git_root as project identity for search (matches sidebar filter).
    let project_for_search = git_root
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(effective_encoded)
        .to_string();

    let search_messages = if validate_source_docs {
        let messages = sanitize_source_search_messages(
            std::mem::take(&mut parse_result.search_messages),
            &mut parse_result.diagnostics,
        );
        let has_summary_candidate = !parsed.preview.is_empty() || !project_for_search.is_empty();
        if has_summary_candidate {
            note_rejected_derived_source_doc(&mut parse_result.diagnostics);
        }
        messages
    } else {
        std::mem::take(&mut parse_result.search_messages)
    };

    IndexedSession {
        parsed,
        turns: parse_result.turns.clone(),
        models_seen: parse_result.models_seen.clone(),
        classified_invocations: classified,
        search_messages,
        cwd: cwd_owned,
        git_root,
        project_for_search,
        diagnostics: parse_result.diagnostics.clone(),
        hook_progress_events: parse_result.deep.hook_progress_events.clone(),
    }
}
