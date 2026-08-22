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

/// Start a fsnotify watcher over every root in `projects_dirs` and forward
/// filtered events through `tx`.
///
/// Returns the watcher handle (which **must be kept alive** for the
/// duration of monitoring — dropping it stops the watch) plus an atomic
/// counter the orchestrator can poll for dropped-event backpressure.
///
/// A root that does not exist is skipped with a warning rather than
/// failing the call, so an empty or partially-present set still yields a
/// valid idle watcher (useful at first-run startup). One `notify` watcher
/// serves all roots; each is registered with a separate `watch()` call.
pub fn start_watcher(
    projects_dirs: Vec<PathBuf>,
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
    let roots_for_filter: Vec<PathBuf> = projects_dirs
        .iter()
        .map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.clone()))
        .collect();

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
                    .filter(|p| is_parent_session_jsonl_any(p, &roots_for_filter))
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

    let mut watched = 0usize;
    for projects_dir in &projects_dirs {
        if projects_dir.exists() {
            watcher.watch(projects_dir, RecursiveMode::Recursive)?;
            watched += 1;
            tracing::info!(
                projects_dir = %projects_dir.display(),
                "indexer_v2 fsnotify watcher started"
            );
        } else {
            warn!(
                projects_dir = %projects_dir.display(),
                "indexer_v2 watcher: projects dir missing — not watched"
            );
        }
    }

    if watched == 0 {
        warn!(
            roots = projects_dirs.len(),
            "indexer_v2 watcher: no existing projects dir — watcher is idle"
        );
    }

    Ok((watcher, dropped_events))
}

/// Returns `true` when `path` is a parent-session JSONL under **any** of
/// `roots`.
///
/// Roots are not assumed to be disjoint: a user may point two config dirs
/// at the same tree via symlinks. Matching stops at the first hit, so an
/// overlapping root cannot produce duplicate events.
fn is_parent_session_jsonl_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .any(|root| is_parent_session_jsonl(path, root))
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
        let result = start_watcher(vec![PathBuf::from("/no/such/projects/dir")], tx);
        assert!(
            result.is_ok(),
            "start_watcher must tolerate a missing root, got {:?}",
            result.err()
        );
    }

    #[test]
    fn start_watcher_with_no_roots_is_idle_not_an_error() {
        let (tx, _rx) = mpsc::channel::<FileEvent>(8);
        let result = start_watcher(Vec::new(), tx);
        assert!(
            result.is_ok(),
            "an empty root set must yield an idle watcher, got {:?}",
            result.err()
        );
    }

    #[test]
    fn multi_root_filter_accepts_each_root() {
        let primary = PathBuf::from("/home/user/.claude/projects");
        let secondary = PathBuf::from("/home/user/.claude-work/projects");
        let roots = vec![primary.clone(), secondary.clone()];

        assert!(is_parent_session_jsonl_any(
            &primary.join("proj").join("a.jsonl"),
            &roots
        ));
        assert!(is_parent_session_jsonl_any(
            &secondary.join("proj").join("b.jsonl"),
            &roots
        ));
        // Still depth-2 only, per root.
        assert!(!is_parent_session_jsonl_any(
            &secondary
                .join("proj")
                .join("b")
                .join("subagents")
                .join("agent.jsonl"),
            &roots
        ));
        // Outside every root.
        assert!(!is_parent_session_jsonl_any(
            &PathBuf::from("/tmp/other.jsonl"),
            &roots
        ));
    }
}
