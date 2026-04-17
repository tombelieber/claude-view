//! CQRS Phase 1 rollup crate — Stage C aggregation + `PeriodStats` + associative-monoid
//! merge primitives.
//!
//! PR 1.1 scaffold. PR 1.3 adds `rollup`, `merge`, `period`, `bucket`, `version` modules
//! with proptest-verified associativity/commutativity. See design doc §2.2.

/// Rollup protocol version. Bump when aggregate semantics change (bucket boundaries,
/// new rollup fields). Forces rollup-only recompute without re-running Stages A/B.
pub const ROLLUP_VERSION: u32 = 0;
