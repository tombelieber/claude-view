//! P1 — Rust parse latency benchmark.
//!
//! Companion to `scripts/bench-jsonl-ondemand.ts` (bun). Measures the
//! same size buckets on the same corpus (live + claude-backup gz),
//! but parses with Rust `serde_json` to get real numbers instead of
//! the bun upper bound.
//!
//! Two measurements per file:
//!   1. `value_parse` — `serde_json::from_str::<Value>` per line
//!      (apples-to-apples with bun's `JSON.parse`).
//!   2. `typed_parse` — `serde_json::from_str::<MinLine>` per line
//!      (what an on-demand reader would actually do — typed projection).
//!
//! Plus a dedicated section for gzipped backup files, which measures
//! the full on-demand read path for historical sessions:
//!   read + gunzip + parse.
//!
//! Run with:
//!   ./scripts/cq run --release -p claude-view-db --bin bench_jsonl_parse

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use flate2::read::GzDecoder;
use serde::Deserialize;
use walkdir::WalkDir;

const BUCKETS: &[(&str, u64, u64)] = &[
    ("< 10 KB", 0, 10_000),
    ("10-100 KB", 10_000, 100_000),
    ("100 KB-1 MB", 100_000, 1_000_000),
    ("1-10 MB", 1_000_000, 10_000_000),
    ("10-100 MB", 10_000_000, 100_000_000),
    (">= 100 MB", 100_000_000, u64::MAX),
];

const SAMPLES_PER_BUCKET: usize = 10;

#[derive(Debug, Clone)]
struct FileSample {
    path: PathBuf,
    bytes: u64,
    is_compressed: bool,
}

#[derive(Debug, Clone, Copy)]
struct ParseMeasurement {
    bytes: u64,
    /// Decompress time (0 for uncompressed files).
    decompress_us: u128,
    value_parse_us: u128,
    typed_parse_us: u128,
    line_count: u32,
    value_errors: u32,
    typed_errors: u32,
}

/// Minimal typed projection of an assistant line. Exercises nested
/// deserialisation without coupling the bench to `claude-view-core`'s
/// full `AssistantLine` (which may evolve).
#[derive(Deserialize)]
struct MinLine {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    line_type: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    message: Option<MinMessage>,
}

#[derive(Deserialize)]
struct MinMessage {
    #[serde(default)]
    #[allow(dead_code)]
    model: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<MinUsage>,
}

#[derive(Deserialize)]
struct MinUsage {
    #[serde(default)]
    #[allow(dead_code)]
    input_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    output_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    cache_creation_input_tokens: Option<u64>,
}

fn bucket_of(bytes: u64) -> &'static str {
    for (name, lo, hi) in BUCKETS {
        if bytes >= *lo && bytes < *hi {
            return name;
        }
    }
    "?"
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

/// Walk a single root, looking for files with the given extension
/// suffix. Parent-session paths have `<root>/<project>/<sid>.<suffix>`.
fn walk_one_root(root: &Path, suffix: &str, is_compressed: bool) -> Vec<FileSample> {
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
            let bytes = e.metadata().ok()?.len();
            Some(FileSample {
                path: path.to_path_buf(),
                bytes,
                is_compressed,
            })
        })
        .collect()
}

/// Walk live `~/.claude/projects` + all `~/.claude-backup/machines/*/projects`.
fn walk_all_sources() -> (Vec<FileSample>, usize, usize) {
    let home = std::env::var("HOME").expect("HOME unset");
    let live_root = PathBuf::from(&home).join(".claude").join("projects");
    let mut out = Vec::new();

    let live = walk_one_root(&live_root, ".jsonl", false);
    let live_count = live.len();
    out.extend(live);

    let backup_machines = PathBuf::from(&home).join(".claude-backup").join("machines");
    let mut backup_count = 0;
    if let Ok(entries) = fs::read_dir(&backup_machines) {
        for entry in entries.flatten() {
            let projects = entry.path().join("projects");
            if !projects.is_dir() {
                continue;
            }
            let rows = walk_one_root(&projects, ".jsonl.gz", true);
            backup_count += rows.len();
            out.extend(rows);
        }
    }

    (out, live_count, backup_count)
}

/// Measure a single file. For `.jsonl.gz` files, decompression is
/// included in `decompress_us`; parse times are measured against the
/// decompressed text.
fn parse_file(sample: &FileSample) -> Option<ParseMeasurement> {
    let buf = fs::read(&sample.path).ok()?;
    let (text_bytes, decompress_us) = if sample.is_compressed {
        let start = Instant::now();
        let mut decoder = GzDecoder::new(&buf[..]);
        let mut out = Vec::with_capacity(buf.len() * 3);
        decoder.read_to_end(&mut out).ok()?;
        (out, start.elapsed().as_micros())
    } else {
        (buf, 0u128)
    };
    let text = std::str::from_utf8(&text_bytes).ok()?;

    // Phase A: serde_json::Value per line
    let v_start = Instant::now();
    let mut line_count: u32 = 0;
    let mut value_errors: u32 = 0;
    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        line_count += 1;
        if serde_json::from_str::<serde_json::Value>(line).is_err() {
            value_errors += 1;
        }
    }
    let value_parse_us = v_start.elapsed().as_micros();

    // Phase B: typed projection
    let t_start = Instant::now();
    let mut typed_errors: u32 = 0;
    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        if serde_json::from_str::<MinLine>(line).is_err() {
            typed_errors += 1;
        }
    }
    let typed_parse_us = t_start.elapsed().as_micros();

    Some(ParseMeasurement {
        bytes: sample.bytes,
        decompress_us,
        value_parse_us,
        typed_parse_us,
        line_count,
        value_errors,
        typed_errors,
    })
}

fn pct_sorted(sorted: &[u128], p: u32) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * sorted.len() as f64).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn stride_sample<T: Clone>(arr: &[T], n: usize) -> Vec<T> {
    if arr.len() <= n {
        return arr.to_vec();
    }
    let stride = arr.len() / n;
    (0..n).map(|i| arr[i * stride].clone()).collect()
}

fn bench_bucket(name: &str, samples: &[FileSample], include_decompress: bool) {
    if samples.is_empty() {
        println!("    {:<16}  (empty)", name);
        return;
    }
    let picked = stride_sample(samples, SAMPLES_PER_BUCKET);
    let results: Vec<ParseMeasurement> = picked.iter().filter_map(parse_file).collect();
    if results.is_empty() {
        println!("    {:<16}  (parse errors)", name);
        return;
    }

    let mut v_times: Vec<u128> = results.iter().map(|r| r.value_parse_us).collect();
    v_times.sort_unstable();
    let mut t_times: Vec<u128> = results.iter().map(|r| r.typed_parse_us).collect();
    t_times.sort_unstable();
    let mut d_times: Vec<u128> = results.iter().map(|r| r.decompress_us).collect();
    d_times.sort_unstable();

    let total_bytes_s: u64 = results.iter().map(|r| r.bytes).sum();
    let total_us_s: u128 = results.iter().map(|r| r.value_parse_us).sum();
    let mbps = if total_us_s > 0 {
        (total_bytes_s as f64 / 1_048_576.0) / (total_us_s as f64 / 1_000_000.0)
    } else {
        0.0
    };

    if include_decompress {
        println!(
            "    {:<16}  {:>6}  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms",
            name,
            results.len(),
            pct_sorted(&d_times, 50) as f64 / 1000.0,
            pct_sorted(&d_times, 95) as f64 / 1000.0,
            pct_sorted(&v_times, 50) as f64 / 1000.0,
            pct_sorted(&v_times, 95) as f64 / 1000.0,
            pct_sorted(&t_times, 50) as f64 / 1000.0,
            pct_sorted(&t_times, 95) as f64 / 1000.0,
        );
    } else {
        println!(
            "    {:<16}  {:>6}  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms  {:>6.2}ms  {:>8.0}",
            name,
            results.len(),
            pct_sorted(&v_times, 50) as f64 / 1000.0,
            pct_sorted(&v_times, 95) as f64 / 1000.0,
            pct_sorted(&t_times, 50) as f64 / 1000.0,
            pct_sorted(&t_times, 95) as f64 / 1000.0,
            mbps,
        );
    }
}

fn main() {
    println!("\n=== P1 — Rust parse latency benchmark (live + backup) ===\n");
    println!("Corpus: ~/.claude/projects (live) + ~/.claude-backup/machines/*/projects (gz).\n");

    // Phase 1 — discovery
    let walk_start = Instant::now();
    let (files, live_count, backup_count) = walk_all_sources();
    let walk_ms = walk_start.elapsed().as_millis();

    if files.is_empty() {
        println!("  ⚠ No JSONL files found. Exiting.");
        return;
    }

    let total_bytes: u64 = files.iter().map(|f| f.bytes).sum();
    let live_bytes: u64 = files
        .iter()
        .filter(|f| !f.is_compressed)
        .map(|f| f.bytes)
        .sum();
    let backup_bytes: u64 = files
        .iter()
        .filter(|f| f.is_compressed)
        .map(|f| f.bytes)
        .sum();

    println!("Phase 1 — discovery");
    println!("  walked in             {} ms", walk_ms);
    println!(
        "  live (.jsonl):        {} files, {}",
        live_count,
        fmt_bytes(live_bytes)
    );
    println!(
        "  backup (.jsonl.gz):   {} files, {} (compressed on disk)",
        backup_count,
        fmt_bytes(backup_bytes)
    );
    println!("  total files:          {}", files.len());
    println!("  total bytes:          {}", fmt_bytes(total_bytes));
    println!();

    // Bucket by file size (compressed for backup, raw for live)
    let mut live_by_bucket: HashMap<&str, Vec<FileSample>> = HashMap::new();
    let mut gz_by_bucket: HashMap<&str, Vec<FileSample>> = HashMap::new();
    for (name, _, _) in BUCKETS {
        live_by_bucket.insert(name, Vec::new());
        gz_by_bucket.insert(name, Vec::new());
    }
    for f in &files {
        let bucket = bucket_of(f.bytes);
        if f.is_compressed {
            gz_by_bucket.get_mut(bucket).unwrap().push(f.clone());
        } else {
            live_by_bucket.get_mut(bucket).unwrap().push(f.clone());
        }
    }

    // Warm page cache
    println!("  warming disk cache (first 50 files)...");
    for f in files.iter().take(50) {
        let _ = fs::read(&f.path);
    }
    println!();

    // Phase 2 — uncompressed (live)
    println!(
        "Phase 2a — live .jsonl parse  ({} samples per bucket)\n",
        SAMPLES_PER_BUCKET
    );
    println!(
        "    {:<16}  {:>6}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}",
        "bucket", "n", "val_p50", "val_p95", "typ_p50", "typ_p95", "MB/s"
    );
    println!("    {}", "-".repeat(78));
    for (name, _, _) in BUCKETS {
        let bucket_samples = live_by_bucket[name].clone();
        bench_bucket(name, &bucket_samples, false);
    }
    println!();

    // Phase 3 — compressed (backup)
    println!(
        "Phase 2b — backup .jsonl.gz parse  ({} samples per bucket, gunzip included)\n",
        SAMPLES_PER_BUCKET
    );
    println!(
        "    {:<16}  {:>6}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}",
        "bucket", "n", "gz_p50", "gz_p95", "val_p50", "val_p95", "typ_p50", "typ_p95"
    );
    println!("    {}", "-".repeat(82));
    for (name, _, _) in BUCKETS {
        let bucket_samples = gz_by_bucket[name].clone();
        bench_bucket(name, &bucket_samples, true);
    }
    println!();

    println!("=== Verdict hints ===");
    println!();
    println!("P1 gate: Rust p95 typed parse ≤ 3 ms at p95 session size (100 KB-1 MB).");
    println!("For gzipped backups, total on-demand read = gz_p95 + typ_p95.");
    println!("If gz_p95 + typ_p95 ≤ 10 ms at p95 size, backup reads are viable");
    println!("on demand without any cache layer.");
    println!();
}
