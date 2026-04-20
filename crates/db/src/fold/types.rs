//! Types consumed + produced by the PR 5.3 fold task.
//!
//! `ActionEvent` mirrors one row of `session_action_log`. `ClassifyPayload`
//! is the JSON shape the classify / reclassify actions serialise (PR 5.2
//! writes this in `batch_update_session_classifications`).

use serde::Deserialize;

/// One row of `session_action_log` as consumed by the fold task.
#[derive(Debug, Clone)]
pub struct ActionEvent {
    pub seq: i64,
    pub session_id: String,
    pub action: String,
    pub payload: String,
    #[allow(dead_code)] // PR 5.4 shadow parity joins on actor.
    pub actor: String,
    /// Unix ms — used as the LWW timestamp on classify events.
    pub at: i64,
}

/// JSON body serialised by PR 5.2's classify writer.
///
/// Fields mirror `session_flags.category_*`. Missing l2 / l3 are tolerated
/// (classifier may emit just l1 on low-confidence runs); serde(default)
/// keeps the fold robust to payload drift.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassifyPayload {
    pub l1: String,
    #[serde(default)]
    pub l2: String,
    #[serde(default)]
    pub l3: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub source: String,
}

/// Return value of a single fold-batch pass.
///
/// `rows_applied` is the number of events whose fold actually ran;
/// `rows_skipped_lww` is events the fold deliberately ignored because a
/// newer classification was already on `session_flags`. `max_seq` is the
/// highest `seq` observed — equal to the new `applied_seq` after commit.
#[derive(Debug, Clone, Default)]
pub struct FoldBatchSummary {
    pub rows_observed: u64,
    pub rows_applied: u64,
    pub rows_skipped_lww: u64,
    pub rows_skipped_unknown: u64,
    pub max_seq: i64,
}
