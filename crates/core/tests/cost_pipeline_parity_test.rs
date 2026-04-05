//! Cross-pipeline cost parity test.
//!
//! Regression test for the 1hr-caching cost drift bug (fixed 2026-04-05).
//! Pre-fix, `TurnBoundaryAccumulator::calculate_cost()` hardcoded 5m/1h cache
//! tokens to 0, causing a ~37.5% undercharge whenever Claude Code used 1-hour
//! caching (its default). This made the per-turn boundary cost visibly disagree
//! with the session-level cost shown in the Cost tab.
//!
//! This test runs the same 1hr-caching JSONL fixture through BOTH accumulator
//! pipelines and asserts their costs agree within 0.1%.

use claude_view_core::accumulator::SessionAccumulator;
use claude_view_core::block_accumulator::BlockAccumulator;
use claude_view_core::block_types::ConversationBlock;
use claude_view_core::live_parser::{parse_tail, TailFinders};
use claude_view_core::pricing::load_pricing;
use std::path::Path;

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/block_accumulator/turn_with_1hr_cache.jsonl")
}

/// Session-level cost from the main pipeline (SessionAccumulator → RichSessionData.cost)
fn compute_session_cost() -> f64 {
    let path = fixture_path();
    let finders = TailFinders::new();
    let (lines, _) = parse_tail(&path, 0, &finders).expect("fixture readable");

    let pricing = load_pricing();
    let mut acc = SessionAccumulator::new();
    for line in &lines {
        acc.process_line(line, 0, &pricing);
    }
    let data = acc.finish(&pricing);
    data.cost.total_usd
}

/// Sum of TurnBoundaryBlock.total_cost_usd from the block pipeline.
fn compute_turn_boundary_cost_sum() -> f64 {
    let path = fixture_path();
    let content = std::fs::read_to_string(&path).expect("fixture readable");

    let mut acc = BlockAccumulator::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSONL");
        acc.process_line(&parsed);
    }
    let blocks = acc.finalize();

    blocks
        .iter()
        .filter_map(|b| match b {
            ConversationBlock::TurnBoundary(tb) => Some(tb.total_cost_usd),
            _ => None,
        })
        .sum()
}

#[test]
fn turn_boundary_cost_matches_session_cost_with_1hr_caching() {
    let session_cost = compute_session_cost();
    let turn_boundary_cost_sum = compute_turn_boundary_cost_sum();

    // Both pipelines must produce non-zero cost with the 1hr fixture.
    assert!(
        session_cost > 0.0,
        "session cost must be > 0 to be meaningful, got ${}",
        session_cost
    );
    assert!(
        turn_boundary_cost_sum > 0.0,
        "turn boundary cost sum must be > 0, got ${}",
        turn_boundary_cost_sum
    );

    let drift = (session_cost - turn_boundary_cost_sum).abs() / session_cost;
    assert!(
        drift < 0.001,
        "session cost ${:.6} disagrees with Σ turn boundary costs ${:.6} (drift {:.3}%)",
        session_cost,
        turn_boundary_cost_sum,
        drift * 100.0
    );
}

#[test]
fn fixture_uses_1hr_rate_not_5m_rate() {
    // Lock in: the fixture's 1hr caching at opus-4-6 rates MUST produce a cost
    // materially larger than the 5m-rate equivalent. This is the signature of
    // the bug being fixed.
    //
    // Fixture: 100_000 cache_creation tokens on opus-4-6, all 1hr TTL.
    // Plus small input/output/cache_read contributions from 2 assistant lines.
    //
    // Opus 4.6 rates (per MTok):
    //   - input: $5.00
    //   - output: $25.00
    //   - cache_read: $0.50
    //   - cache_creation (5m): $6.25
    //   - cache_creation (1hr): $10.00
    //
    // Fixture totals (summed across 2 assistant lines):
    //   - input: 1000 + 100 = 1100 tokens  → $0.0055
    //   - output: 50 + 20 = 70 tokens      → $0.00175
    //   - cache_read: 0 + 100000 = 100000  → $0.05
    //   - cache_creation: 100000 (1hr)     → $1.00 at 1hr, $0.625 at 5m
    //
    // Expected total at 1hr rate: ≈ $1.057
    // If the bug regressed, the 5m rate would give ≈ $0.682.
    let session_cost = compute_session_cost();

    // Must be well above the 5m-rate-only result
    assert!(
        session_cost > 0.9,
        "cost ${:.4} is suspiciously low — 1hr rate must be applied (5m rate would give ~$0.68)",
        session_cost
    );
    // Sanity upper bound
    assert!(
        session_cost < 1.2,
        "cost ${:.4} is unexpectedly high — check fixture or pricing table",
        session_cost
    );
}
