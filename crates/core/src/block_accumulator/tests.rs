// crates/core/src/block_accumulator/tests.rs
//
// Unit and integration tests for BlockAccumulator.

use super::*;
use std::path::PathBuf;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/block_accumulator")
}

#[test]
fn simple_turn_produces_correct_blocks() {
    let fixture = std::fs::read_to_string(fixtures_path().join("simple_turn.jsonl")).unwrap();
    let mut acc = BlockAccumulator::new();
    for line in fixture.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line).unwrap();
        acc.process_line(&entry);
    }
    let blocks = acc.finalize();

    // Expected: UserBlock, AssistantBlock (tool_use text), ProgressBlock,
    //           AssistantBlock (final text), TurnBoundaryBlock
    assert!(
        blocks.len() >= 3,
        "Expected at least 3 blocks, got {}",
        blocks.len()
    );

    // First block should be UserBlock
    assert!(matches!(&blocks[0], ConversationBlock::User(_)));

    // Should contain a ProgressBlock
    assert!(blocks
        .iter()
        .any(|b| matches!(b, ConversationBlock::Progress(_))));

    // Should contain an AssistantBlock with segments
    let assistant = blocks
        .iter()
        .find(|b| matches!(b, ConversationBlock::Assistant(_)));
    assert!(assistant.is_some());
    if let ConversationBlock::Assistant(a) = assistant.unwrap() {
        assert!(!a.segments.is_empty());
    }

    // Last block should be TurnBoundaryBlock
    assert!(matches!(
        blocks.last().unwrap(),
        ConversationBlock::TurnBoundary(_)
    ));
}

#[test]
fn empty_file_produces_no_blocks() {
    let mut acc = BlockAccumulator::new();
    let blocks = acc.finalize();
    assert!(blocks.is_empty());
}

#[test]
fn system_only_session() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "ai-title",
        "sessionId": "sess-1",
        "aiTitle": "Test Session"
    });
    acc.process_line(&entry);
    let entry = serde_json::json!({
        "type": "last-prompt",
        "sessionId": "sess-1",
        "lastPrompt": "hello"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 2);
    assert!(blocks
        .iter()
        .all(|b| matches!(b, ConversationBlock::System(_))));
}

#[test]
fn standalone_progress_entries() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "progress",
        "data": {
            "type": "hook_progress",
            "hookEvent": "PreToolUse",
            "hookName": "live-monitor",
            "command": "echo test",
            "statusMessage": "running"
        },
        "timestamp": "2026-03-21T01:00:00.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], ConversationBlock::Progress(_)));
}

#[test]
fn forked_from_extraction() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "user",
        "uuid": "u-1",
        "message": {"content": [{"type": "text", "text": "hello"}]},
        "forkedFrom": {"sessionId": "parent-sess", "messageUuid": "parent-msg"},
        "timestamp": "2026-03-21T01:00:00.000Z"
    });
    acc.process_line(&entry);
    assert!(acc.forked_from().is_some());
    let fk = acc.forked_from().unwrap();
    assert_eq!(fk["sessionId"], "parent-sess");
}

// ── Extended integration tests (fixture-based) ───────────────

#[test]
fn multi_turn_produces_two_boundaries() {
    let fixture = std::fs::read_to_string(fixtures_path().join("multi_turn.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);
    let boundaries: Vec<_> = blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::TurnBoundary(_)))
        .collect();
    assert_eq!(
        boundaries.len(),
        2,
        "Expected 2 TurnBoundaryBlocks for 2 turns"
    );
}

#[test]
fn ask_user_question_creates_assistant_with_tool() {
    let fixture = std::fs::read_to_string(fixtures_path().join("with_interactions.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);
    let assistant = blocks
        .iter()
        .find(|b| matches!(b, ConversationBlock::Assistant(_)));
    assert!(assistant.is_some(), "Should have an AssistantBlock");
    if let ConversationBlock::Assistant(a) = assistant.unwrap() {
        let has_ask = a.segments.iter().any(|s| {
            if let AssistantSegment::Tool { execution } = s {
                execution.tool_name == "AskUserQuestion"
            } else {
                false
            }
        });
        assert!(has_ask, "Should have AskUserQuestion tool");
    }
}

#[test]
fn notices_from_compact_and_errors() {
    let fixture = std::fs::read_to_string(fixtures_path().join("with_notices.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);
    let notices: Vec<_> = blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::Notice(_)))
        .collect();
    assert!(
        notices.len() >= 2,
        "Expected at least 2 notices (rate_limit + context_compacted), got {}",
        notices.len()
    );
}

#[test]
fn forked_from_extracted_from_fixture() {
    let fixture = std::fs::read_to_string(fixtures_path().join("with_forked_from.jsonl")).unwrap();
    let mut acc = BlockAccumulator::new();
    acc.process_all(&fixture);
    assert!(acc.forked_from().is_some());
    let fk = acc.forked_from().unwrap();
    assert!(fk.get("sessionId").is_some());
    assert!(fk.get("messageUuid").is_some());
    assert_eq!(fk["sessionId"], "parent-session-abc");
    assert_eq!(fk["messageUuid"], "parent-msg-xyz");
}

#[test]
fn orphaned_tool_result_emits_system_block() {
    let fixture =
        std::fs::read_to_string(fixtures_path().join("orphaned_tool_result.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);
    let unknown_systems: Vec<_> = blocks
        .iter()
        .filter(|b| {
            if let ConversationBlock::System(s) = b {
                matches!(s.variant, SystemVariant::Unknown)
            } else {
                false
            }
        })
        .collect();
    assert!(
        !unknown_systems.is_empty(),
        "Expected at least 1 SystemBlock(Unknown) for orphaned tool_result"
    );
}

#[test]
fn system_only_session_no_crash() {
    let fixture = std::fs::read_to_string(fixtures_path().join("system_only.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);
    assert_eq!(
        blocks.len(),
        5,
        "Expected 5 SystemBlocks, got {}",
        blocks.len()
    );
    for block in &blocks {
        assert!(
            matches!(block, ConversationBlock::System(_)),
            "Expected all SystemBlocks"
        );
    }
    let variants: Vec<_> = blocks
        .iter()
        .map(|b| {
            if let ConversationBlock::System(s) = b {
                s.variant
            } else {
                panic!("Expected SystemBlock")
            }
        })
        .collect();
    assert!(variants.contains(&SystemVariant::AiTitle));
    assert!(variants.contains(&SystemVariant::LastPrompt));
    assert!(variants.contains(&SystemVariant::QueueOperation));
    assert!(variants.contains(&SystemVariant::FileHistorySnapshot));
}

/// Regression test: CC CLI writes incremental assistant entries with the
/// same message.id (thinking, text, tool_use as separate lines). The
/// persistent accumulator must merge them into ONE AssistantBlock with
/// all segments, not produce separate blocks that replace each other.
#[test]
fn incremental_assistant_entries_merge_into_one_block() {
    let fixture =
        std::fs::read_to_string(fixtures_path().join("incremental_assistant.jsonl")).unwrap();
    let blocks = parse_session_as_blocks(&fixture);

    // Should have: User, Assistant(msg-inc-001), Assistant(msg-inc-002), TurnBoundary
    let assistants: Vec<_> = blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::Assistant(_)))
        .collect();
    assert_eq!(
        assistants.len(),
        2,
        "Expected 2 AssistantBlocks (msg-inc-001 merged + msg-inc-002), got {}",
        assistants.len()
    );

    // The first assistant block (msg-inc-001) must have ALL segments from the
    // three incremental entries: thinking + text + tool_use
    if let ConversationBlock::Assistant(a) = assistants[0] {
        assert_eq!(a.id, "msg-inc-001");
        assert!(
            a.thinking.is_some(),
            "msg-inc-001 should have thinking from first incremental entry"
        );

        let text_segs: Vec<_> = a
            .segments
            .iter()
            .filter(|s| matches!(s, AssistantSegment::Text { .. }))
            .collect();
        assert!(
            !text_segs.is_empty(),
            "msg-inc-001 should have text segment from second entry"
        );

        let tool_segs: Vec<_> = a
            .segments
            .iter()
            .filter(|s| matches!(s, AssistantSegment::Tool { .. }))
            .collect();
        assert!(
            !tool_segs.is_empty(),
            "msg-inc-001 should have tool segment from third entry"
        );

        // Tool result should be attached (from the user tool_result entry)
        if let AssistantSegment::Tool { execution } = tool_segs[0] {
            assert!(
                execution.result.is_some(),
                "Tool should have result attached from user tool_result"
            );
            assert_eq!(execution.status, ToolStatus::Complete);
        }
    } else {
        panic!("Expected AssistantBlock");
    }
}

/// Test that snapshot() returns in-progress assistant blocks without
/// consuming the accumulator state.
#[test]
fn snapshot_returns_in_progress_assistant() {
    let mut acc = BlockAccumulator::new();

    // Feed first incremental entry (thinking only)
    let entry1 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-snap-001",
            "content": [{"type": "thinking", "thinking": "Let me think..."}]
        },
        "timestamp": "2026-03-23T01:00:00.000Z"
    });
    acc.process_line(&entry1);

    // Snapshot should show the in-progress assistant
    let snap1 = acc.snapshot();
    assert_eq!(snap1.len(), 1);
    if let ConversationBlock::Assistant(a) = &snap1[0] {
        assert_eq!(a.id, "msg-snap-001");
        assert!(a.thinking.is_some());
    } else {
        panic!("Expected AssistantBlock in snapshot");
    }

    // Feed second entry (text) -- same message.id, should accumulate
    let entry2 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-snap-001",
            "content": [{"type": "text", "text": "I'll read the file"}]
        }
    });
    acc.process_line(&entry2);

    // Second snapshot should have BOTH thinking and text
    let snap2 = acc.snapshot();
    assert_eq!(snap2.len(), 1);
    if let ConversationBlock::Assistant(a) = &snap2[0] {
        assert!(a.thinking.is_some());
        assert_eq!(a.segments.len(), 1); // one text segment
    } else {
        panic!("Expected AssistantBlock in second snapshot");
    }

    // Feed third entry (tool_use) -- same message.id
    let entry3 = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-snap-001",
            "content": [{"type": "tool_use", "id": "tu-1", "name": "Read", "input": {}}],
            "stop_reason": "tool_use"
        }
    });
    acc.process_line(&entry3);

    let snap3 = acc.snapshot();
    assert_eq!(snap3.len(), 1);
    if let ConversationBlock::Assistant(a) = &snap3[0] {
        assert!(a.thinking.is_some());
        assert_eq!(a.segments.len(), 2); // text + tool
    } else {
        panic!("Expected AssistantBlock in third snapshot");
    }

    // Accumulator should still be usable (not consumed)
    let entry4 = serde_json::json!({
        "type": "user",
        "uuid": "u-1",
        "message": {"content": [{"type": "tool_result", "tool_use_id": "tu-1", "content": "result", "is_error": false}]}
    });
    acc.process_line(&entry4);
    let snap4 = acc.snapshot();
    // Still 1 block -- tool result attached to existing assistant
    assert_eq!(snap4.len(), 1);
    if let ConversationBlock::Assistant(a) = &snap4[0] {
        let tool_seg = a
            .segments
            .iter()
            .find(|s| matches!(s, AssistantSegment::Tool { .. }));
        if let Some(AssistantSegment::Tool { execution }) = tool_seg {
            assert!(execution.result.is_some(), "Tool result should be attached");
        }
    }
}

#[test]
fn reset_clears_accumulator_state() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-reset",
            "content": [{"type": "text", "text": "before reset"}]
        }
    });
    acc.process_line(&entry);
    assert_eq!(acc.snapshot().len(), 1);

    acc.reset();
    assert!(acc.snapshot().is_empty());
    assert!(acc.finalize().is_empty());
}

#[test]
fn worktree_state_creates_system_block() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "worktree-state",
        "worktreeSession": {
            "originalCwd": "/Users/test/project",
            "worktreePath": "/Users/test/project/.claude/worktrees/feature",
            "worktreeName": "feature",
            "worktreeBranch": "worktree-feature",
            "originalBranch": "main",
            "originalHeadCommit": "abc123"
        },
        "sessionId": "sess-wt-1"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    if let ConversationBlock::System(s) = &blocks[0] {
        assert_eq!(s.variant, SystemVariant::WorktreeState);
        assert_eq!(s.data["worktreeSession"]["worktreeName"], "feature");
    } else {
        panic!("Expected SystemBlock with WorktreeState variant");
    }
}

#[test]
fn parse_session_returns_forked_from() {
    let content = r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hi"}]},"forkedFrom":{"sessionId":"parent-abc","messageUuid":"msg-xyz"},"timestamp":"2026-03-24T01:00:00.000Z"}"#;
    let parsed = super::parse_session(content);
    assert!(!parsed.blocks.is_empty());
    assert!(parsed.forked_from.is_some());
    let fk = parsed.forked_from.unwrap();
    assert_eq!(fk["sessionId"], "parent-abc");
}

#[test]
fn parent_uuid_propagated_to_user_block() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "user",
        "uuid": "u-child",
        "parentUuid": "u-parent",
        "message": {"content": [{"type": "text", "text": "sub-agent message"}]},
        "timestamp": "2026-03-24T01:00:00.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    if let ConversationBlock::User(u) = &blocks[0] {
        assert_eq!(u.parent_uuid, Some("u-parent".to_string()));
    } else {
        panic!("Expected UserBlock");
    }
}

#[test]
fn parent_uuid_propagated_to_assistant_block() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "assistant",
        "parentUuid": "u-parent",
        "message": {
            "id": "msg-child",
            "content": [{"type": "text", "text": "sub-agent reply"}],
            "stop_reason": "end_turn"
        },
        "timestamp": "2026-03-24T01:00:01.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    if let ConversationBlock::Assistant(a) = &blocks[0] {
        assert_eq!(a.parent_uuid, Some("u-parent".to_string()));
    } else {
        panic!("Expected AssistantBlock");
    }
}

#[test]
fn parent_uuid_none_when_absent_from_jsonl() {
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "user",
        "uuid": "u-top",
        "message": {"content": [{"type": "text", "text": "top-level message"}]},
        "timestamp": "2026-03-24T01:00:00.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    if let ConversationBlock::User(u) = &blocks[0] {
        assert_eq!(u.parent_uuid, None);
    } else {
        panic!("Expected UserBlock");
    }
}

#[test]
fn user_message_string_content_creates_block() {
    // Claude CLI sometimes writes message.content as a plain string
    // instead of the array-of-blocks format. Both must produce a UserBlock.
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "user",
        "uuid": "u-str",
        "message": {"content": "commit  n  push"},
        "parentUuid": "p-1",
        "permissionMode": "default",
        "timestamp": "2026-04-04T01:00:00.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(
        blocks.len(),
        1,
        "string-content user message must produce a block"
    );
    if let ConversationBlock::User(u) = &blocks[0] {
        assert_eq!(u.id, "u-str");
        assert_eq!(u.text, "commit  n  push");
        assert_eq!(u.parent_uuid, Some("p-1".to_string()));
        assert_eq!(u.permission_mode, Some("default".to_string()));
    } else {
        panic!("Expected UserBlock, got {:?}", blocks[0]);
    }
}

#[test]
fn user_message_array_content_still_works() {
    // Ensure array-format content (the standard path) is not broken.
    let mut acc = BlockAccumulator::new();
    let entry = serde_json::json!({
        "type": "user",
        "uuid": "u-arr",
        "message": {"content": [{"type": "text", "text": "hello world"}]},
        "timestamp": "2026-04-04T01:00:01.000Z"
    });
    acc.process_line(&entry);
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    if let ConversationBlock::User(u) = &blocks[0] {
        assert_eq!(u.text, "hello world");
    } else {
        panic!("Expected UserBlock");
    }
}

#[path = "tests_regression.rs"]
mod tests_regression;
