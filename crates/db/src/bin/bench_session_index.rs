//! P2 — Session list query latency benchmark.
//!
//! Answers: can a minimal `session_index` table serve typical list
//! queries within 30 ms p95 for the full corpus (live + claude-backup)?
//! And does SQLite even help here, or is a plain in-memory
//! `Vec<SessionIndexRow>` faster?
//!
//! Data sources:
//!   live    — ~/.claude/projects/<proj>/<sid>.jsonl
//!   backup  — ~/.claude-backup/machines/*/projects/<proj>/<sid>.jsonl.gz
//!
//! Dedup strategy: if a session id appears in both, live wins (it is
//! the more recent copy; backup is at most 1 day behind per the
//! daily backup schedule).
//!
//! Run:
//!   ./scripts/cq run --release -p claude-view-db --bin bench_session_index

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use walkdir::WalkDir;

const LIST_LIMIT: usize = 50;
const ITERS: usize = 1_000;

#[derive(Debug, Clone)]
struct SessionIndexRow {
    id: String,
    file_path: String,
    mtime: i64,
    bytes: i64,
    last_ts: i64, // proxy: mtime (real impl would read last JSONL line)
    project_id: String,
    is_compressed: bool,
}

#[derive(Debug, Default)]
struct WalkStats {
    live_walked: usize,
    backup_walked: usize,
    backup_unique: usize,
    total: usize,
}

/// Walk a single root, looking for files whose name ends with the
/// given suffix. Parent sessions live at `<root>/<project>/<sid><suffix>`.
fn walk_one_root(root: &Path, suffix: &str, is_compressed: bool) -> Vec<SessionIndexRow> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.ends_with(suffix))
        })
        .filter_map(|e| {
            let path = e.path();
            let rel = path.strip_prefix(root).ok()?;
            let components: Vec<_> = rel.components().collect();
            if components.len() != 2 {
                return None;
            }
            let project_id = components[0].as_os_str().to_str()?.to_string();
            let filename = path.file_name()?.to_str()?;
            let id = filename.strip_suffix(suffix)?.to_string();
            let meta = e.metadata().ok()?;
            let mtime = meta
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            Some(SessionIndexRow {
                id,
                file_path: path.to_string_lossy().to_string(),
                mtime,
                bytes: meta.len() as i64,
                last_ts: mtime,
                project_id,
                is_compressed,
            })
        })
        .collect()
}

/// Walk live + backup, dedup by session id (live wins).
fn walk_all_sources() -> (Vec<SessionIndexRow>, WalkStats) {
    let home = std::env::var("HOME").expect("HOME unset");
    let mut stats = WalkStats::default();
    let mut by_id: HashMap<String, SessionIndexRow> = HashMap::new();

    // Pass 1 — live (preferred)
    let live_root = PathBuf::from(&home).join(".claude").join("projects");
    for row in walk_one_root(&live_root, ".jsonl", false) {
        stats.live_walked += 1;
        by_id.insert(row.id.clone(), row);
    }

    // Pass 2 — backup (only insert ids not already present)
    let backup_machines = PathBuf::from(&home).join(".claude-backup").join("machines");
    if let Ok(entries) = fs::read_dir(&backup_machines) {
        for entry in entries.flatten() {
            let projects = entry.path().join("projects");
            if !projects.is_dir() {
                continue;
            }
            for row in walk_one_root(&projects, ".jsonl.gz", true) {
                stats.backup_walked += 1;
                if !by_id.contains_key(&row.id) {
                    stats.backup_unique += 1;
                    by_id.insert(row.id.clone(), row);
                }
            }
        }
    }

    stats.total = by_id.len();
    (by_id.into_values().collect(), stats)
}

fn build_sqlite_index(rows: &[SessionIndexRow]) -> rusqlite::Result<(Connection, PathBuf)> {
    let tmp = std::env::temp_dir().join("bench_session_index.db");
    let _ = fs::remove_file(&tmp);
    let conn = Connection::open(&tmp)?;

    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;

    conn.execute_batch(
        r#"
        CREATE TABLE session_index (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            mtime INTEGER NOT NULL,
            bytes INTEGER NOT NULL,
            last_ts INTEGER NOT NULL,
            project_id TEXT NOT NULL,
            is_compressed INTEGER NOT NULL
        );
        CREATE INDEX idx_last_ts ON session_index(last_ts DESC);
        CREATE INDEX idx_project_last_ts ON session_index(project_id, last_ts DESC);
        "#,
    )?;

    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare("INSERT INTO session_index VALUES (?, ?, ?, ?, ?, ?, ?)")?;
        for r in rows {
            stmt.execute(params![
                r.id,
                r.file_path,
                r.mtime,
                r.bytes,
                r.last_ts,
                r.project_id,
                r.is_compressed as i64,
            ])?;
        }
    }
    tx.commit()?;
    conn.execute_batch("ANALYZE;")?;

    Ok((conn, tmp))
}

/// Time an operation `iters` times; return (p50_us, p95_us, total_us).
fn time_iter<F: FnMut() -> usize>(iters: usize, mut f: F) -> (u128, u128, u128) {
    let _ = f(); // warm-up (not counted)
    let mut times: Vec<u128> = Vec::with_capacity(iters);
    let start_total = Instant::now();
    for _ in 0..iters {
        let start = Instant::now();
        let _ = f();
        times.push(start.elapsed().as_micros());
    }
    let total_us = start_total.elapsed().as_micros();
    times.sort_unstable();
    let p50 = times[times.len() / 2];
    let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
    (p50, p95, total_us)
}

fn fmt_us(us: u128) -> String {
    if us < 1_000 {
        format!("{:>5} µs", us)
    } else {
        format!("{:>4.1} ms", us as f64 / 1000.0)
    }
}

fn report(
    name: &str,
    sql_p50: u128,
    sql_p95: u128,
    vec_p50: u128,
    vec_p95: u128,
    result_rows: usize,
) {
    let ratio = if sql_p50 > 0 {
        vec_p50 as f64 / sql_p50 as f64
    } else {
        0.0
    };
    let verdict = if vec_p50 < sql_p50 {
        "vec wins"
    } else {
        "sql wins"
    };
    println!(
        "  {:<32}  {}  {}  {}  {}  rows={:>4}  ratio={:.2}× ({})",
        name,
        fmt_us(sql_p50),
        fmt_us(sql_p95),
        fmt_us(vec_p50),
        fmt_us(vec_p95),
        result_rows,
        ratio,
        verdict
    );
}

fn main() -> rusqlite::Result<()> {
    println!("\n=== P2 — Session list query latency benchmark (live + backup) ===\n");
    println!("Gate: p95 ≤ 30 ms for session list queries (any store).");
    println!("Also measured: SQLite index vs plain in-memory Vec, same queries.\n");

    // Phase 1 — walk both sources
    println!("Phase 1 — walk live + backup");
    let walk_start = Instant::now();
    let (rows, stats) = walk_all_sources();
    let walk_ms = walk_start.elapsed().as_millis();

    if rows.is_empty() {
        println!("  ⚠ No JSONL files found. Exiting.");
        return Ok(());
    }

    let mut proj_counts = HashMap::<String, usize>::new();
    for r in &rows {
        *proj_counts.entry(r.project_id.clone()).or_default() += 1;
    }
    let projects = proj_counts.len();
    let largest_proj = proj_counts
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(k, n)| (k.clone(), *n))
        .unwrap();

    let compressed_count = rows.iter().filter(|r| r.is_compressed).count();

    println!("  walked in            {} ms", walk_ms);
    println!("  live rows walked:    {}", stats.live_walked);
    println!("  backup rows walked:  {}", stats.backup_walked);
    println!("  backup unique (new): {}", stats.backup_unique);
    println!("  total after dedup:   {}", stats.total);
    println!(
        "    of which live:     {} ({} compressed)",
        rows.len() - compressed_count,
        0
    );
    println!("    of which backup:   {}", compressed_count);
    println!("  distinct projects:   {}", projects);
    println!(
        "  largest project:     {} ({} sessions)",
        largest_proj.0, largest_proj.1
    );
    println!();

    // Phase 2 — build SQLite
    println!("Phase 2 — build SQLite index");
    let build_start = Instant::now();
    let (conn, db_path) = build_sqlite_index(&rows)?;
    let build_ms = build_start.elapsed().as_millis();
    let db_size = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    println!("  build in             {} ms", build_ms);
    println!(
        "  db size:             {:.1} KB  ({} bytes / row)",
        db_size as f64 / 1024.0,
        if rows.is_empty() {
            0
        } else {
            db_size as usize / rows.len()
        }
    );
    println!();

    // Phase 3 — queries
    println!(
        "Phase 3 — run queries  ({} iterations each, warm caches)\n",
        ITERS
    );
    println!(
        "  {:<32}  {:>8}  {:>8}  {:>8}  {:>8}",
        "query", "sql_p50", "sql_p95", "vec_p50", "vec_p95"
    );
    println!("  {}", "-".repeat(95));

    // Q1 — recent 50 across all projects
    let q1_rows = std::cmp::min(LIST_LIMIT, rows.len());
    let (q1_sql_p50, q1_sql_p95, _) = time_iter(ITERS, || {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, project_id, last_ts FROM session_index ORDER BY last_ts DESC LIMIT ?",
            )
            .unwrap();
        stmt.query_map(params![LIST_LIMIT as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .count()
    });
    let (q1_vec_p50, q1_vec_p95, _) = time_iter(ITERS, || {
        let mut sorted: Vec<&SessionIndexRow> = rows.iter().collect();
        sorted.sort_unstable_by_key(|r| std::cmp::Reverse(r.last_ts));
        sorted.into_iter().take(LIST_LIMIT).count()
    });
    report(
        "Q1 recent 50 (all projects)",
        q1_sql_p50,
        q1_sql_p95,
        q1_vec_p50,
        q1_vec_p95,
        q1_rows,
    );

    // Q2 — recent 50 in the largest project
    let target_project = largest_proj.0.clone();
    let q2_rows = std::cmp::min(LIST_LIMIT, largest_proj.1);
    let (q2_sql_p50, q2_sql_p95, _) = time_iter(ITERS, || {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, project_id, last_ts FROM session_index \
                 WHERE project_id = ? ORDER BY last_ts DESC LIMIT ?",
            )
            .unwrap();
        stmt.query_map(params![&target_project, LIST_LIMIT as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .count()
    });
    let (q2_vec_p50, q2_vec_p95, _) = time_iter(ITERS, || {
        let mut filtered: Vec<&SessionIndexRow> = rows
            .iter()
            .filter(|r| r.project_id == target_project)
            .collect();
        filtered.sort_unstable_by_key(|r| std::cmp::Reverse(r.last_ts));
        filtered.into_iter().take(LIST_LIMIT).count()
    });
    report(
        "Q2 recent 50 (one project)",
        q2_sql_p50,
        q2_sql_p95,
        q2_vec_p50,
        q2_vec_p95,
        q2_rows,
    );

    // Q3 — count sessions per project
    let q3_rows = projects;
    let (q3_sql_p50, q3_sql_p95, _) = time_iter(ITERS, || {
        let mut stmt = conn
            .prepare_cached("SELECT project_id, COUNT(*) FROM session_index GROUP BY project_id")
            .unwrap();
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .count()
    });
    let (q3_vec_p50, q3_vec_p95, _) = time_iter(ITERS, || {
        let mut map = HashMap::<&str, i64>::new();
        for r in &rows {
            *map.entry(r.project_id.as_str()).or_default() += 1;
        }
        map.len()
    });
    report(
        "Q3 count per project",
        q3_sql_p50,
        q3_sql_p95,
        q3_vec_p50,
        q3_vec_p95,
        q3_rows,
    );

    // Q4 — sessions in last 7 days
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let week_ago = now - 7 * 24 * 3600;
    let q4_rows = rows.iter().filter(|r| r.last_ts > week_ago).count();
    let (q4_sql_p50, q4_sql_p95, _) = time_iter(ITERS, || {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, project_id FROM session_index \
                 WHERE last_ts > ? ORDER BY last_ts DESC",
            )
            .unwrap();
        stmt.query_map(params![week_ago], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .count()
    });
    let (q4_vec_p50, q4_vec_p95, _) = time_iter(ITERS, || {
        let mut filtered: Vec<&SessionIndexRow> =
            rows.iter().filter(|r| r.last_ts > week_ago).collect();
        filtered.sort_unstable_by_key(|r| std::cmp::Reverse(r.last_ts));
        filtered.len()
    });
    report(
        "Q4 last 7 days (all projects)",
        q4_sql_p50,
        q4_sql_p95,
        q4_vec_p50,
        q4_vec_p95,
        q4_rows,
    );

    println!();

    // Verdict
    println!("=== Verdict ===\n");
    let worst_sql_p95 = [q1_sql_p95, q2_sql_p95, q3_sql_p95, q4_sql_p95]
        .into_iter()
        .max()
        .unwrap();
    let worst_vec_p95 = [q1_vec_p95, q2_vec_p95, q3_vec_p95, q4_vec_p95]
        .into_iter()
        .max()
        .unwrap();
    println!("  worst sql p95: {}", fmt_us(worst_sql_p95));
    println!("  worst vec p95: {}", fmt_us(worst_vec_p95));
    let gate_us = 30_000u128; // 30 ms
    println!(
        "  gate = 30 ms.  SQL: {}.  Vec: {}.",
        if worst_sql_p95 <= gate_us {
            "PASS"
        } else {
            "FAIL"
        },
        if worst_vec_p95 <= gate_us {
            "PASS"
        } else {
            "FAIL"
        },
    );
    println!();

    // Cleanup
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(format!("{}-wal", db_path.display()));
    let _ = fs::remove_file(format!("{}-shm", db_path.display()));

    Ok(())
}
