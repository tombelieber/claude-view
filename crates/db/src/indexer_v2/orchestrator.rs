//! Phase 2 indexer_v2 orchestrator — single-file index helper +
//! `full_rebuild` driver.
//!
//! `index_session` is the smallest standalone unit of work: parse a JSONL
//! file, extract stats, upsert into `session_stats`. It owns no I/O of
//! its own beyond reading the source file (handed to it by path) and the
//! single SQL UPSERT performed by [`super::writer::upsert_session_stats`].
//!
//! `full_rebuild` walks every JSONL under `~/.claude/projects/` and runs
//! `index_session` on each — the building block used by both the
//! 100-session parity harness (PR 2.2.2) and the eventual fsnotify
//! orchestrator (PR 2.2.1's `spawn_shadow_indexer`, currently scaffold
//! only).
//!
//! No fsnotify wiring lives in this module yet. `spawn_shadow_indexer`
//! is intentionally `unimplemented!()` so server startup crash-loops
//! if it tries to wire in indexer_v2 before the broadcast tap lands.

use std::path::Path;
use std::sync::Arc;

use claude_view_session_parser::{extract_stats, parse_jsonl, PARSER_VERSION, STATS_VERSION};
use thiserror::Error;

use super::config::StatsDelta;
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

/// Spawn the shadow indexer task. **Not yet implemented.**
///
/// The fsnotify wiring + per-session debouncer ship in a follow-up
/// commit (the broadcast tap on the manager's mpsc, plus the
/// `tokio::sync::broadcast::Receiver<FileEvent>` consumer). Until then,
/// `index_session` and `full_rebuild` are the only entry points;
/// invoking this function panics on purpose to prevent accidental
/// wiring before the orchestrator has been reviewed.
pub fn spawn_shadow_indexer(_db: Arc<Database>) {
    unimplemented!(
        "indexer_v2 orchestrator not yet wired — server startup must NOT \
         call spawn_shadow_indexer until the fsnotify+debouncer commit lands. \
         Use index_session(path, session_id) for ad-hoc indexing in the meantime."
    )
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
