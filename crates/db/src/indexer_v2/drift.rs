//! Phase 2 indexer_v2 drift comparator — stub.
//!
//! The 100-session fixture parity test + the `shadow_diff_total{field}`
//! metric (Phase 1-7 design §3.3 exit criteria) ship in PR 2.2.2 once
//! the orchestrator is live. Until then this module is reserved so the
//! file-layout matches the design and follow-up commits don't have to
//! introduce a new file.

#![allow(dead_code)]

/// Per-field drift counter exported as `shadow_diff_total{field}`.
/// Kept private until the comparator lands so no consumer can read it.
struct DriftCounters;
