// Cost consistency test.
//
// Verifies that per-turn cost calculations produce consistent results
// when aggregated, and that the flat-rate aggregate approach produces
// a predictable relationship to tiered per-turn costs.

use claude_view_core::pricing::{calculate_cost, finalize_cost_breakdown, load_pricing, CostBreakdown, TokenUsage};

/// Helper: simulate a multi-turn session and compute cost both ways:
/// 1. Per-turn tiered (sum of individual turn costs)
/// 2. Aggregate flat (total tokens × base rate)
fn compute_both_costs(
    turns: &[(u64, u64, u64, u64)], // (input, output, cache_read, cache_create)
    model: &str,
) -> (f64, f64) {
    let pricing = load_pricing();

    // Method 1: Per-turn tiered cost (how session cost is computed)
    let per_turn_total: f64 = turns
        .iter()
        .map(|(input, output, cache_read, cache_create)| {
            let tokens = TokenUsage {
                input_tokens: *input,
                output_tokens: *output,
                cache_read_tokens: *cache_read,
                cache_creation_tokens: *cache_create,
                total_tokens: input + output + cache_read + cache_create,
                ..Default::default()
            };
            let cost = calculate_cost(&tokens, Some(model), &pricing);
            cost.total_usd
        })
        .sum();

    // Method 2: Aggregate flat cost (how dashboard stats are computed)
    let total_input: u64 = turns.iter().map(|t| t.0).sum();
    let total_output: u64 = turns.iter().map(|t| t.1).sum();
    let total_cache_read: u64 = turns.iter().map(|t| t.2).sum();
    let total_cache_create: u64 = turns.iter().map(|t| t.3).sum();

    let mp = pricing.get(model).expect("model should have pricing");
    let flat_cost = total_input as f64 * mp.input_cost_per_token
        + total_output as f64 * mp.output_cost_per_token
        + total_cache_read as f64 * mp.cache_read_cost_per_token
        + total_cache_create as f64 * mp.cache_creation_cost_per_token;

    (per_turn_total, flat_cost)
}

#[test]
fn flat_rate_model_costs_match_exactly() {
    // Opus 4.6 has flat pricing (no tiering) — both methods MUST produce identical results.
    let turns = vec![
        (50_000, 10_000, 100_000, 5_000),
        (80_000, 15_000, 120_000, 8_000),
        (30_000, 5_000, 60_000, 3_000),
    ];

    let (per_turn, flat) = compute_both_costs(&turns, "claude-opus-4-6");

    assert!(
        (per_turn - flat).abs() < 1e-9,
        "flat-rate model: per-turn ({:.6}) must exactly match aggregate ({:.6})",
        per_turn,
        flat
    );
}

#[test]
fn tiered_model_per_turn_cost_lte_aggregate_when_below_threshold() {
    // When every turn's token count is below 200k threshold,
    // tiered per-turn = flat aggregate (both use base rate).
    let turns = vec![
        (50_000, 10_000, 0, 0),
        (80_000, 15_000, 0, 0),
        (30_000, 5_000, 0, 0),
    ];

    let (per_turn, flat) = compute_both_costs(&turns, "claude-sonnet-4-5-20250929");

    assert!(
        (per_turn - flat).abs() < 1e-9,
        "sub-200k turns: per-turn ({:.6}) must match flat ({:.6})",
        per_turn,
        flat
    );
}

#[test]
fn tiered_model_aggregate_always_gte_per_turn_when_above_threshold() {
    // When individual turns exceed 200k tokens, per-turn tiering costs MORE
    // because each turn hits the higher rate independently.
    // Flat aggregate uses base rate for everything — costs LESS.
    let turns = vec![
        (300_000, 50_000, 0, 0), // exceeds 200k threshold
        (250_000, 40_000, 0, 0), // exceeds 200k threshold
    ];

    let (per_turn, flat) = compute_both_costs(&turns, "claude-sonnet-4-5-20250929");

    assert!(
        per_turn >= flat,
        "tiered per-turn ({:.6}) must be >= flat aggregate ({:.6}) when turns exceed 200k",
        per_turn,
        flat
    );
}

#[test]
fn zero_token_session_costs_zero_both_ways() {
    let turns = vec![(0, 0, 0, 0), (0, 0, 0, 0)];

    let (per_turn, flat) = compute_both_costs(&turns, "claude-opus-4-6");

    assert_eq!(per_turn, 0.0, "zero tokens must produce zero per-turn cost");
    assert_eq!(flat, 0.0, "zero tokens must produce zero flat cost");
}

#[test]
fn unknown_model_produces_zero_cost() {
    let pricing = load_pricing();
    let tokens = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 500_000,
        total_tokens: 1_500_000,
        ..Default::default()
    };

    let cost = calculate_cost(&tokens, Some("gpt-4o-unknown"), &pricing);
    assert_eq!(cost.total_usd, 0.0);
    assert!(cost.has_unpriced_usage);
    assert_eq!(cost.unpriced_input_tokens, 1_000_000);
    assert_eq!(cost.unpriced_output_tokens, 500_000);
    assert_eq!(cost.total_cost_source, "computed_priced_tokens_partial");
}

#[test]
fn session_with_mixed_priced_unpriced_turns() {
    let pricing = load_pricing();

    // Turn 1: priced model
    let t1_tokens = TokenUsage {
        input_tokens: 100_000,
        output_tokens: 20_000,
        total_tokens: 120_000,
        ..Default::default()
    };
    let t1_cost = calculate_cost(&t1_tokens, Some("claude-opus-4-6"), &pricing);

    // Turn 2: unpriced model
    let t2_tokens = TokenUsage {
        input_tokens: 50_000,
        output_tokens: 10_000,
        total_tokens: 60_000,
        ..Default::default()
    };
    let t2_cost = calculate_cost(&t2_tokens, Some("unknown-model-xyz"), &pricing);

    // Verify: priced turn has real cost, unpriced has zero
    assert!(t1_cost.total_usd > 0.0, "priced turn must have non-zero cost");
    assert_eq!(t2_cost.total_usd, 0.0, "unpriced turn must have zero cost");

    // Session total = sum of both
    let session_total = t1_cost.total_usd + t2_cost.total_usd;
    assert_eq!(
        session_total, t1_cost.total_usd,
        "session total equals priced turn only (unpriced contributes $0)"
    );

    // Finalize should mark partial
    let mut session_cost = CostBreakdown {
        total_usd: session_total,
        input_cost_usd: t1_cost.input_cost_usd,
        output_cost_usd: t1_cost.output_cost_usd,
        unpriced_input_tokens: t2_cost.unpriced_input_tokens,
        unpriced_output_tokens: t2_cost.unpriced_output_tokens,
        ..Default::default()
    };
    let total_tokens = TokenUsage {
        input_tokens: 150_000,
        output_tokens: 30_000,
        total_tokens: 180_000,
        ..Default::default()
    };
    finalize_cost_breakdown(&mut session_cost, &total_tokens);

    assert!(session_cost.has_unpriced_usage);
    assert_eq!(session_cost.total_cost_source, "computed_priced_tokens_partial");
    // Coverage = priced tokens / total tokens = 120k / 180k ≈ 0.667
    let expected_coverage = 120_000.0 / 180_000.0;
    assert!(
        (session_cost.priced_token_coverage - expected_coverage).abs() < 1e-6,
        "coverage should be {:.4}, got {:.4}",
        expected_coverage,
        session_cost.priced_token_coverage
    );
}
