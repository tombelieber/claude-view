//! Evidence audit: validates parser/indexer type coverage against real JSONL data.
//!
//! Scans Claude Code JSONL files and compares structural inventories
//! against `evidence-baseline.json`. Catches drift before release.

mod extraction;
pub(crate) mod scanning;
mod types;
mod validation;

#[cfg(test)]
mod tests;

// ─── Re-exports ─────────────────────────────────────────────────
// Preserves the original flat `evidence_audit::*` public API.

// Types
pub use types::{
    AggregatedSignals, AuditResult, Baseline, CheckResult, ContentBlockTypes, FieldExtractionPaths,
    LineSignals, ProgressDataTypes, SystemSubtypes, ThinkingBlockKeys, TopLevelTypes,
};

// Extraction
pub use extraction::extract_line_signals;

// Scanning
pub use scanning::{
    discover_jsonl_files, load_baseline, scan_directory_parallel,
    scan_directory_parallel_with_pipeline, scan_file_with_pipeline,
};

// Validation
pub use validation::{check_set_diff, run_audit_checks, run_phase3_checks};
