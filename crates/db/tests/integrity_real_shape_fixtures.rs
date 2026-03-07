use std::fs;
use std::path::PathBuf;

const FIXTURES: &[&str] = &[
    "tests/golden_fixtures/integrity_nonzero_tool_index.jsonl",
    "tests/golden_fixtures/integrity_progress_nested_content.jsonl",
    "tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl",
    "tests/golden_fixtures/integrity_type_substring_noise.jsonl",
];

fn crate_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn integrity_fixture_lines_are_valid_json_with_type() {
    for fixture in FIXTURES {
        let fixture_path = crate_path(fixture);
        let content = fs::read_to_string(&fixture_path).expect("fixture should be readable");
        assert!(
            !content.trim().is_empty(),
            "fixture must not be empty: {fixture}"
        );

        for (line_no, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("fixture line must be valid JSON");
            assert!(
                parsed.get("type").and_then(|v| v.as_str()).is_some(),
                "line {} in {} must have string type",
                line_no + 1,
                fixture
            );
        }
    }
}

#[test]
fn integrity_expected_json_shape_is_valid() {
    let expected_path = crate_path("tests/golden_fixtures/integrity_expected.json");
    let expected_raw = fs::read_to_string(expected_path).expect("expected JSON must be readable");
    let expected: serde_json::Value =
        serde_json::from_str(&expected_raw).expect("expected JSON must be valid");

    let required_source_fields = expected
        .get("required_source_fields")
        .and_then(|v| v.as_array())
        .expect("required_source_fields must be an array");
    assert!(
        !required_source_fields.is_empty(),
        "required_source_fields must not be empty"
    );
    assert!(required_source_fields.iter().all(|v| v.is_string()));

    for key in [
        "unknown_source_role_count",
        "unknown_top_level_type_count",
        "unknown_required_path_count",
        "legacy_fallback_path_count",
        "min_files_touched_extraction_count",
    ] {
        assert!(
            expected
                .get("counters")
                .and_then(|c| c.get(key))
                .and_then(|v| v.as_f64())
                .is_some(),
            "missing numeric counters.{}",
            key
        );
    }

    for key in [
        "min_loc_nonzero_rate_edit_write",
        "min_source_field_completeness_rate",
    ] {
        assert!(
            expected
                .get("rates")
                .and_then(|r| r.get(key))
                .and_then(|v| v.as_f64())
                .is_some(),
            "missing numeric rates.{}",
            key
        );
    }
}

#[test]
fn integrity_manifest_and_checksum_reference_expected_assets() {
    let manifest_path = crate_path("tests/integrity_corpus_snapshot/manifest.json");
    let manifest_raw = fs::read_to_string(manifest_path).expect("manifest should be readable");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("manifest should be valid JSON");

    let baseline = manifest
        .get("baseline_git_commit")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        !baseline.is_empty(),
        "baseline_git_commit must be non-empty"
    );

    let fixture_paths = manifest
        .get("fixtures")
        .and_then(|v| v.as_array())
        .expect("manifest.fixtures must be an array");
    assert!(
        !fixture_paths.is_empty(),
        "manifest.fixtures must not be empty"
    );

    for fixture in fixture_paths {
        let fixture_path = fixture
            .as_str()
            .expect("manifest fixture path entries must be strings");
        assert!(
            repo_path(fixture_path).exists(),
            "missing fixture: {fixture_path}"
        );
    }

    let checksum_path = crate_path("tests/golden_fixtures/integrity_fixture.sha256");
    let checksum_raw = fs::read_to_string(checksum_path).expect("checksum file must be readable");
    for fixture in FIXTURES {
        assert!(
            checksum_raw.contains(&format!("crates/db/{fixture}")),
            "checksum file missing fixture entry: {fixture}"
        );
    }
    assert!(
        checksum_raw.contains("crates/db/tests/golden_fixtures/integrity_expected.json"),
        "checksum file missing integrity_expected.json entry"
    );
}
