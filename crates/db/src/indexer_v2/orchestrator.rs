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

use super::config::{DeltaSource, StatsDelta, DEBOUNCE_MS, STATS_DELTA_CHANNEL_CAPACITY};
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
    let delta = build_delta_sync(path, session_id, DeltaSource::Indexer, 0)?;
    upsert_session_stats(db, &delta).await?;
    Ok(())
}

/// Async wrapper over [`build_delta_sync`] — parses one JSONL file into
/// a [`StatsDelta`] on a `spawn_blocking` worker, without touching the
/// database.
///
/// Used by producers that publish deltas through the shared mpsc
/// channel (live-tail watcher, drift healer) rather than calling
/// `upsert_session_stats` directly. The file I/O + parse can take up to
/// a few milliseconds on large sessions, so offloading to a blocking
/// thread keeps producer hot paths (live-tail watcher, fsnotify event
/// loop) off the reactor.
pub async fn build_delta_from_file(
    path: PathBuf,
    session_id: String,
    source: DeltaSource,
    seq: u64,
) -> Result<StatsDelta, IndexSessionError> {
    tokio::task::spawn_blocking(move || build_delta_sync(&path, &session_id, source, seq))
        .await
        .map_err(|join_err| IndexSessionError::Io(std::io::Error::other(join_err.to_string())))?
}

/// Synchronous core of the parse + extract + hash pipeline that every
/// producer shares. Reading the file + computing hashes is unavoidable
/// blocking work; running it off-runtime is the caller's responsibility
/// ([`build_delta_from_file`] wraps this in `spawn_blocking`; the
/// existing `index_session` path runs it inline, matching pre-Phase-2.5
/// behavior).
fn build_delta_sync(
    path: &Path,
    session_id: &str,
    source: DeltaSource,
    seq: u64,
) -> Result<StatsDelta, IndexSessionError> {
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

    Ok(StatsDelta {
        session_id: session_id.to_string(),
        source_content_hash: head_tail,
        source_size: i64::try_from(size).unwrap_or(i64::MAX),
        source_inode: file_inode(&metadata),
        source_mid_hash: mid_hash,
        stats,
        // Phase 2.5 lineage: producers that can't cheaply compute a
        // previous snapshot (indexer cold start, fresh live-tail event)
        // send `None`. Phase 4 Stage C synthesises `old` via
        // `get_stats_header` + a column read when rollup deltas need it.
        old: None,
        seq,
        source,
    })
}

/// Spawn the shared StatsDelta writer consumer (Phase 2.5).
///
/// Returns `(Sender<StatsDelta>, JoinHandle<()>)`. The sender is cloned
/// to each producer (live-tail watcher today; drift healer in Phase 7);
/// the task runs for the process lifetime. When the runtime shuts down
/// it drops the sender, the channel closes, the consumer exits.
///
/// Every delta that comes through gets routed to
/// [`upsert_session_stats`] — the single writer gateway for
/// `session_stats`. Upsert errors log but do not kill the consumer so a
/// single bad row can't stall the whole stream.
///
/// Back-pressure contract (SOTA §10 Phase 2.5):
/// - Channel capacity [`STATS_DELTA_CHANNEL_CAPACITY`] (1024).
/// - Producers `try_send` only. On `TrySendError::Full` they bump
///   `stage_c_producer_drop_total{producer=<label>}` and rely on the
///   fsnotify shadow-indexer path (500 ms debounce) to cover the drop.
pub fn spawn_delta_consumer(db: Arc<Database>) -> (mpsc::Sender<StatsDelta>, JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<StatsDelta>(STATS_DELTA_CHANNEL_CAPACITY);
    let handle = tokio::spawn(async move {
        tracing::info!(
            capacity = STATS_DELTA_CHANNEL_CAPACITY,
            "indexer_v2: StatsDelta consumer running"
        );
        while let Some(delta) = rx.recv().await {
            let source_label = delta.source.metric_label();
            if let Err(e) = upsert_session_stats(&db, &delta).await {
                tracing::warn!(
                    session_id = %delta.session_id,
                    source = source_label,
                    seq = delta.seq,
                    error = %e,
                    "indexer_v2: delta consumer upsert failed"
                );
            }
        }
        tracing::info!("indexer_v2: StatsDelta consumer exiting (channel closed)");
    });
    (tx, handle)
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
                    // Kernel queue overflowed (or a watcher error
                    // demanded re-sync). Walk the projects tree and
                    // re-index every parent-session JSONL we find.
                    //
                    // This is intentionally a foreground await on the
                    // orchestrator loop: while the rebuild runs, new
                    // Modified events accumulate in the mpsc backlog
                    // (capacity 512) and are processed once we drain
                    // back to recv. The drop counter surfaces overflow
                    // if the rebuild takes longer than the backlog can
                    // absorb — operators see it in the same log line.
                    let db_clone = db.clone();
                    let root = projects_dir.clone();
                    let report = full_rebuild(db_clone, root).await;
                    tracing::warn!(
                        scanned = report.scanned,
                        indexed = report.indexed,
                        skipped_unchanged = report.skipped_unchanged,
                        errors = report.errors,
                        elapsed_ms = report.elapsed_ms,
                        "indexer_v2: full_rebuild after Rescan event finished"
                    );
                }
            }
        }

        tracing::info!("indexer_v2: watcher channel closed; shadow indexer exiting");
    })
}

/// Aggregate outcome of a [`full_rebuild`] pass. All counters are
/// monotonic across the rebuild; nothing is reset partway. Reported via
/// the orchestrator's tracing event so operators can spot rebuild bursts
/// in production logs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RebuildReport {
    /// Total parent-session `.jsonl` files visited.
    pub scanned: usize,
    /// Files where the on-disk content hash differed from the stored
    /// `session_stats.source_content_hash` (or no row existed) — these
    /// were re-parsed and upserted.
    pub indexed: usize,
    /// Files whose hash matched the stored row — skipped, no work done.
    /// On a freshly-warm DB this is the steady-state majority.
    pub skipped_unchanged: usize,
    /// Files that failed (IO, parse, or DB error). Not fatal — the
    /// rebuild keeps going; counts surface in the report so an
    /// operator can grep for them.
    pub errors: usize,
    /// Wall-clock duration of the whole pass.
    pub elapsed_ms: u128,
}

/// Walk `projects_dir` for every parent-session JSONL file (depth 2,
/// `.jsonl` extension) and run a hash-gated re-index against each.
///
/// **Phase 2 use cases:**
///
/// 1. Servicing `FileEvent::Rescan` from the watcher (kernel queue
///    overflow, watcher error). Called from the orchestrator loop.
/// 2. One-shot backfill: a Phase 3 PR can call this once at server
///    startup so `session_stats.row_count` converges to
///    `sessions.row_count` before the read-side cutover lands. Phase 2
///    deliberately does **not** wire this into startup — shadow mode
///    is opt-in convergence; readers don't depend on it yet.
///
/// Re-uses `run_one_index` so the per-file work matches every other
/// code path (hash gate, error tolerance, tracing).
pub async fn full_rebuild(db: Arc<Database>, projects_dir: PathBuf) -> RebuildReport {
    let started = std::time::Instant::now();
    let mut report = RebuildReport::default();

    let canonical = projects_dir
        .canonicalize()
        .unwrap_or_else(|_| projects_dir.clone());

    if !canonical.exists() {
        tracing::warn!(
            projects_dir = %canonical.display(),
            "indexer_v2: full_rebuild — projects dir missing, nothing to scan"
        );
        report.elapsed_ms = started.elapsed().as_millis();
        return report;
    }

    // walkdir + min/max depth = 2 selects exactly the parent-session
    // files (`{project}/{sessionId}.jsonl`), matching the watcher's
    // depth-2 filter. Subagent files at depth 4 are skipped without
    // a per-file string check.
    let walker = walkdir::WalkDir::new(&canonical)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|x| x.to_str()) == Some("jsonl")
        });

    for entry in walker {
        report.scanned += 1;
        let path = entry.path().to_path_buf();
        let Some(sid) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };

        // Inline hash gate so we can tally indexed vs skipped without
        // run_one_index swallowing the distinction. Same logic, just
        // accounting at the boundary.
        let on_disk = match blake3_head_tail(&path) {
            Ok(h) => h.to_vec(),
            Err(_) => {
                report.errors += 1;
                continue;
            }
        };
        let header = match db.get_stats_header(&sid).await {
            Ok(h) => h,
            Err(_) => {
                report.errors += 1;
                continue;
            }
        };
        if header
            .as_ref()
            .map(|h| h.source_content_hash.as_slice() == on_disk.as_slice())
            .unwrap_or(false)
        {
            report.skipped_unchanged += 1;
            continue;
        }
        match index_session(&db, &path, &sid).await {
            Ok(()) => report.indexed += 1,
            Err(_) => report.errors += 1,
        }
    }

    report.elapsed_ms = started.elapsed().as_millis();
    report
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

    /// Helper: build a `{root}/{project}/{sid}.jsonl` file populated
    /// with the minimal one-user-line fixture.
    fn write_session(root: &Path, project: &str, sid: &str) -> PathBuf {
        let project_dir = root.join(project);
        std::fs::create_dir_all(&project_dir).unwrap();
        let path = project_dir.join(format!("{sid}.jsonl"));
        std::fs::write(&path, minimal_jsonl_with_one_user_message()).unwrap();
        path
    }

    #[tokio::test]
    async fn full_rebuild_indexes_every_parent_session() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        write_session(&root, "proj-a", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        write_session(&root, "proj-a", "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        write_session(&root, "proj-b", "cccccccc-cccc-cccc-cccc-cccccccccccc");

        let report = full_rebuild(db.clone(), root).await;

        assert_eq!(report.scanned, 3, "must visit every parent session");
        assert_eq!(report.indexed, 3, "must index every untouched session");
        assert_eq!(report.skipped_unchanged, 0);
        assert_eq!(report.errors, 0);

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_stats")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 3, "session_stats should hold one row per session");
    }

    #[tokio::test]
    async fn full_rebuild_skips_subagent_files() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // One real parent session + one subagent file at depth 4 that
        // must be ignored by the depth filter (mirrors the watcher's
        // structural rule).
        write_session(&root, "proj-x", "11111111-1111-1111-1111-111111111111");
        let subagent_dir = root
            .join("proj-x")
            .join("11111111-1111-1111-1111-111111111111")
            .join("subagents");
        std::fs::create_dir_all(&subagent_dir).unwrap();
        std::fs::write(
            subagent_dir.join("agent-foo.jsonl"),
            minimal_jsonl_with_one_user_message(),
        )
        .unwrap();

        let report = full_rebuild(db.clone(), root).await;

        assert_eq!(report.scanned, 1, "depth filter must skip subagent files");
        assert_eq!(report.indexed, 1);
    }

    #[tokio::test]
    async fn full_rebuild_skips_unchanged_sessions_via_hash_gate() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        write_session(&root, "proj-y", "22222222-2222-2222-2222-222222222222");

        // First pass — fresh DB, indexes 1.
        let first = full_rebuild(db.clone(), root.clone()).await;
        assert_eq!(first.indexed, 1);
        assert_eq!(first.skipped_unchanged, 0);

        // Second pass on the same content — every file's stored hash
        // matches on-disk, so the hash gate must short-circuit.
        let second = full_rebuild(db.clone(), root).await;
        assert_eq!(second.scanned, 1);
        assert_eq!(second.indexed, 0);
        assert_eq!(second.skipped_unchanged, 1);
        assert_eq!(second.errors, 0);
    }

    #[tokio::test]
    async fn full_rebuild_on_missing_root_returns_empty_report() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let report = full_rebuild(db, PathBuf::from("/no/such/projects/dir")).await;
        assert_eq!(
            report,
            RebuildReport::default().with_elapsed(report.elapsed_ms)
        );
    }

    // ── Phase 2.5: delta channel + build_delta_from_file ──

    /// `build_delta_from_file` must produce the same writer-relevant
    /// output as the sync indexer path. This is the parity the live-tail
    /// migration rides on: live-tail sends via the channel, indexer
    /// sends direct, both paths call `build_delta_sync` underneath so
    /// the resulting `session_stats` row is byte-identical.
    #[tokio::test]
    async fn build_delta_from_file_matches_index_session_output() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp
            .path()
            .join("cccccccc-cccc-cccc-cccc-cccccccccccc.jsonl");
        std::fs::write(&path, minimal_jsonl_with_one_user_message()).unwrap();

        // Path 1 — index_session → direct upsert.
        let db_a = Database::new_in_memory().await.unwrap();
        index_session(&db_a, &path, "cccccccc-cccc-cccc-cccc-cccccccccccc")
            .await
            .unwrap();

        // Path 2 — build_delta_from_file → manual upsert (simulates the
        // live-tail → channel → consumer shape without spawning a task).
        let db_b = Database::new_in_memory().await.unwrap();
        let delta = build_delta_from_file(
            path.clone(),
            "cccccccc-cccc-cccc-cccc-cccccccccccc".to_string(),
            DeltaSource::LiveTail,
            42,
        )
        .await
        .unwrap();
        assert_eq!(delta.source, DeltaSource::LiveTail);
        assert_eq!(delta.seq, 42);
        upsert_session_stats(&db_b, &delta).await.unwrap();

        // The staleness headers must match — same bytes → same hash,
        // size, inode, mid_hash.
        let header_a = db_a
            .get_stats_header("cccccccc-cccc-cccc-cccc-cccccccccccc")
            .await
            .unwrap()
            .unwrap();
        let header_b = db_b
            .get_stats_header("cccccccc-cccc-cccc-cccc-cccccccccccc")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(header_a.source_content_hash, header_b.source_content_hash);
        assert_eq!(header_a.source_size, header_b.source_size);
        assert_eq!(header_a.source_inode, header_b.source_inode);
        assert_eq!(header_a.source_mid_hash, header_b.source_mid_hash);

        // The observable stats columns must match.
        let cols_a: (i64, i64) = sqlx::query_as(
            "SELECT user_prompt_count, line_count FROM session_stats WHERE session_id = ?",
        )
        .bind("cccccccc-cccc-cccc-cccc-cccccccccccc")
        .fetch_one(db_a.pool())
        .await
        .unwrap();
        let cols_b: (i64, i64) = sqlx::query_as(
            "SELECT user_prompt_count, line_count FROM session_stats WHERE session_id = ?",
        )
        .bind("cccccccc-cccc-cccc-cccc-cccccccccccc")
        .fetch_one(db_b.pool())
        .await
        .unwrap();
        assert_eq!(
            cols_a, cols_b,
            "live-tail delta path must produce the same row as direct index_session"
        );
    }

    /// Deltas sent through the consumer channel must land in
    /// `session_stats` the same way direct upserts do. Verifies the
    /// single-writer-gateway invariant end-to-end: channel → consumer
    /// → writer.
    #[tokio::test]
    async fn spawn_delta_consumer_routes_deltas_to_upsert_session_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp
            .path()
            .join("dddddddd-dddd-dddd-dddd-dddddddddddd.jsonl");
        std::fs::write(&path, minimal_jsonl_with_one_user_message()).unwrap();

        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let (tx, _handle) = spawn_delta_consumer(db.clone());

        let delta = build_delta_from_file(
            path.clone(),
            "dddddddd-dddd-dddd-dddd-dddddddddddd".to_string(),
            DeltaSource::LiveTail,
            1,
        )
        .await
        .unwrap();
        tx.try_send(delta).expect("fresh channel must accept send");

        // Consumer runs on the runtime; poll until the row appears
        // (bounded wait so a regression times out instead of hanging).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if db
                .get_stats_header("dddddddd-dddd-dddd-dddd-dddddddddddd")
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("consumer did not upsert delta within 2s");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let row: (i64, i64) = sqlx::query_as(
            "SELECT user_prompt_count, line_count FROM session_stats WHERE session_id = ?",
        )
        .bind("dddddddd-dddd-dddd-dddd-dddddddddddd")
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, 1);
        assert_eq!(row.1, 1);
    }

    /// Once every producer drops its sender, the consumer must exit
    /// cleanly — the JoinHandle completes without panicking. This is
    /// how tokio shutdown reaps the consumer task.
    #[tokio::test]
    async fn spawn_delta_consumer_exits_when_senders_drop() {
        let db = Arc::new(Database::new_in_memory().await.unwrap());
        let (tx, handle) = spawn_delta_consumer(db);
        drop(tx);
        // If the consumer somehow loops forever, this await will hang
        // and the test runner will time out.
        handle.await.expect("consumer task must exit cleanly");
    }
}

#[cfg(test)]
impl RebuildReport {
    /// Test helper: build a default report with a custom `elapsed_ms`
    /// so tests can assert structural equality without caring about
    /// the wall-clock value.
    fn with_elapsed(self, elapsed_ms: u128) -> Self {
        Self { elapsed_ms, ..self }
    }
}
