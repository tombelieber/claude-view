//! Phase 2 pipeline invariant checks — validates parser + accumulator
//! correctness against real JSONL data.

mod field_coverage;
mod per_line_checks;
mod per_session_checks;
mod types;

#[cfg(test)]
mod tests;

pub use field_coverage::{check_field_coverage, check_phantom_fields};
pub use per_line_checks::{
    check_cache_token_split, check_content_preview, check_cost_requires_model,
    check_file_path_tool_presence, check_model_extraction, check_role_classification,
    check_timestamp_extraction, check_token_extraction, check_tool_name_extraction,
    run_per_line_checks,
};
pub use per_session_checks::run_per_session_checks;
pub use types::{CheckAccum, LineOffsets, PipelineCheckResult, PipelineSignals, Violation};
