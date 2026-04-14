// crates/db/src/indexer_parallel/orchestrator/discovery.rs
// Filesystem walk to collect all .jsonl session files under ~/.claude/projects/.

use std::path::{Path, PathBuf};

/// Collect all `.jsonl` files at depth 2: `{projects_dir}/{project_encoded}/{session_id}.jsonl`.
/// Returns `(file_path, project_encoded, session_id)` triples.
#[tracing::instrument(skip_all)]
pub(crate) fn discover_jsonl_files(
    projects_dir: &Path,
) -> Result<Vec<(PathBuf, String, String)>, String> {
    let mut files: Vec<(PathBuf, String, String)> = Vec::new();
    let project_entries = std::fs::read_dir(projects_dir)
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

    Ok(files)
}
