//! Phase 2 indexer_v2 — shared types + tunables.
//!
//! `StatsDelta` is the single message format on the writer channel. The
//! orchestrator parses + extracts a [`SessionStats`] and packages it
//! together with the staleness header that lets `get_stats_header` skip
//! re-parsing on the next fsnotify event for the same file.

use claude_view_session_parser::SessionStats;

/// Fsnotify event coalesce window — applied per-`session_id` so a burst
/// of writes within `DEBOUNCE_MS` only triggers a single re-index.
///
/// 500 ms is the D1 sign-off value (Phase 1-7 design doc §1).
pub const DEBOUNCE_MS: u64 = 500;

/// Single payload pushed onto the writer channel after a successful
/// parse + extract. Owned (no borrows) so it's `Send + 'static` for
/// `mpsc::Sender::send`.
#[derive(Debug, Clone)]
pub struct StatsDelta {
    /// Session UUID — matches `session_stats.session_id`.
    pub session_id: String,
    /// blake3 head+tail of the source JSONL bytes at parse time.
    pub source_content_hash: Vec<u8>,
    /// Byte length of the source JSONL at parse time.
    pub source_size: i64,
    /// Filesystem inode at parse time (None on platforms without inodes).
    pub source_inode: Option<i64>,
    /// blake3 of the file's mid 64 KB (only computed for files >1 MiB).
    pub source_mid_hash: Option<Vec<u8>>,
    /// Parsed + extracted statistics that populate the 24 stats columns.
    pub stats: SessionStats,
}
