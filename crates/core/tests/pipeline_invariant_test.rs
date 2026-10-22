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

#[test]
fn legacy_tool_paths_remain_proven_without_appearing_in_current_sessions() {
    use claude_view_core::evidence_audit::{load_baseline, run_phase3_checks};
    use claude_view_core::field_inventory::FieldInventory;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut baseline =
        load_baseline(&root.join("scripts/integrity/evidence-baseline.json")).unwrap();
    baseline.field_extraction_paths.as_mut().unwrap().extracted = vec![
        "message.content[].input.name".into(),
        "message.content[].input.todos".into(),
        "toolUseResult.totalDurationMs".into(),
        "toolUseResult.totalToolUseCount".into(),
    ];
    let checks = run_phase3_checks(&FieldInventory::default(), &baseline);
    assert!(
        checks[1].passed,
        "shipped legacy fixtures must prove supported optional paths"
    );
    baseline
        .field_extraction_paths
        .as_mut()
        .unwrap()
        .extracted
        .push("message.content[].input.invented_field".into());
    let checks = run_phase3_checks(&FieldInventory::default(), &baseline);
    assert!(
        !checks[1].passed,
        "fixtures must not hide genuinely phantom fields"
    );
}

#[test]
fn legacy_tool_fixture_reaches_real_blocks_without_losing_tool_inputs() {
    let blocks = claude_view_core::block_accumulator::parse_session_as_blocks(include_str!(
        "fixtures/legacy_tool_inputs.jsonl"
    ));
    let json = serde_json::to_value(blocks).unwrap();
    let serialized = json.to_string();
    assert!(serialized.contains("fixture-agent"));
    assert!(serialized.contains("Check fixture"));
    assert!(serialized.contains("TodoWrite"));
}

#[test]
fn legacy_agent_totals_fixture_reaches_existing_live_parser() {
    use claude_view_core::live_parser::{parse_single_line, TailFinders};
    let finders = TailFinders::new();
    let results: Vec<_> = include_str!("fixtures/legacy_tool_inputs.jsonl")
        .lines()
        .filter_map(|line| parse_single_line(line.as_bytes(), &finders).sub_agent_result)
        .collect();
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.tool_use_id, "legacy-agent");
    assert_eq!(result.agent_id.as_deref(), Some("fixture-subagent"));
    assert_eq!(result.total_duration_ms, Some(42));
    assert_eq!(result.total_tool_use_count, Some(3));
}

#[test]
fn reviewed_metadata_does_not_mask_unknown_type_or_field_drift() {
    use claude_view_core::evidence_audit::{load_baseline, run_audit_checks, AggregatedSignals};
    use claude_view_core::field_inventory::{extract_field_inventory, FieldInventory};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let baseline = load_baseline(&root.join("scripts/integrity/evidence-baseline.json")).unwrap();
    let mut inventory = FieldInventory::default();
    let mut signals = AggregatedSignals::default();
    let records = [
        r#"{"type":"fork-context-ref","agentId":"fixture-agent","parentSessionId":"fixture-parent","parentLastUuid":"fixture-message","contextLength":42}"#,
        r#"{"type":"frame-link","sessionId":"fixture-session","timestamp":"2026-01-01T00:00:00Z","path":"/synthetic/frame","frameUrl":"https://example.invalid/frame","title":"Synthetic frame"}"#,
        r#"{"type":"assistant","apiErrorIsTransient":true}"#,
        r#"{"type":"user","stackedExpansion":false,"stackedOriginalInput":"Original synthetic input","message":{"content":"Expanded synthetic input"}}"#,
        r#"{"type":"user","userFeedback":"Synthetic feedback","message":{"content":[{"type":"tool_result","tool_use_id":"fixture-tool","content":"Synthetic feedback in tool result"}]}}"#,
    ];
    for line in records {
        inventory.merge(&extract_field_inventory(line.as_bytes()));
        let entry: serde_json::Value = serde_json::from_str(line).unwrap();
        signals
            .top_level_types
            .insert(entry["type"].as_str().unwrap().into());
    }
    let paths = baseline.field_extraction_paths.as_ref().unwrap();
    assert!(check_field_coverage(&inventory, paths).passed);
    assert!(run_audit_checks(&signals, &baseline).passed);
    let blocks = claude_view_core::block_accumulator::parse_session_as_blocks(&records.join("\n"));
    let rendered = serde_json::to_string(&blocks).unwrap();
    assert!(rendered.contains("Expanded synthetic input"));
    assert!(rendered.contains("Synthetic feedback in tool result"));
    assert!(!rendered.contains("Original synthetic input"));

    inventory.paths.insert("unreviewed_fixture_field".into(), 1);
    assert!(
        !check_field_coverage(&inventory, paths).passed,
        "reviewed metadata must not allow unrelated unknown fields"
    );
    signals
        .top_level_types
        .insert("unreviewed-fixture-type".into());
    assert!(
        !run_audit_checks(&signals, &baseline).passed,
        "reviewed metadata must not allow unrelated unknown record types"
    );
}

#[test]
fn current_diagnostics_preserve_messages_and_do_not_double_count_checkpoints() {
    use claude_view_core::block_accumulator::parse_session_as_blocks;
    use claude_view_core::block_types::{ConversationBlock, SystemVariant};
    let content = concat!(
        "{\"type\":\"atis-latch\",\"atis\":\"synthetic\"}\n",
        "{\"type\":\"cost-state\",\"totalCostUSD\":1.0}\n",
        "{\"type\":\"file-history-delta\",\"backup\":{}}\n",
        "{\"type\":\"system\",\"subtype\":\"permission_retry\",\"content\":\"Synthetic retry\",\"commands\":[\"example\"]}\n",
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"fallback\",\"from\":{},\"to\":{}},{\"type\":\"text\",\"text\":\"Kept text\"}]},\"stopReason\":\"end_turn\"}\n"
    );
    let blocks = parse_session_as_blocks(content);
    assert_eq!(
        blocks.len(),
        2,
        "checkpoints must not become duplicate conversation actions"
    );
    assert!(
        matches!(&blocks[0], ConversationBlock::System(s) if s.variant == SystemVariant::Informational && s.data["commands"][0] == "example")
    );
    assert!(serde_json::to_string(&blocks)
        .unwrap()
        .contains("Kept text"));
}
