//! TUI progress task — prints the "Ready" line after Pass 1, then streams a
//! deep-indexing spinner while Pass 2 runs in the background.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Messaging (wording,
//! characters, throughput formatting) is unchanged so existing docs and
//! screenshots stay valid.

use std::sync::Arc;
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressStyle};

use crate::startup::background::format_bytes;
use crate::{IndexingState, IndexingStatus};

/// Spawn the TUI task. `startup_start` is the `Instant` captured at the top
/// of `main()` so the "Ready" duration matches user-perceived wall time.
pub fn spawn_tui_task(indexing: Arc<IndexingState>, startup_start: Instant, port: u16) {
    let tui_state = indexing;
    tokio::spawn(async move {
        // Wait for Pass 1 to complete (status transitions out of Idle/ReadingIndexes)
        loop {
            let status = tui_state.status();
            if status != IndexingStatus::Idle && status != IndexingStatus::ReadingIndexes {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Wait briefly for session data when hints report 0 sessions.
        // on_total_known fires during scan_and_index_all with the filesystem count —
        // typically within a few hundred ms of entering DeepIndexing.
        if tui_state.sessions_found() == 0 {
            for _ in 0..20 {
                let status = tui_state.status();
                if status == IndexingStatus::Done
                    || status == IndexingStatus::Error
                    || tui_state.sessions_found() > 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Print the "Ready" line with Pass 1 results.
        // Use max(sessions_found, total) — same defense-in-depth as the SSE ready event.
        let elapsed = startup_start.elapsed();
        let projects = tui_state.projects_found();
        let sessions = std::cmp::max(tui_state.sessions_found(), tui_state.total());
        eprintln!(
            "  \u{2713} Ready in {} \u{2014} {} projects, {} sessions",
            claude_view_core::format_duration(elapsed),
            projects,
            sessions,
        );
        // In dev mode, open the Vite dev server; otherwise open the server directly.
        // VITE_PORT env var or RUST_LOG presence signals dev mode.
        let browse_url = std::env::var("VITE_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .map(|vite_port| format!("http://localhost:{}", vite_port))
            .unwrap_or_else(|| format!("http://localhost:{}", port));
        eprintln!("  \u{2192} {}\n", browse_url);

        // Auto-open browser on first startup only (not cargo-watch restarts).
        // We detect restarts via a lock file that persists across restarts.
        let lock_dir = claude_view_core::paths::lock_dir().unwrap_or_else(std::env::temp_dir);
        let _ = std::fs::create_dir_all(&lock_dir);
        let lock_path = lock_dir.join(format!("claude-view-{}.lock", port));
        let should_open = if lock_path.exists() {
            // Lock exists — check if it's stale (older than 5 seconds means fresh start)
            lock_path
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(5))
                .unwrap_or(true)
        } else {
            true
        };
        // Touch the lock file
        let _ = std::fs::write(&lock_path, b"");

        // Only open browser if not suppressed (hook starts set CLAUDE_VIEW_NO_OPEN=1)
        if should_open && std::env::var("CLAUDE_VIEW_NO_OPEN").unwrap_or_default() != "1" {
            if let Err(e) = open::that(&browse_url) {
                tracing::debug!("Could not open browser: {e}");
            }
        }

        // Show deep indexing spinner if Pass 2 is running
        if tui_state.status() == IndexingStatus::DeepIndexing {
            run_spinner(&tui_state).await;
        } else if tui_state.status() == IndexingStatus::Error {
            if let Some(err) = tui_state.error() {
                eprintln!("  \u{2717} Indexing error: {}\n", err);
            }
        }
    });
}

async fn run_spinner(tui_state: &Arc<IndexingState>) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner} Deep indexing {msg}")
            .expect("valid spinner template"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let deep_start = Instant::now();
    loop {
        let status = tui_state.status();
        if status == IndexingStatus::Done || status == IndexingStatus::Error {
            break;
        }
        let indexed = tui_state.indexed();
        let total = tui_state.total();
        let bp = tui_state.bytes_processed();
        let bt = tui_state.bytes_total();
        if total > 0 {
            let elapsed_secs = deep_start.elapsed().as_secs_f64();
            let throughput = if elapsed_secs > 0.1 {
                format!("  ({}/s)", format_bytes((bp as f64 / elapsed_secs) as u64))
            } else {
                String::new()
            };
            if bt > 0 {
                pb.set_message(format!(
                    "{} / {}{}  {}/{} sessions...",
                    format_bytes(bp),
                    format_bytes(bt),
                    throughput,
                    indexed,
                    total,
                ));
            } else {
                pb.set_message(format!("{}/{} sessions...", indexed, total));
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    pb.finish_and_clear();

    if tui_state.status() == IndexingStatus::Done {
        let deep_elapsed = deep_start.elapsed();
        let total = tui_state.sessions_found();
        let bp = tui_state.bytes_processed();
        eprintln!(
            "  \u{2713} Deep index complete \u{2014} {} sessions, {} processed ({})\n",
            total,
            format_bytes(bp),
            claude_view_core::format_duration(deep_elapsed),
        );
    } else if let Some(err) = tui_state.error() {
        eprintln!("  \u{2717} Indexing error: {}\n", err);
    }
}
