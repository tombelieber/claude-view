//! Binary entry point for the evidence audit — pre-release JSONL schema guard.
//!
//! Usage:
//!   cargo run -p claude-view-core --bin evidence-audit [data_dir]
//!
//! Environment:
//!   EVIDENCE_QUICK=1  — run types-only check (skip deep checks)

use std::path::{Path, PathBuf};
use std::time::Instant;

use claude_view_core::evidence_audit::{
    check_set_diff, load_baseline, run_audit_checks, run_phase3_checks, scan_directory_parallel,
    scan_directory_parallel_with_pipeline, AuditResult,
};

// ANSI color codes
const RED: &str = "\x1b[0;31m";
const GREEN: &str = "\x1b[0;32m";
const YELLOW: &str = "\x1b[1;33m";
const BLUE: &str = "\x1b[0;34m";
const NC: &str = "\x1b[0m";

fn find_baseline() -> Option<PathBuf> {
    // 1. Try CARGO_MANIFEST_DIR ancestors (workspace root) for scripts/integrity/evidence-baseline.json
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest = PathBuf::from(manifest_dir);
        // crates/core -> go up 2 to workspace root
        if let Some(workspace_root) = manifest.ancestors().nth(2) {
            let candidate = workspace_root.join("scripts/integrity/evidence-baseline.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. Try current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("scripts/integrity/evidence-baseline.json");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 3. Fallback: alongside the binary executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join("evidence-baseline.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

fn write_manifest(
    data_dir: &Path,
    baseline_path: &Path,
    result: &AuditResult,
    elapsed_secs: f64,
    quick_mode: bool,
) {
    // Find artifacts dir via CARGO_MANIFEST_DIR or fallback
    let artifacts_dir = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest = PathBuf::from(manifest_dir);
        manifest
            .ancestors()
            .nth(2)
            .map(|root| root.join("artifacts/integrity"))
            .unwrap_or_else(|| PathBuf::from("artifacts/integrity"))
    } else {
        PathBuf::from("artifacts/integrity")
    };

    if let Err(e) = std::fs::create_dir_all(&artifacts_dir) {
        eprintln!(
            "{}warning:{} could not create artifacts dir: {}",
            YELLOW, NC, e
        );
        return;
    }

    let manifest_path = artifacts_dir.join("evidence-manifest.json");
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let manifest = serde_json::json!({
        "generated_at": now,
        "data_dir": data_dir.display().to_string(),
        "baseline": baseline_path.display().to_string(),
        "drift_detected": !result.passed,
        "checks_run": result.checks.len(),
        "cpu_cores": cpu_cores,
        "elapsed_seconds": (elapsed_secs * 10.0).round() / 10.0,
        "files_scanned": result.files_scanned,
        "lines_scanned": result.lines_scanned,
        "mode": if quick_mode { "quick" } else { "full" },
    });

    match serde_json::to_string_pretty(&manifest) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&manifest_path, json) {
                eprintln!("{}warning:{} could not write manifest: {}", YELLOW, NC, e);
            } else {
                eprintln!("  {}Manifest:{} {}", BLUE, NC, manifest_path.display());
            }
        }
        Err(e) => {
            eprintln!(
                "{}warning:{} could not serialize manifest: {}",
                YELLOW, NC, e
            );
        }
    }
}

fn main() {
    let start = Instant::now();

    // Parse CLI args: first arg = data directory
    let args: Vec<String> = std::env::args().collect();
    let data_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        dirs::home_dir()
            .map(|h| h.join(".claude/projects"))
            .unwrap_or_else(|| PathBuf::from(".claude/projects"))
    };

    let quick_mode = std::env::var("EVIDENCE_QUICK")
        .map(|v| v == "1")
        .unwrap_or(false);

    // Find baseline
    let baseline_path = match find_baseline() {
        Some(p) => p,
        None => {
            eprintln!(
                "\n{}ERROR:{} Could not find evidence-baseline.json",
                RED, NC
            );
            eprintln!("  Searched:");
            eprintln!("    - CARGO_MANIFEST_DIR/../../scripts/integrity/");
            eprintln!("    - <cwd>/scripts/integrity/");
            eprintln!("    - <exe_dir>/");
            std::process::exit(2);
        }
    };

    // Load baseline
    let baseline = match load_baseline(&baseline_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("\n{}ERROR:{} {}", RED, NC, e);
            std::process::exit(2);
        }
    };

    // Validate data directory exists before proceeding
    if !data_dir.is_dir() {
        eprintln!(
            "{}MISSING:{} JSONL data directory at {}",
            RED,
            NC,
            data_dir.display()
        );
        eprintln!("  Hint: pass a custom path as the first argument");
        std::process::exit(2);
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mode_label = if quick_mode {
        "QUICK (types only)"
    } else {
        "FULL (6 type + 12 pipeline + 2 field)"
    };

    // Print banner (file count filled in after scan)
    println!();
    println!("{}", "\u{2550}".repeat(59));
    println!(
        "  {}EVIDENCE AUDIT{} \u{2014} Pre-Release JSONL Schema Guard (Rust)",
        BLUE, NC
    );
    println!("{}", "\u{2550}".repeat(59));
    println!();
    println!("  {}Data dir:{}  {}", BLUE, NC, data_dir.display());
    println!("  {}Baseline:{} {}", BLUE, NC, baseline_path.display());
    println!("  {}Threads:{}  {}", BLUE, NC, num_threads);
    println!("  {}Mode:{}     {}", BLUE, NC, mode_label);

    // Scan directory (single walk — no separate discover step)
    let (signals, pipeline_results) = if quick_mode {
        let signals = scan_directory_parallel(&data_dir);
        (signals, None)
    } else {
        let (signals, pipeline) = scan_directory_parallel_with_pipeline(&data_dir);
        (signals, Some(pipeline))
    };
    let scan_elapsed = start.elapsed();

    let file_count = signals.files_scanned;
    println!("  {}Files:{}    {} JSONL files", BLUE, NC, file_count);

    if file_count < 10 && file_count > 0 {
        println!("  {YELLOW}WARNING: Only {file_count} files — results may be incomplete{NC}");
    }

    if file_count == 0 {
        println!();
        println!(
            "  {}No JSONL files found{} in {}",
            YELLOW,
            NC,
            data_dir.display()
        );
        println!();
        std::process::exit(2);
    }

    println!();
    println!(
        "  Scanned {} files, {} lines in {:.1}ms",
        signals.files_scanned,
        signals.lines_scanned,
        scan_elapsed.as_secs_f64() * 1000.0,
    );
    println!();

    // Run checks
    let result = if quick_mode {
        // Quick mode: only top-level types check
        let expected_top = baseline.top_level_types.all_known();
        let check = check_set_diff("top-level types", &signals.top_level_types, &expected_top);
        let passed = check.passed;
        AuditResult {
            passed,
            checks: vec![check],
            nesting_direct_count: 0,
            nesting_nested_count: 0,
            files_scanned: signals.files_scanned,
            lines_scanned: signals.lines_scanned,
            errors: signals.errors,
        }
    } else {
        run_audit_checks(&signals, &baseline)
    };

    // Print results
    let total_checks = result.checks.len();
    for (i, check) in result.checks.iter().enumerate() {
        let idx = i + 1;
        if check.passed {
            println!(
                "  [{}/{}] {}: {}OK{}",
                idx, total_checks, check.name, GREEN, NC
            );
        } else {
            println!(
                "  [{}/{}] {}: {}DRIFT{}",
                idx, total_checks, check.name, YELLOW, NC
            );
            for item in &check.new_items {
                println!("         {}+ {}{}", YELLOW, item, NC);
            }
        }

        if !check.absent_items.is_empty() {
            for item in &check.absent_items {
                println!("         (absent: {})", item);
            }
        }
    }

    // Nesting stats (only meaningful in full mode)
    if !quick_mode {
        println!();
        println!(
            "  {}Nesting:{} direct={}, nested={}",
            BLUE, NC, result.nesting_direct_count, result.nesting_nested_count
        );
    }

    let mut overall_passed = result.passed;

    if let Some(pipeline) = pipeline_results {
        println!();
        println!("  {BLUE}── Phase 2: Pipeline Invariants ──{NC}");
        let results = pipeline.into_results();
        let phase2_total = results.len();
        let total_checks = result.checks.len();
        for (i, check) in results.iter().enumerate() {
            let idx = total_checks + i + 1;
            let grand_total = total_checks + phase2_total;

            if check.skipped {
                println!(
                    "  [{}/{}] {}: {}SKIPPED{} ({})",
                    idx,
                    grand_total,
                    check.name,
                    YELLOW,
                    NC,
                    check
                        .sample_violations
                        .first()
                        .map(|v| v.detail.as_str())
                        .unwrap_or("deferred")
                );
            } else if check.passed {
                println!(
                    "  [{}/{}] {}: {}OK{} ({} lines, 0 violations)",
                    idx, grand_total, check.name, GREEN, NC, check.lines_checked
                );
            } else {
                println!(
                    "  [{}/{}] {}: {}FAIL{} ({} violations in {} lines)",
                    idx,
                    grand_total,
                    check.name,
                    RED,
                    NC,
                    check.violation_count,
                    check.lines_checked
                );
                for v in &check.sample_violations {
                    println!("           {}:{}", v.file, v.line_number);
                    println!("             {}", v.detail);
                }
            }
        }

        let phase2_passed = results.iter().all(|r| r.skipped || r.passed);
        overall_passed = overall_passed && phase2_passed;

        // Phase 3: Field Coverage
        println!("\n  {BLUE}── Phase 3: Field Coverage ──{NC}");
        let phase3_results = run_phase3_checks(&signals.field_inventory, &baseline);
        let phase2_count = results.len();
        let phase1_count = result.checks.len();
        for (i, check) in phase3_results.iter().enumerate() {
            let idx = phase1_count + phase2_count + i + 1;
            let grand_total = phase1_count + phase2_count + phase3_results.len();
            if check.skipped {
                println!(
                    "  [{idx}/{grand_total}] {}: {YELLOW}SKIPPED{NC} ({})",
                    check.name,
                    check
                        .sample_violations
                        .first()
                        .map(|v| v.detail.as_str())
                        .unwrap_or("deferred")
                );
            } else if check.passed {
                println!(
                    "  [{idx}/{grand_total}] {}: {GREEN}OK{NC} ({} paths, 0 violations)",
                    check.name, check.lines_checked
                );
            } else {
                println!(
                    "  [{idx}/{grand_total}] {}: {RED}FAIL{NC} ({} violations)",
                    check.name, check.violation_count
                );
                for v in &check.sample_violations {
                    println!("         {RED}→{NC} {}", v.detail);
                }
            }
        }
        let phase3_passed = phase3_results.iter().all(|r| r.skipped || r.passed);
        overall_passed = overall_passed && phase3_passed;
    }

    let total_elapsed = start.elapsed();
    let elapsed_secs = total_elapsed.as_secs_f64();
    let elapsed_ms = elapsed_secs * 1000.0;

    // Write manifest
    write_manifest(&data_dir, &baseline_path, &result, elapsed_secs, quick_mode);

    // Final summary
    println!();
    println!("{}", "\u{2500}".repeat(59));
    if overall_passed {
        println!(
            "  {}ALL CHECKS PASSED{} \u{2014} parser matches real data ({:.1}ms)",
            GREEN, NC, elapsed_ms
        );
    } else {
        let failed_count = result.checks.iter().filter(|c| !c.passed).count();
        println!(
            "  {}DRIFT DETECTED{} \u{2014} {} check(s) failed ({:.1}ms)",
            RED, NC, failed_count, elapsed_ms
        );
        println!();
        println!("  {}Fix steps:{}", YELLOW, NC);
        println!("    1. Fix the parser to handle new types/fields");
        println!("    2. Update scripts/integrity/evidence-baseline.json");
        println!("    3. Update docs/architecture/message-types.md");
        println!("    4. Re-run this audit to verify");
    }
    println!("{}", "\u{2500}".repeat(59));
    println!();

    if result.errors > 0 {
        eprintln!(
            "  {}note:{} {} parse errors encountered during scan",
            YELLOW, NC, result.errors
        );
    }

    std::process::exit(if overall_passed { 0 } else { 1 });
}
