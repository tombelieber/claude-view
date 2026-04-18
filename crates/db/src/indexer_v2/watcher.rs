//! fsnotify watcher for `~/.claude/projects/` — emits `FileEvent`s on
//! parent-session JSONL changes.
//!
//! This is the indexer_v2-owned watcher (Option B per the Phase 2
//! handoff). It runs *in parallel* with the live manager's watcher in
//! `crates/server/src/live/watcher.rs`. Both share kernel-level fsnotify
//! resources, so the OS overhead of running two `notify::Watcher`s on
//! the same root is negligible (the kernel coalesces inode-level
//! subscriptions). The trade-off is one duplicated user-space callback
//! invocation per event versus the architectural cost of refactoring
//! the load-bearing live manager — Option B chose the cheaper trade.
//!
//! ## Path filtering
//!
//! Only **parent session** JSONL files are forwarded. The Claude Code
//! tree layout is:
//!
//! ```text
//! ~/.claude/projects/
//! ├── {project}/
//! │   ├── {sessionId}.jsonl              ← depth 2 — WATCH
//! │   ├── {sessionId}/
//! │   │   ├── subagents/agent-*.jsonl    ← depth 4 — IGNORE
//! │   │   └── tool-results/*.txt         ← depth 4 — IGNORE
//! ```
//!
//! Filtering by depth (rather than substring matching on path components)
//! keeps the rule structural and robust to directory renames.
//!
//! ## Backpressure
//!
//! The mpsc channel is bounded at 512. On overflow the dropped-event
//! counter is bumped and a warning is logged at 1, 100, 200… so the
//! orchestrator can decide whether to trigger a `Rescan`. fsnotify
//! itself can also overflow at the kernel level (`EventKind::Other`),
//! and we forward those as `FileEvent::Rescan` directly.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{error, warn};

/// Events emitted to the orchestrator. Mirrors the live watcher's enum
/// shape so callers can read either one with the same handler skeleton.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A parent session JSONL file was created or modified.
    Modified(PathBuf),
    /// A parent session JSONL file was removed from disk.
    Removed(PathBuf),
    /// fsnotify queue overflowed — caller should trigger a full rescan.
    Rescan,
}

/// Default channel capacity. Same value the live watcher picks; large
/// enough that bursty appends during an active session don't overflow,
/// small enough that backlog can't grow unbounded if the orchestrator
/// stalls (which would be a bug, not a steady state).
pub const FILE_EVENT_CHANNEL_CAPACITY: usize = 512;

/// Start a fsnotify watcher rooted at `projects_dir` and forward filtered
/// events through `tx`.
///
/// Returns the watcher handle (which **must be kept alive** for the
/// duration of monitoring — dropping it stops the watch) plus an atomic
/// counter the orchestrator can poll for dropped-event backpressure.
///
/// If `projects_dir` does not exist, returns a watcher that watches
/// nothing (still valid; useful at first-run startup).
pub fn start_watcher(
    projects_dir: PathBuf,
    tx: mpsc::Sender<FileEvent>,
) -> notify::Result<(RecommendedWatcher, Arc<AtomicU64>)> {
    let dropped_events = Arc::new(AtomicU64::new(0));
    let dropped_counter = dropped_events.clone();

    // Canonicalize so the depth-2 filter compares apples-to-apples.
    // On macOS `std::env::temp_dir()` returns `/var/folders/...` but
    // `notify` receives canonical `/private/var/folders/...` paths;
    // without this normalization the `strip_prefix` filter rejects
    // every event silently. Production paths under `~/.claude/projects/`
    // are typically already canonical but the call is cheap and idempotent.
    let root_for_filter = projects_dir
        .canonicalize()
        .unwrap_or_else(|_| projects_dir.clone());

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                // Kernel queue overflow → request full rescan. Must
                // come before path filtering: overflow events carry no
                // paths and would otherwise be dropped silently.
                if event.kind == EventKind::Other {
                    if tx.try_send(FileEvent::Rescan).is_err() {
                        warn!("indexer_v2 watcher channel full — Rescan event dropped");
                    }
                    return;
                }

                let jsonl_paths: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|p| is_parent_session_jsonl(p, &root_for_filter))
                    .collect();

                for path in jsonl_paths {
                    let file_event = match event.kind {
                        EventKind::Remove(_) => FileEvent::Removed(path),
                        EventKind::Modify(_) | EventKind::Create(_) => FileEvent::Modified(path),
                        _ => continue,
                    };
                    if tx.try_send(file_event).is_err() {
                        let count = dropped_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        if count == 1 || count.is_multiple_of(100) {
                            warn!(
                                dropped_total = count,
                                "indexer_v2 watcher channel full — event dropped"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                error!("indexer_v2 watcher error: {e} — requesting rescan");
                if tx.try_send(FileEvent::Rescan).is_err() {
                    warn!("indexer_v2 watcher channel full — error-rescan dropped");
                }
            }
        })?;

    if projects_dir.exists() {
        watcher.watch(&projects_dir, RecursiveMode::Recursive)?;
        tracing::info!(
            projects_dir = %projects_dir.display(),
            "indexer_v2 fsnotify watcher started"
        );
    } else {
        warn!(
            projects_dir = %projects_dir.display(),
            "indexer_v2 watcher: projects dir missing — watcher is idle"
        );
    }

    Ok((watcher, dropped_events))
}

/// Returns `true` for paths matching `{root}/{project}/{sessionId}.jsonl`
/// (depth 2 from `root`, `.jsonl` extension). This rejects subagent and
/// tool-result files that live deeper in the tree.
fn is_parent_session_jsonl(path: &Path, root: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return false;
    }
    match path.strip_prefix(root) {
        Ok(rel) => rel.components().count() == 2,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/home/user/.claude/projects")
    }

    #[test]
    fn parent_session_paths_are_accepted() {
        let r = root();
        assert!(is_parent_session_jsonl(
            &r.join("my-project").join("abc123.jsonl"),
            &r
        ));
        assert!(is_parent_session_jsonl(
            &r.join("another-project")
                .join("11111111-2222-3333-4444-555555555555.jsonl"),
            &r
        ));
    }

    #[test]
    fn subagent_paths_are_rejected() {
        let r = root();
        let subagent = r
            .join("my-project")
            .join("abc123")
            .join("subagents")
            .join("agent-foo.jsonl");
        assert!(!is_parent_session_jsonl(&subagent, &r));
    }

    #[test]
    fn tool_result_paths_are_rejected() {
        let r = root();
        let tool_result = r
            .join("my-project")
            .join("abc123")
            .join("tool-results")
            .join("toolu_xyz.txt");
        assert!(!is_parent_session_jsonl(&tool_result, &r));
    }

    #[test]
    fn non_jsonl_extensions_are_rejected() {
        let r = root();
        assert!(!is_parent_session_jsonl(
            &r.join("proj").join("README.md"),
            &r
        ));
        assert!(!is_parent_session_jsonl(
            &r.join("proj").join("session.txt"),
            &r
        ));
    }

    #[test]
    fn paths_outside_root_are_rejected() {
        let r = root();
        let outside = PathBuf::from("/tmp/other.jsonl");
        assert!(!is_parent_session_jsonl(&outside, &r));
    }

    #[test]
    fn start_watcher_on_missing_dir_returns_idle_watcher() {
        // Should not error even if the directory doesn't exist —
        // mirrors the live watcher's first-run behaviour.
        let (tx, _rx) = mpsc::channel::<FileEvent>(8);
        let result = start_watcher(PathBuf::from("/no/such/projects/dir"), tx);
        assert!(
            result.is_ok(),
            "start_watcher must tolerate a missing root, got {:?}",
            result.err()
        );
    }
}
