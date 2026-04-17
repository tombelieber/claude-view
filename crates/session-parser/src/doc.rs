//! Stage A output: a parsed session as a vector of typed lines.
//!
//! `SessionDoc` is the input to Stage B (`extract_stats`) and, after
//! further reduction, Stage C (rollups). Keeping it as a typed `Vec`
//! rather than a raw JSONL string means every downstream consumer pays
//! the parse cost exactly once.

use claude_view_core::session_stats::StatsLine;

/// A parsed JSONL session document.
pub struct SessionDoc {
    /// Deserialised `StatsLine` entries in file order.
    pub lines: Vec<StatsLine>,
}
