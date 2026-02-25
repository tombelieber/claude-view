// crates/db/examples/bench_indexing.rs
// Performance benchmark for the unified indexing pipeline.
//
// Run with: cargo run --example bench_indexing -p claude-view-db --release
//
// Measures against real ~/.claude directory:
//   Initial scan (parse + upsert):       target <1s for ~807MB
//   Subsequent scan (no changes):        target <10ms

use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let claude_dir = dirs::home_dir()
        .expect("home dir")
        .join(".claude");

    if !claude_dir.exists() {
        eprintln!("Error: {} does not exist. This benchmark requires a real ~/.claude directory.",
            claude_dir.display());
        std::process::exit(1);
    }

    println!("=== claude-view indexing benchmark ===");
    println!("Claude dir: {}", claude_dir.display());
    println!();

    // Use in-memory DB for clean benchmarks (no disk I/O overhead)
    let db = claude_view_db::Database::new_in_memory().await?;

    // --- Build hints ---
    let start = Instant::now();
    let hints = claude_view_db::indexer_parallel::build_index_hints(&claude_dir);
    let hints_elapsed = start.elapsed();
    println!("Build hints: {} sessions in {:.1}ms",
        hints.len(), hints_elapsed.as_secs_f64() * 1000.0);

    // --- Initial scan benchmark ---
    let start = Instant::now();
    let (indexed, skipped) = claude_view_db::indexer_parallel::scan_and_index_all(
        &claude_dir, &db, &hints, None, None, |_session_id| {},
    )
    .await
    .map_err(|e| format!("Scan failed: {}", e))?;
    let scan_elapsed = start.elapsed();

    println!("Initial scan: {} indexed, {} skipped in {:.1}ms",
        indexed, skipped, scan_elapsed.as_secs_f64() * 1000.0);

    // --- Subsequent scan benchmark (should skip all) ---
    let start = Instant::now();
    let (indexed2, skipped2) = claude_view_db::indexer_parallel::scan_and_index_all(
        &claude_dir, &db, &hints, None, None, |_session_id| {},
    )
    .await
    .map_err(|e| format!("Rescan failed: {}", e))?;
    let rescan_elapsed = start.elapsed();

    println!("Subsequent scan: {} indexed, {} skipped in {:.1}ms",
        indexed2, skipped2, rescan_elapsed.as_secs_f64() * 1000.0);

    // --- Summary ---
    println!();
    println!("--- Summary ---");
    let scan_ms = scan_elapsed.as_secs_f64() * 1000.0;
    let rescan_ms = rescan_elapsed.as_secs_f64() * 1000.0;

    println!("Initial scan: {:>8.1}ms  (target: <1000ms)   {}",
        scan_ms, if scan_ms < 1000.0 { "PASS" } else { "MISS" });
    println!("Rescan:       {:>8.1}ms  (target: <10ms)    {}",
        rescan_ms, if rescan_ms < 10.0 { "PASS" } else { "MISS" });

    Ok(())
}
