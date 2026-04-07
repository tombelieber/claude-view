//! Diff engine -- computes unified diff between two version files.

use similar::{ChangeTag, TextDiff};
use std::path::Path;

use super::types::{DiffHunk, DiffLine, DiffLineKind, DiffStats, FileDiffResponse};

/// Compute a structured diff between two backup files.
///
/// `context_lines` controls how many unchanged lines surround each change (default 3).
pub fn compute_diff(
    history_dir: &Path,
    session_id: &str,
    file_hash: &str,
    from_version: u32,
    to_version: u32,
    file_path: &str,
    context_lines: usize,
) -> Result<FileDiffResponse, String> {
    let session_dir = history_dir.join(session_id);
    let to_file = session_dir.join(format!("{file_hash}@v{to_version}"));

    // from_version=0 means "diff against empty" -- shows all lines as added
    let from_text = if from_version == 0 {
        String::new()
    } else {
        let from_file = session_dir.join(format!("{file_hash}@v{from_version}"));
        std::fs::read_to_string(&from_file)
            .map_err(|e| format!("Cannot read v{from_version}: {e}"))?
    };
    let to_text =
        std::fs::read_to_string(&to_file).map_err(|e| format!("Cannot read v{to_version}: {e}"))?;

    let diff = TextDiff::from_lines(&from_text, &to_text);
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut total_added: u32 = 0;
    let mut total_removed: u32 = 0;

    for group in diff.grouped_ops(context_lines) {
        let mut hunk_lines: Vec<DiffLine> = Vec::new();
        let mut old_start: u32 = 0;
        let mut new_start: u32 = 0;
        let mut old_count: u32 = 0;
        let mut new_count: u32 = 0;
        let mut first = true;

        // iter over each op in the group, then iter_changes on each op
        for op in &group {
            for change in diff.iter_changes(op) {
                let (old_line, new_line) = match change.tag() {
                    ChangeTag::Equal => {
                        old_count += 1;
                        new_count += 1;
                        let old_idx = change.old_index().map(|i| i as u32 + 1);
                        let new_idx = change.new_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = old_idx.unwrap_or(1);
                            new_start = new_idx.unwrap_or(1);
                            first = false;
                        }
                        (old_idx, new_idx)
                    }
                    ChangeTag::Delete => {
                        old_count += 1;
                        total_removed += 1;
                        let old_idx = change.old_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = old_idx.unwrap_or(1);
                            new_start = change.new_index().map(|i| i as u32 + 1).unwrap_or(1);
                            first = false;
                        }
                        (old_idx, None)
                    }
                    ChangeTag::Insert => {
                        new_count += 1;
                        total_added += 1;
                        let new_idx = change.new_index().map(|i| i as u32 + 1);
                        if first {
                            old_start = change.old_index().map(|i| i as u32 + 1).unwrap_or(1);
                            new_start = new_idx.unwrap_or(1);
                            first = false;
                        }
                        (None, new_idx)
                    }
                };

                let kind = match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Delete => DiffLineKind::Remove,
                    ChangeTag::Insert => DiffLineKind::Add,
                };

                // Strip trailing newline from content
                let content = change.value().trim_end_matches('\n').to_string();

                hunk_lines.push(DiffLine {
                    kind,
                    content,
                    old_line_no: old_line,
                    new_line_no: new_line,
                });
            }
        }

        if !hunk_lines.is_empty() {
            hunks.push(DiffHunk {
                old_start,
                old_lines: old_count,
                new_start,
                new_lines: new_count,
                lines: hunk_lines,
            });
        }
    }

    Ok(FileDiffResponse {
        file_path: file_path.to_string(),
        from_version,
        to_version,
        stats: DiffStats {
            added: total_added,
            removed: total_removed,
        },
        hunks,
    })
}

/// Resolve the ~/.claude/file-history/ directory.
pub fn claude_file_history_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("file-history"))
}
