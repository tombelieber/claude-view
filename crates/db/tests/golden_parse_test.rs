// Golden tests for parse_bytes correctness.
// These verify that JSONL parsing produces expected ParseResult/ExtendedMetadata
// across a variety of inputs: empty, single message, multi-turn, truncation, and invalid data.

use vibe_recall_db::indexer_parallel::parse_bytes;

#[test]
fn golden_empty_file() {
    let result = parse_bytes(b"");
    assert_eq!(result.deep.turn_count, 0);
    assert!(result.deep.last_message.is_empty());
    assert!(result.deep.tool_counts.is_empty());
    assert!(result.deep.skills_used.is_empty());
    assert!(result.deep.files_touched.is_empty());
    assert!(result.raw_invocations.is_empty());
}

#[test]
fn golden_single_user_message() {
    let data = br#"{"type":"user","message":{"content":"Hello world"}}"#;
    let result = parse_bytes(data);
    assert_eq!(result.deep.last_message, "Hello world");
    assert_eq!(result.deep.turn_count, 0); // No assistant response yet
}

#[test]
fn golden_full_conversation() {
    let data = br#"{"type":"user","message":{"content":"Fix the bug"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"text","text":"Let me read the file"}]}}
{"type":"user","message":{"content":"Thanks, now edit it"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/src/lib.rs"}}]}}
{"type":"user","message":{"content":"Now run the tests"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}]}}
"#;
    let result = parse_bytes(data);
    assert_eq!(result.deep.turn_count, 3);
    assert_eq!(result.deep.last_message, "Now run the tests");
    assert_eq!(result.deep.tool_counts.read, 2);
    assert_eq!(result.deep.tool_counts.edit, 1);
    assert_eq!(result.deep.tool_counts.bash, 1);
    assert_eq!(result.deep.tool_counts.write, 0);
    assert!(result.deep.files_touched.contains(&"/src/main.rs".to_string()));
    assert!(result.deep.files_touched.contains(&"/src/lib.rs".to_string()));
}

#[test]
fn golden_long_message_truncated() {
    let long_msg = "a".repeat(300);
    let data = format!(
        r#"{{"type":"user","message":{{"content":"{}"}}}}"#,
        long_msg
    );
    let result = parse_bytes(data.as_bytes());
    assert_eq!(result.deep.last_message.len(), 203); // 200 + "..."
    assert!(result.deep.last_message.ends_with("..."));
}

#[test]
fn golden_non_utf8_no_panic() {
    let mut data = br#"{"type":"user","message":{"content":"hello"}}
"#
    .to_vec();
    data.extend_from_slice(&[0xFF, 0xFE, b'\n']); // Invalid UTF-8
    data.extend_from_slice(br#"{"type":"assistant","message":{"content":"response"}}"#);
    let result = parse_bytes(&data);
    // Should not panic, may have partial results
    assert!(result.deep.turn_count <= 1);
}

#[test]
fn golden_realistic_session() {
    let data = br#"{"type":"user","message":{"content":"Help me refactor"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"I'll help"},{"type":"tool_use","name":"Read","input":{"file_path":"/src/app.ts"}}]}}
{"type":"user","message":{"content":"Good, now write tests"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"/tests/app.test.ts"}},{"type":"tool_use","name":"Bash","input":{"command":"npm test"}}]}}
{"type":"user","message":{"content":"commit please"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"git add ."}}]}}
"#;
    let result = parse_bytes(data);
    assert_eq!(result.deep.turn_count, 3);
    assert_eq!(result.deep.last_message, "commit please");
    assert_eq!(result.deep.tool_counts.read, 1);
    assert_eq!(result.deep.tool_counts.write, 1);
    assert_eq!(result.deep.tool_counts.bash, 2);
    assert_eq!(result.deep.tool_counts.edit, 0);
    assert!(result.deep.files_touched.contains(&"/src/app.ts".to_string()));
    assert!(
        result.deep.files_touched
            .contains(&"/tests/app.test.ts".to_string())
    );
}

#[test]
fn golden_turns_extracted_from_assistant_with_usage() {
    // JSONL with assistant messages containing model + usage data
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"Hello"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"model":"claude-opus-4-5-20251101","role":"assistant","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":3,"output_tokens":9,"cache_read_input_tokens":24206,"cache_creation_input_tokens":12454,"service_tier":"standard"}}}
{"type":"user","uuid":"u2","message":{"content":"Thanks"}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":"2026-01-28T10:01:00Z","message":{"model":"claude-sonnet-4-20250514","role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}],"usage":{"input_tokens":100,"output_tokens":500,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200,"service_tier":"standard"}}}
"#;
    let result = parse_bytes(data);

    // Verify turns were extracted
    assert_eq!(result.turns.len(), 2, "Should extract 2 turns");
    assert_eq!(result.models_seen.len(), 2, "Should see 2 unique models");

    // First turn
    assert_eq!(result.turns[0].uuid, "a1");
    assert_eq!(result.turns[0].model_id, "claude-opus-4-5-20251101");
    assert_eq!(result.turns[0].seq, 0);
    assert_eq!(result.turns[0].input_tokens, Some(3));
    assert_eq!(result.turns[0].output_tokens, Some(9));
    assert_eq!(result.turns[0].cache_read_tokens, Some(24206));
    assert_eq!(result.turns[0].cache_creation_tokens, Some(12454));
    assert_eq!(result.turns[0].service_tier.as_deref(), Some("standard"));
    assert_eq!(result.turns[0].content_type, "text");

    // Second turn
    assert_eq!(result.turns[1].uuid, "a2");
    assert_eq!(result.turns[1].model_id, "claude-sonnet-4-20250514");
    assert_eq!(result.turns[1].seq, 1);
    assert_eq!(result.turns[1].input_tokens, Some(100));
    assert_eq!(result.turns[1].content_type, "tool_use");

    // Models seen
    assert!(result.models_seen.contains(&"claude-opus-4-5-20251101".to_string()));
    assert!(result.models_seen.contains(&"claude-sonnet-4-20250514".to_string()));
}

#[test]
fn golden_no_turns_without_usage_data() {
    // Assistant messages without usage data should not produce turns
    let data = br#"{"type":"user","message":{"content":"Hello"}}
{"type":"assistant","message":{"content":"Hi there!"}}
"#;
    let result = parse_bytes(data);
    assert!(result.turns.is_empty(), "No usage data = no turns");
    assert!(result.models_seen.is_empty(), "No model data = no models");
}
