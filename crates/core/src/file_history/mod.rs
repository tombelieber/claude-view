//! Scanner and diff engine for ~/.claude/file-history/{sessionId}/{hash}@v{N}.

mod diff;
mod helpers;
mod jsonl;
mod scanner;
mod types;

#[cfg(test)]
mod tests;

pub use diff::{claude_file_history_dir, compute_diff};
pub use helpers::validate_file_hash;
pub use jsonl::extract_file_path_map;
pub use scanner::scan_file_history;
pub use types::{
    DiffHunk, DiffLine, DiffLineKind, DiffStats, DiffSummary, FileChange, FileDiffResponse,
    FileHistoryResponse, FileVersion,
};
