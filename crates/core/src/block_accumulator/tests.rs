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

/// Parity guard for [`handled_record_types`]: every declared type must, when fed a
/// representative record, produce at least one block (i.e. reach a real dispatch arm,
/// not the `_ => {}` skip). If someone removes a `process_line` arm but leaves the type
/// in the list — or the reverse — this fails. The cc-compat oracle relies on this list
/// being an accurate mirror of the dispatch.
#[test]
fn handled_record_types_each_produces_a_block() {
    // Minimal-but-valid representative record per handled type.
    let rep = |t: &str| -> serde_json::Value {
        match t {
            "user" => serde_json::json!({"type":"user","message":{"role":"user","content":"hi"}}),
            "assistant" => serde_json::json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]}}),
            "progress" => serde_json::json!({"type":"progress","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"h","command":"c","statusMessage":"m"}}),
            "system" => serde_json::json!({"type":"system","subtype":"init"}),
            "queue-operation" => serde_json::json!({"type":"queue-operation","operation":"enqueue","sessionId":"s"}),
            "file-history-snapshot" => serde_json::json!({"type":"file-history-snapshot","snapshot":{},"isSnapshotUpdate":false}),
            "ai-title" => serde_json::json!({"type":"ai-title","aiTitle":"t","sessionId":"s"}),
            "last-prompt" => serde_json::json!({"type":"last-prompt","lastPrompt":"p","sessionId":"s"}),
            "worktree-state" => serde_json::json!({"type":"worktree-state","worktreeSession":{},"sessionId":"s"}),
            "pr-link" => serde_json::json!({"type":"pr-link","prNumber":1,"prUrl":"u","sessionId":"s"}),
            "custom-title" => serde_json::json!({"type":"custom-title","customTitle":"t","sessionId":"s"}),
            "agent-name" => serde_json::json!({"type":"agent-name","agentName":"a","sessionId":"s"}),
            "attachment" => serde_json::json!({"type":"attachment","attachment":{},"sessionId":"s"}),
            "permission-mode" => serde_json::json!({"type":"permission-mode","permissionMode":"default","sessionId":"s"}),
            "mode" => serde_json::json!({"type":"mode","mode":"normal","sessionId":"s"}),
            other => panic!("handled_record_types lists `{other}` but the parity test has no representative record — add one (and confirm process_line dispatches it)"),
        }
    };

    for t in handled_record_types() {
        let mut acc = BlockAccumulator::new();
        acc.process_line(&rep(t));
        let blocks = acc.finalize();
        assert!(
            !blocks.is_empty(),
            "handled type `{t}` produced no block — its process_line arm is missing or broken (it would be silently dropped)"
        );
    }
}

/// An unknown/未來 record type must NOT silently masquerade as handled: it produces no
/// block (forward-compatible skip) AND is absent from the declared surface.
#[test]
fn unknown_record_type_is_not_in_handled_surface_and_drops() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({"type":"some-future-cc-type-2099","sessionId":"s"}));
    assert!(
        acc.finalize().is_empty(),
        "unknown type should skip, not emit"
    );
    assert!(
        !handled_record_types().contains(&"some-future-cc-type-2099"),
        "sanity: the test's synthetic type must not be in the declared surface"
    );
}

/// CC 2.1.191+ fork/rewind lineage pointer: recognized as deliberately-ignored
/// bookkeeping (like `bridge-session`), NOT silently dropped as an unknown gap.
#[test]
fn fork_context_ref_is_intentionally_ignored_not_dropped_as_unknown() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "fork-context-ref",
        "agentId": "ac11b33f323fbbc29",
        "parentSessionId": "9184fcc3-16d2-4aa6-9f91-90f30e61c054",
        "parentLastUuid": "5b2ea007-4ca0-4560-a2ca-48b89fe34cdb",
        "contextLength": 3328
    }));
    // A lineage pointer produces no conversation block …
    assert!(
        acc.finalize().is_empty(),
        "fork-context-ref must not emit a block"
    );
    // … but it is DECLARED-ignored, so the oracle won't flag it as a live gap,
    // and it is NOT in the handled-dispatch surface.
    assert!(intentionally_ignored_record_types().contains(&"fork-context-ref"));
    assert!(!handled_record_types().contains(&"fork-context-ref"));
}

/// CC 2.1.178+ mid-turn model fallback (`{type:"fallback", from, to}` inside
/// assistant `content[]`) must reach the AssistantBlock, not be dropped.
#[test]
fn fallback_content_block_reaches_assistant_block() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {"type": "text", "text": "working on it"},
                {"type": "fallback", "from": {"model": "claude-fable-5"}, "to": {"model": "claude-opus-4-8"}}
            ]
        }
    }));
    let blocks = acc.finalize();
    let assistant = blocks
        .iter()
        .find_map(|b| match b {
            ConversationBlock::Assistant(a) => Some(a),
            _ => None,
        })
        .expect("expected an assistant block");
    assert_eq!(
        assistant.model_fallbacks.len(),
        1,
        "fallback block was dropped"
    );
    assert_eq!(assistant.model_fallbacks[0].from_model, "claude-fable-5");
    assert_eq!(assistant.model_fallbacks[0].to_model, "claude-opus-4-8");
}

#[path = "tests_regression.rs"]
mod tests_regression;
