//! Block accumulator invariant test: verifies that the block_accumulator
//! doesn't read phantom fields (JSONL fields that don't exist in real data),
//! and that the evidence-baseline.json stays in sync with the parser.

use claude_view_core::block_accumulator::parse_session_as_blocks;
use claude_view_core::block_types::*;
use std::collections::HashSet;
use std::path::PathBuf;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/block_accumulator")
}

/// Verify that a fixture produces blocks with non-trivial content
/// (not just default/empty values).
#[test]
fn simple_turn_produces_non_trivial_blocks() {
    let content = std::fs::read_to_string(fixtures_path().join("simple_turn.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&content);

    // UserBlock should have real text (not empty)
    let user = blocks
        .iter()
        .find_map(|b| {
            if let ConversationBlock::User(u) = b {
                Some(u)
            } else {
                None
            }
        })
        .expect("Should have a UserBlock");
    assert!(
        !user.text.is_empty(),
        "UserBlock.text should not be empty — phantom field check"
    );
    assert!(
        user.timestamp > 0.0,
        "UserBlock.timestamp should be > 0 — phantom field check"
    );

    // AssistantBlock should have segments with real tool names
    let assistant = blocks
        .iter()
        .find_map(|b| {
            if let ConversationBlock::Assistant(a) = b {
                Some(a)
            } else {
                None
            }
        })
        .expect("Should have an AssistantBlock");
    assert!(
        !assistant.segments.is_empty(),
        "AssistantBlock should have segments"
    );

    // Check that at least one segment has actual content
    let has_real_content = assistant.segments.iter().any(|seg| match seg {
        AssistantSegment::Text { text, .. } => !text.is_empty(),
        AssistantSegment::Tool { execution } => !execution.tool_name.is_empty(),
    });
    assert!(
        has_real_content,
        "AssistantBlock segments should have real content"
    );

    // TurnBoundaryBlock should have real duration (not 0)
    let boundary = blocks
        .iter()
        .find_map(|b| {
            if let ConversationBlock::TurnBoundary(tb) = b {
                Some(tb)
            } else {
                None
            }
        })
        .expect("Should have a TurnBoundaryBlock");
    assert!(
        boundary.duration_ms > 0,
        "TurnBoundary.duration_ms should be > 0 — phantom field check"
    );

    // ProgressBlock should have real data
    let progress = blocks
        .iter()
        .find_map(|b| {
            if let ConversationBlock::Progress(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .expect("Should have a ProgressBlock");
    assert!(progress.ts > 0.0, "ProgressBlock.ts should be > 0");

    // Verify ProgressBlock has non-empty data
    match &progress.data {
        ProgressData::Bash(bash) => {
            // At least one of these should be non-trivial
            assert!(
                bash.total_lines > 0 || bash.total_bytes > 0 || !bash.output.is_empty(),
                "BashProgress should have non-trivial data"
            );
        }
        _ => {} // Other progress types are OK
    }
}

/// Verify that no blocks reference fields that aren't in the fixture
#[test]
fn no_phantom_fields_in_tool_execution() {
    let content = std::fs::read_to_string(fixtures_path().join("simple_turn.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&content);

    // Find AssistantBlock with tool segments
    for block in &blocks {
        if let ConversationBlock::Assistant(assistant) = block {
            for seg in &assistant.segments {
                if let AssistantSegment::Tool { execution } = seg {
                    // tool_name must be a real tool name from the fixture
                    assert!(
                        [
                            "Bash", "Read", "Write", "Edit", "Grep", "Glob", "Agent", "Task",
                            "Skill"
                        ]
                        .contains(&execution.tool_name.as_str())
                            || execution.tool_name.starts_with("mcp__"),
                        "ToolExecution.tool_name '{}' should be a known tool name",
                        execution.tool_name
                    );

                    // tool_use_id should not be empty
                    assert!(
                        !execution.tool_use_id.is_empty(),
                        "ToolExecution.tool_use_id should not be empty"
                    );
                }
            }
        }
    }
}

/// Verify SystemBlock variants match what's actually in the fixtures
#[test]
fn system_only_fixture_produces_correct_variants() {
    let content = std::fs::read_to_string(fixtures_path().join("system_only.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&content);

    let variants: Vec<_> = blocks
        .iter()
        .filter_map(|b| {
            if let ConversationBlock::System(s) = b {
                Some(s.variant)
            } else {
                None
            }
        })
        .collect();

    // These specific variants should be produced by the system_only fixture
    assert!(
        variants.contains(&SystemVariant::AiTitle),
        "Should have AiTitle variant"
    );
    assert!(
        variants.contains(&SystemVariant::LastPrompt),
        "Should have LastPrompt variant"
    );
    assert!(
        variants.contains(&SystemVariant::QueueOperation),
        "Should have QueueOperation variant"
    );
    assert!(
        variants.contains(&SystemVariant::FileHistorySnapshot),
        "Should have FileHistorySnapshot variant"
    );
    assert!(
        variants.contains(&SystemVariant::AgentName),
        "Should have AgentName variant"
    );
}

// ── Baseline ↔ Parser sync guard ──────────────────────────────────────────
//
// ROOT CAUSE FIX: The evidence-baseline.json `handled` list and the block
// accumulator's match arms are two sources of truth. This test ensures they
// agree. If you add a match arm, you must update the baseline (and vice versa).

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("scripts/integrity/evidence-baseline.json")
}

/// Minimal JSONL for each type that the accumulator should produce a block from.
/// Types that need richer structure to parse (user, assistant, system, progress)
/// have the minimum fields; simple metadata types just need `{"type":"..."}`.
fn minimal_jsonl_for(entry_type: &str) -> String {
    match entry_type {
        "user" => r#"{"type":"user","message":{"content":[{"type":"text","text":"hi"}]}}"#.to_string(),
        "assistant" => r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]},"stopReason":"end_turn"}"#.to_string(),
        "system" => r#"{"type":"system","subtype":"informational","content":"test"}"#.to_string(),
        "progress" => r#"{"type":"progress","data":{"type":"bash_progress","tool_use_id":"tu1","output":"","total_lines":0,"total_bytes":0}}"#.to_string(),
        _ => format!(r#"{{"type":"{}","sessionId":"test"}}"#, entry_type),
    }
}

/// For every type in baseline `handled` + `handled_as_progress`, feed a minimal
/// JSONL entry through the accumulator and assert at least one block comes out.
/// This prevents the baseline from claiming "handled" when the parser silently drops it.
#[test]
fn baseline_handled_types_actually_produce_blocks() {
    let baseline_json =
        std::fs::read_to_string(baseline_path()).expect("evidence-baseline.json must exist");
    let baseline: serde_json::Value = serde_json::from_str(&baseline_json).unwrap();

    let handled: Vec<&str> = baseline["top_level_types"]["handled"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            baseline["top_level_types"]["handled_as_progress"]
                .as_array()
                .unwrap()
                .iter(),
        )
        .map(|v| v.as_str().unwrap())
        .collect();

    for entry_type in &handled {
        let jsonl = minimal_jsonl_for(entry_type);
        let blocks = parse_session_as_blocks(&jsonl);
        assert!(
            !blocks.is_empty(),
            "Baseline claims '{}' is handled, but the accumulator produced 0 blocks for it. \
             Either add a match arm in block_accumulator or move it out of `handled` in evidence-baseline.json.",
            entry_type
        );
    }
}

/// The inverse: every type the accumulator has a match arm for must be in
/// `handled` or `handled_as_progress` — NOT in `silently_ignored`.
/// This catches both "added a match arm, forgot to update the baseline"
/// AND "baseline says silently_ignored but parser actually handles it"
/// (the exact bug from 2026-04-02).
#[test]
fn parser_match_arms_are_in_baseline_handled() {
    let baseline_json =
        std::fs::read_to_string(baseline_path()).expect("evidence-baseline.json must exist");
    let baseline: serde_json::Value = serde_json::from_str(&baseline_json).unwrap();

    let handled: HashSet<String> = baseline["top_level_types"]["handled"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            baseline["top_level_types"]["handled_as_progress"]
                .as_array()
                .unwrap()
                .iter(),
        )
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    let silently_ignored: HashSet<String> = baseline["top_level_types"]["silently_ignored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    // These are the types the accumulator has explicit match arms for.
    // IMPORTANT: when you add a new match arm to BlockAccumulator::push_entry,
    // add the type string here too. This test will fail if you forget to update
    // the baseline.
    let parser_arms = [
        "user",
        "assistant",
        "progress",
        "system",
        "queue-operation",
        "file-history-snapshot",
        "ai-title",
        "last-prompt",
        "worktree-state",
        "pr-link",
        "custom-title",
        "agent-name",
    ];

    for arm in &parser_arms {
        assert!(
            !silently_ignored.contains(*arm),
            "Parser has a match arm for '{}' but baseline says it's silently_ignored. \
             Move it to `handled` in evidence-baseline.json.",
            arm
        );
        assert!(
            handled.contains(*arm),
            "Parser has a match arm for '{}' but it's not in baseline `handled` or `handled_as_progress`. \
             Add it to the correct category in evidence-baseline.json.",
            arm
        );
    }
}
