//! Retired indexer_v2 drift comparator.
//!
//! CQRS Phase 7.h makes `session_stats` the sole session row table, so there
//! is no legacy row left to compare. The public API remains as a no-op for the
//! startup sampler and old parity harnesses.

use crate::{Database, DbResult};

/// Single per-field drift between legacy and shadow rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    /// Stable identifier — used as the `field` label by old drift consumers.
    pub field: &'static str,
    /// Stringified legacy value.
    pub legacy: String,
    /// Stringified shadow value.
    pub shadow: String,
}

/// Drift comparison between the retired legacy row and `session_stats`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftReport {
    pub session_id: String,
    pub diffs: Vec<FieldDiff>,
}

impl DriftReport {
    pub fn is_clean(&self) -> bool {
        self.diffs.is_empty()
    }
}

pub async fn compare_session(db: &Database, session_id: &str) -> DbResult<Option<DriftReport>> {
    let _ = (db, session_id);
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compare_is_noop_after_legacy_table_retirement() {
        let db = Database::new_in_memory().await.unwrap();
        let report = compare_session(&db, "ghost").await.unwrap();
        assert!(report.is_none());
    }
}
