//! Integration test: runs Phase 2 pipeline invariant checks against the all_types fixture.

use claude_view_core::evidence_audit::scan_file_with_pipeline;
use std::path::Path;

#[test]
fn pipeline_invariants_pass_on_fixture() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
    let (_agg, pipeline) = scan_file_with_pipeline(&fixture_path);

    let results = pipeline.into_results();
    let mut failures = Vec::new();
    for r in &results {
        if r.skipped {
            continue; // Deferred checks are not failures
        }
        if !r.passed {
            failures.push(format!(
                "{}: {} violations (e.g. {:?})",
                r.name,
                r.violation_count,
                r.sample_violations.first().map(|v| &v.detail)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Pipeline invariant failures:\n{}",
        failures.join("\n")
    );
}
