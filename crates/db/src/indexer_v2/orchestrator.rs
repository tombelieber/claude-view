//! Phase 2 indexer_v2 orchestrator — single-file index helper +
//! fsnotify shadow-indexer driver.
//!
//! Two public entry points:
//!
//! 1. [`index_session`] — synchronous per-file work unit: parse JSONL,
//!    extract stats, upsert into `session_stats`. Owns no I/O of its
//!    own beyond reading the source file and the single SQL UPSERT
//!    performed by [`super::writer::upsert_session_stats`].
//!
//! 2. [`spawn_shadow_indexer`] — long-running background task: watches
//!    `~/.claude/projects/` for parent-session JSONL changes, debounces
//!    bursts per session ID, hash-gates redundant work, and re-indexes
//!    each changed file via `index_session`.
//!
//! ## Why a parallel watcher (Option B)
//!
//! The Phase 2 design (handoff §3.2) outlined two ways to drive
//! indexer_v2 from fsnotify events: (A) tap the live manager's mpsc
//! channel via a broadcast bus, or (B) spin up an independent
//! `notify::Watcher` rooted at the same directory. We picked (B) for
//! Phase 2 because:
//!
//! * It avoids touching the load-bearing live manager (least-risk
//!   change).
//! * fsnotify's per-inode subscriptions are kernel-coalesced, so two
//!   user-space watchers on the same root don't double the OS cost —
//!   only the user-space callback fires twice, which is microseconds.
//! * If Phase 7 needs the broadcast contract for ops fan-out, we can
//!   promote (B) to (A) without changing this module's public API.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use claude_view_session_parser::{
    blake3_head_tail, extract_stats, parse_jsonl, PARSER_VERSION, STATS_VERSION,
};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::config::{StatsDelta, DEBOUNCE_MS};
use super::debouncer::Debouncer;
use super::watcher::{start_watcher, FileEvent, FILE_EVENT_CHANNEL_CAPACITY};
use super::writer::upsert_session_stats;
use crate::{Database, DbError};

/// File-size threshold (1 MiB) above which the orchestrator computes a
/// blake3 hash of the file's middle 64 KB in addition to head+tail.
/// Mirrors design decision D2 — incremental parse only kicks in for
/// files >1 MB initially.
const MID_HASH_THRESHOLD_BYTES: u64 = 1 << 20; // 1 MiB

/// Parse one JSONL session file and upsert the resulting stats into
/// `session_stats`.
///
/// `session_id` is derived from the file stem (Claude Code's JSONL
/// filenames are the session UUID by convention). Pass it explicitly
/// rather than re-deriving inside the orchestrator so callers control
/// the source-of-truth and tests can inject synthetic IDs.
///
/// Returns `Ok(())` on a successful UPSERT or one of:
///   - [`IndexSessionError::Io`]              — could not read the file
///   - [`IndexSessionError::Parse`]           — parser refused the JSONL
///   - [`IndexSessionError::Db`]              — UPSERT failed
pub async fn index_session(
    db: &Database,
    path: &Path,
    session_id: &str,
) -> Result<(), IndexSessionError> {
    let bytes = std::fs::read(path)?;
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();

    let doc = parse_jsonl(&bytes, PARSER_VERSION)?;
    let stats = extract_stats(&doc, STATS_VERSION);

    let head_tail = claude_view_session_parser::blake3_head_tail(path)?.to_vec();
    let mid_hash = if size > MID_HASH_THRESHOLD_BYTES {
        Some(claude_view_session_parser::blake3_mid(path)?.to_vec())
    } else {
        None
    };

    let delta = StatsDelta {
        session_id: session_id.to_string(),
        source_content_hash: head_tail,
        source_size: i64::try_from(size).unwrap_or(i64::MAX),
        source_inode: file_inode(&metadata),
        source_mid_hash: mid_hash,
        stats,
    };

    upsert_session_stats(db, &delta).await?;
    Ok(())
}

/// Default fsnotify root: `~/.claude/projects/`. Returned as `None` if
/// `HOME` is unset (CI, broken environments) so callers can no-op
/// instead of crashing.
pub fn default_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

/// Spawn the indexer_v2 shadow-indexer task and return its handle.
///
/// Runs forever (until the watcher channel closes — i.e. process
/// shutdown). The returned [`JoinHandle`] is informational; dropping it
/// does not stop the task. `notify::Watcher` is moved into the task and
/// kept alive there for the duration of monitoring.
///
/// Uses `default_projects_dir()` for the watch root. If `HOME` is
/// unavailable, logs a warning and returns a handle to an immediately-
/// completed task (no watcher started, no indexer running).
///
/// For test injection or non-default roots, see
/// [`spawn_shadow_indexer_with_root`].
pub fn spawn_shadow_indexer(db: Arc<Database>) -> JoinHandle<()> {
    match default_projects_dir() {
        Some(root) => spawn_shadow_indexer_with_root(db, root),
        None => {
            tracing::warn!(
                "indexer_v2: HOME not set — shadow indexer disabled. \
                 session_stats will not auto-update."
            );
            tokio::spawn(async {})
        }
    }
}

/// Same as [`spawn_shadow_indexer`] but takes an explicit watch root.
/// Used by integration tests that want to point the indexer at a
/// tempdir instead of the user's `~/.claude/projects/`.
pub fn spawn_shadow_indexer_with_root(db: Arc<Database>, projects_dir: PathBuf) -> JoinHandle<()> {
    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::channel::<FileEvent>(FILE_EVENT_CHANNEL_CAPACITY);
        let (_watcher_handle, dropped) = match start_watcher(projects_dir.clone(), tx) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::error!(error = %e, "indexer_v2: start_watcher failed; shadow indexer exiting");
                return;
            }
        };

        let debouncer: Debouncer<String> = Debouncer::new(Duration::from_millis(DEBOUNCE_MS));
        let mut last_dropped = 0u64;

        tracing::info!(
            projects_dir = %projects_dir.display(),
            debounce_ms = DEBOUNCE_MS,
            "indexer_v2 shadow indexer running"
        );

        while let Some(event) = rx.recv().await {
            let cur_dropped = dropped.load(Ordering::Relaxed);
            if cur_dropped > last_dropped {
                last_dropped = cur_dropped;
                tracing::warn!(
                    dropped_total = cur_dropped,
                    "indexer_v2: cumulative dropped events"
                );
            }

            match event {
                FileEvent::Modified(path) => {
                    let Some(sid) = path.file_stem().and_then(|s| s.to_str()).map(String::from)
                    else {
                        // Skip files whose stem is not valid UTF-8 —
                        // never happens for Claude Code's UUID-named
                        // files, but defensive against future surprises.
                        continue;
                    };
                    let db_clone = db.clone();
                    let path_clone = path.clone();
                    let sid_for_task = sid.clone();
                    debouncer
                        .schedule(sid, move || async move {
                            run_one_index(db_clone, path_clone, sid_for_task).await;
                        })
                        .await;
                }
                FileEvent::Removed(path) => {
                    // Phase 2: log + ignore. session_stats rows for
                    // deleted files stay; Phase 3 may add archival.
                    tracing::debug!(?path, "indexer_v2: ignoring file removal");
                }
                FileEvent::Rescan => {
                    // Phase 2: a full rescan driver lives in a future
                    // commit (`full_rebuild`). Until it lands, log so
                    // operators can correlate kernel-overflow events
                    // with stale session_stats rows.
                    tracing::warn!(
                        "indexer_v2: Rescan event received — full_rebuild not yet implemented"
                    );
                }
            }
        }

        tracing::info!("indexer_v2: watcher channel closed; shadow indexer exiting");
    })
}

/// Body of one debounced re-index. Public for tests + for callers that
/// want to drive an out-of-band single-file index without going through
/// the watcher channel.
///
/// Hash-gated: skips the parse + write if `blake3_head_tail` of the
/// file on disk matches the `source_content_hash` already stored in
/// `session_stats`. The hash is recomputed inside `index_session` too,
/// so the gate's cost (~microseconds) only saves work in the hit-path
/// where the file genuinely hasn't changed since the last write.
pub async fn run_one_index(db: Arc<Database>, path: PathBuf, session_id: String) {
    let on_disk = match blake3_head_tail(&path) {
        Ok(hash) => hash.to_vec(),
        Err(e) => {
            // File disappeared between the fsnotify event and the
            // debounce sleep, or the OS is misbehaving. Either way
            // there's nothing to index.
            tracing::debug!(?e, ?path, "indexer_v2: blake3_head_tail failed");
            return;
        }
    };

    let header = match db.get_stats_header(&session_id).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(?e, session_id, "indexer_v2: get_stats_header failed");
            return;
        }
    };

    if header
        .as_ref()
        .map(|h| h.source_content_hash.as_slice() == on_disk.as_slice())
        .unwrap_or(false)
    {
        // Unchanged — skip the parse. Cheapest possible code path on
        // the hot loop for sessions that fire many fsnotify events
        // without the file actually changing (rare, but possible
        // under tools that touch mtime without changing content).
        return;
    }

    if let Err(e) = index_session(&db, &path, &session_id).await {
        tracing::warn!(?e, ?path, session_id, "indexer_v2: index_session failed");
    }
}

/// Errors returned by [`index_session`].
#[derive(Debug, Error)]
pub enum IndexSessionError {
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(#[from] claude_view_session_parser::ParseError),

    #[error("db error: {0}")]
    Db(#[from] DbError),
}

#[cfg(unix)]
fn file_inode(metadata: &std::fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino() as i64)
}

#[cfg(not(unix))]
fn file_inode(_metadata: &std::fs::Metadata) -> Option<i64> {
    None
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// Smallest valid JSONL the parser will accept — one user line with
    /// a timestamp. Mirrors session-parser's parse fixture format.
    fn minimal_jsonl_with_one_user_message() -> &'static str {
        r#"{"type":"user","timestamp":"2026-04-18T10:30:00Z","message":{"role":"user","content":"hello"}}
"#
    }

    #[tokio::test]
    async fn index_session_round_trips_a_minimal_jsonl() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp
            .path()
            .join("11111111-2222-3333-4444-555555555555.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(minimal_jsonl_with_one_user_message().as_bytes())
            .unwrap();

        index_session(&db, &path, "11111111-2222-3333-4444-555555555555")
            .await
            .unwrap();

        let header = db
            .get_stats_header("11111111-2222-3333-4444-555555555555")
            .await
            .unwrap()
            .expect("session_stats row must exist after index_session");

        // Staleness header populated.
        assert_eq!(header.session_id, "11111111-2222-3333-4444-555555555555");
        assert_eq!(header.source_content_hash.len(), 32);
        assert_eq!(
            header.source_size,
            minimal_jsonl_with_one_user_message().len() as i64
        );
        // mid_hash only computed for files >1 MiB; this fixture is 100 bytes.
        assert!(header.source_mid_hash.is_none());

        // Stats columns populated by the parser. Verify a coarse
        // signature — exact values are session-parser's contract.
        let row: (i64, i64) = sqlx::query_as(
            "SELECT user_prompt_count, line_count FROM session_stats WHERE session_id = ?",
        )
        .bind("11111111-2222-3333-4444-555555555555")
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, 1, "one user line → user_prompt_count = 1");
        assert_eq!(row.1, 1, "one JSONL line → line_count = 1");
    }

    #[tokio::test]
    async fn index_session_returns_io_error_for_missing_file() {
        let db = Database::new_in_memory().await.unwrap();
        let err = index_session(&db, Path::new("/no/such/file.jsonl"), "missing-sess")
            .await
            .unwrap_err();
        assert!(
            matches!(err, IndexSessionError::Io(_)),
            "missing file must return IndexSessionError::Io, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn index_session_returns_parse_error_for_malformed_json() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.jsonl");
        std::fs::write(&path, b"this is not json\n").unwrap();

        let err = index_session(&db, &path, "bad-sess").await.unwrap_err();
        assert!(
            matches!(err, IndexSessionError::Parse(_)),
            "malformed JSON must return IndexSessionError::Parse, got {:?}",
            err
        );
    }

    #[tokio::test]
    async fn index_session_re_index_overwrites_previous_row() {
        let db = Database::new_in_memory().await.unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");

        // First write — one line.
        std::fs::write(&path, minimal_jsonl_with_one_user_message()).unwrap();
        index_session(&db, &path, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .unwrap();

        // Append a second line.
        let two_lines = format!(
            "{}{}",
            minimal_jsonl_with_one_user_message(),
            r#"{"type":"user","timestamp":"2026-04-18T10:31:00Z","message":{"role":"user","content":"second"}}
"#
        );
        std::fs::write(&path, two_lines).unwrap();
        index_session(&db, &path, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .unwrap();

        let row: (i64, i64) = sqlx::query_as(
            "SELECT line_count, user_prompt_count FROM session_stats WHERE session_id = ?",
        )
        .bind("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, 2, "re-index must reflect the appended line");
        assert_eq!(row.1, 2);

        // Still exactly one row (UPSERT, not double-insert).
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_stats")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }
}
