// crates/db/src/indexer_parallel/backup.rs
// Backup ingest: optimistically import sessions from ~/.claude-backup.

use claude_view_core::{classify_work_type, count_ai_lines, ClassificationInput};
use std::collections::HashMap;
use std::sync::Arc;

use crate::Database;

use super::cost::*;
use super::helpers::*;
use super::parser::*;
use super::types::*;

/// Discover the backup machines dir. Returns None if the backup repo doesn't
/// exist or has an unexpected layout -- purely optimistic, never errors.
fn backup_machines_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let machines = home.join(".claude-backup").join("machines");
    if machines.is_dir() {
        Some(machines)
    } else {
        None
    }
}

/// Decompress a gzip buffer. Returns None on any error (optimistic -- skip bad files).
fn decompress_gz(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).ok()?;
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Walk `~/.claude-backup/machines/**/*.jsonl.gz`, skip subagents,
/// gunzip -> parse -> upsert any session UUIDs not already in the DB.
///
/// This is a best-effort ingest: corrupt gz files, parse failures, and
/// missing dirs are silently skipped. Returns (imported, skipped).
pub async fn ingest_backup_sessions(
    db: &Database,
    search_index: Option<Arc<claude_view_search::SearchIndex>>,
) -> (usize, usize) {
    let machines_dir = match backup_machines_dir() {
        Some(d) => d,
        None => {
            tracing::debug!("No ~/.claude-backup/machines/ found -- skipping backup ingest");
            return (0, 0);
        }
    };

    // Collect all .jsonl.gz files (skip subagents/)
    let mut gz_files: Vec<(std::path::PathBuf, String, String)> = Vec::new(); // (path, project_encoded, session_id)
    let walker = walkdir::WalkDir::new(&machines_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());

    for entry in walker {
        let path = entry.path();
        // Must be a .jsonl.gz file
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".jsonl.gz") => n,
            _ => continue,
        };
        // Skip subagent files
        if path.components().any(|c| c.as_os_str() == "subagents") {
            continue;
        }
        // Extract session UUID from filename (strip .jsonl.gz)
        let session_id = name.trim_end_matches(".jsonl.gz").to_string();
        // Validate UUID-ish format (contains hyphens, >30 chars)
        if session_id.len() < 30 || !session_id.contains('-') {
            continue;
        }
        // Extract project_encoded from parent dir name
        let project_encoded = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        gz_files.push((path.to_path_buf(), project_encoded, session_id));
    }

    if gz_files.is_empty() {
        tracing::debug!("No .jsonl.gz files found in backup -- skipping");
        return (0, 0);
    }

    // Batch-check which session IDs already exist in the DB
    let existing_ids = match db.get_all_session_ids().await {
        Ok(ids) => ids
            .into_iter()
            .collect::<std::collections::HashSet<String>>(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to query existing session IDs for backup dedup");
            return (0, 0);
        }
    };

    // Filter to backup-only sessions
    let new_files: Vec<_> = gz_files
        .into_iter()
        .filter(|(_, _, id)| !existing_ids.contains(id))
        .collect();

    let total_new = new_files.len();
    if total_new == 0 {
        tracing::debug!("All backup sessions already in DB -- skipping");
        return (0, 0);
    }

    tracing::info!(
        total_backup_only = total_new,
        "Ingesting backup sessions from ~/.claude-backup"
    );

    let pricing = Arc::new(load_indexing_pricing());
    let semaphore = Arc::new(tokio::sync::Semaphore::new(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    ));

    // Phase 1: Parse (parallel)
    let mut handles = Vec::with_capacity(total_new);
    for (path, project_encoded, session_id) in new_files {
        let sem = semaphore.clone();
        let pricing = pricing.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| format!("{e}"))?;

            // Gunzip the file
            let gz_bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(path = %path.display(), error = %e, "Skipping unreadable backup file");
                    return Ok::<(Option<IndexedSession>, String), String>((None, session_id));
                }
            };
            let decompressed = match decompress_gz(&gz_bytes) {
                Some(d) => d,
                None => {
                    tracing::debug!(path = %path.display(), "Skipping corrupt/invalid gz backup file");
                    return Ok((None, session_id));
                }
            };

            // Parse the JSONL bytes (same parser as live sessions)
            let mut parse_result = parse_bytes(&decompressed);
            // Note: no subagent merge for backup files (subagents are skipped)

            let meta = &mut parse_result.deep;
            if meta.last_timestamp.is_none() {
                return Ok((None, session_id));
            }

            // Resolve project info
            let resolved = claude_view_core::discovery::resolve_project_path_with_cwd(
                &project_encoded,
                parse_result.cwd.as_deref(),
            );

            // Backup files in subagents/ dirs are already filtered out above.
            let is_sidechain = false;
            let git_branch = parse_result.git_branch.clone().or_else(|| {
                parse_result
                    .cwd
                    .as_deref()
                    .and_then(claude_view_core::resolve_worktree_branch)
            });

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

            // Recompute ai_lines from raw_invocations (in-memory parser
            // output, same as merge_subagent_workload path — NOT the
            // retired `invocations` DB table).
            let ai_line_count = count_ai_lines(
                parse_result
                    .raw_invocations
                    .iter()
                    .map(|inv| (inv.name.as_str(), &inv.input))
                    .filter_map(|(name, input)| input.as_ref().map(|i| (name, i))),
            );
            meta.ai_lines_added = ai_line_count.lines_added as u32;
            meta.ai_lines_removed = ai_line_count.lines_removed as u32;

            let work_type_input = ClassificationInput::new(
                meta.duration_seconds,
                meta.turn_count as u32,
                meta.files_edited_count,
                meta.ai_lines_added,
                meta.skills_used.clone(),
            );
            let work_type = classify_work_type(&work_type_input);

            // Build ParsedSession -- mirrors scan_and_index_all exactly
            let preview = meta.first_user_prompt.clone().unwrap_or_default();
            let summary = meta.summary_text.clone();
            let message_count = (meta.user_prompt_count + meta.api_call_count) as i32;
            let total_cost_usd = calculate_per_turn_cost(&parse_result.turns, &pricing);
            let primary_model = compute_primary_model(&parse_result.turns);
            let decompressed_len = decompressed.len() as i64;

            let parsed = ParsedSession {
                id: session_id.clone(),
                project_id: project_encoded.clone(),
                project_display_name: resolved.display_name.clone(),
                project_path: resolved.full_path.clone(),
                file_path: path.to_string_lossy().to_string(),
                preview,
                summary,
                message_count,
                last_message_at: meta.last_timestamp.unwrap_or(0),
                first_message_at: meta.first_timestamp.unwrap_or(0),
                git_branch,
                is_sidechain,
                size_bytes: decompressed_len,
                last_message: meta.last_message.clone(),
                turn_count: meta.turn_count as i32,
                tool_counts_edit: meta.tool_counts.edit as i32,
                tool_counts_read: meta.tool_counts.read as i32,
                tool_counts_bash: meta.tool_counts.bash as i32,
                tool_counts_write: meta.tool_counts.write as i32,
                files_touched: serde_json::to_string(&meta.files_touched).unwrap_or_default(),
                skills_used: serde_json::to_string(&meta.skills_used).unwrap_or_default(),
                user_prompt_count: meta.user_prompt_count as i32,
                api_call_count: meta.api_call_count as i32,
                tool_call_count: meta.tool_call_count as i32,
                files_read: serde_json::to_string(&meta.files_read).unwrap_or_default(),
                files_edited: serde_json::to_string(&meta.files_edited).unwrap_or_default(),
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
                file_size_at_index: decompressed_len,
                file_mtime_at_index: 0, // backup files don't have meaningful mtime
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

            let project_for_search = parse_result
                .cwd
                .as_deref()
                .and_then(claude_view_core::infer_git_root_from_worktree_path)
                .unwrap_or_else(|| resolved.full_path.clone());

            let turns = std::mem::take(&mut parse_result.turns);
            let models_seen: Vec<String> = turns
                .iter()
                .map(|t| t.model_id.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            // Backup files skip invocation classification (no registry available).
            // Commit invocations are still tracked via commit_count.
            let classified_invocations: Vec<(String, i64, String, String, String, i64)> =
                Vec::new();

            Ok((
                Some(IndexedSession {
                    parsed,
                    turns,
                    models_seen,
                    classified_invocations,
                    search_messages: std::mem::take(&mut parse_result.search_messages),
                    cwd: parse_result.cwd.clone(),
                    git_root: parse_result
                        .cwd
                        .as_deref()
                        .and_then(claude_view_core::infer_git_root_from_worktree_path),
                    project_for_search,
                    diagnostics: parse_result.diagnostics,
                    hook_progress_events: parse_result.deep.hook_progress_events,
                }),
                session_id,
            ))
        });

        handles.push(handle);
    }

    // Collect parse results
    let mut indexed_sessions: Vec<IndexedSession> = Vec::new();
    let mut skipped = 0usize;
    for h in handles {
        match h.await {
            Ok(Ok((Some(session), _))) => indexed_sessions.push(session),
            Ok(Ok((None, _))) => skipped += 1,
            Ok(Err(e)) => {
                tracing::debug!(error = %e, "Backup session parse error");
                skipped += 1;
            }
            Err(e) => {
                tracing::debug!(error = %e, "Backup session join error");
                skipped += 1;
            }
        }
    }

    if indexed_sessions.is_empty() {
        tracing::debug!("No valid backup sessions to import after parse");
        return (0, skipped);
    }

    tracing::info!(
        parsed = indexed_sessions.len(),
        skipped,
        "Backup parse complete, writing to DB"
    );

    // Phase 2: DB write (chunked, same as scan_and_index_all)
    let seen_at = chrono::Utc::now().timestamp();
    let mut imported = 0usize;

    for chunk in indexed_sessions.chunks(200) {
        let tx_result: Result<(), String> = async {
            let mut tx = db
                .pool()
                .begin()
                .await
                .map_err(|e| format!("Failed to begin backup write tx: {e}"))?;

            sqlx::query("PRAGMA busy_timeout = 30000")
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("PRAGMA busy_timeout: {e}"))?;

            // Upsert models
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
                    .map_err(|e| format!("Upsert models: {e}"))?;
            }

            for session in chunk {
                // Upsert session (legacy sessions table)
                crate::queries::sessions::execute_upsert_parsed_session(&mut *tx, &session.parsed)
                    .await
                    .map_err(|e| format!("Upsert backup session {}: {e}", session.parsed.id))?;

                // CQRS Phase 7.h.3: dual-write the backup-import path into
                // `session_stats` so every imported session lands on both
                // tables for the 7.h.4 reader flip.
                crate::queries::sessions::execute_upsert_session_stats_from_parsed(
                    &mut *tx,
                    &session.parsed,
                )
                .await
                .map_err(|e| format!("Upsert backup session_stats {}: {e}", session.parsed.id))?;

                // Topology
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
                    .map_err(|e| format!("Topology update {}: {e}", session.parsed.id))?;
                }

                // CQRS Phase 6.4: turns + invocations tables retired in
                // migration 87. Per-model and per-invocable aggregates now
                // live on `session_stats` JSON columns (written by
                // indexer_v2). The parser fields `session.turns` and
                // `session.classified_invocations` remain computed; they
                // go dead when indexer_parallel is retired in E.5.
            }

            tx.commit()
                .await
                .map_err(|e| format!("Commit backup tx: {e}"))?;
            Ok(())
        }
        .await;

        match tx_result {
            Ok(()) => imported += chunk.len(),
            Err(e) => {
                tracing::warn!(error = %e, "Backup chunk write failed -- skipping chunk");
                skipped += chunk.len();
            }
        }
    }

    // Phase 3: Search index (optional)
    if let Some(ref search) = search_index {
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
                tracing::debug!(session_id = %session.parsed.id, error = %e, "Backup search index error");
            }
        }
        let _ = search.commit();
        let _ = search.reader.reload();
    }

    tracing::info!(imported, skipped, "Backup ingest complete");
    (imported, skipped)
}
