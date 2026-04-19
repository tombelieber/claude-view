//! Multi-writer concurrency bench — CQRS Phase 4 PR 4.0.5b.
//!
//! Validates that the starter PRAGMA tuning from PR 4.0.5 (`mmap_size`
//! = 256 MiB, `journal_size_limit` = 128 MiB, `wal_autocheckpoint` =
//! 2000 pages) holds up under the 8-writer load the §13 perf gate
//! targets once Phase 5 action handlers + Phase 4 Stage C + live-tail
//! + indexer-v2 + git-sync + drift-detector all write concurrently.
//!
//! ## What it measures
//!
//! Eight concurrent writer tasks run at 10 QPS each for a configurable
//! duration, and a single reader task queries `sum_global_stats_in_range`
//! at the same 10 QPS over the duration. Per-operation latencies are
//! captured and summarised as p50 / p99 / max. The write mix is split
//! between the two hottest producer paths:
//!
//! - half the writers UPSERT into `session_stats` (the Phase 2.5 write
//!   path that Stage C fans out from)
//! - half INSERT into `hook_events` (the highest-QPS non-rollup writer
//!   observed in production — 5-10 Hz per active Claude Code session)
//!
//! Stage C's own 12 UPSERTs per delta run as part of the
//! `session_stats` path because the shared-delta-consumer wiring lives
//! off that write. Running Stage C inline in the bench avoids a
//! separate tokio task whose scheduling would mask write contention.
//!
//! ## How to run
//!
//! Fast mode — default, honours `#[ignore]`:
//!   `./scripts/cq test -p claude-view-db --test multi_writer_bench -- --ignored --nocapture`
//!
//! Full mode — 60 s × 8 × 10 QPS = 4800 write ops + 600 read ops:
//!   `CLAUDE_VIEW_BENCH_FULL=1 ./scripts/cq test -p claude-view-db --test multi_writer_bench -- --ignored --nocapture`
//!
//! The `--nocapture` flag surfaces the summary table; without it the
//! numbers stay hidden by cargo test's stdout capture.
//!
//! ## Output format
//!
//! The bench prints a human-readable table AND emits a JSON line
//! (prefixed `BENCH_JSON:`) with the raw numbers. A shell harness can
//! grep that line out and append to a `benchmarks/<date>-*.md` log.
//! Format (keys stable across runs):
//!
//! ```json
//! {"mode":"fast","writers":8,"qps_per_writer":10,"duration_s":5,
//!  "session_stats_upsert":{"n":25,"p50_ms":0.5,"p99_ms":2.1,"max_ms":2.4},
//!  "stage_c_fanout":    {"n":25,"p50_ms":3.2,"p99_ms":9.8,"max_ms":11.0},
//!  "hook_events_insert":{"n":25,"p50_ms":0.3,"p99_ms":1.1,"max_ms":1.3},
//!  "rollup_read":       {"n":50,"p50_ms":0.8,"p99_ms":2.5,"max_ms":3.0}}
//! ```
//!
//! ## §13 pass / fail thresholds
//!
//! §13 targets (with starter PRAGMAs): write p99 ≤ 20 ms, read p99 ≤
//! 5 ms on dashboard reads. The bench asserts nothing — it logs. If
//! the operator sees p99 > threshold in full mode, they tune PRAGMAs
//! and re-run. The "bench-log-or-it-didn't-happen" doctrine applies:
//! no PRAGMA change without an attached bench log in
//! `private/config/docs/plans/benchmarks/`.

use chrono::NaiveDate;
use claude_view_core::session_stats::SessionStats;
use claude_view_db::indexer_v2::{DeltaSource, StatsDelta};
use claude_view_db::stage_c::apply_stats_delta;
use claude_view_db::Database;
use claude_view_stats_rollup::{sum_global_stats_in_range, Bucket};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

const WRITERS: usize = 8;
const QPS_PER_WRITER: usize = 10;

/// Collected per-op latency samples, grouped by op kind.
#[derive(Default)]
struct LatencyBuckets {
    session_stats_upsert: Vec<Duration>,
    stage_c_fanout: Vec<Duration>,
    hook_events_insert: Vec<Duration>,
    rollup_read: Vec<Duration>,
}

impl LatencyBuckets {
    fn merge(&mut self, other: LatencyBuckets) {
        self.session_stats_upsert.extend(other.session_stats_upsert);
        self.stage_c_fanout.extend(other.stage_c_fanout);
        self.hook_events_insert.extend(other.hook_events_insert);
        self.rollup_read.extend(other.rollup_read);
    }
}

#[derive(serde::Serialize)]
struct OpSummary {
    n: usize,
    p50_ms: f64,
    p99_ms: f64,
    max_ms: f64,
}

impl OpSummary {
    fn from_samples(mut samples: Vec<Duration>) -> Self {
        if samples.is_empty() {
            return Self {
                n: 0,
                p50_ms: 0.0,
                p99_ms: 0.0,
                max_ms: 0.0,
            };
        }
        samples.sort();
        let n = samples.len();
        let p50 = samples[n / 2];
        // Linear interpolation-free p99 — good enough for observational
        // bench output; `p99_index = ceil(0.99 * n) - 1`.
        let p99_idx = ((n as f64 * 0.99).ceil() as usize).saturating_sub(1);
        let p99 = samples[p99_idx.min(n - 1)];
        let max = *samples.last().unwrap();
        Self {
            n,
            p50_ms: ms(p50),
            p99_ms: ms(p99),
            max_ms: ms(max),
        }
    }
}

fn ms(d: Duration) -> f64 {
    (d.as_micros() as f64) / 1000.0
}

/// Runs the bench once and returns per-op latency samples.
async fn run_bench(db: &Database, duration: Duration) -> LatencyBuckets {
    // Seed: insert a small fixed set of session_stats rows so the
    // reader has something to scan (prevents "empty table" making
    // p99 artificially fast).
    let seed_ts = NaiveDate::from_ymd_opt(2026, 4, 20)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    for i in 0..20 {
        let sid = format!("seed-{i:03}");
        sqlx::query(
            r"INSERT OR REPLACE INTO session_stats (
                session_id, source_content_hash, source_size,
                parser_version, stats_version, indexed_at,
                total_input_tokens, last_message_at, project_id
            ) VALUES (?, X'01', 0, 1, 1, 0, ?, ?, 'bench-seed')",
        )
        .bind(sid)
        .bind((i as i64) * 100)
        .bind(seed_ts)
        .execute(db.pool())
        .await
        .expect("seed row");
    }
    claude_view_db::stage_c::full_rebuild_from_session_stats(db.pool())
        .await
        .expect("initial rebuild for read baseline");

    let start = Instant::now();
    let deadline = start + duration;
    // Shared ID counter so different writers don't upsert the same
    // session_id; lets us measure true concurrent contention rather
    // than effectively-serial updates to one row.
    let id_counter = Arc::new(AtomicU64::new(1_000));

    let mut writers: JoinSet<LatencyBuckets> = JoinSet::new();
    for writer_idx in 0..WRITERS {
        let pool = db.pool().clone();
        let id_counter = id_counter.clone();
        writers.spawn(async move { writer_loop(&pool, writer_idx, deadline, id_counter).await });
    }

    // Reader runs on the task itself — one fewer JoinSet boundary.
    let pool = db.pool().clone();
    let read_samples = reader_loop(&pool, deadline).await;

    let mut buckets = LatencyBuckets {
        rollup_read: read_samples,
        ..Default::default()
    };
    while let Some(res) = writers.join_next().await {
        let per_writer = res.expect("writer task panic");
        buckets.merge(per_writer);
    }
    buckets
}

/// Writer loop: alternates between session_stats UPSERT (+ Stage C
/// fanout) and hook_events INSERT at ~10 QPS until the deadline.
async fn writer_loop(
    pool: &SqlitePool,
    writer_idx: usize,
    deadline: Instant,
    id_counter: Arc<AtomicU64>,
) -> LatencyBuckets {
    let mut buckets = LatencyBuckets::default();
    let tick = Duration::from_millis(1000 / QPS_PER_WRITER as u64);
    let mut next = Instant::now();
    let mut op_idx = 0usize;
    while Instant::now() < deadline {
        // Even writers → session_stats + Stage C; odd → hook_events.
        // Each writer's mix stays consistent so per-op samples are
        // comparable.
        if writer_idx.is_multiple_of(2) {
            let id = id_counter.fetch_add(1, Ordering::Relaxed);
            let session_id = format!("bench-{writer_idx:02}-{id:06}");
            let ts = deadline.elapsed().as_secs() as i64
                + NaiveDate::from_ymd_opt(2026, 4, 20)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp();

            let t0 = Instant::now();
            sqlx::query(
                r"INSERT OR REPLACE INTO session_stats (
                    session_id, source_content_hash, source_size,
                    parser_version, stats_version, indexed_at,
                    total_input_tokens, last_message_at, project_id,
                    git_branch, primary_model
                ) VALUES (?, X'01', 0, 1, 1, 0, ?, ?, ?, 'main', 'claude-opus-4-7')",
            )
            .bind(&session_id)
            .bind((op_idx as i64) * 50)
            .bind(ts)
            .bind(format!("bench-proj-{}", writer_idx % 3))
            .execute(pool)
            .await
            .expect("session_stats upsert");
            buckets.session_stats_upsert.push(t0.elapsed());

            let delta = StatsDelta {
                session_id: session_id.clone(),
                source_content_hash: vec![1],
                source_size: 0,
                source_inode: None,
                source_mid_hash: None,
                project_id: format!("bench-proj-{}", writer_idx % 3),
                source_file_path: String::new(),
                is_compressed: false,
                source_mtime: ts,
                stats: SessionStats {
                    total_input_tokens: (op_idx as u64) * 50,
                    last_message_at: Some(
                        chrono::DateTime::from_timestamp(ts, 0)
                            .unwrap()
                            .to_rfc3339(),
                    ),
                    primary_model: Some("claude-opus-4-7".to_string()),
                    git_branch: Some("main".to_string()),
                    ..Default::default()
                },
                old: None,
                seq: 0,
                source: DeltaSource::Indexer,
            };
            let t0 = Instant::now();
            apply_stats_delta(pool, &delta)
                .await
                .expect("stage_c apply_stats_delta");
            buckets.stage_c_fanout.push(t0.elapsed());
        } else {
            let session_id = format!("bench-hook-{writer_idx:02}");
            let ts = chrono::Utc::now().timestamp_millis();
            let t0 = Instant::now();
            sqlx::query(
                r"INSERT INTO hook_events (
                    session_id, timestamp, event_name, tool_name,
                    label, group_name, context
                ) VALUES (?, ?, 'BenchEvent', 'bench', 'bench-label', 'bench-group', NULL)",
            )
            .bind(&session_id)
            .bind(ts)
            .execute(pool)
            .await
            .expect("hook_events insert");
            buckets.hook_events_insert.push(t0.elapsed());
        }

        op_idx += 1;
        next += tick;
        let now = Instant::now();
        if next > now {
            tokio::time::sleep_until(next.into()).await;
        } else {
            // Missed our slot — reset so we don't burst.
            next = now + tick;
        }
    }
    buckets
}

/// Reader loop: hits `sum_global_stats_in_range` at 10 QPS.
async fn reader_loop(pool: &SqlitePool, deadline: Instant) -> Vec<Duration> {
    let mut samples = Vec::new();
    let tick = Duration::from_millis(100); // 10 QPS
    let mut next = Instant::now();
    let base = NaiveDate::from_ymd_opt(2026, 4, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end = base + 30 * 86400;

    while Instant::now() < deadline {
        let t0 = Instant::now();
        let _ = sum_global_stats_in_range(pool, Bucket::Daily, base, end)
            .await
            .expect("rollup read");
        samples.push(t0.elapsed());

        next += tick;
        let now = Instant::now();
        if next > now {
            tokio::time::sleep_until(next.into()).await;
        } else {
            next = now + tick;
        }
    }
    samples
}

/// Entry point — `#[ignore]` so `cargo test` doesn't walk the bench
/// into CI. Run manually per module-level doc.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "bench — run explicitly with --ignored --nocapture"]
async fn multi_writer_bench() {
    let tmp = tempfile::tempdir().expect("tempdir for bench DB");
    let db_path = tmp.path().join("multi_writer_bench.db");
    let db = Database::new(&db_path)
        .await
        .expect("open file-backed DB for bench");

    let full = std::env::var("CLAUDE_VIEW_BENCH_FULL").as_deref() == Ok("1");
    let duration = if full {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(5)
    };
    let mode = if full { "full" } else { "fast" };

    eprintln!(
        "multi_writer_bench: mode={mode} writers={WRITERS} qps_per_writer={QPS_PER_WRITER} \
         duration_s={}",
        duration.as_secs()
    );

    let buckets = run_bench(&db, duration).await;

    let session_stats = OpSummary::from_samples(buckets.session_stats_upsert);
    let stage_c = OpSummary::from_samples(buckets.stage_c_fanout);
    let hooks = OpSummary::from_samples(buckets.hook_events_insert);
    let reads = OpSummary::from_samples(buckets.rollup_read);

    eprintln!();
    eprintln!(
        "{:<28} {:>8} {:>10} {:>10} {:>10}",
        "op", "n", "p50_ms", "p99_ms", "max_ms"
    );
    eprintln!("{:-<68}", "");
    for (name, s) in [
        ("session_stats_upsert", &session_stats),
        ("stage_c_fanout", &stage_c),
        ("hook_events_insert", &hooks),
        ("rollup_read", &reads),
    ] {
        eprintln!(
            "{name:<28} {:>8} {:>10.3} {:>10.3} {:>10.3}",
            s.n, s.p50_ms, s.p99_ms, s.max_ms
        );
    }

    // Machine-readable line. Shell harness greps `^BENCH_JSON:` and
    // appends the JSON to the bench log; Rust stays schema-stable.
    #[derive(serde::Serialize)]
    struct BenchReport<'a> {
        mode: &'a str,
        writers: usize,
        qps_per_writer: usize,
        duration_s: u64,
        session_stats_upsert: &'a OpSummary,
        stage_c_fanout: &'a OpSummary,
        hook_events_insert: &'a OpSummary,
        rollup_read: &'a OpSummary,
    }
    let report = BenchReport {
        mode,
        writers: WRITERS,
        qps_per_writer: QPS_PER_WRITER,
        duration_s: duration.as_secs(),
        session_stats_upsert: &session_stats,
        stage_c_fanout: &stage_c,
        hook_events_insert: &hooks,
        rollup_read: &reads,
    };
    println!("BENCH_JSON:{}", serde_json::to_string(&report).unwrap());
}
