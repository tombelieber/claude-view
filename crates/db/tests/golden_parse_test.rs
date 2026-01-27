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
