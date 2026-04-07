//! Audit comparison: set-diff checks and multi-phase audit runners.

use super::types::{AggregatedSignals, AuditResult, Baseline, CheckResult, Sorted};
use std::collections::{BTreeSet, HashSet};

/// Compare actual vs expected sets, reporting new (drift) and absent items.
///
/// `new_items` = in actual but not expected (potential drift).
/// `absent_items` = in expected but not actual (may be fine — not all types appear in every corpus).
/// Only `new_items` being non-empty means `passed = false`.
pub fn check_set_diff(
    name: &str,
    actual: &HashSet<String>,
    expected: &HashSet<String>,
) -> CheckResult {
    let new_items: Vec<String> = actual.difference(expected).cloned().sorted();
    let absent_items: Vec<String> = expected.difference(actual).cloned().sorted();
    let passed = new_items.is_empty();

    CheckResult {
        name: name.to_string(),
        passed,
        new_items,
        absent_items,
    }
}

/// Run all 6 audit checks comparing scanned signals against the baseline.
pub fn run_audit_checks(signals: &AggregatedSignals, baseline: &Baseline) -> AuditResult {
    let mut checks = Vec::new();

    // 1. Top-level types
    let expected_top = baseline.top_level_types.all_known();
    checks.push(check_set_diff(
        "Top-level types",
        &signals.top_level_types,
        &expected_top,
    ));

    // 2. Assistant content block types
    let expected_content: HashSet<String> = baseline
        .content_block_types
        .assistant
        .iter()
        .cloned()
        .collect();
    checks.push(check_set_diff(
        "Assistant content block types",
        &signals.assistant_content_block_types,
        &expected_content,
    ));

    // 3. System subtypes
    let expected_sys: HashSet<String> = baseline.system_subtypes.known.iter().cloned().collect();
    checks.push(check_set_diff(
        "System subtypes",
        &signals.system_subtypes,
        &expected_sys,
    ));

    // 4. Progress data.type values
    let expected_prog: HashSet<String> =
        baseline.progress_data_types.known.iter().cloned().collect();
    checks.push(check_set_diff(
        "Progress data types",
        &signals.progress_data_types,
        &expected_prog,
    ));

    // 5. Thinking block keys — check each unique key-shape variant
    let expected_thinking: BTreeSet<String> = baseline
        .thinking_block_keys
        .required
        .iter()
        .cloned()
        .collect();

    // Empty corpus = pass with note (not drift)
    let thinking_check = if signals.thinking_key_sets.is_empty() {
        let absent: Vec<String> = expected_thinking.iter().cloned().collect();
        CheckResult {
            name: "Thinking block keys".to_string(),
            passed: true,
            new_items: vec![],
            absent_items: absent,
        }
    } else {
        // Every observed key-shape must exactly match expected
        let all_match = signals
            .thinking_key_sets
            .iter()
            .all(|ks| *ks == expected_thinking);
        if all_match {
            CheckResult {
                name: "Thinking block keys".to_string(),
                passed: true,
                new_items: vec![],
                absent_items: vec![],
            }
        } else {
            // Collect deviating variants
            let mut new_items = Vec::new();
            for ks in &signals.thinking_key_sets {
                if *ks != expected_thinking {
                    let extra: Vec<String> = ks.difference(&expected_thinking).cloned().collect();
                    let missing: Vec<String> = expected_thinking.difference(ks).cloned().collect();
                    new_items.push(format!(
                        "variant {{{}}} — extra: {:?}, missing: {:?}",
                        ks.iter().cloned().collect::<Vec<_>>().join(", "),
                        extra,
                        missing,
                    ));
                }
            }
            CheckResult {
                name: "Thinking block keys".to_string(),
                passed: false,
                new_items,
                absent_items: vec![],
            }
        }
    };
    checks.push(thinking_check);

    // 6. Agent progress nesting
    // Fail if we see direct agent_progress but never the nested content path
    let nesting_passed = !(signals.nesting_direct_count > 0 && signals.nesting_nested_count == 0);
    checks.push(CheckResult {
        name: "Agent progress nesting".to_string(),
        passed: nesting_passed,
        new_items: if nesting_passed {
            vec![]
        } else {
            vec![format!(
                "direct={} but nested=0 — double-nesting path not validated",
                signals.nesting_direct_count
            )]
        },
        absent_items: vec![],
    });

    let passed = checks.iter().all(|c| c.passed);

    AuditResult {
        passed,
        checks,
        nesting_direct_count: signals.nesting_direct_count,
        nesting_nested_count: signals.nesting_nested_count,
        files_scanned: signals.files_scanned,
        lines_scanned: signals.lines_scanned,
        errors: signals.errors,
    }
}

/// Run Phase 3 field-coverage checks against the inventory and baseline.
/// Returns skipped results if baseline lacks `field_extraction_paths`.
pub fn run_phase3_checks(
    inventory: &crate::field_inventory::FieldInventory,
    baseline: &Baseline,
) -> Vec<crate::pipeline_checks::PipelineCheckResult> {
    match &baseline.field_extraction_paths {
        Some(paths) => vec![
            crate::pipeline_checks::check_field_coverage(inventory, paths),
            crate::pipeline_checks::check_phantom_fields(inventory, paths),
        ],
        None => vec![
            crate::pipeline_checks::PipelineCheckResult::new_skipped(
                "Field coverage",
                "Baseline lacks field_extraction_paths section",
            ),
            crate::pipeline_checks::PipelineCheckResult::new_skipped(
                "Phantom field detection",
                "Baseline lacks field_extraction_paths section",
            ),
        ],
    }
}
