//! Stage B — `SessionDoc` → `SessionStats`.
//!
//! Reduction stage: counts user prompts, dedups assistant messages by
//! ID and picks last-occurrence usage, rolls up tool-call breakdowns,
//! primary model, timestamps, duration, git branch, and preview/last
//! message text.
//!
//! The heavy lifting lives in `claude_view_core::session_stats::compute_stats`
//! so the v1 and v2 code paths stay byte-identical on the same input —
//! only the *input* source differs (JSONL on disk vs `SessionDoc`).
//!
//! The `StatsVersion` argument is currently unused; it exists so the
//! signature is stable when a future extraction change needs per-
//! version branching.

use claude_view_core::session_stats::{compute_stats, SessionStats};

use crate::doc::SessionDoc;
use crate::version::StatsVersion;

/// Reduce a parsed `SessionDoc` into a `SessionStats` snapshot.
///
/// Pure function; no I/O, no allocation beyond what `compute_stats`
/// already performs.
pub fn extract_stats(doc: &SessionDoc, _v: StatsVersion) -> SessionStats {
    compute_stats(&doc.lines)
}
