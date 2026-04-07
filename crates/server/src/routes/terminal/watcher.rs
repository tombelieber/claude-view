//! File watcher for JSONL live streaming.
//!
//! Watches a JSONL file for modifications and sends events through
//! an async channel for the WebSocket event loop to consume.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use super::types::WatchEvent;

/// Start a notify watcher for a single JSONL file.
///
/// Watches the file's parent directory (notify cannot watch individual files
/// on all platforms) and filters events to only the target file.
/// Modified events are sent through the `mpsc::Sender<WatchEvent>` channel.
pub(crate) fn start_file_watcher(
    file_path: &std::path::Path,
    tx: mpsc::Sender<WatchEvent>,
) -> notify::Result<RecommendedWatcher> {
    // Canonicalize the target path so that the comparison against event paths
    // works on macOS where symlinks like /var -> /private/var cause mismatches
    // (e.g. NamedTempFile returns /var/folders/... but FSEvents reports
    // /private/var/folders/...).
    let canonical_path = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    let target_for_closure = canonical_path.clone();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Filter to only events for our target file
                    let is_target = event.paths.iter().any(|p| p == &target_for_closure);
                    if !is_target {
                        return;
                    }

                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            // Best-effort send; if the channel is full, skip this event
                            // (the next modify event will pick up all new lines)
                            let _ = tx.try_send(WatchEvent::Modified);
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    let _ = tx.try_send(WatchEvent::Error(e.to_string()));
                }
            }
        })?;

    // Watch the parent directory since notify may not support watching
    // individual files on all platforms (e.g., macOS FSEvents).
    // Use the canonical path's parent so the watched directory
    // matches the resolved event paths.
    let watch_dir = canonical_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;

    Ok(watcher)
}
