//! Integration test: runs Phase 2 pipeline invariant checks against the all_types fixture.

use claude_view_core::evidence_audit::{scan_file_with_pipeline, FieldExtractionPaths};
use claude_view_core::pipeline_checks::{check_field_coverage, check_phantom_fields};
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

#[test]
fn test_field_coverage_detects_unknown_path() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
    let (agg, _) = scan_file_with_pipeline(&fixture_path);
    let baseline_paths = FieldExtractionPaths {
        extracted: vec!["type".into(), "timestamp".into()],
        intentionally_ignored: vec![],
    };
    let result = check_field_coverage(&agg.field_inventory, &baseline_paths);
    assert!(!result.passed, "should detect unaccounted paths in data");
}

#[test]
fn test_phantom_detection() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
    let (agg, _) = scan_file_with_pipeline(&fixture_path);
    let baseline_paths = FieldExtractionPaths {
        extracted: vec![
            "type".into(),
            "message.content[].input.team_name".into(), // phantom — not in fixture data
        ],
        intentionally_ignored: vec![],
    };
    let result = check_phantom_fields(&agg.field_inventory, &baseline_paths);
    assert!(!result.passed, "should detect phantom path");
    assert!(
        !result.sample_violations.is_empty(),
        "expected at least one violation"
    );
    assert!(result.sample_violations[0].detail.contains("team_name"));
}
