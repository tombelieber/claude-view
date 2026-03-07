use claude_view_db::indexer_parallel::parse_bytes;

const NONZERO_TOOL_INDEX: &str = include_str!("golden_fixtures/integrity_nonzero_tool_index.jsonl");
const PROGRESS_NESTED_CONTENT: &str =
    include_str!("golden_fixtures/integrity_progress_nested_content.jsonl");
const TYPE_SUBSTRING_NOISE: &str =
    include_str!("golden_fixtures/integrity_type_substring_noise.jsonl");

#[test]
fn integrity_nonzero_tool_index_extracts_expected_paths() {
    let result = parse_bytes(NONZERO_TOOL_INDEX.as_bytes());

    assert!(
        result
            .deep
            .files_touched
            .contains(&"src/main.rs".to_string()),
        "expected src/main.rs in files_touched"
    );
    assert!(
        result
            .deep
            .files_touched
            .contains(&"src/lib.rs".to_string()),
        "expected src/lib.rs in files_touched"
    );
    assert!(
        result.deep.tool_counts.edit >= 1,
        "expected at least one Edit"
    );
    assert!(
        result.deep.tool_counts.write >= 1,
        "expected at least one Write"
    );
    assert!(
        result.deep.ai_lines_added > 0 || result.deep.ai_lines_removed > 0,
        "expected non-zero AI LOC from Edit/Write payload"
    );
}

#[test]
fn integrity_progress_nested_content_counts_agent_progress() {
    let result = parse_bytes(PROGRESS_NESTED_CONTENT.as_bytes());

    assert!(
        result.deep.agent_spawn_count >= 1,
        "expected agent_progress lines to be counted"
    );
}

#[test]
fn integrity_type_substring_noise_does_not_create_unknown_types() {
    let result = parse_bytes(TYPE_SUBSTRING_NOISE.as_bytes());

    assert_eq!(
        result.diagnostics.lines_unknown_type, 0,
        "substring noise should not produce unknown top-level types"
    );
    assert!(
        result.deep.user_prompt_count >= 1,
        "expected at least one user prompt"
    );
    assert!(
        result.deep.api_call_count >= 1,
        "expected at least one assistant line"
    );
}
