#[cfg(test)]
mod tests {
    use crate::evidence_audit::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn test_baseline_deserializes_from_real_file() {
        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/integrity/evidence-baseline.json");
        let baseline = load_baseline(&baseline_path).expect("should deserialize baseline");

        // Verify top-level types
        assert!(
            baseline
                .top_level_types
                .handled
                .contains(&"assistant".to_string()),
            "handled should contain 'assistant'"
        );
        assert!(
            baseline
                .top_level_types
                .handled
                .contains(&"user".to_string()),
            "handled should contain 'user'"
        );
        assert!(
            baseline
                .top_level_types
                .handled_as_progress
                .contains(&"progress".to_string()),
            "handled_as_progress should contain 'progress'"
        );
        assert!(
            baseline
                .top_level_types
                .handled
                .contains(&"pr-link".to_string()),
            "handled should contain 'pr-link'"
        );

        // Verify all_known includes everything
        let all = baseline.top_level_types.all_known();
        assert!(all.contains("assistant"));
        assert!(all.contains("progress"));
        assert!(all.contains("pr-link"));
        assert!(all.contains("agent-name"));
        assert!(all.contains("hook_event")); // zero_occurrence_but_parser_has_arm

        // Verify content block types
        assert!(
            baseline
                .content_block_types
                .assistant
                .contains(&"thinking".to_string()),
            "assistant content blocks should contain 'thinking'"
        );

        // Verify system subtypes
        assert!(
            baseline
                .system_subtypes
                .known
                .contains(&"turn_duration".to_string()),
            "system subtypes should contain 'turn_duration'"
        );

        // Verify progress data types
        assert!(
            baseline
                .progress_data_types
                .known
                .contains(&"agent_progress".to_string()),
            "progress data types should contain 'agent_progress'"
        );

        // Verify thinking block keys
        assert!(
            baseline
                .thinking_block_keys
                .required
                .contains(&"signature".to_string()),
            "thinking block keys should contain 'signature'"
        );
    }

    #[test]
    fn test_extract_signals_assistant_with_thinking() {
        let line = br#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:01:00Z","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"thinking","thinking":"hmm","signature":"sig1"},{"type":"text","text":"hello"},{"type":"tool_use","id":"tu1","name":"Read","input":{}}]}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("assistant"));
        assert_eq!(signals.subtype, None);
        assert_eq!(signals.data_type, None);

        // Content block types
        assert_eq!(signals.content_block_types.len(), 3);
        assert!(signals
            .content_block_types
            .contains(&"thinking".to_string()));
        assert!(signals.content_block_types.contains(&"text".to_string()));
        assert!(signals
            .content_block_types
            .contains(&"tool_use".to_string()));

        // Thinking key sets
        assert_eq!(signals.thinking_key_sets.len(), 1);
        let keys = &signals.thinking_key_sets[0];
        assert!(keys.contains("type"));
        assert!(keys.contains("thinking"));
        assert!(keys.contains("signature"));
    }

    #[test]
    fn test_extract_signals_system() {
        let line = br#"{"type":"system","uuid":"s1","timestamp":"2026-01-28T10:03:05Z","subtype":"turn_duration","durationMs":5000}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("system"));
        assert_eq!(signals.subtype.as_deref(), Some("turn_duration"));
        assert!(signals.content_block_types.is_empty());
    }

    #[test]
    fn test_extract_signals_progress() {
        let line = br#"{"type":"progress","uuid":"p1","timestamp":"2026-01-28T10:03:10Z","data":{"type":"hook_progress","hookEvent":"PreToolUse"}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("progress"));
        assert_eq!(signals.data_type.as_deref(), Some("hook_progress"));
    }

    #[test]
    fn test_extract_signals_agent_progress_nesting() {
        // agent_progress with double-nested message.message.content[]
        let line = br#"{"type":"progress","uuid":"p2","data":{"type":"agent_progress","message":{"uuid":"x","type":"message","timestamp":"t","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]}}}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("progress"));
        assert_eq!(signals.data_type.as_deref(), Some("agent_progress"));
        assert!(
            signals.nesting_direct,
            "should detect direct agent_progress"
        );
        assert!(
            signals.nesting_nested,
            "should detect nested message.message.content[]"
        );
    }

    #[test]
    fn test_no_misclassify_string_content_as_type() {
        // User message with string content containing "type":"assistant" — must NOT extract content blocks
        let line = br#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"The type:assistant message was sent"}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("user"));
        // Content blocks should be empty because: (1) string content yields Other, (2) non-assistant clears them
        assert!(
            signals.content_block_types.is_empty(),
            "user message should not have content block types"
        );
    }

    #[test]
    fn test_extract_signals_malformed_line() {
        let line = b"not valid json at all {{{";
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type, None);
        assert!(signals.content_block_types.is_empty());
    }

    #[test]
    fn test_scan_file_aggregates_signals() {
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
        let agg = crate::evidence_audit::scanning::scan_file(&fixture_path);

        assert_eq!(agg.files_scanned, 1);
        assert!(agg.lines_scanned >= 10, "should scan multiple lines");
        assert_eq!(agg.errors, 0, "fixture should have no parse errors");

        // Top-level types from fixture
        assert!(
            agg.top_level_types.contains("user"),
            "should find 'user' type"
        );
        assert!(
            agg.top_level_types.contains("assistant"),
            "should find 'assistant' type"
        );
        assert!(
            agg.top_level_types.contains("system"),
            "should find 'system' type"
        );
        assert!(
            agg.top_level_types.contains("progress"),
            "should find 'progress' type"
        );
        assert!(
            agg.top_level_types.contains("queue-operation"),
            "should find 'queue-operation' type"
        );
        assert!(
            agg.top_level_types.contains("file-history-snapshot"),
            "should find 'file-history-snapshot' type"
        );

        // Content block types from assistant messages
        assert!(
            agg.assistant_content_block_types.contains("thinking"),
            "should find 'thinking' content block"
        );
        assert!(
            agg.assistant_content_block_types.contains("text"),
            "should find 'text' content block"
        );
        assert!(
            agg.assistant_content_block_types.contains("tool_use"),
            "should find 'tool_use' content block"
        );

        // System subtypes
        assert!(
            agg.system_subtypes.contains("turn_duration"),
            "should find 'turn_duration' subtype"
        );

        // Progress data types
        assert!(
            agg.progress_data_types.contains("hook_progress"),
            "should find 'hook_progress' data type"
        );

        // Thinking block key audit
        assert!(
            !agg.thinking_key_sets.is_empty(),
            "should have thinking key sets"
        );
        let first_keys = agg.thinking_key_sets.iter().next().unwrap();
        assert!(
            first_keys.contains("type"),
            "thinking keys should include 'type'"
        );
        assert!(
            first_keys.contains("thinking"),
            "thinking keys should include 'thinking'"
        );
        assert!(
            first_keys.contains("signature"),
            "thinking keys should include 'signature'"
        );
    }

    #[test]
    fn test_check_result_no_drift() {
        let actual: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let expected: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let result = check_set_diff("test", &actual, &expected);

        assert!(result.passed, "no new items means pass");
        assert!(result.new_items.is_empty());
        assert_eq!(result.absent_items, vec!["c".to_string()]);
    }

    #[test]
    fn test_check_result_with_drift() {
        let actual: HashSet<String> = ["a", "b", "new_type"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let expected: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let result = check_set_diff("test", &actual, &expected);

        assert!(!result.passed, "new items means fail");
        assert!(result.new_items.contains(&"new_type".to_string()));
        assert!(result.absent_items.is_empty());
    }

    #[test]
    fn test_full_audit_pass() {
        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/integrity/evidence-baseline.json");
        let baseline = load_baseline(&baseline_path).expect("should load baseline");

        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
        let signals = crate::evidence_audit::scanning::scan_file(&fixture_path);

        let result = run_audit_checks(&signals, &baseline);

        // The fixture covers the basic types — should pass
        assert!(
            result.passed,
            "audit should pass for fixture. Failed checks: {:?}",
            result
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| format!("{}: new={:?}", c.name, c.new_items))
                .collect::<Vec<_>>()
        );

        // Verify stats are populated
        assert_eq!(result.files_scanned, 1);
        assert!(result.lines_scanned > 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_scan_file_returns_pipeline_signals() {
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
        let (agg, pipeline) = scan_file_with_pipeline(&fixture_path);
        assert!(agg.files_scanned == 1);
        let results = pipeline.into_results();
        for r in &results {
            if r.skipped {
                continue;
            }
            assert!(
                r.passed,
                "check {} should pass on fixture: {:?}",
                r.name, r.sample_violations
            );
        }
    }
}
