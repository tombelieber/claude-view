// Golden tests for parse_bytes correctness.
// These verify that JSONL parsing produces expected ParseResult/ExtendedMetadata
// across a variety of inputs: empty, single message, multi-turn, truncation, and invalid data.

use claude_view_db::indexer_parallel::parse_bytes;

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

// ============================================================================
// Phase 3 Golden Tests: Full Pipeline Verification
// ============================================================================

/// Golden fixture with known exact values for all Phase 3 metrics.
/// This JSONL represents a realistic session with:
/// - 4 user prompts
/// - 5 API calls (assistant messages)
/// - 8 tool calls across the responses
/// - 3 unique files read: /src/main.rs, /src/lib.rs, /src/config.rs
/// - 4 files edited (with edits): /src/main.rs x2, /src/lib.rs x1, /src/new.rs x1
/// - Re-edited: /src/main.rs (edited 2 times)
/// - Duration: 1800 seconds (30 minutes)
/// - Commit skill invoked once
const PHASE3_GOLDEN_FIXTURE: &[u8] = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"Help me refactor the code"}}
{"type":"assistant","timestamp":"2026-01-27T10:05:00Z","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/src/lib.rs"}}],"usage":{"input_tokens":100,"output_tokens":200,"cache_read_input_tokens":1000,"cache_creation_input_tokens":500,"service_tier":"standard"}}}
{"type":"user","timestamp":"2026-01-27T10:10:00Z","message":{"content":"Good analysis. Now edit main.rs"}}
{"type":"assistant","timestamp":"2026-01-27T10:15:00Z","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/src/config.rs"}}],"usage":{"input_tokens":150,"output_tokens":300,"cache_read_input_tokens":2000,"cache_creation_input_tokens":100,"service_tier":"standard"}}}
{"type":"user","timestamp":"2026-01-27T10:20:00Z","message":{"content":"Also update lib.rs and create new.rs"}}
{"type":"assistant","timestamp":"2026-01-27T10:22:00Z","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","name":"Write","input":{"file_path":"/src/new.rs"}}],"usage":{"input_tokens":200,"output_tokens":400,"cache_read_input_tokens":1500,"cache_creation_input_tokens":200,"service_tier":"standard"}}}
{"type":"assistant","timestamp":"2026-01-27T10:25:00Z","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}}],"usage":{"input_tokens":50,"output_tokens":100,"cache_read_input_tokens":500,"cache_creation_input_tokens":50,"service_tier":"standard"}}}
{"type":"user","timestamp":"2026-01-27T10:28:00Z","message":{"content":"Commit the changes"}}
{"type":"assistant","timestamp":"2026-01-27T10:30:00Z","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","name":"Skill","input":{"skill":"commit"}}],"usage":{"input_tokens":30,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":20,"service_tier":"standard"}}}
"#;

#[test]
fn golden_phase3_user_prompt_count() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);
    assert_eq!(result.deep.user_prompt_count, 4, "Should count exactly 4 user prompts");
}

#[test]
fn golden_phase3_api_call_count() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);
    assert_eq!(result.deep.api_call_count, 5, "Should count exactly 5 API calls (assistant messages)");
}

#[test]
fn golden_phase3_tool_call_count() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);
    // Read x3 + Edit x3 + Write x1 + Skill x1 = 8
    assert_eq!(result.deep.tool_call_count, 8, "Should count exactly 8 tool calls");
}

#[test]
fn golden_phase3_files_read() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // 3 unique files read: /src/main.rs, /src/lib.rs, /src/config.rs
    assert_eq!(result.deep.files_read_count, 3, "Should have 3 unique files read");
    assert!(result.deep.files_read.contains(&"/src/main.rs".to_string()));
    assert!(result.deep.files_read.contains(&"/src/lib.rs".to_string()));
    assert!(result.deep.files_read.contains(&"/src/config.rs".to_string()));
}

#[test]
fn golden_phase3_files_edited() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // files_edited stores ALL occurrences (not deduplicated)
    // Edit /src/main.rs x2, Edit /src/lib.rs x1, Write /src/new.rs x1 = 4 total
    assert_eq!(result.deep.files_edited.len(), 4, "Should store all 4 edit occurrences");

    // Unique files: main.rs, lib.rs, new.rs = 3
    assert_eq!(result.deep.files_edited_count, 3, "Should have 3 unique files edited");
}

#[test]
fn golden_phase3_reedited_files() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // Only /src/main.rs was edited 2+ times
    assert_eq!(result.deep.reedited_files_count, 1, "Only main.rs was re-edited");
}

#[test]
fn golden_phase3_duration() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // 10:00:00 to 10:30:00 = 30 minutes = 1800 seconds
    assert_eq!(result.deep.duration_seconds, 1800, "Duration should be 1800 seconds (30 minutes)");
    assert!(result.deep.first_timestamp.is_some());
    assert!(result.deep.last_timestamp.is_some());
}

#[test]
fn golden_phase3_turn_count() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // turn_count = min(user_count, assistant_count) = min(4, 5) = 4
    assert_eq!(result.deep.turn_count, 4, "Turn count should be 4 (min of user/assistant)");
}

#[test]
fn golden_phase3_tool_counts() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    assert_eq!(result.deep.tool_counts.read, 3, "Should have 3 Read tool uses");
    assert_eq!(result.deep.tool_counts.edit, 3, "Should have 3 Edit tool uses");
    assert_eq!(result.deep.tool_counts.write, 1, "Should have 1 Write tool use");
    assert_eq!(result.deep.tool_counts.bash, 0, "Should have 0 Bash tool uses");
}

#[test]
fn golden_phase3_raw_invocations() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // Total tool_use blocks: 8
    assert_eq!(result.raw_invocations.len(), 8, "Should extract 8 raw invocations");

    // Verify Skill invocation for commit detection
    let skill_invocations: Vec<_> = result.raw_invocations.iter()
        .filter(|inv| inv.name == "Skill")
        .collect();
    assert_eq!(skill_invocations.len(), 1, "Should have 1 Skill invocation");

    let skill = &skill_invocations[0];
    assert_eq!(
        skill.input.as_ref().and_then(|v| v.get("skill")).and_then(|v| v.as_str()),
        Some("commit"),
        "Skill invocation should be for 'commit'"
    );
}

#[test]
fn golden_phase3_turns_with_tokens() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // 5 assistant messages with usage data
    assert_eq!(result.turns.len(), 5, "Should extract 5 turns");

    // Verify total tokens across all turns
    let total_input: u64 = result.turns.iter().filter_map(|t| t.input_tokens).sum();
    let total_output: u64 = result.turns.iter().filter_map(|t| t.output_tokens).sum();

    // 100 + 150 + 200 + 50 + 30 = 530 input
    assert_eq!(total_input, 530, "Total input tokens should be 530");
    // 200 + 300 + 400 + 100 + 50 = 1050 output
    assert_eq!(total_output, 1050, "Total output tokens should be 1050");
}

#[test]
fn golden_phase3_models_seen() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // All turns use claude-opus-4-5-20251101
    assert_eq!(result.models_seen.len(), 1, "Should see 1 unique model");
    assert!(result.models_seen.contains(&"claude-opus-4-5-20251101".to_string()));
}

#[test]
fn golden_phase3_last_message() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);
    assert_eq!(result.deep.last_message, "Commit the changes");
}

/// Comprehensive golden test verifying all metrics together.
#[test]
fn golden_phase3_complete_metrics() {
    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // Atomic units
    assert_eq!(result.deep.user_prompt_count, 4);
    assert_eq!(result.deep.api_call_count, 5);
    assert_eq!(result.deep.tool_call_count, 8);
    assert_eq!(result.deep.files_read_count, 3);
    assert_eq!(result.deep.files_edited_count, 3);
    assert_eq!(result.deep.reedited_files_count, 1);
    assert_eq!(result.deep.duration_seconds, 1800);

    // Tool counts
    assert_eq!(result.deep.tool_counts.read, 3);
    assert_eq!(result.deep.tool_counts.edit, 3);
    assert_eq!(result.deep.tool_counts.write, 1);
    assert_eq!(result.deep.tool_counts.bash, 0);

    // Turns and models
    assert_eq!(result.turns.len(), 5);
    assert_eq!(result.models_seen.len(), 1);

    // Legacy metrics still work
    assert_eq!(result.deep.turn_count, 4);
    assert!(!result.deep.last_message.is_empty());
}

// ============================================================================
// Phase 3 Golden Test: Derived Metrics Calculation
// ============================================================================

#[test]
fn golden_phase3_derived_metrics() {
    use claude_view_core::metrics::{
        tokens_per_prompt, reedit_rate, tool_density, edit_velocity, read_to_edit_ratio
    };

    let result = parse_bytes(PHASE3_GOLDEN_FIXTURE);

    // Calculate totals from turns
    let total_input: u64 = result.turns.iter().filter_map(|t| t.input_tokens).sum();
    let total_output: u64 = result.turns.iter().filter_map(|t| t.output_tokens).sum();

    // A2.1 Tokens Per Prompt: (530 + 1050) / 4 = 395.0
    let tpp = tokens_per_prompt(total_input, total_output, result.deep.user_prompt_count);
    assert_eq!(tpp, Some(395.0));

    // A2.2 Re-edit Rate: 1 / 3 = 0.333...
    let rr = reedit_rate(result.deep.reedited_files_count, result.deep.files_edited_count);
    assert!(rr.is_some());
    assert!((rr.unwrap() - 0.333333).abs() < 0.001);

    // A2.3 Tool Density: 8 / 5 = 1.6
    let td = tool_density(result.deep.tool_call_count, result.deep.api_call_count);
    assert_eq!(td, Some(1.6));

    // A2.4 Edit Velocity: 3 / (1800 / 60) = 3 / 30 = 0.1 files/min
    let ev = edit_velocity(result.deep.files_edited_count, result.deep.duration_seconds);
    assert_eq!(ev, Some(0.1));

    // A2.5 Read-to-Edit Ratio: 3 / 3 = 1.0
    let rte = read_to_edit_ratio(result.deep.files_read_count, result.deep.files_edited_count);
    assert_eq!(rte, Some(1.0));
}

// ============================================================================
// gitBranch extraction tests
// ============================================================================

#[test]
fn golden_git_branch_extracted() {
    let data = br#"{"type":"user","message":{"content":"hello"},"gitBranch":"feature/foo","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]},"gitBranch":"feature/foo","timestamp":"2026-01-01T00:00:01Z"}"#;
    let result = parse_bytes(data);
    assert_eq!(result.git_branch.as_deref(), Some("feature/foo"));
}

#[test]
fn golden_git_branch_none_when_absent() {
    let data = br#"{"type":"user","message":{"content":"hello"},"timestamp":"2026-01-01T00:00:00Z"}"#;
    let result = parse_bytes(data);
    assert_eq!(result.git_branch, None);
}

#[test]
fn golden_git_branch_first_line_wins() {
    // gitBranch from first line should be captured even if later lines differ
    let data = br#"{"type":"user","message":{"content":"hello"},"gitBranch":"main","timestamp":"2026-01-01T00:00:00Z"}
{"type":"user","message":{"content":"bye"},"gitBranch":"develop","timestamp":"2026-01-01T00:00:01Z"}"#;
    let result = parse_bytes(data);
    assert_eq!(result.git_branch.as_deref(), Some("main"));
}
