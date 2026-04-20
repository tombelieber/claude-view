//! Periodic background tasks — git sync + contribution-snapshot refresh.
//!
//! Extracted from `main.rs` in CQRS Phase 7.c. The metrics shape
//! (`record_sync`) and structured-log fields are unchanged so existing
//! dashboards keep working.

use std::time::Instant;

use claude_view_db::Database;

use crate::record_sync;

/// Run git sync with structured logging. Used by both initial and periodic sync.
pub async fn run_git_sync_logged(db: &Database, label: &str) {
    let start = Instant::now();
    tracing::info!(sync_type = label, "Starting git sync");

    match claude_view_db::git_correlation::run_git_sync(db, |_| {}).await {
        Ok(r) => {
            let duration = start.elapsed();
            if r.repos_scanned > 0 || r.links_created > 0 {
                tracing::info!(
                    sync_type = label,
                    repos_scanned = r.repos_scanned,
                    commits_found = r.commits_found,
                    links_created = r.links_created,
                    duration_secs = duration.as_secs_f64(),
                    "Git sync complete"
                );
            } else {
                tracing::debug!(
                    sync_type = label,
                    duration_secs = duration.as_secs_f64(),
                    "Git sync: no changes"
                );
            }
            if !r.errors.is_empty() {
                tracing::warn!(
                    sync_type = label,
                    error_count = r.errors.len(),
                    errors = ?r.errors,
                    "Git sync had errors"
                );
            }
            record_sync("git", duration, Some(r.commits_found as u64));
        }
        Err(e) => {
            let duration = start.elapsed();
            tracing::warn!(
                sync_type = label,
                error = %e,
                duration_secs = duration.as_secs_f64(),
                "Git sync failed (non-fatal)"
            );
            record_sync("git", duration, None);
        }
    }
}

/// Generate contribution snapshots for historical days.
/// Initial run refreshes 365 days; periodic runs refresh 2 days
/// (today + yesterday).
pub async fn run_snapshot_generation(db: &Database, label: &str) {
    let days_back = if label == "initial" { 365 } else { 2 };
    match db.generate_missing_snapshots(days_back).await {
        Ok(count) => {
            if count > 0 {
                tracing::info!("{} snapshot refresh: {} snapshots updated", label, count);
            } else {
                tracing::debug!("{} snapshot refresh: no active dates in range", label);
            }
        }
        Err(e) => {
            tracing::warn!("{} snapshot generation failed (non-fatal): {}", label, e);
        }
    }
}

/// Format a byte count as a human-readable string (e.g. "23.4 GB", "512 MB").
pub fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
