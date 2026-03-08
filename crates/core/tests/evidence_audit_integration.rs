//! Integration test: runs the evidence audit against real ~/.claude data.
//! Skipped by default (requires real JSONL files on disk).
//!
//! Run with: cargo test -p claude-view-core --test evidence_audit_integration -- --ignored

use std::path::Path;

#[test]
#[ignore]
fn evidence_audit_against_real_data() {
    let home = dirs::home_dir().expect("cannot determine home");
    let data_dir = home.join(".claude/projects");
    if !data_dir.is_dir() {
        eprintln!("Skipping: {} does not exist", data_dir.display());
        return;
    }

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let baseline_path = repo_root.join("scripts/integrity/evidence-baseline.json");

    let baseline = claude_view_core::evidence_audit::load_baseline(&baseline_path)
        .expect("baseline should load");
    let signals = claude_view_core::evidence_audit::scan_directory_parallel(&data_dir);

    eprintln!(
        "Scanned {} files, {} lines",
        signals.files_scanned, signals.lines_scanned
    );
    eprintln!("Types found: {:?}", signals.top_level_types);

    let result = claude_view_core::evidence_audit::run_audit_checks(&signals, &baseline);

    for check in &result.checks {
        if check.passed {
            eprintln!("  OK: {}", check.name);
        } else {
            eprintln!("  DRIFT: {}", check.name);
            for item in &check.new_items {
                eprintln!("    + {item}");
            }
        }
    }

    assert!(
        result.passed,
        "Evidence audit detected drift — update baseline or parser"
    );
}
