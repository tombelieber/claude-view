// crates/db/src/indexer_parallel/pipeline/pass_1.rs
// Legacy pass-1: read sessions-index.json and insert/update sessions in DB.

use claude_view_core::{
    discover_orphan_sessions, read_all_session_indexes, resolve_cwd_for_project,
    resolve_project_path_with_cwd, resolve_worktree_parent,
};
use std::path::Path;

use crate::Database;

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
