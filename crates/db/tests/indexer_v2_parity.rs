//! Phase 2 exit gate (PR 2.2.3 / handoff §3.3): writer round-trip parity
//! check for `indexer_v2` against **real production session JSONL files**.
//!
//! ## Why two reports
//!
//! ### 1. Writer round-trip parity (asserted; must be zero drift)
//!
//! For each sampled JSONL file:
//!   1. Compute the canonical [`SessionStats`] in-memory by calling the
//!      same pipeline server route handlers use:
//!      `parse_jsonl(bytes, PARSER_VERSION)` → `extract_stats(doc, STATS_VERSION)`.
//!   2. Call `index_session(db, path, sid)` → writes `session_stats` row.
//!   3. Read every shadow column back and assert it equals the in-memory
//!      [`SessionStats`] field — modulo the `RFC3339 → unix-seconds`
//!      normalization the writer performs on `first_message_at` /
//!      `last_message_at`.
//!
//! Any drift here = the writer is bugged (wrong column order, wrong type
//! cast, missing field). This must be `0/N` to ship Phase 2.
//!
//! ### 2. Legacy → shadow drift report (informational; never fails)
//!
//! For the same sampled sessions, runs `compare_session(db, sid)` which
//! diffs the legacy `sessions` row against the freshly-written
//! `session_stats` row. **High drift here is expected** — most legacy
//! rows were indexed at older `parse_version`s before the JSONL files
//! were re-appended, and the legacy parser (`indexer_parallel`) computes
//! `preview` / `last_message` / token totals via a different code path
//! than [`compute_stats`]. The point of the indexer_v2 redesign is to
//! retire that legacy path; this report just measures how far the
//! freshly-canonical view has diverged from the historical mirror.
//!
//! ## Running
//!
//! `#[ignore]` so CI defaults to skipping (CI hosts have no
//! `~/.claude-view/claude-view.db`). Run on demand:
//!
//! ```bash
//! ./scripts/cq test -p claude-view-db --test indexer_v2_parity \
//!     -- --ignored --nocapture
//!
//! # Tune sample size (default 100):
//! INDEXER_V2_PARITY_N=500 ./scripts/cq test -p claude-view-db \
//!     --test indexer_v2_parity -- --ignored --nocapture
//! ```
//!
//! Sessions whose JSONL is missing or malformed are counted-and-skipped,
//! not failed — they reflect data integrity issues that pre-date Phase 2.

use std::path::{Path, PathBuf};

use claude_view_db::indexer_v2::{compare_session, index_session, IndexSessionError};
use claude_view_db::Database;
use claude_view_session_parser::{
    blake3_head_tail, extract_stats, parse_jsonl, SessionStats, PARSER_VERSION, STATS_VERSION,
};

/// Default sample size when `INDEXER_V2_PARITY_N` is not set.
const DEFAULT_SAMPLE_N: usize = 100;

/// Resolve the prod DB path under `~/.claude-view/`. Mirrors the project
/// rule "ALL app data lives under `~/.claude-view/`" — never use XDG dirs.
fn prod_db_path() -> PathBuf {
    dirs::home_dir()
        .expect("HOME must be set")
        .join(".claude-view")
        .join("claude-view.db")
}

#[tokio::test]
#[ignore = "requires ~/.claude-view/claude-view.db with real session data"]
async fn indexer_v2_parity_against_prod_sessions() {
    let db_path = prod_db_path();
    if !db_path.exists() {
        // Even when --ignored is forced, skip-with-message rather than
        // panic if the DB hasn't been bootstrapped — keeps the test
        // useful on a fresh checkout.
        eprintln!(
            "skipping parity test: {} does not exist (no claude-view DB to test against)",
            db_path.display()
        );
        return;
    }

    let n = std::env::var("INDEXER_V2_PARITY_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SAMPLE_N);

    let db = Database::new(&db_path)
        .await
        .expect("open prod claude-view.db");

    let sessions: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT session_id AS id, file_path FROM session_stats
            WHERE file_path IS NOT NULL
              AND file_path LIKE '%.jsonl'
            ORDER BY RANDOM()
            LIMIT ?"#,
    )
    .bind(n as i64)
    .fetch_all(db.pool())
    .await
    .expect("sample sessions query");

    assert!(
        !sessions.is_empty(),
        "session_stats appears empty — populate ~/.claude-view/claude-view.db before running parity"
    );

    let mut writer = WriterReport::default();
    let mut legacy = LegacyDriftReport::default();

    for (sid, file_path) in &sessions {
        let path = Path::new(file_path);
        if !path.exists() {
            writer.skipped_missing_file += 1;
            continue;
        }

        // Canonical parse — what server route handlers see when they
        // re-derive stats on demand. Capture the file's content hash
        // both BEFORE the parse and AFTER the writer runs; if it
        // changed in between, a live session was appending to the
        // JSONL and the canonical-vs-shadow comparison is racy. Skip
        // those instead of failing — the writer is correct, the test
        // just couldn't get a stable snapshot.
        let hash_before = match blake3_head_tail(path) {
            Ok(h) => h,
            Err(_) => {
                writer.skipped_io += 1;
                continue;
            }
        };
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => {
                writer.skipped_io += 1;
                continue;
            }
        };
        let doc = match parse_jsonl(&bytes, PARSER_VERSION) {
            Ok(d) => d,
            Err(_) => {
                writer.skipped_parse += 1;
                continue;
            }
        };
        let canonical = extract_stats(&doc, STATS_VERSION);

        // Write via the indexer_v2 path. Same parser, same extractor —
        // any divergence between this row and `canonical` is purely a
        // writer bug (column order, type cast, missing bind).
        match index_session(&db, path, sid).await {
            Ok(()) => {}
            Err(IndexSessionError::Io(_)) => {
                writer.skipped_io += 1;
                continue;
            }
            Err(IndexSessionError::Parse(_)) => {
                writer.skipped_parse += 1;
                continue;
            }
            Err(e) => panic!("index_session({sid}) failed unexpectedly: {e}"),
        }

        let hash_after = blake3_head_tail(path).ok();
        if hash_after.as_ref() != Some(&hash_before) {
            // File changed between the canonical read and the writer
            // call — live session appending. Skip; this isn't a
            // correctness failure of the writer.
            writer.skipped_live += 1;
            continue;
        }

        let shadow = read_shadow_stats(&db, sid).await;
        let mismatches = diff_writer_round_trip(&canonical, &shadow);
        if mismatches.is_empty() {
            writer.clean += 1;
        } else {
            for (field, msg) in &mismatches {
                *writer
                    .field_histogram
                    .entry((*field).to_string())
                    .or_insert(0) += 1;
                if writer.examples.len() < 5 {
                    writer
                        .examples
                        .push(format!("session={sid} field={field} {msg}"));
                }
            }
            writer.drifted += 1;
        }

        // Informational: legacy → shadow drift. This will be high on a
        // long-lived prod DB and is not a failure signal.
        if let Some(report) = compare_session(&db, sid)
            .await
            .expect("compare_session query")
        {
            if report.is_clean() {
                legacy.clean += 1;
            } else {
                for diff in &report.diffs {
                    *legacy
                        .field_histogram
                        .entry(diff.field.to_string())
                        .or_insert(0) += 1;
                }
                legacy.drifted += 1;
            }
        } else {
            legacy.skipped_no_legacy += 1;
        }
    }

    writer.print(n);
    legacy.print();

    let compared = writer.clean + writer.drifted;
    assert!(
        compared > 0,
        "no sessions could be compared — every sampled session was skipped (check JSONL paths)"
    );
    assert_eq!(
        writer.drifted, 0,
        "writer round-trip must be bit-perfect: {} of {} sessions had a column that did not equal the in-memory SessionStats. \
         See histogram + first 5 examples above. This is a writer bug — `index_session` did not write what `extract_stats` produced.",
        writer.drifted, compared
    );
}

// ── Round-trip diffing ──────────────────────────────────────────────

/// Subset of `session_stats` we materialize for the round-trip diff.
/// Mirrors every column populated by `upsert_session_stats` except the
/// staleness header (covered by other tests) and `per_model_tokens_json`
/// (currently a stub `'{}'`).
struct ShadowStatsRow {
    total_input_tokens: i64,
    total_output_tokens: i64,
    cache_read_tokens: i64,
    cache_creation_tokens: i64,
    cache_creation_5m_tokens: i64,
    cache_creation_1hr_tokens: i64,
    turn_count: i64,
    user_prompt_count: i64,
    line_count: i64,
    tool_call_count: i64,
    thinking_block_count: i64,
    api_error_count: i64,
    files_read_count: i64,
    files_edited_count: i64,
    bash_count: i64,
    agent_spawn_count: i64,
    first_message_at: Option<i64>,
    last_message_at: Option<i64>,
    duration_seconds: i64,
    primary_model: Option<String>,
    git_branch: Option<String>,
    preview: String,
    last_message: String,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for ShadowStatsRow {
    // Workspace sqlx is built without the `derive` feature
    // (verified at crates/db/Cargo.toml — no `sqlx::FromRow` derive
    // available). Manual `FromRow` mirrors `OverlapRow` in
    // `indexer_v2/drift.rs`.
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            total_input_tokens: row.try_get("total_input_tokens")?,
            total_output_tokens: row.try_get("total_output_tokens")?,
            cache_read_tokens: row.try_get("cache_read_tokens")?,
            cache_creation_tokens: row.try_get("cache_creation_tokens")?,
            cache_creation_5m_tokens: row.try_get("cache_creation_5m_tokens")?,
            cache_creation_1hr_tokens: row.try_get("cache_creation_1hr_tokens")?,
            turn_count: row.try_get("turn_count")?,
            user_prompt_count: row.try_get("user_prompt_count")?,
            line_count: row.try_get("line_count")?,
            tool_call_count: row.try_get("tool_call_count")?,
            thinking_block_count: row.try_get("thinking_block_count")?,
            api_error_count: row.try_get("api_error_count")?,
            files_read_count: row.try_get("files_read_count")?,
            files_edited_count: row.try_get("files_edited_count")?,
            bash_count: row.try_get("bash_count")?,
            agent_spawn_count: row.try_get("agent_spawn_count")?,
            first_message_at: row.try_get("first_message_at")?,
            last_message_at: row.try_get("last_message_at")?,
            duration_seconds: row.try_get("duration_seconds")?,
            primary_model: row.try_get("primary_model")?,
            git_branch: row.try_get("git_branch")?,
            preview: row.try_get("preview")?,
            last_message: row.try_get("last_message")?,
        })
    }
}

async fn read_shadow_stats(db: &Database, sid: &str) -> ShadowStatsRow {
    sqlx::query_as::<_, ShadowStatsRow>(
        r#"SELECT total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
                  cache_creation_5m_tokens, cache_creation_1hr_tokens,
                  turn_count, user_prompt_count, line_count, tool_call_count,
                  thinking_block_count, api_error_count,
                  files_read_count, files_edited_count, bash_count, agent_spawn_count,
                  first_message_at, last_message_at, duration_seconds,
                  primary_model, git_branch,
                  preview, last_message
             FROM session_stats WHERE session_id = ?"#,
    )
    .bind(sid)
    .fetch_one(db.pool())
    .await
    .expect("session_stats row must exist after index_session")
}

fn rfc3339_to_unix(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Returns `(field_name, descriptive_message)` for each mismatch.
fn diff_writer_round_trip(
    canonical: &SessionStats,
    shadow: &ShadowStatsRow,
) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    macro_rules! check {
        ($field:literal, $a:expr, $b:expr) => {{
            let a = $a;
            let b = $b;
            if a != b {
                out.push(($field, format!("canonical={a:?} shadow={b:?}")));
            }
        }};
    }

    check!(
        "total_input_tokens",
        canonical.total_input_tokens as i64,
        shadow.total_input_tokens
    );
    check!(
        "total_output_tokens",
        canonical.total_output_tokens as i64,
        shadow.total_output_tokens
    );
    check!(
        "cache_read_tokens",
        canonical.cache_read_tokens as i64,
        shadow.cache_read_tokens
    );
    check!(
        "cache_creation_tokens",
        canonical.cache_creation_tokens as i64,
        shadow.cache_creation_tokens
    );
    check!(
        "cache_creation_5m_tokens",
        canonical.cache_creation_5m_tokens as i64,
        shadow.cache_creation_5m_tokens
    );
    check!(
        "cache_creation_1hr_tokens",
        canonical.cache_creation_1hr_tokens as i64,
        shadow.cache_creation_1hr_tokens
    );
    check!("turn_count", canonical.turn_count as i64, shadow.turn_count);
    check!(
        "user_prompt_count",
        canonical.user_prompt_count as i64,
        shadow.user_prompt_count
    );
    check!("line_count", canonical.line_count as i64, shadow.line_count);
    check!(
        "tool_call_count",
        canonical.tool_call_count as i64,
        shadow.tool_call_count
    );
    check!(
        "thinking_block_count",
        canonical.thinking_block_count as i64,
        shadow.thinking_block_count
    );
    check!(
        "api_error_count",
        canonical.api_error_count as i64,
        shadow.api_error_count
    );
    check!(
        "files_read_count",
        canonical.files_read_count as i64,
        shadow.files_read_count
    );
    check!(
        "files_edited_count",
        canonical.files_edited_count as i64,
        shadow.files_edited_count
    );
    check!("bash_count", canonical.bash_count as i64, shadow.bash_count);
    check!(
        "agent_spawn_count",
        canonical.agent_spawn_count as i64,
        shadow.agent_spawn_count
    );
    check!(
        "duration_seconds",
        canonical.duration_seconds as i64,
        shadow.duration_seconds
    );
    check!(
        "first_message_at",
        canonical
            .first_message_at
            .as_deref()
            .and_then(rfc3339_to_unix),
        shadow.first_message_at
    );
    check!(
        "last_message_at",
        canonical
            .last_message_at
            .as_deref()
            .and_then(rfc3339_to_unix),
        shadow.last_message_at
    );
    check!(
        "primary_model",
        canonical.primary_model.as_deref(),
        shadow.primary_model.as_deref()
    );
    check!(
        "git_branch",
        canonical.git_branch.as_deref(),
        shadow.git_branch.as_deref()
    );
    check!(
        "preview",
        canonical.preview.as_str(),
        shadow.preview.as_str()
    );
    check!(
        "last_message",
        canonical.last_message.as_str(),
        shadow.last_message.as_str()
    );

    out
}

// ── Reporting ───────────────────────────────────────────────────────

#[derive(Default)]
struct WriterReport {
    clean: usize,
    drifted: usize,
    skipped_missing_file: usize,
    skipped_io: usize,
    skipped_parse: usize,
    /// File content hash changed between the canonical read and the
    /// writer call — a live session was appending. Not a writer bug.
    skipped_live: usize,
    field_histogram: std::collections::HashMap<String, usize>,
    examples: Vec<String>,
}

impl WriterReport {
    fn print(&self, sampled: usize) {
        let compared = self.clean + self.drifted;
        let total_skipped =
            self.skipped_missing_file + self.skipped_io + self.skipped_parse + self.skipped_live;
        println!();
        println!("════════════════════════════════════════════════════");
        println!(" indexer_v2 writer round-trip parity (must be 0 drift)");
        println!("════════════════════════════════════════════════════");
        println!(" sampled        : {sampled}");
        println!(" compared       : {compared}");
        println!(
            " clean          : {} ({:.1} %)",
            self.clean,
            pct(self.clean, compared)
        );
        println!(
            " drifted        : {} ({:.1} %)",
            self.drifted,
            pct(self.drifted, compared)
        );
        println!(" skipped (file) : {}", self.skipped_missing_file);
        println!(" skipped (io)   : {}", self.skipped_io);
        println!(" skipped (parse): {}", self.skipped_parse);
        println!(" skipped (live) : {}", self.skipped_live);
        println!(" total skipped  : {total_skipped}");
        if !self.field_histogram.is_empty() {
            println!();
            println!(" field drift histogram (writer bug — should be empty):");
            let mut entries: Vec<_> = self.field_histogram.iter().collect();
            entries.sort_by(|a, b| b.1.cmp(a.1));
            for (field, count) in entries {
                println!("   {field:>26} : {count}");
            }
        }
        if !self.examples.is_empty() {
            println!();
            println!(" first {} mismatch examples:", self.examples.len());
            for ex in &self.examples {
                println!("   {ex}");
            }
        }
        println!("════════════════════════════════════════════════════");
    }
}

#[derive(Default)]
struct LegacyDriftReport {
    clean: usize,
    drifted: usize,
    skipped_no_legacy: usize,
    field_histogram: std::collections::HashMap<String, usize>,
}

impl LegacyDriftReport {
    fn print(&self) {
        let compared = self.clean + self.drifted;
        println!();
        println!("════════════════════════════════════════════════════");
        println!(" legacy ↔ shadow drift (informational — staleness expected)");
        println!("════════════════════════════════════════════════════");
        println!(" compared       : {compared}");
        println!(
            " clean          : {} ({:.1} %)",
            self.clean,
            pct(self.clean, compared)
        );
        println!(
            " drifted        : {} ({:.1} %)",
            self.drifted,
            pct(self.drifted, compared)
        );
        println!(" no legacy row  : {}", self.skipped_no_legacy);
        if !self.field_histogram.is_empty() {
            println!();
            println!(" legacy drift histogram (sorted desc):");
            let mut entries: Vec<_> = self.field_histogram.iter().collect();
            entries.sort_by(|a, b| b.1.cmp(a.1));
            for (field, count) in entries {
                println!("   {field:>26} : {count}");
            }
        }
        println!("════════════════════════════════════════════════════");
        println!();
    }
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        100.0 * n as f64 / d as f64
    }
}
