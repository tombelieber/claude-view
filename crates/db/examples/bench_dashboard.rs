// crates/db/examples/bench_dashboard.rs
// Performance benchmark for dashboard query optimizations.
//
// Run with: cargo run --example bench_dashboard -p vibe-recall-db --release
//
// Seeds an in-memory DB with ~500 sessions (with turns, commits, invocations)
// then measures each dashboard query function 10× and reports avg/min/max.
//
// Targets:
//   get_trends_for_periods:        <5ms  for 500 sessions
//   get_dashboard_stats:           <10ms for 500 sessions
//   get_all_time_metrics:          <5ms  for 500 sessions
//   get_dashboard_stats_with_range: <10ms for 500 sessions

use std::time::Instant;
use vibe_recall_db::Database;

const NUM_SESSIONS: usize = 500;
const NUM_PROJECTS: usize = 10;
const TURNS_PER_SESSION: usize = 5;
const COMMITS_PER_SESSION: usize = 2;
const BENCH_ITERATIONS: usize = 10;

async fn seed_database(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let pool = db.pool();
    let now = chrono::Utc::now().timestamp();
    let one_week = 7 * 86400;

    println!("Seeding {} sessions with turns, commits, invocations...", NUM_SESSIONS);
    let start = Instant::now();

    // Batch all inserts in a single transaction
    let mut tx = pool.begin().await?;

    // Insert model (referenced by turns FK)
    sqlx::query(
        "INSERT OR IGNORE INTO models (id, provider, family, first_seen, last_seen) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind("claude-sonnet-4-5-20250929")
    .bind("anthropic")
    .bind("claude-4.5")
    .bind(now - 30 * 86400)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Insert invocables (skills, commands, mcp_tools, agents)
    let kinds = ["skill", "command", "mcp_tool", "agent"];
    for (i, kind) in kinds.iter().enumerate() {
        for j in 0..5 {
            let id = format!("{}-{}", kind, j);
            let name = format!("{}_{}", kind, j);
            sqlx::query(
                "INSERT OR IGNORE INTO invocables (id, name, kind, description) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&id)
            .bind(&name)
            .bind(*kind)
            .bind(format!("A {} invocable", kind))
            .execute(&mut *tx)
            .await?;
            let _ = i; // suppress unused warning
        }
    }

    // Insert sessions spread across current week and previous week
    for i in 0..NUM_SESSIONS {
        let project_idx = i % NUM_PROJECTS;
        let project_id = format!("proj-{}", project_idx);
        let session_id = format!("sess-{:04}", i);
        let branch = if i % 3 == 0 { "main" } else { "develop" };

        // Spread sessions across two weeks: half in current, half in previous
        let ts = if i < NUM_SESSIONS / 2 {
            now - (i as i64 * 600) // current week: spread across recent hours
        } else {
            now - one_week - ((i - NUM_SESSIONS / 2) as i64 * 600) // previous week
        };

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, project_id, project_display_name, project_path, file_path,
                preview, last_message, last_message_at,
                turn_count, message_count, size_bytes,
                tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
                is_sidechain, user_prompt_count, files_edited_count, reedited_files_count,
                duration_seconds, commit_count, git_branch,
                files_touched, skills_used, files_read, files_edited,
                api_call_count, tool_call_count, files_read_count,
                indexed_at,
                total_input_tokens, total_output_tokens
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8,
                ?9, ?10, ?11,
                ?12, ?13, ?14, ?15,
                0, ?16, ?17, ?18,
                ?19, ?20, ?21,
                '[]', '[]', '[]', '[]',
                ?22, ?23, ?24,
                ?25,
                ?26, ?27
            )
            "#,
        )
        .bind(&session_id)
        .bind(&project_id)
        .bind(format!("Project {}", project_idx))
        .bind(format!("/home/user/proj-{}", project_idx))
        .bind(format!("/tmp/{}.jsonl", session_id))
        .bind(format!("Session {} preview", i))
        .bind(format!("Last message {}", i))
        .bind(ts)
        .bind(TURNS_PER_SESSION as i32)
        .bind(TURNS_PER_SESSION as i32 * 2)
        .bind(2048_i64)
        .bind((i % 10) as i32)    // edit
        .bind((i % 20) as i32)    // read
        .bind((i % 5) as i32)     // bash
        .bind((i % 3) as i32)     // write
        .bind((i % 15 + 1) as i32) // user_prompt_count
        .bind((i % 8) as i32)     // files_edited_count
        .bind((i % 3) as i32)     // reedited_files_count
        .bind((i * 120) as i32)   // duration_seconds
        .bind(COMMITS_PER_SESSION as i32)
        .bind(branch)
        .bind((i % 10 + 5) as i32) // api_call_count
        .bind((i % 20 + 10) as i32) // tool_call_count
        .bind((i % 6) as i32)     // files_read_count
        .bind(ts)                  // indexed_at
        .bind((i * 1000 + 5000) as i64) // total_input_tokens
        .bind((i * 500 + 2000) as i64)  // total_output_tokens
        .execute(&mut *tx)
        .await?;

        // Insert turns for this session
        for t in 0..TURNS_PER_SESSION {
            let turn_uuid = format!("{}-turn-{}", session_id, t);
            sqlx::query(
                r#"
                INSERT INTO turns (session_id, uuid, seq, model_id, input_tokens, output_tokens, timestamp)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&session_id)
            .bind(&turn_uuid)
            .bind(t as i32)
            .bind("claude-sonnet-4-5-20250929")
            .bind((i * 200 + t * 100 + 1000) as i64)
            .bind((i * 100 + t * 50 + 500) as i64)
            .bind(ts + t as i64 * 60)
            .execute(&mut *tx)
            .await?;
        }

        // Insert commits for this session
        for c in 0..COMMITS_PER_SESSION {
            let commit_hash = format!("abc{:04}{:02}", i, c);
            // Insert into commits table first
            sqlx::query(
                "INSERT OR IGNORE INTO commits (hash, repo_path, message, author, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&commit_hash)
            .bind(format!("/home/user/proj-{}", project_idx))
            .bind(format!("Commit {} for session {}", c, i))
            .bind("bench <bench@test.com>")
            .bind(ts + c as i64 * 300)
            .execute(&mut *tx)
            .await?;

            // Link commit to session
            sqlx::query(
                "INSERT OR IGNORE INTO session_commits (session_id, commit_hash, tier, evidence) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&session_id)
            .bind(&commit_hash)
            .bind(1)
            .bind("{}")
            .execute(&mut *tx)
            .await?;
        }

        // Insert invocations (2 per session)
        for inv in 0..2 {
            let kind_idx = (i + inv) % 4;
            let item_idx = (i + inv) % 5;
            let invocable_id = format!("{}-{}", kinds[kind_idx], item_idx);
            let source_file = format!("/tmp/{}.jsonl", session_id);
            let byte_offset = (inv * 10000 + i * 100) as i64;
            sqlx::query(
                "INSERT OR IGNORE INTO invocations (source_file, byte_offset, invocable_id, session_id, project, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(&source_file)
            .bind(byte_offset)
            .bind(&invocable_id)
            .bind(&session_id)
            .bind(&project_id)
            .bind(ts + inv as i64 * 30)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    let seed_elapsed = start.elapsed();
    println!(
        "Seeded {} sessions ({} turns, {} commits, {} invocations) in {:.1}ms",
        NUM_SESSIONS,
        NUM_SESSIONS * TURNS_PER_SESSION,
        NUM_SESSIONS * COMMITS_PER_SESSION,
        NUM_SESSIONS * 2,
        seed_elapsed.as_secs_f64() * 1000.0
    );

    Ok(())
}

struct BenchResult {
    name: &'static str,
    avg_ms: f64,
    min_ms: f64,
    max_ms: f64,
    target_ms: f64,
}

impl BenchResult {
    fn passed(&self) -> bool {
        self.avg_ms < self.target_ms
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vibe-recall dashboard query benchmark ===");
    println!();

    let db = Database::new_in_memory().await?;
    seed_database(&db).await?;
    println!();

    let now = chrono::Utc::now().timestamp();
    let one_week = 7 * 86400;
    let curr_start = now - one_week;
    let curr_end = now;

    let mut results = Vec::new();

    // --- Benchmark get_trends_for_periods ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db
                .get_trends_with_range(curr_start, curr_end, None, None)
                .await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "get_trends_for_periods",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 5.0,
        });
    }

    // --- Benchmark get_dashboard_stats ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db.get_dashboard_stats(None, None).await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "get_dashboard_stats",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 10.0,
        });
    }

    // --- Benchmark get_all_time_metrics ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db.get_all_time_metrics(None, None).await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "get_all_time_metrics",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 5.0,
        });
    }

    // --- Benchmark get_dashboard_stats_with_range ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db
                .get_dashboard_stats_with_range(Some(curr_start), Some(curr_end), None, None)
                .await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "get_dashboard_stats_with_range",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 10.0,
        });
    }

    // --- Benchmark get_storage_counts ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db.get_storage_counts().await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "get_storage_counts",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 5.0,
        });
    }

    // --- Benchmark with project filter ---
    {
        let mut times = Vec::with_capacity(BENCH_ITERATIONS);
        for _ in 0..BENCH_ITERATIONS {
            let start = Instant::now();
            let _ = db
                .get_trends_with_range(curr_start, curr_end, Some("proj-0"), None)
                .await?;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        results.push(BenchResult {
            name: "trends (project filter)",
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            target_ms: 5.0,
        });
    }

    // --- Summary ---
    println!("--- Results ({} iterations each) ---", BENCH_ITERATIONS);
    println!(
        "{:<35} {:>8} {:>8} {:>8} {:>8}  {}",
        "Function", "Avg(ms)", "Min(ms)", "Max(ms)", "Target", "Status"
    );
    println!("{}", "-".repeat(88));

    let mut all_pass = true;
    for r in &results {
        let status = if r.passed() { "PASS" } else { "MISS" };
        if !r.passed() {
            all_pass = false;
        }
        println!(
            "{:<35} {:>8.2} {:>8.2} {:>8.2} {:>7.0}ms  {}",
            r.name, r.avg_ms, r.min_ms, r.max_ms, r.target_ms, status
        );
    }

    println!();
    if all_pass {
        println!("All benchmarks PASSED!");
    } else {
        println!("Some benchmarks MISSED targets.");
    }

    Ok(())
}
