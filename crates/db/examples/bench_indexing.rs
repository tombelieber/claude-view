// crates/db/examples/bench_indexing.rs
// Performance benchmark for the two-pass indexing pipeline.
//
// Run with: cargo run --example bench_indexing -p vibe-recall-db --release
//
// Measures against real ~/.claude directory:
//   Pass 1 (read sessions-index.json): target <10ms for ~10 projects
//   Pass 2 (deep JSONL parsing):       target <1s for ~807MB
//   Subsequent launch (no changes):    target <10ms

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

    println!("=== vibe-recall indexing benchmark ===");
    println!("Claude dir: {}", claude_dir.display());
    println!();

    // Use in-memory DB for clean benchmarks (no disk I/O overhead)
    let db = vibe_recall_db::Database::new_in_memory().await?;

    // --- Pass 1 benchmark ---
    let start = Instant::now();
    let (projects, sessions) = vibe_recall_db::indexer_parallel::pass_1_read_indexes(&claude_dir, &db)
        .await
        .map_err(|e| format!("Pass 1 failed: {}", e))?;
    let pass1_elapsed = start.elapsed();

    println!("Pass 1: {} projects, {} sessions in {:.1}ms",
        projects, sessions, pass1_elapsed.as_secs_f64() * 1000.0);

    // --- Pass 2 benchmark ---
    let start = Instant::now();
    let indexed = vibe_recall_db::indexer_parallel::pass_2_deep_index(&db, None, |indexed, total| {
        if indexed % 50 == 0 || indexed == total {
            eprint!("\r  Pass 2: {}/{}", indexed, total);
        }
    })
    .await
    .map_err(|e| format!("Pass 2 failed: {}", e))?;
    let pass2_elapsed = start.elapsed();
    eprintln!();

    println!("Pass 2: {} sessions deep-indexed in {:.1}ms",
        indexed, pass2_elapsed.as_secs_f64() * 1000.0);

    // --- Subsequent launch benchmark (Pass 2 should skip all) ---
    let start = Instant::now();
    let indexed2 = vibe_recall_db::indexer_parallel::pass_2_deep_index(&db, None, |_, _| {})
        .await
        .map_err(|e| format!("Pass 2 rerun failed: {}", e))?;
    let rerun_elapsed = start.elapsed();

    println!("Subsequent Pass 2: {} sessions (should be 0) in {:.1}ms",
        indexed2, rerun_elapsed.as_secs_f64() * 1000.0);

    // --- Summary ---
    println!();
    println!("--- Summary ---");
    let pass1_ms = pass1_elapsed.as_secs_f64() * 1000.0;
    let pass2_ms = pass2_elapsed.as_secs_f64() * 1000.0;
    let rerun_ms = rerun_elapsed.as_secs_f64() * 1000.0;

    println!("Pass 1:      {:>8.1}ms  (target: <10ms)    {}",
        pass1_ms, if pass1_ms < 10.0 { "PASS" } else { "MISS" });
    println!("Pass 2:      {:>8.1}ms  (target: <1000ms)   {}",
        pass2_ms, if pass2_ms < 1000.0 { "PASS" } else { "MISS" });
    println!("Rerun:       {:>8.1}ms  (target: <10ms)    {}",
        rerun_ms, if rerun_ms < 10.0 { "PASS" } else { "MISS" });

    Ok(())
}
