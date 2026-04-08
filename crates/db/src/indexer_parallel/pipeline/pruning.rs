// crates/db/src/indexer_parallel/pipeline/pruning.rs
// Prune sessions whose JSONL files no longer exist on disk.

use std::path::Path;

use crate::Database;

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
