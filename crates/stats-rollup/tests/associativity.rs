//! Monoid laws for `merge` proven via proptest (1000 cases each).
//!
//! `merge` is pointwise addition; these tests pin the contract that
//! `stats-rollup` is an associative, commutative monoid with identity.
//! The identity element is `PeriodStats::EMPTY`.
//!
//! ## Integer fields: exact equality
//!
//! Every `u64` counter (tokens, counts, durations) obeys *exact* monoid
//! laws — integer addition in the wrap-free domain is both associative
//! and commutative with `0` as identity. Generator bounds keep every
//! sum at least 3 orders of magnitude under `u64::MAX`, so no overflow
//! ever flips commutativity via wrap-around.
//!
//! ## f64 reedit_rate_sum: associativity up to epsilon
//!
//! IEEE-754 double-precision addition is **not** strictly associative —
//! `(a + b) + c` can differ from `a + (b + c)` by one ULP when the
//! intermediate sum's mantissa needs different rounding. Real example
//! found by proptest on the first run:
//!
//!   (0.3342935736112978 + 0.6262197051280775) + 0.9236280582446476
//!     = 1.884141336984023
//!   0.3342935736112978 + (0.6262197051280775 + 0.9236280582446476)
//!     = 1.8841413369840228   (1 ULP difference)
//!
//! So the true contract on f64 fields is **epsilon-associativity**: the
//! absolute difference between the two orderings is bounded by some
//! small `ε`. With `reedit_rate_sum ∈ [0.0, 1.0]` and three summands,
//! worst-case accumulated rounding is ~3 × 2⁻⁵² ≈ 6.7 × 10⁻¹⁶. We use
//! `ε = 1e-10` as a generous bound that stays well under the noise
//! floor of any statistic we'd actually display.
//!
//! Commutativity on f64 *is* exact — `a + b == b + a` for all IEEE-754
//! doubles — so `merge_is_commutative` still uses exact `PartialEq`.
//!
//! Identity on f64 is also exact — `x + 0.0 == x` for all finite `x`.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

use claude_view_stats_rollup::{merge, period::PeriodStats};
use proptest::prelude::*;

/// Maximum absolute difference allowed on `reedit_rate_sum` when checking
/// associativity. Chosen well above worst-case accumulated FP rounding
/// for our bounded input range (3 × 2⁻⁵² ≈ 6.7e-16) and well below any
/// user-visible precision threshold.
const F64_ASSOC_EPSILON: f64 = 1e-10;

fn any_period_stats() -> impl Strategy<Value = PeriodStats> {
    (
        (
            0u64..1_000_000,
            0u64..1_000_000,
            0u64..1_000_000,
            0u64..100_000,
            0u64..100_000,
            0u64..10_000,
        ),
        (
            0u64..10_000,
            0u64..10_000,
            0u64..1_000_000,
            0u64..1_000_000,
            0.0f64..1.0,
            0u64..1_000,
        ),
    )
        .prop_map(
            |((sc, tt, tc, pc, fc, la), (lr, cc, dsm, dc, rrs, rrc))| PeriodStats {
                session_count: sc,
                total_tokens: tt,
                total_cost_cents: tc,
                prompt_count: pc,
                file_count: fc,
                lines_added: la,
                lines_removed: lr,
                commit_count: cc,
                duration_sum_ms: dsm,
                duration_count: dc,
                reedit_rate_sum: rrs,
                reedit_rate_count: rrc,
            },
        )
}

/// Assert two `PeriodStats` agree on every integer field exactly and on
/// `reedit_rate_sum` up to `F64_ASSOC_EPSILON`.
fn assert_period_stats_approx_eq(
    lhs: &PeriodStats,
    rhs: &PeriodStats,
) -> Result<(), TestCaseError> {
    prop_assert_eq!(lhs.session_count, rhs.session_count);
    prop_assert_eq!(lhs.total_tokens, rhs.total_tokens);
    prop_assert_eq!(lhs.total_cost_cents, rhs.total_cost_cents);
    prop_assert_eq!(lhs.prompt_count, rhs.prompt_count);
    prop_assert_eq!(lhs.file_count, rhs.file_count);
    prop_assert_eq!(lhs.lines_added, rhs.lines_added);
    prop_assert_eq!(lhs.lines_removed, rhs.lines_removed);
    prop_assert_eq!(lhs.commit_count, rhs.commit_count);
    prop_assert_eq!(lhs.duration_sum_ms, rhs.duration_sum_ms);
    prop_assert_eq!(lhs.duration_count, rhs.duration_count);
    prop_assert_eq!(lhs.reedit_rate_count, rhs.reedit_rate_count);
    let diff = (lhs.reedit_rate_sum - rhs.reedit_rate_sum).abs();
    prop_assert!(
        diff < F64_ASSOC_EPSILON,
        "reedit_rate_sum diff {} exceeds epsilon {} (lhs={}, rhs={})",
        diff,
        F64_ASSOC_EPSILON,
        lhs.reedit_rate_sum,
        rhs.reedit_rate_sum,
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Associativity up to FP epsilon on `reedit_rate_sum`; exact on every
    /// other field. See module docstring for the epsilon justification.
    #[test]
    fn merge_is_associative(
        a in any_period_stats(),
        b in any_period_stats(),
        c in any_period_stats(),
    ) {
        let lhs = merge(&merge(&a, &b), &c);
        let rhs = merge(&a, &merge(&b, &c));
        assert_period_stats_approx_eq(&lhs, &rhs)?;
    }

    /// Commutativity is *exact* — IEEE-754 f64 addition is commutative
    /// bit-for-bit, and `u64` addition is trivially commutative.
    #[test]
    fn merge_is_commutative(
        a in any_period_stats(),
        b in any_period_stats(),
    ) {
        prop_assert_eq!(merge(&a, &b), merge(&b, &a));
    }

    /// `PeriodStats::EMPTY` is the *exact* identity — adding 0 preserves
    /// every field bit-for-bit.
    #[test]
    fn merge_with_empty_is_identity(a in any_period_stats()) {
        prop_assert_eq!(merge(&a, &PeriodStats::EMPTY), a.clone());
        prop_assert_eq!(merge(&PeriodStats::EMPTY, &a), a);
    }
}
