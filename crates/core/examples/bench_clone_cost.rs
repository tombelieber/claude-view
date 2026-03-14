// crates/core/examples/bench_clone_cost.rs
// Measures the REAL cost of cloning serde_json::Value for raw_json preservation.
//
// Run with: cargo run --example bench_clone_cost -p claude-view-core --release
//
// Tests against real ~/.claude session files at multiple size tiers.

use std::path::{Path, PathBuf};
use std::time::Instant;

fn find_session_files(claude_dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    walk_jsonl(claude_dir, &mut files);
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files
}

fn walk_jsonl(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_jsonl(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            if let Ok(meta) = std::fs::metadata(&path) {
                out.push((path, meta.len()));
            }
        }
    }
}

struct BenchResult {
    label: String,
    file_size_kb: u64,
    line_count: usize,
    parse_only_ms: f64,
    parse_plus_clone_ms: f64,
    clone_overhead_ms: f64,
    clone_overhead_pct: f64,
    total_cloned_bytes: usize,
    peak_values_in_memory: usize,
}

fn bench_file(path: &Path, label: &str) -> BenchResult {
    let raw = std::fs::read_to_string(path).expect("read file");
    let file_size_kb = raw.len() as u64 / 1024;
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    let line_count = lines.len();

    // --- Pass 1: Parse only (current behavior) ---
    let start = Instant::now();
    let mut parsed_count = 0usize;
    for line in &lines {
        if let Ok(_value) = serde_json::from_str::<serde_json::Value>(line) {
            parsed_count += 1;
            // Simulate current parser: extract a few fields, drop value
        }
    }
    let parse_only = start.elapsed();

    // --- Pass 2: Parse + clone (proposed behavior) ---
    let start = Instant::now();
    let mut cloned_values: Vec<serde_json::Value> = Vec::with_capacity(line_count);
    let mut total_cloned_bytes = 0usize;
    for line in &lines {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            let cloned = value.clone();
            // Approximate size: serialized length of the clone
            total_cloned_bytes += line.len();
            cloned_values.push(cloned);
            // Original `value` drops here (simulates current parser behavior)
        }
    }
    let parse_plus_clone = start.elapsed();

    let parse_only_ms = parse_only.as_secs_f64() * 1000.0;
    let parse_plus_clone_ms = parse_plus_clone.as_secs_f64() * 1000.0;
    let clone_overhead_ms = parse_plus_clone_ms - parse_only_ms;
    let clone_overhead_pct = if parse_only_ms > 0.0 {
        (clone_overhead_ms / parse_only_ms) * 100.0
    } else {
        0.0
    };

    BenchResult {
        label: label.to_string(),
        file_size_kb,
        line_count,
        parse_only_ms,
        parse_plus_clone_ms,
        clone_overhead_ms,
        clone_overhead_pct,
        total_cloned_bytes,
        peak_values_in_memory: cloned_values.len(),
    }
}

fn bench_file_arc(path: &Path) -> (f64, f64) {
    let raw = std::fs::read_to_string(path).expect("read file");
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();

    // --- Arc<Value> approach ---
    let start = Instant::now();
    let mut arc_values: Vec<std::sync::Arc<serde_json::Value>> = Vec::with_capacity(lines.len());
    for line in &lines {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            let arc = std::sync::Arc::new(value);
            // Simulate 3 consumers each holding a reference
            let _ref1 = std::sync::Arc::clone(&arc);
            let _ref2 = std::sync::Arc::clone(&arc);
            let _ref3 = std::sync::Arc::clone(&arc);
            arc_values.push(arc);
        }
    }
    let arc_ms = start.elapsed().as_secs_f64() * 1000.0;

    // --- Plain clone approach for comparison ---
    let start = Instant::now();
    let mut clone_values: Vec<serde_json::Value> = Vec::with_capacity(lines.len());
    for line in &lines {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            clone_values.push(value.clone());
        }
    }
    let clone_ms = start.elapsed().as_secs_f64() * 1000.0;

    (arc_ms, clone_ms)
}

fn main() {
    let claude_dir = dirs::home_dir().expect("home dir").join(".claude");
    if !claude_dir.exists() {
        eprintln!("Error: ~/.claude does not exist");
        std::process::exit(1);
    }

    println!("=== claude-view raw_json clone cost benchmark ===");
    println!("Running against real ~/.claude session data");
    println!();

    let files = find_session_files(&claude_dir);
    println!("Found {} JSONL session files", files.len());
    println!();

    // Pick representative files at different size tiers
    let tiers: Vec<(&str, Box<dyn Fn(&(PathBuf, u64)) -> bool>)> = vec![
        ("tiny (<10KB)", Box::new(|f: &(PathBuf, u64)| f.1 < 10_000)),
        (
            "small (10-100KB)",
            Box::new(|f: &(PathBuf, u64)| f.1 >= 10_000 && f.1 < 100_000),
        ),
        (
            "medium (100KB-1MB)",
            Box::new(|f: &(PathBuf, u64)| f.1 >= 100_000 && f.1 < 1_000_000),
        ),
        (
            "large (1-10MB)",
            Box::new(|f: &(PathBuf, u64)| f.1 >= 1_000_000 && f.1 < 10_000_000),
        ),
        (
            "huge (>10MB)",
            Box::new(|f: &(PathBuf, u64)| f.1 >= 10_000_000),
        ),
    ];

    let mut results: Vec<BenchResult> = Vec::new();

    for (label, filter) in &tiers {
        let tier_files: Vec<&(PathBuf, u64)> = files.iter().filter(|f| filter(f)).collect();
        if tier_files.is_empty() {
            println!("  {} — no files found, skipping", label);
            continue;
        }

        // Pick the median file from this tier
        let median = tier_files[tier_files.len() / 2];
        // Run 3 iterations, take the median
        let mut runs: Vec<BenchResult> = Vec::new();
        for _ in 0..3 {
            runs.push(bench_file(&median.0, label));
        }
        runs.sort_by(|a, b| a.parse_only_ms.partial_cmp(&b.parse_only_ms).unwrap());
        results.push(runs.remove(1)); // median
    }

    // Also bench the LARGEST file specifically
    if let Some(largest) = files.last() {
        let mut runs: Vec<BenchResult> = Vec::new();
        for _ in 0..3 {
            runs.push(bench_file(&largest.0, "LARGEST FILE"));
        }
        runs.sort_by(|a, b| a.parse_only_ms.partial_cmp(&b.parse_only_ms).unwrap());
        results.push(runs.remove(1));
    }

    // Print results table
    println!();
    println!(
        "{:<20} {:>8} {:>6} {:>10} {:>10} {:>10} {:>8} {:>10}",
        "Tier", "Size", "Lines", "Parse(ms)", "+Clone(ms)", "Δ(ms)", "Δ(%)", "CloneKB"
    );
    println!("{}", "-".repeat(92));

    for r in &results {
        println!(
            "{:<20} {:>6}KB {:>6} {:>10.2} {:>10.2} {:>10.2} {:>7.1}% {:>8}KB",
            r.label,
            r.file_size_kb,
            r.line_count,
            r.parse_only_ms,
            r.parse_plus_clone_ms,
            r.clone_overhead_ms,
            r.clone_overhead_pct,
            r.total_cloned_bytes / 1024,
        );
    }

    // Arc vs Clone comparison on largest file
    if let Some(largest) = files.last() {
        println!();
        println!("=== Arc<Value> vs Value::clone() on largest file ===");
        let (arc_ms, clone_ms) = bench_file_arc(&largest.0);
        let parse_baseline = results.last().map(|r| r.parse_only_ms).unwrap_or(0.0);
        println!("  Parse only:       {:>8.2}ms", parse_baseline);
        println!(
            "  Parse + clone:    {:>8.2}ms  (+{:.2}ms / +{:.1}%)",
            clone_ms,
            clone_ms - parse_baseline,
            ((clone_ms - parse_baseline) / parse_baseline) * 100.0
        );
        println!(
            "  Parse + Arc::new: {:>8.2}ms  (+{:.2}ms / +{:.1}%)",
            arc_ms,
            arc_ms - parse_baseline,
            ((arc_ms - parse_baseline) / parse_baseline) * 100.0
        );
        println!(
            "  Arc savings vs clone: {:.2}ms ({:.1}%)",
            clone_ms - arc_ms,
            ((clone_ms - arc_ms) / clone_ms) * 100.0
        );
    }

    // Batch simulation: what if we clone during full indexing?
    println!();
    println!(
        "=== Batch indexing simulation (all {} files) ===",
        files.len()
    );

    let total_size_mb: f64 = files.iter().map(|f| f.1 as f64).sum::<f64>() / (1024.0 * 1024.0);
    println!(
        "  Total corpus: {:.1}MB across {} files",
        total_size_mb,
        files.len()
    );

    // Sample 50 files across the distribution, extrapolate
    let sample_size = 50.min(files.len());
    let step = files.len() / sample_size;
    let mut sample_parse_ms = 0.0f64;
    let mut sample_clone_ms = 0.0f64;
    let mut sample_bytes = 0u64;

    for i in 0..sample_size {
        let idx = i * step;
        let file = &files[idx];
        let raw = std::fs::read_to_string(&file.0).unwrap_or_default();
        let file_lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        sample_bytes += file.1;

        let start = Instant::now();
        for line in &file_lines {
            let _ = serde_json::from_str::<serde_json::Value>(line);
        }
        sample_parse_ms += start.elapsed().as_secs_f64() * 1000.0;

        let start = Instant::now();
        for line in &file_lines {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let _ = v.clone();
            }
        }
        sample_clone_ms += start.elapsed().as_secs_f64() * 1000.0;
    }

    let scale = total_size_mb / (sample_bytes as f64 / (1024.0 * 1024.0));
    let est_parse_ms = sample_parse_ms * scale;
    let est_clone_ms = sample_clone_ms * scale;
    let est_overhead_ms = est_clone_ms - est_parse_ms;

    println!(
        "  Estimated full parse (no clone): {:>8.0}ms ({:.2}s)",
        est_parse_ms,
        est_parse_ms / 1000.0
    );
    println!(
        "  Estimated parse + clone:         {:>8.0}ms ({:.2}s)",
        est_clone_ms,
        est_clone_ms / 1000.0
    );
    println!(
        "  Clone overhead for full index:   {:>8.0}ms ({:.2}s) — +{:.1}%",
        est_overhead_ms,
        est_overhead_ms / 1000.0,
        (est_overhead_ms / est_parse_ms) * 100.0
    );

    println!();
    println!("=== Verdict ===");
    if let Some(largest_result) = results.last() {
        let single_session_overhead = largest_result.clone_overhead_ms;
        println!(
            "  Single session view (worst case): +{:.2}ms clone overhead",
            single_session_overhead
        );
        println!(
            "  Full indexing (all files):         +{:.0}ms clone overhead",
            est_overhead_ms
        );
        if single_session_overhead < 50.0 {
            println!("  → Single session: NEGLIGIBLE (< 50ms human perception threshold)");
        } else {
            println!("  → Single session: NOTICEABLE — consider conditional raw_json");
        }
        if est_overhead_ms < 1000.0 {
            println!("  → Full indexing:  ACCEPTABLE (< 1s added)");
        } else {
            println!("  → Full indexing:  SIGNIFICANT — do NOT clone during batch indexing");
        }
    }
}
