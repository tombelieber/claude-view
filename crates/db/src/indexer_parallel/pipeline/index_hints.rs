// crates/db/src/indexer_parallel/pipeline/index_hints.rs
// Build session_id -> IndexHints map from sessions-index.json files. Pure data extraction.

use claude_view_core::resolve_cwd_for_project;
use std::collections::HashMap;
use std::path::Path;

use super::super::types::*;

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
