//! P4 — Rollup rebuild benchmark.
//!
//! Full-rebuild of the proposed `analytics_rollup` feature from scratch:
//! walk live + backup, parse every `.jsonl` and `.jsonl.gz`, accumulate
//! (date, project_id) buckets, report wall clock and aggregates.
//!
//! This is the "self-heal" path for the rollup feature — the design
//! doc claims this is fast enough to run on startup + every 6 hours.
//! This bench verifies the claim.
//!
//! Measures:
//!   - wall clock time for full rebuild (sequential, single-threaded)
//!   - per-session average cost
//!   - bucket count
//!   - aggregate token totals (sanity check vs current DB state)
//!
//! Run:
//!   ./scripts/cq run --release -p claude-view-db --bin bench_rollup_rebuild

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct FileRef {
    path: PathBuf,
    id: String,
    project_id: String,
    bytes: u64,
    is_compressed: bool,
}

#[derive(Deserialize)]
struct MinLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<MinMessage>,
}

#[derive(Deserialize)]
struct MinMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    usage: Option<MinUsage>,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
struct MinUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

#[derive(Default, Debug, Clone)]
struct RollupBucket {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
    session_count: u32,
}

type RollupKey = (String, String); // (date YYYY-MM-DD, project_id)

fn walk_one_root(root: &Path, suffix: &str, is_compressed: bool) -> Vec<FileRef> {
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
            let bytes = e.metadata().ok()?.len();
            Some(FileRef {
                path: path.to_path_buf(),
                id,
                project_id,
                bytes,
                is_compressed,
            })
        })
        .collect()
}

fn walk_all() -> Vec<FileRef> {
    let home = std::env::var("HOME").expect("HOME unset");
    let mut out: HashMap<String, FileRef> = HashMap::new();

    let live_root = PathBuf::from(&home).join(".claude").join("projects");
    for f in walk_one_root(&live_root, ".jsonl", false) {
        out.insert(f.id.clone(), f);
    }

    let backup_machines = PathBuf::from(&home).join(".claude-backup").join("machines");
    if let Ok(entries) = fs::read_dir(&backup_machines) {
        for entry in entries.flatten() {
            let projects = entry.path().join("projects");
            if !projects.is_dir() {
                continue;
            }
            for f in walk_one_root(&projects, ".jsonl.gz", true) {
                if !out.contains_key(&f.id) {
                    out.insert(f.id.clone(), f);
                }
            }
        }
    }

    out.into_values().collect()
}

fn date_bucket(ts: &str) -> Option<String> {
    // Claude Code uses ISO 8601 with millis, e.g. "2026-04-16T12:34:56.789Z"
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).format("%Y-%m-%d").to_string())
}

/// Apply one file's usage lines to the rollup accumulator.
/// Returns (lines_processed, usage_lines, bytes_read_uncompressed).
fn apply_file(file: &FileRef, rollup: &mut HashMap<RollupKey, RollupBucket>) -> (u32, u32, u64) {
    let buf = match fs::read(&file.path) {
        Ok(b) => b,
        Err(_) => return (0, 0, 0),
    };
    let text_bytes = if file.is_compressed {
        let mut decoder = GzDecoder::new(&buf[..]);
        let mut out = Vec::with_capacity(buf.len() * 3);
        if decoder.read_to_end(&mut out).is_err() {
            return (0, 0, buf.len() as u64);
        }
        out
    } else {
        buf
    };
    let text = match std::str::from_utf8(&text_bytes) {
        Ok(s) => s,
        Err(_) => return (0, 0, text_bytes.len() as u64),
    };

    let mut seen_msg_ids: HashSet<String> = HashSet::new();
    let mut dates_touched: HashSet<String> = HashSet::new();
    let mut lines_processed: u32 = 0;
    let mut usage_lines: u32 = 0;

    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        lines_processed += 1;

        let parsed: MinLine = match serde_json::from_str(line) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if parsed.line_type.as_deref() != Some("assistant") {
            continue;
        }

        let Some(message) = parsed.message else {
            continue;
        };
        let Some(usage) = message.usage else {
            continue;
        };

        // Dedup on message id — match should_count_usage_block semantics.
        if let Some(msg_id) = &message.id {
            if !seen_msg_ids.insert(msg_id.clone()) {
                continue;
            }
        }

        let Some(ts) = parsed.timestamp else {
            continue;
        };
        let Some(date) = date_bucket(&ts) else {
            continue;
        };

        usage_lines += 1;
        let key = (date.clone(), file.project_id.clone());
        let bucket = rollup.entry(key).or_default();
        bucket.input_tokens += usage.input_tokens.unwrap_or(0);
        bucket.output_tokens += usage.output_tokens.unwrap_or(0);
        bucket.cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
        bucket.cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
        dates_touched.insert(date);
    }

    for date in dates_touched {
        let key = (date, file.project_id.clone());
        if let Some(bucket) = rollup.get_mut(&key) {
            bucket.session_count += 1;
        }
    }

    (lines_processed, usage_lines, text_bytes.len() as u64)
}

fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{} B", n);
    }
    if n < 1024 * 1024 {
        return format!("{:.1} KB", n as f64 / 1024.0);
    }
    if n < 1024 * 1024 * 1024 {
        return format!("{:.1} MB", n as f64 / 1_048_576.0);
    }
    format!("{:.2} GB", n as f64 / 1_073_741_824.0)
}

fn fmt_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.insert(0, ',');
        }
        out.insert(0, c);
    }
    out
}

fn main() {
    println!("\n=== P4 — Rollup rebuild benchmark ===\n");
    println!("Simulates the 'self-heal' path for the analytics_rollup feature:");
    println!("walk live + backup, parse every JSONL/.jsonl.gz, accumulate");
    println!("(date, project_id) buckets. Measures full-rebuild wall clock.\n");

    // Phase 1 — walk
    let walk_start = Instant::now();
    let files = walk_all();
    let walk_ms = walk_start.elapsed().as_millis();
    if files.is_empty() {
        println!("  ⚠ No JSONL files found. Exiting.");
        return;
    }
    let total_bytes_on_disk: u64 = files.iter().map(|f| f.bytes).sum();
    let live_count = files.iter().filter(|f| !f.is_compressed).count();
    let gz_count = files.iter().filter(|f| f.is_compressed).count();
    println!("Phase 1 — discovery");
    println!("  walked in             {} ms", walk_ms);
    println!(
        "  files:                {} ({} live + {} gz)",
        files.len(),
        live_count,
        gz_count
    );
    println!("  bytes on disk:        {}", fmt_bytes(total_bytes_on_disk));
    println!();

    // Phase 2 — sequential rebuild
    println!("Phase 2 — full rollup rebuild (single-threaded)");
    let rebuild_start = Instant::now();
    let mut rollup: HashMap<RollupKey, RollupBucket> = HashMap::new();
    let mut total_lines: u64 = 0;
    let mut total_usage_lines: u64 = 0;
    let mut total_text_bytes: u64 = 0;
    let mut last_progress = Instant::now();

    for (i, file) in files.iter().enumerate() {
        let (lines, usage, bytes) = apply_file(file, &mut rollup);
        total_lines += lines as u64;
        total_usage_lines += usage as u64;
        total_text_bytes += bytes;

        if last_progress.elapsed().as_secs() >= 2 {
            println!(
                "  progress: {}/{} files, {} buckets, {} usage lines",
                i + 1,
                files.len(),
                fmt_count(rollup.len() as u64),
                fmt_count(total_usage_lines)
            );
            last_progress = Instant::now();
        }
    }
    let rebuild_ms = rebuild_start.elapsed().as_millis();
    let rebuild_s = rebuild_ms as f64 / 1000.0;

    println!();
    println!("Phase 3 — results\n");
    println!(
        "  rebuild wall clock:           {:.2} s  ({} ms)",
        rebuild_s, rebuild_ms
    );
    println!("  sessions processed:           {}", files.len());
    println!(
        "  per-session avg:              {:.2} ms",
        rebuild_ms as f64 / files.len() as f64
    );
    println!("  lines parsed (JSONL):         {}", fmt_count(total_lines));
    println!(
        "  usage-bearing lines:          {}",
        fmt_count(total_usage_lines)
    );
    println!(
        "  bytes read (uncompressed):    {}",
        fmt_bytes(total_text_bytes)
    );
    if rebuild_ms > 0 {
        let mbps = (total_text_bytes as f64 / 1_048_576.0) / rebuild_s;
        println!("  throughput:                   {:.0} MB/s", mbps);
    }
    println!();
    println!(
        "  rollup_daily buckets built:   {}",
        fmt_count(rollup.len() as u64)
    );

    let total_in: u64 = rollup.values().map(|b| b.input_tokens).sum();
    let total_out: u64 = rollup.values().map(|b| b.output_tokens).sum();
    let total_cr: u64 = rollup.values().map(|b| b.cache_read_tokens).sum();
    let total_cc: u64 = rollup.values().map(|b| b.cache_creation_tokens).sum();
    println!();
    println!("  aggregate totals across all buckets:");
    println!("    input_tokens               {}", fmt_count(total_in));
    println!("    output_tokens              {}", fmt_count(total_out));
    println!("    cache_read_tokens          {}", fmt_count(total_cr));
    println!("    cache_creation_tokens      {}", fmt_count(total_cc));
    println!();

    // Approx memory: ~100 bytes per bucket (key string + 40-byte struct)
    let approx_bytes = rollup.len() as u64 * 150;
    println!(
        "  approx rollup memory:         {} ({} buckets × ~150 B)",
        fmt_bytes(approx_bytes),
        fmt_count(rollup.len() as u64)
    );
    println!();

    println!("=== Verdict ===\n");
    if rebuild_s < 10.0 {
        println!("  ✅ PASSED — full rebuild under 10 s.");
        println!("     Design decision: run full rebuild on startup AND every 6h.");
        println!("     Incremental updates become an optimisation, not a requirement.");
    } else if rebuild_s < 60.0 {
        println!("  🟡 ACCEPTABLE — full rebuild 10-60 s.");
        println!("     Design decision: run full rebuild on startup + nightly.");
        println!("     Incremental updates needed to stay fresh intra-day.");
    } else {
        println!("  ⚠ SLOW — full rebuild over 1 minute.");
        println!("     Design decision: incremental is mandatory, full rebuild weekly.");
        println!("     Consider parallelism (rayon) to cut rebuild time.");
    }
    println!();
}
