//! P2-G/H — Handler-level list latency benchmark.
//!
//! Loads the real `SessionCatalog` from the live filesystem and
//! measures the JSONL-first `GET /api/sessions` pipeline at the
//! function-call layer — including DTO mapping and
//! `serde_json::to_string` serialisation (axum's `Json<T>` does
//! the same work).
//!
//! Goal: prove the in-memory catalog list path can serve
//! sub-millisecond responses without a DB round-trip. The canonical
//! Axum handler lives in `crates/server/src/routes/sessions/list.rs`;
//! this bench keeps a skinny pure-function mirror of that pipeline so
//! the JSONL compute cost can be measured in isolation from axum.
//!
//! Run:
//!   ./scripts/cq run --release -p claude-view-db --bin bench_list_handler

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use claude_view_core::session_catalog::{CatalogRow, Filter, SessionCatalog, Sort};
use serde::Serialize;

const ITERS: usize = 1_000;

// Bench-local DTOs — intentionally mirror the shape of the axum
// response so `serde_json::to_string` pays the real cost.

#[derive(Debug, Clone, Serialize)]
struct SessionListItem {
    id: String,
    project_id: String,
    file_path: String,
    is_compressed: bool,
    bytes: u64,
    mtime: i64,
}

impl From<CatalogRow> for SessionListItem {
    fn from(r: CatalogRow) -> Self {
        Self {
            id: r.id,
            project_id: r.project_id,
            file_path: r.file_path.to_string_lossy().into_owned(),
            is_compressed: r.is_compressed,
            bytes: r.bytes,
            mtime: r.mtime,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SessionsListResponse {
    total: usize,
    items: Vec<SessionListItem>,
}

fn list_sessions(
    idx: &SessionCatalog,
    filter: &Filter,
    sort: Sort,
    limit: usize,
) -> SessionsListResponse {
    let rows = idx.list(filter, sort, limit);
    SessionsListResponse {
        total: idx.len(),
        items: rows.into_iter().map(Into::into).collect(),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSummary {
    project_id: String,
    session_count: usize,
}

fn list_projects(idx: &SessionCatalog) -> Vec<ProjectSummary> {
    let mut out: Vec<ProjectSummary> = idx
        .projects()
        .into_iter()
        .map(|(project_id, session_count)| ProjectSummary {
            project_id,
            session_count,
        })
        .collect();
    out.sort_unstable_by(|a, b| b.session_count.cmp(&a.session_count));
    out
}

fn pct(sorted: &[u128], p: u32) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * sorted.len() as f64).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn fmt_us(us: u128) -> String {
    if us < 1_000 {
        format!("{:>5} µs", us)
    } else {
        format!("{:>5.2} ms", us as f64 / 1000.0)
    }
}

fn time_iter<F: FnMut()>(iters: usize, mut f: F) -> (u128, u128, u128) {
    // warm-up
    f();
    let mut times: Vec<u128> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let start = Instant::now();
        f();
        times.push(start.elapsed().as_micros());
    }
    times.sort_unstable();
    (
        pct(&times, 50),
        pct(&times, 95),
        *times.last().unwrap_or(&0),
    )
}

fn main() {
    println!("\n=== P2-G/H — Handler-level list latency benchmark ===\n");
    println!("Goal: prove that `GET /api/sessions` can serve sub-millisecond");
    println!("responses from the in-memory SessionCatalog, including DTO");
    println!("mapping + JSON serialisation cost.\n");

    // Load the full catalog from the real filesystem
    let home = std::env::var("HOME").expect("HOME unset");
    let live_root = PathBuf::from(&home).join(".claude").join("projects");
    let backup = PathBuf::from(&home).join(".claude-backup").join("machines");

    let idx = SessionCatalog::new();
    let load_start = Instant::now();
    let stats = idx
        .rebuild_from_filesystem(&live_root, &backup)
        .expect("rebuild failed");
    let load_ms = load_start.elapsed().as_millis();

    println!("Phase 1 — load");
    println!("  walk ms:             {}", load_ms);
    println!("  total rows:          {}", stats.total_after_dedup);
    println!();

    // Pick the largest project as target
    let projects = list_projects(&idx);
    let target_project = projects.first().expect("no projects").project_id.clone();

    println!(
        "  largest project:     {} ({} sessions)",
        target_project,
        projects.first().unwrap().session_count
    );
    println!();

    // Q1 — recent 50 all projects
    let q1_filter = Filter::default();
    let (q1_p50, q1_p95, q1_max) = time_iter(ITERS, || {
        let resp: SessionsListResponse = list_sessions(&idx, &q1_filter, Sort::LastTsDesc, 50);
        let _json = serde_json::to_string(&resp).unwrap();
    });

    // Q2 — recent 50 one project
    let q2_filter = Filter::by_project(target_project.clone());
    let (q2_p50, q2_p95, q2_max) = time_iter(ITERS, || {
        let resp = list_sessions(&idx, &q2_filter, Sort::LastTsDesc, 50);
        let _json = serde_json::to_string(&resp).unwrap();
    });

    // Q3 — projects summary
    let (q3_p50, q3_p95, q3_max) = time_iter(ITERS, || {
        let resp = list_projects(&idx);
        let _json = serde_json::to_string(&resp).unwrap();
    });

    // Q4 — recent 200 last 7 days
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let q4_filter = Filter {
        project_id: None,
        min_last_ts: Some(now - 7 * 24 * 3600),
        max_last_ts: None,
    };
    let (q4_p50, q4_p95, q4_max) = time_iter(ITERS, || {
        let resp = list_sessions(&idx, &q4_filter, Sort::LastTsDesc, 200);
        let _json = serde_json::to_string(&resp).unwrap();
    });

    // Print table
    println!("Phase 2 — handler bench ({} iterations per query)\n", ITERS);
    println!(
        "  {:<40}  {:>10}  {:>10}  {:>10}",
        "query", "p50", "p95", "max"
    );
    println!("  {}", "-".repeat(78));
    println!(
        "  {:<40}  {}  {}  {}",
        "Q1 /api/sessions?limit=50 (all)",
        fmt_us(q1_p50),
        fmt_us(q1_p95),
        fmt_us(q1_max)
    );
    println!(
        "  {:<40}  {}  {}  {}",
        "Q2 /api/sessions?project=X (top)",
        fmt_us(q2_p50),
        fmt_us(q2_p95),
        fmt_us(q2_max)
    );
    println!(
        "  {:<40}  {}  {}  {}",
        "Q3 /api/projects",
        fmt_us(q3_p50),
        fmt_us(q3_p95),
        fmt_us(q3_max)
    );
    println!(
        "  {:<40}  {}  {}  {}",
        "Q4 /api/sessions?since=7d&limit=200",
        fmt_us(q4_p50),
        fmt_us(q4_p95),
        fmt_us(q4_max)
    );
    println!();

    // Measure the JSON size of a typical response
    let sample: SessionsListResponse =
        list_sessions(&idx, &Filter::default(), Sort::LastTsDesc, 50);
    let sample_json = serde_json::to_string(&sample).unwrap();
    println!(
        "  sample response size (Q1): {} bytes ({} items)",
        sample_json.len(),
        sample.items.len()
    );
    println!();

    // Verdict
    let worst_p95 = [q1_p95, q2_p95, q3_p95, q4_p95].into_iter().max().unwrap();
    let gate_us = 2_000u128; // 2 ms

    println!("=== Verdict ===\n");
    println!("  worst handler p95: {}", fmt_us(worst_p95));
    println!("  handler gate: 2 ms p95");
    if worst_p95 <= gate_us {
        println!("  ✅ PASSED by {}×", gate_us / worst_p95.max(1));
    } else {
        println!("  ⚠ over budget");
    }
    println!();
    println!("  Note: this is handler-function latency including DTO");
    println!("  mapping + serde_json::to_string. Real HTTP adds ~20-50 µs");
    println!("  for axum routing + response serialisation to the wire,");
    println!("  which is constant and dominated by kernel network write.");
    println!();
}
