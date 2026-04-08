//! Internal helpers shared across file_history submodules.

use similar::{ChangeTag, TextDiff};
use std::path::Path;

use super::types::DiffStats;

/// Validate that a file hash is safe to use in filesystem paths.
///
/// Rejects any value containing path traversal characters (`/`, `\`, `..`).
/// Real backup filenames are hex hashes -- no path separators.
pub fn validate_file_hash(hash: &str) -> Result<(), String> {
    if hash.is_empty() {
        return Err("File hash is empty".to_string());
    }
    if hash.contains('/') || hash.contains('\\') || hash.contains("..") {
        return Err(format!("Invalid file hash: {hash}"));
    }
    Ok(())
}

/// Parse "hash@vN" -> (hash, N)
pub(super) fn parse_backup_filename(name: &str) -> Option<(String, u32)> {
    let at_pos = name.rfind('@')?;
    let hash = &name[..at_pos];
    let version_str = name.get(at_pos + 1..)?;
    if !version_str.starts_with('v') {
        return None;
    }
    let version: u32 = version_str[1..].parse().ok()?;
    Some((hash.to_string(), version))
}

/// Quick diff stats between two files (just counts, no hunk structure).
pub(super) fn quick_diff_stats(from_path: &Path, to_path: &Path) -> DiffStats {
    let from_text = match std::fs::read_to_string(from_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                path = %from_path.display(),
                error = %e,
                "Failed to read 'from' file for diff stats"
            );
            return DiffStats::default();
        }
    };
    let to_text = match std::fs::read_to_string(to_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                path = %to_path.display(),
                error = %e,
                "Failed to read 'to' file for diff stats"
            );
            return DiffStats::default();
        }
    };
    let diff = TextDiff::from_lines(&from_text, &to_text);

    let mut added: u32 = 0;
    let mut removed: u32 = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    DiffStats { added, removed }
}
