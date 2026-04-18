//! Phase 2 indexer_v2 — shadow-mode writer for the `session_stats` table.
//!
//! Runs in parallel with the legacy `live/manager` watcher; the legacy
//! path remains authoritative through Phase 3. Indexer_v2 owns
//! `session_stats` exclusively (no other writer touches that table —
//! enforced by the writer ownership registry, design §10.2).
//!
//! Module layout (CQRS Phase 1-7 design §3.1 PR 2.2):
//!
//! | Sub-module      | Status (PR 2.2) | Purpose                                 |
//! |-----------------|-----------------|-----------------------------------------|
//! | `config`        | live            | `DEBOUNCE_MS` + `StatsDelta` payload    |
//! | `writer`        | live            | `upsert_session_stats` (single SQL gw)  |
//! | `orchestrator`  | live            | `index_session` + `spawn_shadow_indexer`|
//! | `watcher`       | live            | `start_watcher` (fsnotify, depth-2 fil) |
//! | `debouncer`     | live            | per-session 500 ms coalesce             |
//! | `drift`         | live            | `compare_session` (parity test helper)  |

mod config;
mod debouncer;
mod drift;
mod orchestrator;
mod watcher;
mod writer;

pub use config::{DeltaSource, StatsDelta, DEBOUNCE_MS, STATS_DELTA_CHANNEL_CAPACITY};
pub use debouncer::Debouncer;
pub use drift::{compare_session, DriftReport, FieldDiff};
pub use orchestrator::{
    build_delta_from_file, default_projects_dir, full_rebuild, index_session, run_one_index,
    spawn_delta_consumer, spawn_shadow_indexer, spawn_shadow_indexer_with_root, IndexSessionError,
    RebuildReport,
};
pub use watcher::{start_watcher, FileEvent, FILE_EVENT_CHANNEL_CAPACITY};
pub use writer::upsert_session_stats;

#[cfg(test)]
mod tests {
    use claude_view_session_parser::{SessionStats, PARSER_VERSION, STATS_VERSION};

    use super::{upsert_session_stats, DeltaSource, StatsDelta};
    use crate::Database;

    /// Shared default for test-only lineage fields (`old`/`seq`/`source`).
    /// Writer ignores these; having a helper keeps the four upsert tests
    /// focused on the writer payload.
    fn indexer_lineage() -> (Option<SessionStats>, u64, DeltaSource) {
        (None, 0, DeltaSource::Indexer)
    }

    /// Shared default for Phase 3 PR 3.a filesystem-mirror fields. Tests
    /// that care about project/file_path shape set these explicitly; the
    /// writer round-trip tests in this module only need them populated so
    /// the new NOT-NULL column constraints on the writer path are satisfied.
    fn phase3_fs_fields() -> (String, String, bool, i64) {
        (
            "test-project".into(),
            "/tmp/test-project/sess.jsonl".into(),
            false,
            0,
        )
    }

    fn empty_stats() -> SessionStats {
        SessionStats {
            total_input_tokens: 0,
            total_output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 0,
            turn_count: 0,
            user_prompt_count: 0,
            line_count: 0,
            tool_call_count: 0,
            thinking_block_count: 0,
            api_error_count: 0,
            files_read_count: 0,
            files_edited_count: 0,
            bash_count: 0,
            agent_spawn_count: 0,
            first_message_at: None,
            last_message_at: None,
            duration_seconds: 0,
            primary_model: None,
            git_branch: None,
            preview: String::new(),
            last_message: String::new(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn upsert_inserts_then_reads_back_via_get_stats_header() {
        let db = Database::new_in_memory().await.unwrap();

        let (old, seq, source) = indexer_lineage();
        let (project_id, source_file_path, is_compressed, source_mtime) = phase3_fs_fields();
        let delta = StatsDelta {
            session_id: "sess-insert".into(),
            source_content_hash: vec![0x01, 0x02, 0x03],
            source_size: 4096,
            source_inode: Some(7),
            source_mid_hash: Some(vec![0xAA, 0xBB]),
            project_id,
            source_file_path,
            is_compressed,
            source_mtime,
            stats: empty_stats(),
            old,
            seq,
            source,
        };

        upsert_session_stats(&db, &delta).await.unwrap();
        let header = db
            .get_stats_header("sess-insert")
            .await
            .unwrap()
            .expect("inserted row must be readable");

        assert_eq!(header.session_id, "sess-insert");
        assert_eq!(header.source_content_hash, vec![0x01, 0x02, 0x03]);
        assert_eq!(header.source_size, 4096);
        assert_eq!(header.source_inode, Some(7));
        assert_eq!(header.source_mid_hash, Some(vec![0xAA, 0xBB]));
        assert_eq!(header.parser_version, i64::from(PARSER_VERSION.0));
        assert_eq!(header.stats_version, i64::from(STATS_VERSION.0));
        assert!(header.indexed_at > 0, "indexed_at must be set to now");
    }

    #[tokio::test]
    async fn upsert_overwrites_on_conflict_with_session_id_key() {
        let db = Database::new_in_memory().await.unwrap();

        let mut stats = empty_stats();
        stats.total_input_tokens = 100;
        let (old, seq, source) = indexer_lineage();
        let (project_id, source_file_path, is_compressed, source_mtime) = phase3_fs_fields();
        let first = StatsDelta {
            session_id: "sess-conflict".into(),
            source_content_hash: vec![0x01],
            source_size: 1,
            source_inode: None,
            source_mid_hash: None,
            project_id: project_id.clone(),
            source_file_path: source_file_path.clone(),
            is_compressed,
            source_mtime,
            stats,
            old: old.clone(),
            seq,
            source,
        };
        upsert_session_stats(&db, &first).await.unwrap();

        // Second write with different stats + larger size — should replace.
        let mut stats2 = empty_stats();
        stats2.total_input_tokens = 999;
        stats2.turn_count = 5;
        let second = StatsDelta {
            session_id: "sess-conflict".into(),
            source_content_hash: vec![0xFF, 0xFE],
            source_size: 8192,
            source_inode: Some(123),
            source_mid_hash: None,
            project_id,
            source_file_path,
            is_compressed,
            source_mtime,
            stats: stats2,
            old,
            seq,
            source,
        };
        upsert_session_stats(&db, &second).await.unwrap();

        let row: (String, i64, i64, i64) = sqlx::query_as(
            r#"SELECT session_id, source_size, total_input_tokens, turn_count
                 FROM session_stats WHERE session_id = ?"#,
        )
        .bind("sess-conflict")
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, "sess-conflict");
        assert_eq!(row.1, 8192, "source_size must follow the latest write");
        assert_eq!(row.2, 999, "total_input_tokens must be overwritten");
        assert_eq!(row.3, 5, "turn_count must be overwritten");

        // Exactly one row exists — no duplicate insert.
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_stats")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn upsert_normalizes_rfc3339_timestamps_to_unix_seconds() {
        let db = Database::new_in_memory().await.unwrap();

        let mut stats = empty_stats();
        stats.first_message_at = Some("2026-04-18T10:30:00Z".into());
        stats.last_message_at = Some("2026-04-18T11:00:00Z".into());
        let (old, seq, source) = indexer_lineage();
        let (project_id, source_file_path, is_compressed, source_mtime) = phase3_fs_fields();
        let delta = StatsDelta {
            session_id: "sess-ts".into(),
            source_content_hash: vec![0x01],
            source_size: 1,
            source_inode: None,
            source_mid_hash: None,
            project_id,
            source_file_path,
            is_compressed,
            source_mtime,
            stats,
            old,
            seq,
            source,
        };
        upsert_session_stats(&db, &delta).await.unwrap();

        let row: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT first_message_at, last_message_at FROM session_stats WHERE session_id = ?",
        )
        .bind("sess-ts")
        .fetch_one(db.pool())
        .await
        .unwrap();

        // Compute the expected unix seconds via chrono itself rather than
        // hardcoding — the offset arithmetic is easy to get wrong by hand.
        let expected_first = chrono::DateTime::parse_from_rfc3339("2026-04-18T10:30:00Z")
            .unwrap()
            .timestamp();
        let expected_last = chrono::DateTime::parse_from_rfc3339("2026-04-18T11:00:00Z")
            .unwrap()
            .timestamp();
        assert_eq!(row.0, Some(expected_first));
        assert_eq!(row.1, Some(expected_last));
        assert_eq!(
            expected_last - expected_first,
            1800,
            "30 minutes = 1800 seconds — sanity check on chrono parse"
        );
    }

    #[tokio::test]
    async fn upsert_stores_null_for_missing_or_unparseable_timestamps() {
        let db = Database::new_in_memory().await.unwrap();

        let mut stats = empty_stats();
        stats.first_message_at = Some("not a date".into());
        // last_message_at left as None
        let (old, seq, source) = indexer_lineage();
        let (project_id, source_file_path, is_compressed, source_mtime) = phase3_fs_fields();
        let delta = StatsDelta {
            session_id: "sess-bad-ts".into(),
            source_content_hash: vec![0x01],
            source_size: 1,
            source_inode: None,
            source_mid_hash: None,
            project_id,
            source_file_path,
            is_compressed,
            source_mtime,
            stats,
            old,
            seq,
            source,
        };
        upsert_session_stats(&db, &delta).await.unwrap();

        let row: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT first_message_at, last_message_at FROM session_stats WHERE session_id = ?",
        )
        .bind("sess-bad-ts")
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.0, None, "unparseable timestamp must store NULL");
        assert_eq!(row.1, None, "missing timestamp must store NULL");
    }
}
