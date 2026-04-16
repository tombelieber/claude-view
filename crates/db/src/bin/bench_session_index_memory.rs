//! P2-I — Session index memory footprint benchmark.
//!
//! Loads the real `SessionIndex` from `~/.claude/projects` +
//! `~/.claude-backup/machines/*/projects`, then computes an
//! approximate memory footprint:
//!
//!   struct bytes:   rows × sizeof(SessionIndexRow)
//!   string heap:    Σ (id.len + file_path.len + project_id.len)
//!
//! Also reads macOS `task_info` via `libc::mach_task_self()` to get
//! an approximate peak RSS (best-effort — skipped if unavailable).
//!
//! Run:
//!   ./scripts/cq run --release -p claude-view-db --bin bench_session_index_memory

use std::mem::size_of;
use std::path::PathBuf;
use std::time::Instant;

use claude_view_db::jsonl_first_poc::session_index::{SessionIndex, SessionIndexRow};

fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.2} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", n as f64 / 1_073_741_824.0)
    }
}

fn rss_bytes() -> Option<u64> {
    // macOS: read from /usr/bin/ps — avoids pulling in libc::mach_task_self().
    // If ps is unavailable, return None.
    let pid = std::process::id();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let s = String::from_utf8(output.stdout).ok()?;
    let kb: u64 = s.trim().parse().ok()?;
    Some(kb * 1024)
}

fn main() {
    println!("\n=== P2-I — Session index memory footprint bench ===\n");

    let home = std::env::var("HOME").expect("HOME unset");
    let live_root = PathBuf::from(&home).join(".claude").join("projects");
    let backup_machines = PathBuf::from(&home).join(".claude-backup").join("machines");

    let rss_before = rss_bytes();

    println!("Phase 1 — rebuild session_index from filesystem");
    let idx = SessionIndex::new();
    let start = Instant::now();
    let stats = idx
        .rebuild_from_filesystem(&live_root, &backup_machines)
        .expect("rebuild failed");
    let elapsed = start.elapsed();

    println!("  walk time:         {} ms", elapsed.as_millis());
    println!("  live found:        {}", stats.live_found);
    println!("  backup found:      {}", stats.backup_found);
    println!("  backup unique:     {}", stats.backup_unique);
    println!("  total after dedup: {}", stats.total_after_dedup);
    println!();

    // Snapshot the rows for measurement
    let all_rows: Vec<SessionIndexRow> = idx.list(
        &Default::default(),
        claude_view_db::jsonl_first_poc::session_index::Sort::LastTsDesc,
        usize::MAX,
    );
    assert_eq!(all_rows.len(), stats.total_after_dedup);

    // Struct bytes
    let row_size = size_of::<SessionIndexRow>();
    let struct_bytes = (all_rows.len() * row_size) as u64;

    // String heap cost — sum of every String / PathBuf byte
    let string_heap: u64 = all_rows
        .iter()
        .map(|r| (r.id.len() + r.file_path.as_os_str().len() + r.project_id.len()) as u64)
        .sum();

    // Approximate HashMap overhead inside SessionIndex inner map:
    //   ~24 bytes per bucket + key String storage (already counted)
    let hashmap_overhead = (all_rows.len() * 48) as u64;

    let approx_total = struct_bytes + string_heap + hashmap_overhead;

    let rss_after = rss_bytes();

    println!("Phase 2 — memory accounting\n");
    println!("  rows:              {}", all_rows.len());
    println!("  sizeof(Row):       {} B", row_size);
    println!(
        "  struct bytes:      {} ({} rows × {} B)",
        fmt_bytes(struct_bytes),
        all_rows.len(),
        row_size
    );
    println!("  string heap:       {}", fmt_bytes(string_heap));
    println!(
        "  hashmap overhead:  {} (~48 B/bucket)",
        fmt_bytes(hashmap_overhead)
    );
    println!("  approx total:      {}", fmt_bytes(approx_total));
    println!();

    if let (Some(before), Some(after)) = (rss_before, rss_after) {
        println!("Phase 3 — process RSS (from `ps -o rss=`)\n");
        println!("  rss before:        {}", fmt_bytes(before));
        println!("  rss after:         {}", fmt_bytes(after));
        let delta = after.saturating_sub(before);
        println!("  rss delta:         {}", fmt_bytes(delta));
        println!();
    } else {
        println!("  (rss measurement unavailable)\n");
    }

    // Verdict
    println!("=== Verdict ===\n");
    let per_row_total = if all_rows.is_empty() {
        0
    } else {
        approx_total / all_rows.len() as u64
    };
    println!("  approx per-row cost: {} B", per_row_total);
    let projected_50k = per_row_total * 50_000;
    println!(
        "  projected @ 50k sessions: {}  (5.8× current corpus)",
        fmt_bytes(projected_50k)
    );
    let projected_500k = per_row_total * 500_000;
    println!(
        "  projected @ 500k sessions: {}  (58× current corpus)",
        fmt_bytes(projected_500k)
    );
    println!();
    if approx_total < 10 * 1024 * 1024 {
        println!("  ✅ PASSED — under 10 MB. No concerns even at 5-10× corpus growth.");
    } else {
        println!("  ⚠ approximate memory exceeds 10 MB — consider trimming row fields.");
    }
    println!();
}
