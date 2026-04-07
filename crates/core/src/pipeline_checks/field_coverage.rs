// ── Phase 3: field coverage checks ──

use std::collections::HashSet;

use crate::evidence_audit::FieldExtractionPaths;
use crate::field_inventory::FieldInventory;

use super::types::{CheckAccum, PipelineCheckResult};

/// Check 19: Field coverage — every path seen in real data is either
/// extracted by the parser or intentionally ignored by the baseline.
pub fn check_field_coverage(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let extracted: HashSet<&str> = baseline.extracted.iter().map(|s| s.as_str()).collect();
    let ignored: HashSet<&str> = baseline
        .intentionally_ignored
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut accum = CheckAccum::default();

    for (path, count) in &inventory.paths {
        let is_known = extracted.contains(path.as_str())
            || ignored.contains(path.as_str())
            || baseline.intentionally_ignored.iter().any(|ign| {
                path.starts_with(&format!("{}.", ign)) || path.starts_with(&format!("{}[", ign))
            });
        if is_known {
            accum.record_pass();
        } else {
            accum.record_violation(
                "corpus-wide",
                0,
                &format!("unhandled field path: \"{}\" ({} occurrences)", path, count),
            );
        }
    }

    accum.into_result("Field coverage")
}

/// Check 20: Phantom field detection — every path the baseline claims is
/// extracted must actually appear in the scanned data.
pub fn check_phantom_fields(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let mut accum = CheckAccum::default();

    for path in &baseline.extracted {
        match inventory.paths.get(path.as_str()) {
            Some(count) if *count > 0 => {
                accum.record_pass();
            }
            _ => {
                accum.record_violation(
                    "corpus-wide",
                    0,
                    &format!(
                        "PHANTOM: parser claims to extract \"{}\" but 0 occurrences across all scanned data ({} distinct paths seen)",
                        path,
                        inventory.paths.len()
                    ),
                );
            }
        }
    }

    accum.into_result("Phantom field detection")
}
