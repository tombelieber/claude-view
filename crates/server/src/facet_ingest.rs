// crates/server/src/facet_ingest.rs
//! Lock-free atomic state for facet ingest progress tracking.
//!
//! Scans the Claude Code facet cache directory, diffs against the DB,
//! and inserts new facets. Progress is reported via atomic counters
//! for wait-free reads from SSE handlers.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use tracing::{info, warn};
use vibe_recall_core::facets::{default_facet_cache_path, scan_facet_cache};
use vibe_recall_db::{Database, FacetRow};

/// Status of the current facet ingest job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IngestStatus {
    Idle = 0,
    Scanning = 1,
    Ingesting = 2,
    Complete = 3,
    Error = 4,
    NoCacheFound = 5,
}

impl IngestStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Scanning,
            2 => Self::Ingesting,
            3 => Self::Complete,
            4 => Self::Error,
            5 => Self::NoCacheFound,
            _ => Self::Idle,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scanning => "scanning",
            Self::Ingesting => "ingesting",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::NoCacheFound => "no_cache_found",
        }
    }
}

/// Lock-free state for facet ingest progress tracking.
///
/// All fields use atomics for wait-free reads from the SSE handler.
pub struct FacetIngestState {
    status: AtomicU8,
    total: AtomicU64,
    ingested: AtomicU64,
    skipped: AtomicU64,
    new_facets: AtomicU64,
}

impl FacetIngestState {
    /// Create a new idle facet ingest state.
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(IngestStatus::Idle as u8),
            total: AtomicU64::new(0),
            ingested: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            new_facets: AtomicU64::new(0),
        }
    }

    /// Get the current status.
    pub fn status(&self) -> IngestStatus {
        IngestStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    /// Get the total number of facet files found.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Get the number of facets ingested so far.
    pub fn ingested(&self) -> u64 {
        self.ingested.load(Ordering::Relaxed)
    }

    /// Get the number of facets skipped (already in DB).
    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }

    /// Get the count of new facets inserted.
    pub fn new_facets(&self) -> u64 {
        self.new_facets.load(Ordering::Relaxed)
    }

    /// Returns `true` if ingest is currently running (Scanning or Ingesting).
    pub fn is_running(&self) -> bool {
        matches!(self.status(), IngestStatus::Scanning | IngestStatus::Ingesting)
    }

    /// Reset all counters to zero and status to Idle.
    pub fn reset(&self) {
        self.status.store(IngestStatus::Idle as u8, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.ingested.store(0, Ordering::Relaxed);
        self.skipped.store(0, Ordering::Relaxed);
        self.new_facets.store(0, Ordering::Relaxed);
    }
}

impl Default for FacetIngestState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan the facet cache directory, diff against the DB, and insert new facets.
///
/// Returns the number of new facets inserted.
///
/// # Arguments
///
/// * `db` - Database handle for querying existing facets and inserting new ones.
/// * `state` - Atomic progress state for SSE streaming.
/// * `cache_dir` - Optional override for the cache directory. Uses the default
///   Claude Code facet cache path if `None`.
pub async fn run_facet_ingest(
    db: &Database,
    state: &FacetIngestState,
    cache_dir: Option<&Path>,
) -> Result<u64, String> {
    // 1. Reset counters
    state.reset();

    // 2. Set status to Scanning
    state.status.store(IngestStatus::Scanning as u8, Ordering::Relaxed);

    // 3. Determine cache directory and scan
    let dir = match cache_dir {
        Some(d) => d.to_path_buf(),
        None => default_facet_cache_path(),
    };

    let cached_facets = scan_facet_cache(&dir).map_err(|e| {
        let msg = format!("Failed to scan facet cache at {}: {}", dir.display(), e);
        warn!("{}", msg);
        state.status.store(IngestStatus::Error as u8, Ordering::Relaxed);
        msg
    })?;

    // 4. If empty, set NoCacheFound and return
    if cached_facets.is_empty() {
        info!("No facet cache files found in {}", dir.display());
        state.status.store(IngestStatus::NoCacheFound as u8, Ordering::Relaxed);
        return Ok(0);
    }

    // 5. Set total and transition to Ingesting
    state.total.store(cached_facets.len() as u64, Ordering::Relaxed);
    state.status.store(IngestStatus::Ingesting as u8, Ordering::Relaxed);

    // 6. Get existing session IDs from DB
    let existing_ids: HashSet<String> = db
        .get_all_facet_session_ids()
        .await
        .map_err(|e| {
            let msg = format!("Failed to query existing facet IDs: {}", e);
            warn!("{}", msg);
            state.status.store(IngestStatus::Error as u8, Ordering::Relaxed);
            msg
        })?
        .into_iter()
        .collect();

    // 7. Filter to new facets only
    let mut new_rows: Vec<FacetRow> = Vec::new();
    let mut skipped_count: u64 = 0;

    for (session_id, facet) in &cached_facets {
        if existing_ids.contains(session_id) {
            skipped_count += 1;
        } else {
            // 8. Convert SessionFacet -> FacetRow
            let row = FacetRow {
                session_id: session_id.clone(),
                source: "insights_cache".to_string(),
                underlying_goal: facet.underlying_goal.clone(),
                goal_categories: serde_json::to_string(&facet.goal_categories)
                    .unwrap_or_else(|_| "{}".to_string()),
                outcome: facet.outcome.clone(),
                satisfaction: facet.dominant_satisfaction().map(|s| s.to_string()),
                user_satisfaction_counts: serde_json::to_string(
                    &facet.user_satisfaction_counts,
                )
                .unwrap_or_else(|_| "{}".to_string()),
                claude_helpfulness: facet.claude_helpfulness.clone(),
                session_type: facet.session_type.clone(),
                friction_counts: serde_json::to_string(&facet.friction_counts)
                    .unwrap_or_else(|_| "{}".to_string()),
                friction_detail: facet.friction_detail.clone(),
                primary_success: facet.primary_success.clone(),
                brief_summary: facet.brief_summary.clone(),
            };
            new_rows.push(row);
        }
    }

    state.skipped.store(skipped_count, Ordering::Relaxed);

    // 9. Batch upsert new facets
    let new_count = new_rows.len() as u64;

    if !new_rows.is_empty() {
        db.batch_upsert_facets(&new_rows).await.map_err(|e| {
            let msg = format!("Failed to upsert facets: {}", e);
            warn!("{}", msg);
            state.status.store(IngestStatus::Error as u8, Ordering::Relaxed);
            msg
        })?;
    }

    // 10. Update ingested counter AFTER DB write
    state.ingested.store(new_count, Ordering::Relaxed);
    state.new_facets.store(new_count, Ordering::Relaxed);

    // 11. Set status to Complete
    info!(
        "Facet ingest complete: {} new, {} skipped, {} total cached",
        new_count, skipped_count, cached_facets.len()
    );
    state.status.store(IngestStatus::Complete as u8, Ordering::Relaxed);

    // 12. Return new_count
    Ok(new_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_ingest_status_from_u8() {
        assert_eq!(IngestStatus::from_u8(0), IngestStatus::Idle);
        assert_eq!(IngestStatus::from_u8(1), IngestStatus::Scanning);
        assert_eq!(IngestStatus::from_u8(2), IngestStatus::Ingesting);
        assert_eq!(IngestStatus::from_u8(3), IngestStatus::Complete);
        assert_eq!(IngestStatus::from_u8(4), IngestStatus::Error);
        assert_eq!(IngestStatus::from_u8(5), IngestStatus::NoCacheFound);
        // Unknown values default to Idle
        assert_eq!(IngestStatus::from_u8(255), IngestStatus::Idle);
    }

    #[test]
    fn test_ingest_status_as_str() {
        assert_eq!(IngestStatus::Idle.as_str(), "idle");
        assert_eq!(IngestStatus::Scanning.as_str(), "scanning");
        assert_eq!(IngestStatus::Ingesting.as_str(), "ingesting");
        assert_eq!(IngestStatus::Complete.as_str(), "complete");
        assert_eq!(IngestStatus::Error.as_str(), "error");
        assert_eq!(IngestStatus::NoCacheFound.as_str(), "no_cache_found");
    }

    #[test]
    fn test_ingest_state_lifecycle() {
        let state = FacetIngestState::new();
        assert_eq!(state.status(), IngestStatus::Idle);
        assert!(!state.is_running());

        state.status.store(IngestStatus::Scanning as u8, Ordering::Relaxed);
        assert!(state.is_running());

        state.status.store(IngestStatus::Ingesting as u8, Ordering::Relaxed);
        assert!(state.is_running());

        state.status.store(IngestStatus::Complete as u8, Ordering::Relaxed);
        assert!(!state.is_running());

        state.reset();
        assert_eq!(state.status(), IngestStatus::Idle);
        assert_eq!(state.total(), 0);
        assert_eq!(state.ingested(), 0);
        assert_eq!(state.skipped(), 0);
        assert_eq!(state.new_facets(), 0);
    }

    #[test]
    fn test_ingest_state_default() {
        let state = FacetIngestState::default();
        assert_eq!(state.status(), IngestStatus::Idle);
        assert_eq!(state.total(), 0);
    }

    fn sample_facet_json() -> &'static str {
        r#"{
            "underlying_goal": "Implement a new feature",
            "goal_categories": {"coding": 3, "debugging": 1},
            "outcome": "fully_achieved",
            "user_satisfaction_counts": {"satisfied": 5, "neutral": 2},
            "claude_helpfulness": "very_helpful",
            "session_type": "single_task",
            "friction_counts": {"slow_response": 1},
            "friction_detail": "Response was slow on large files",
            "primary_success": "Feature implemented correctly",
            "brief_summary": "User implemented a new caching feature with Claude's help",
            "session_id": "abc-123"
        }"#
    }

    #[tokio::test]
    async fn test_run_facet_ingest_empty_dir() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = FacetIngestState::new();
        let dir = TempDir::new().unwrap();

        let result = run_facet_ingest(&db, &state, Some(dir.path())).await;
        assert_eq!(result.unwrap(), 0);
        assert_eq!(state.status(), IngestStatus::NoCacheFound);
    }

    #[tokio::test]
    async fn test_run_facet_ingest_nonexistent_dir() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = FacetIngestState::new();
        let dir = Path::new("/nonexistent/facets/path");

        let result = run_facet_ingest(&db, &state, Some(dir)).await;
        assert_eq!(result.unwrap(), 0);
        assert_eq!(state.status(), IngestStatus::NoCacheFound);
    }

    #[tokio::test]
    async fn test_run_facet_ingest_with_new_facets() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = FacetIngestState::new();
        let dir = TempDir::new().unwrap();

        // Write 2 facet files
        fs::write(dir.path().join("sess-001.json"), sample_facet_json()).unwrap();
        fs::write(dir.path().join("sess-002.json"), sample_facet_json()).unwrap();

        let result = run_facet_ingest(&db, &state, Some(dir.path())).await;
        assert_eq!(result.unwrap(), 2);
        assert_eq!(state.status(), IngestStatus::Complete);
        assert_eq!(state.total(), 2);
        assert_eq!(state.ingested(), 2);
        assert_eq!(state.new_facets(), 2);
        assert_eq!(state.skipped(), 0);
    }

    #[tokio::test]
    async fn test_run_facet_ingest_skips_existing() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = FacetIngestState::new();
        let dir = TempDir::new().unwrap();

        // Write 2 facet files
        fs::write(dir.path().join("sess-001.json"), sample_facet_json()).unwrap();
        fs::write(dir.path().join("sess-002.json"), sample_facet_json()).unwrap();

        // Pre-insert sess-001 into DB
        let existing_row = FacetRow {
            session_id: "sess-001".to_string(),
            source: "insights_cache".to_string(),
            underlying_goal: Some("Already exists".to_string()),
            goal_categories: "{}".to_string(),
            outcome: None,
            satisfaction: None,
            user_satisfaction_counts: "{}".to_string(),
            claude_helpfulness: None,
            session_type: None,
            friction_counts: "{}".to_string(),
            friction_detail: None,
            primary_success: None,
            brief_summary: None,
        };
        db.batch_upsert_facets(&[existing_row]).await.unwrap();

        let result = run_facet_ingest(&db, &state, Some(dir.path())).await;
        assert_eq!(result.unwrap(), 1); // only sess-002 is new
        assert_eq!(state.status(), IngestStatus::Complete);
        assert_eq!(state.total(), 2);
        assert_eq!(state.ingested(), 1);
        assert_eq!(state.new_facets(), 1);
        assert_eq!(state.skipped(), 1);
    }

    #[tokio::test]
    async fn test_run_facet_ingest_resets_on_rerun() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let state = FacetIngestState::new();
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("sess-001.json"), sample_facet_json()).unwrap();

        // First run
        let result1 = run_facet_ingest(&db, &state, Some(dir.path())).await;
        assert_eq!(result1.unwrap(), 1);
        assert_eq!(state.new_facets(), 1);

        // Second run — sess-001 already exists, so 0 new
        let result2 = run_facet_ingest(&db, &state, Some(dir.path())).await;
        assert_eq!(result2.unwrap(), 0);
        assert_eq!(state.status(), IngestStatus::Complete);
        assert_eq!(state.total(), 1);
        assert_eq!(state.ingested(), 0);
        assert_eq!(state.new_facets(), 0);
        assert_eq!(state.skipped(), 1);
    }
}
