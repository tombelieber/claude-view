// crates/db/src/indexer_parallel/pipeline.rs
// Index hints, pass_1_read_indexes, pass_2_deep_index, prune_stale_sessions.

use claude_view_core::{
    discover_orphan_sessions, read_all_session_indexes, resolve_cwd_for_project,
    resolve_project_path_with_cwd, resolve_worktree_parent, ClassifyResult, Registry,
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
use super::writer::*;

/// Read all sessions-index.json files and build a session_id -> hints map.
/// No DB writes. Pure data extraction.
pub fn build_index_hints(claude_dir: &Path) -> HashMap<String, IndexHints> {
    let mut hints = HashMap::new();
    match claude_view_core::session_index::read_all_session_indexes(claude_dir) {
        Ok(indexes) => {
            for (project_encoded, entries) in &indexes {
                // Use cwd from JSONL files -- never naive path decoding
                let project_dir = claude_dir.join("projects").join(project_encoded);
                let cwd = resolve_cwd_for_project(&project_dir);
                let resolved = claude_view_core::discovery::resolve_project_path_with_cwd(
                    project_encoded,
                    cwd.as_deref(),
                );
                for entry in entries {
                    let h = IndexHints {
                        is_sidechain: entry.is_sidechain,
                        project_path: Some(resolved.full_path.clone()),
                        project_display_name: Some(resolved.display_name.clone()),
                        git_branch: entry.git_branch.clone(),
                        summary: entry.summary.clone(),
                    };
                    hints.insert(entry.session_id.clone(), h);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read session indexes: {e}");
        }
    }
    hints
}

/// Pass 1: Read sessions-index.json files and insert/update sessions in DB.
#[deprecated(
    note = "Legacy two-pass pipeline. Use scan_and_index_all + upsert_parsed_session instead."
)]
#[allow(deprecated)]
pub async fn pass_1_read_indexes(
    claude_dir: &Path,
    db: &Database,
) -> Result<(usize, usize), String> {
    let all_indexes = read_all_session_indexes(claude_dir).map_err(|e| e.to_string())?;

    let mut total_projects = 0usize;
    let mut total_sessions = 0usize;

    async fn insert_project_sessions(
        claude_dir: &Path,
        db: &Database,
        project_encoded: &str,
        entries: &[claude_view_core::SessionIndexEntry],
        total_sessions: &mut usize,
    ) -> Result<(), String> {
        let entry_cwd_owned: Option<String> = entries
            .first()
            .and_then(|e| e.session_cwd.clone())
            .or_else(|| {
                let project_dir = claude_dir.join("projects").join(project_encoded);
                resolve_cwd_for_project(&project_dir)
            });
        let entry_cwd = entry_cwd_owned.as_deref();

        let inferred_git_root =
            entry_cwd.and_then(claude_view_core::discovery::infer_git_root_from_worktree_path);
        let inferred_git_root = match (&inferred_git_root, entry_cwd) {
            (Some(_), _) => inferred_git_root,
            (None, Some(cwd)) => claude_view_core::discovery::resolve_git_root(cwd).await,
            (None, None) => None,
        };

        let (effective_encoded, effective_resolved) =
            if let Some(parent_encoded) = resolve_worktree_parent(project_encoded) {
                let resolved = resolve_project_path_with_cwd(&parent_encoded, entry_cwd);
                (parent_encoded, resolved)
            } else {
                (
                    project_encoded.to_string(),
                    resolve_project_path_with_cwd(project_encoded, entry_cwd),
                )
            };

        let project_display_name = &effective_resolved.display_name;
        let project_path = &effective_resolved.full_path;

        for entry in entries {
            let modified_at = entry
                .modified
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let file_path = entry.full_path.clone().unwrap_or_else(|| {
                claude_dir
                    .join("projects")
                    .join(project_encoded)
                    .join(format!("{}.jsonl", &entry.session_id))
                    .to_string_lossy()
                    .to_string()
            });

            let size_bytes = std::fs::metadata(&file_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let preview = entry.first_prompt.as_deref().unwrap_or("");
            let summary = entry.summary.as_deref();
            let message_count = entry.message_count.unwrap_or(0) as i32;
            let git_branch = entry.git_branch.as_deref();
            let is_sidechain = entry.is_sidechain.unwrap_or(false);

            let entry_project_path = entry.project_path.as_deref().unwrap_or(project_path);

            db.insert_session_from_index(
                &entry.session_id,
                &effective_encoded,
                project_display_name,
                entry_project_path,
                &file_path,
                preview,
                summary,
                message_count,
                modified_at,
                git_branch,
                is_sidechain,
                size_bytes,
            )
            .await
            .map_err(|e| format!("Failed to insert session {}: {}", entry.session_id, e))?;

            let session_cwd = entry.session_cwd.as_deref().or(entry_cwd);
            if session_cwd.is_some()
                || entry.parent_session_id.is_some()
                || inferred_git_root.is_some()
            {
                db.update_session_topology(
                    &entry.session_id,
                    session_cwd,
                    entry.parent_session_id.as_deref(),
                    inferred_git_root.as_deref(),
                )
                .await
                .map_err(|e| format!("Failed to update topology {}: {}", entry.session_id, e))?;
            }

            *total_sessions += 1;
        }

        Ok(())
    }

    for (project_encoded, entries) in &all_indexes {
        if entries.is_empty() {
            continue;
        }
        total_projects += 1;
        insert_project_sessions(
            claude_dir,
            db,
            project_encoded,
            entries,
            &mut total_sessions,
        )
        .await?;
    }

    let orphans = discover_orphan_sessions(claude_dir).map_err(|e| e.to_string())?;

    for (project_encoded, entries) in &orphans {
        if entries.is_empty() {
            continue;
        }
        total_projects += 1;
        insert_project_sessions(
            claude_dir,
            db,
            project_encoded,
            entries,
            &mut total_sessions,
        )
        .await?;
    }

    Ok((total_projects, total_sessions))
}

/// Pass 2: Parallel deep JSONL parsing for extended metadata.
#[deprecated(
    note = "Legacy two-pass pipeline. Use scan_and_index_all + upsert_parsed_session instead."
)]
#[allow(deprecated)]
pub async fn pass_2_deep_index<F>(
    db: &Database,
    registry: Option<&Registry>,
    search_index: Option<&claude_view_search::SearchIndex>,
    on_start: impl FnOnce(u64),
    on_file_done: F,
) -> Result<(usize, u64), String>
where
    F: Fn(usize, usize, u64) + Send + Sync + 'static,
{
    let all_sessions = db
        .get_sessions_needing_deep_index()
        .await
        .map_err(|e| format!("Failed to query sessions needing deep index: {}", e))?;

    let force_search_reindex = search_index
        .map(|idx| idx.needs_full_reindex)
        .unwrap_or(false);

    if force_search_reindex {
        tracing::info!(
            sessions = all_sessions.len(),
            "Search index was rebuilt -- forcing full re-parse to repopulate search data"
        );
    }

    let all_sessions_count = all_sessions.len();
    let sessions: Vec<(String, String, String)> = all_sessions
        .into_iter()
        .filter_map(
            |(
                id,
                file_path,
                stored_size,
                stored_mtime,
                deep_indexed_at,
                parse_version,
                project,
                archived_at,
            )| {
                if archived_at.is_some() {
                    return None;
                }

                let needs_index = if force_search_reindex {
                    true
                } else if deep_indexed_at.is_none() {
                    true
                } else if parse_version < CURRENT_PARSE_VERSION {
                    true
                } else if let (Some(sz), Some(mt)) = (stored_size, stored_mtime) {
                    match std::fs::metadata(&file_path) {
                        Ok(meta) => {
                            let current_size = meta.len() as i64;
                            let current_mtime = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);
                            current_size != sz || current_mtime != mt
                        }
                        Err(_) => false,
                    }
                } else {
                    true
                };
                if needs_index {
                    Some((id, file_path, project))
                } else {
                    None
                }
            },
        )
        .collect();

    let phase_start = std::time::Instant::now();
    let skipped = all_sessions_count - sessions.len();

    if sessions.is_empty() {
        return Ok((0, 0));
    }

    let total_bytes: u64 = sessions
        .iter()
        .filter_map(|(_, path, _)| std::fs::metadata(path).ok())
        .map(|m| m.len())
        .sum();

    on_start(total_bytes);

    let registry: Option<Arc<Registry>> = registry.map(|r| Arc::new(r.clone()));

    let total = sessions.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let on_file_done = Arc::new(on_file_done);

    let parallelism = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(parallelism));

    let mut handles = Vec::with_capacity(total);

    for (id, file_path, project) in sessions {
        let sem = semaphore.clone();
        let counter = counter.clone();
        let on_done = on_file_done.clone();
        let registry = registry.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("Semaphore error: {}", e))?;

            let path = std::path::PathBuf::from(&file_path);

            let (parse_result, file_size, file_mtime) = tokio::task::spawn_blocking(move || {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return (ParseResult::default(), 0i64, 0i64),
                };
                let metadata = match file.metadata() {
                    Ok(m) => m,
                    Err(_) => return (ParseResult::default(), 0, 0),
                };
                let fsize = metadata.len() as i64;
                let fmtime = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let len = metadata.len() as usize;
                if len == 0 {
                    return (ParseResult::default(), fsize, fmtime);
                }
                let mut result = if len < 64 * 1024 {
                    match std::fs::read(&path) {
                        Ok(data) => parse_bytes(&data),
                        Err(_) => ParseResult::default(),
                    }
                } else {
                    match unsafe { memmap2::Mmap::map(&file) } {
                        Ok(mmap) => parse_bytes(&mmap),
                        Err(_) => match std::fs::read(&path) {
                            Ok(data) => parse_bytes(&data),
                            Err(_) => ParseResult::default(),
                        },
                    }
                };

                merge_subagent_workload(&path, &mut result);

                (result, fsize, fmtime)
            })
            .await
            .map_err(|e| format!("spawn_blocking join error: {}", e))?;

            let diag = &parse_result.diagnostics;

            if diag.json_parse_failures > 0 || diag.lines_unknown_type > 0 {
                tracing::warn!(
                    session_id = %id,
                    parse_failures = diag.json_parse_failures,
                    unknown_types = diag.lines_unknown_type,
                    total_lines = diag.lines_total,
                    "Parse anomalies detected"
                );
            }

            let classified = if let Some(ref registry) = registry {
                parse_result
                    .raw_invocations
                    .iter()
                    .filter_map(|raw| {
                        let result =
                            claude_view_core::classify_tool_use(&raw.name, &raw.input, registry);
                        match result {
                            ClassifyResult::Valid { invocable_id, .. } => Some((
                                file_path.clone(),
                                raw.byte_offset as i64,
                                invocable_id,
                                id.clone(),
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

            let indexed = counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_done(indexed, total, file_size.max(0) as u64);

            Ok::<Option<DeepIndexResult>, String>(Some(DeepIndexResult {
                session_id: id,
                file_path,
                project,
                parse_result,
                classified_invocations: classified,
                file_size,
                file_mtime,
            }))
        });

        handles.push(handle);
    }

    let mut results = Vec::with_capacity(total);
    let mut parse_errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(result))) => results.push(result),
            Ok(Ok(None)) => {}
            Ok(Err(e)) => parse_errors.push(e),
            Err(e) => parse_errors.push(format!("Task join error: {}", e)),
        }
    }

    let parse_elapsed = phase_start.elapsed();

    if !parse_errors.is_empty() {
        tracing::warn!(
            "pass_2_deep_index parse phase encountered {} errors: {:?}",
            parse_errors.len(),
            parse_errors
        );
    }

    // Write phase
    struct SearchBatch {
        session_id: String,
        project: String,
        branch: Option<String>,
        primary_model: Option<String>,
        messages: Vec<claude_view_core::SearchableMessage>,
        skills: Vec<String>,
    }
    let search_batches: Vec<SearchBatch> = if search_index.is_some() {
        for result in &mut results {
            let search_messages = std::mem::take(&mut result.parse_result.search_messages);
            result.parse_result.search_messages = sanitize_source_search_messages(
                search_messages,
                &mut result.parse_result.diagnostics,
            );

            let preview = result.parse_result.deep.first_user_prompt.as_deref();
            let preview_non_empty = preview.map(|s| !s.is_empty()).unwrap_or(false);
            let has_summary_candidate = preview.is_some()
                && (preview_non_empty
                    || !result.project.is_empty()
                    || !result.parse_result.deep.last_message.is_empty());
            if has_summary_candidate {
                note_rejected_derived_source_doc(&mut result.parse_result.diagnostics);
            }
        }

        results
            .iter()
            .filter(|r| !r.parse_result.search_messages.is_empty())
            .map(|r| SearchBatch {
                session_id: r.session_id.clone(),
                project: r.project.clone(),
                branch: r.parse_result.git_branch.clone(),
                primary_model: compute_primary_model(&r.parse_result.turns),
                messages: r.parse_result.search_messages.clone(),
                skills: r.parse_result.deep.skills_used.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    let indexed = if !results.is_empty() {
        write_results_sqlx(db, &results).await?
    } else {
        0
    };

    // Search index phase
    if let Some(search) = search_index {
        if !search_batches.is_empty() {
            let mut search_errors = 0u32;
            for batch in &search_batches {
                let docs: Vec<claude_view_search::SearchDocument> = batch
                    .messages
                    .iter()
                    .enumerate()
                    .map(|(i, msg)| claude_view_search::SearchDocument {
                        session_id: batch.session_id.clone(),
                        project: batch.project.clone(),
                        branch: batch.branch.clone().unwrap_or_default(),
                        model: batch.primary_model.clone().unwrap_or_default(),
                        role: msg.role.clone(),
                        content: msg.content.clone(),
                        turn_number: (i + 1) as u64,
                        timestamp: msg.timestamp.unwrap_or(0),
                        skills: batch.skills.clone(),
                    })
                    .collect();

                if let Err(e) = search.index_session(&batch.session_id, &docs) {
                    tracing::warn!(
                        session_id = %batch.session_id,
                        error = %e,
                        "Failed to index session for search"
                    );
                    search_errors += 1;
                }
            }
            if let Err(e) = search.commit() {
                tracing::warn!(error = %e, "Failed to commit search index");
            } else {
                if let Err(e) = search.reader.reload() {
                    tracing::warn!(error = %e, "Failed to reload search reader after commit");
                }
                if search_errors > 0 {
                    tracing::info!(
                        indexed = search_batches.len() - search_errors as usize,
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

    let write_elapsed = phase_start.elapsed() - parse_elapsed;

    if !parse_errors.is_empty() {
        tracing::warn!(
            "pass_2_deep_index encountered {} parse errors (still wrote {} results)",
            parse_errors.len(),
            indexed
        );
    }

    tracing::info!(
        sessions_indexed = indexed,
        sessions_total = total,
        parse_version = CURRENT_PARSE_VERSION,
        "Pass 2 deep indexing complete"
    );

    {
        let total_elapsed = phase_start.elapsed();
        tracing::debug!(
            indexed,
            skipped,
            errors = parse_errors.len(),
            parse_phase = %claude_view_core::format_duration(parse_elapsed),
            write_phase = %claude_view_core::format_duration(write_elapsed),
            total = %claude_view_core::format_duration(total_elapsed),
            "deep index perf"
        );
    }

    Ok((indexed, total_bytes))
}

/// Prune sessions from the database whose JSONL files no longer exist on disk.
pub async fn prune_stale_sessions(db: &Database) -> Result<u64, String> {
    let all_paths = db
        .get_all_session_file_paths()
        .await
        .map_err(|e| format!("Failed to query session file paths: {}", e))?;

    if all_paths.is_empty() {
        return Ok(0);
    }

    let valid_paths: Vec<String> = all_paths
        .into_iter()
        .filter(|path| path.contains(".claude-backup") || Path::new(path).exists())
        .collect();

    let pruned = db
        .remove_stale_sessions(&valid_paths)
        .await
        .map_err(|e| format!("Failed to prune stale sessions: {}", e))?;

    if pruned > 0 {
        tracing::info!(
            "Pruned {} stale sessions (JSONL files deleted from disk)",
            pruned
        );
    }

    Ok(pruned)
}
