// crates/db/src/indexer_parallel/orchestrator.rs
// 3-phase startup scan: parse (parallel) -> SQLite write (chunked) -> search index.

use claude_view_core::{
    classify_work_type, resolve_worktree_parent, ClassificationInput, ClassifyResult, Registry,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::Database;

use super::cost::*;
use super::helpers::*;
use super::parser::*;
use super::types::*;
use super::writer::check_token_reconciliation;

/// 3-phase startup scan: parse (parallel) -> SQLite write (chunked) -> search index.
///
/// Phase 1: Parse all changed JSONL files in parallel (CPU-bound, zero DB writes).
/// Phase 2: Write sessions, turns, models, invocations in chunked transactions.
/// Phase 3: Write search index to Tantivy (after SQLite success).
///
/// Returns (indexed_count, skipped_count).
///
/// `on_file_done` fires for **every** `.jsonl` file (parsed or skipped) so
/// callers can drive a progress counter that reaches 100%.
///
/// `on_total_known` fires once with the actual `.jsonl` file count right
/// after the filesystem walk, before any parsing begins. This is the single
/// source of truth for "total sessions to process" -- callers should use it
/// to set their progress total instead of guessing from external sources.
pub async fn scan_and_index_all<F, T, W>(
    claude_dir: &Path,
    db: &Database,
    hints: &HashMap<String, IndexHints>,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,
    registry: Option<Arc<Registry>>,
    on_file_done: F,
    on_total_known: T,
    on_finalize_start: W,
) -> Result<(usize, usize), String>
where
    F: Fn(&str) + Send + Sync + 'static,
    T: FnOnce(usize),
    W: FnOnce(),
{
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Ok((0, 0));
    }

    // When the search index was rebuilt (schema version mismatch), force re-parse
    // of ALL sessions so search_messages get regenerated and fed to Tantivy.
    let force_search_reindex = search_index
        .as_ref()
        .map(|idx| idx.needs_full_reindex)
        .unwrap_or(false);

    if force_search_reindex {
        tracing::info!(
            "Search index was rebuilt -- forcing full re-parse to repopulate search data"
        );
    }
    let source_docs_validation_enabled = search_index.is_some();

    // Collect all .jsonl files at depth 2: {projects_dir}/{project_encoded}/{session_id}.jsonl
    let mut files: Vec<(std::path::PathBuf, String, String)> = Vec::new();
    let project_entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_encoded = project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let session_entries = match std::fs::read_dir(&project_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for file_entry in session_entries.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            files.push((file_path, project_encoded.clone(), session_id));
        }
    }

    // Report actual file count -- single source of truth for progress total.
    on_total_known(files.len());

    // Pre-load all existing session staleness info from DB in one query.
    let existing_sessions = db
        .get_sessions_needing_deep_index()
        .await
        .map_err(|e| format!("Failed to load existing sessions: {}", e))?;
    let existing_map: HashMap<String, (Option<i64>, Option<i64>, i32, Option<String>)> =
        existing_sessions
            .into_iter()
            .map(
                |(id, _fp, stored_size, stored_mtime, _deep_at, pv, _proj, archived_at)| {
                    (id, (stored_size, stored_mtime, pv, archived_at))
                },
            )
            .collect();

    // Use one merged pricing map for this full indexing run.
    let pricing = Arc::new(load_indexing_pricing());

    let semaphore = Arc::new(tokio::sync::Semaphore::new(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    ));

    let skipped = Arc::new(AtomicUsize::new(0));

    // Phase 1: PARSE (parallel, CPU-bound, zero I/O writes)
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

            // Compute derived fields
            let commit_invocations =
                extract_commit_skill_invocations(&parse_result.raw_invocations);
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
            let classified = if let Some(ref registry) = registry {
                parse_result
                    .raw_invocations
                    .iter()
                    .filter_map(|raw| {
                        match claude_view_core::classify_tool_use(&raw.name, &raw.input, registry) {
                            ClassifyResult::Valid { invocable_id, .. } => Some((
                                path.to_string_lossy().to_string(),
                                raw.byte_offset as i64,
                                invocable_id,
                                session_id.clone(),
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
            let git_root = cwd
                .and_then(claude_view_core::discovery::infer_git_root_from_worktree_path)
                .map(|s| s.to_string());

            // Compute cost per-turn (each turn = one API call)
            let total_cost_usd = calculate_per_turn_cost(&parse_result.turns, &pricing);

            // Build ParsedSession
            let sid = session_id.clone();
            let parsed = ParsedSession {
                id: session_id,
                project_id: effective_encoded.clone(),
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
                .unwrap_or(&effective_encoded)
                .to_string();

            let search_messages = if validate_source_docs {
                let messages = sanitize_source_search_messages(
                    std::mem::take(&mut parse_result.search_messages),
                    &mut parse_result.diagnostics,
                );
                let has_summary_candidate =
                    !parsed.preview.is_empty() || !project_for_search.is_empty();
                if has_summary_candidate {
                    note_rejected_derived_source_doc(&mut parse_result.diagnostics);
                }
                messages
            } else {
                std::mem::take(&mut parse_result.search_messages)
            };

            Ok((
                Some(IndexedSession {
                    parsed,
                    turns: parse_result.turns,
                    models_seen: parse_result.models_seen,
                    classified_invocations: classified,
                    search_messages,
                    cwd: cwd_owned,
                    git_root,
                    project_for_search,
                    diagnostics: parse_result.diagnostics,
                    hook_progress_events: parse_result.deep.hook_progress_events,
                }),
                sid,
            ))
        });

        handles.push(handle);
    }

    // Collect all parse results.
    let on_file_done = Arc::new(on_file_done);
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

    if indexed_sessions.is_empty() {
        return Ok((0, skipped.load(Ordering::Relaxed)));
    }

    let total_search_bytes: usize = indexed_sessions
        .iter()
        .map(|s| {
            s.search_messages
                .iter()
                .map(|m| m.content.len())
                .sum::<usize>()
        })
        .sum();
    tracing::info!(
        sessions = indexed_sessions.len(),
        search_bytes = total_search_bytes,
        "Phase 1 parse complete, starting Phase 2 SQLite write"
    );

    // Aggregate per-session ParseDiagnostics into IndexRunIntegrityCounters
    let mut integrity = crate::IndexRunIntegrityCounters::default();
    for session in &indexed_sessions {
        let d = &session.diagnostics;
        integrity.unknown_top_level_type_count += d.lines_unknown_type as i64;
        integrity.dropped_line_invalid_json_count += d.json_parse_failures as i64;
        integrity.unknown_source_role_count += d.unknown_source_role_count as i64;
        integrity.derived_source_message_doc_count += d.derived_source_message_doc_count as i64;
        integrity.source_message_non_source_provenance_count +=
            d.source_message_non_source_provenance_count as i64;
    }

    let index_run_start = std::time::Instant::now();
    let index_run_id = match db.create_index_run("full", None, Some(&integrity)).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create index run");
            None
        }
    };

    // Phase 2: SQLITE WRITE (sequential, chunked, single writer)
    let seen_at = chrono::Utc::now().timestamp();
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
            sqlx::query("DELETE FROM turns WHERE session_id = ?1")
                .bind(&session.parsed.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("DELETE turns for {}: {}", session.parsed.id, e))?;

            sqlx::query("DELETE FROM invocations WHERE session_id = ?1")
                .bind(&session.parsed.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("DELETE invocations for {}: {}", session.parsed.id, e))?;

            crate::queries::sessions::execute_upsert_parsed_session(&mut *tx, &session.parsed)
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
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update topology {}: {}", session.parsed.id, e))?;
            }

            if !session.turns.is_empty() {
                crate::queries::batch_insert_turns_tx(&mut tx, &session.parsed.id, &session.turns)
                    .await
                    .map_err(|e| {
                        format!("Failed to insert turns for {}: {}", session.parsed.id, e)
                    })?;
            }

            if !session.classified_invocations.is_empty() {
                crate::queries::batch_insert_invocations_tx(
                    &mut tx,
                    &session.classified_invocations,
                )
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

                crate::queries::hook_events::insert_hook_events_tx(
                    &mut tx,
                    &session.parsed.id,
                    &events,
                )
                .await
                .map_err(|e| {
                    format!(
                        "Failed to insert hook events for {}: {}",
                        session.parsed.id, e
                    )
                })?;
            }
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

    tracing::info!(
        indexed = indexed_count,
        "Phase 2 SQLite write complete, starting Phase 3 search index"
    );

    on_finalize_start();

    // Phase 3: SEARCH INDEX (sequential, after SQLite success)
    if let Some(ref search) = search_index {
        let mut search_errors = 0u32;
        let mut sessions_indexed = 0u32;

        for session in &indexed_sessions {
            if session.search_messages.is_empty() {
                continue;
            }

            let docs: Vec<claude_view_search::SearchDocument> = session
                .search_messages
                .iter()
                .enumerate()
                .map(|(i, msg)| claude_view_search::SearchDocument {
                    session_id: session.parsed.id.clone(),
                    project: session.project_for_search.clone(),
                    branch: session.parsed.git_branch.clone().unwrap_or_default(),
                    model: session.parsed.primary_model.clone().unwrap_or_default(),
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                    turn_number: (i + 1) as u64,
                    timestamp: msg.timestamp.unwrap_or(0),
                    skills: serde_json::from_str(&session.parsed.skills_used).unwrap_or_default(),
                })
                .collect();

            if let Err(e) = search.index_session(&session.parsed.id, &docs) {
                tracing::warn!(session_id = %session.parsed.id, error = %e, "Failed to index session for search");
                search_errors += 1;
            }
            sessions_indexed += 1;
        }

        if sessions_indexed > 0 {
            if let Err(e) = search.commit() {
                tracing::warn!(error = %e, "Failed to commit search index");
            } else {
                if let Err(e) = search.reader.reload() {
                    tracing::warn!(error = %e, "Failed to reload search reader after commit");
                }
                if search_errors > 0 {
                    tracing::info!(
                        indexed = sessions_indexed,
                        errors = search_errors,
                        "Search index write complete (with errors)"
                    );
                }
                if force_search_reindex {
                    search.mark_schema_synced();
                }
            }
        }
    }

    // Persist index run completion with aggregated integrity counters
    if let Some(run_id) = index_run_id {
        let index_run_duration_ms = index_run_start.elapsed().as_millis() as i64;
        let total_bytes: u64 = indexed_sessions
            .iter()
            .map(|s| s.diagnostics.bytes_total)
            .sum();
        let throughput = if index_run_duration_ms > 0 {
            Some(total_bytes as f64 / (1024.0 * 1024.0) / (index_run_duration_ms as f64 / 1000.0))
        } else {
            None
        };
        if let Err(e) = db
            .complete_index_run(
                run_id,
                Some(indexed_count as i64),
                index_run_duration_ms,
                throughput,
                Some(&integrity),
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to complete index run");
        }
    }

    Ok((indexed_count, skipped.load(Ordering::Relaxed)))
}
