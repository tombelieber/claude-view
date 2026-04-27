// crates/core/src/block_accumulator/tests_regression.rs
//
// Regression tests, zero-gap pipeline tests, and team transcript tests
// for BlockAccumulator.

use super::*;

// ── Regression: user message content format handling ──────────────
// Bug (2026-04-04): handle_user() only handled array content,
// silently dropping string-content user messages. These tests
// ensure BOTH formats always produce UserBlocks.

#[test]
fn regression_string_user_then_assistant_ordering_preserved() {
    // Real pattern: user sends text prompt, assistant replies.
    // String-content user must appear BEFORE assistant in block list.
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-1",
        "message": {"content": "commit n push"},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "content": [{"type": "text", "text": "Done."}],
            "stop_reason": "end_turn"
        },
        "timestamp": "2026-04-04T01:00:01.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 2);
    assert!(matches!(&blocks[0], ConversationBlock::User(_)));
    assert!(matches!(&blocks[1], ConversationBlock::Assistant(_)));
}

#[test]
fn regression_mixed_string_and_array_users_in_same_session() {
    // Real pattern: first prompt is string, tool results are arrays,
    // then another string prompt. All must produce correct blocks.
    let mut acc = BlockAccumulator::new();

    // String user prompt
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-1",
        "message": {"content": "check git status"},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));

    // Assistant with tool_use
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-1",
            "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "tu-1", "name": "Bash", "input": {"command": "git status"}}
            ]
        },
        "timestamp": "2026-04-04T01:00:01.000Z"
    }));

    // Array user with tool_result (should NOT create UserBlock)
    acc.process_line(&serde_json::json!({
        "type": "user",
        "message": {"content": [
            {"type": "tool_result", "tool_use_id": "tu-1", "content": "On branch main"}
        ]},
        "timestamp": "2026-04-04T01:00:02.000Z"
    }));

    // Second string user prompt
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-2",
        "message": {"content": "now push it"},
        "timestamp": "2026-04-04T01:00:03.000Z"
    }));

    let blocks = acc.finalize();
    let user_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::User(_)))
        .collect();
    assert_eq!(
        user_blocks.len(),
        2,
        "both string-content user messages must produce blocks; tool_result must not"
    );
    if let ConversationBlock::User(u1) = user_blocks[0] {
        assert_eq!(u1.text, "check git status");
    }
    if let ConversationBlock::User(u2) = user_blocks[1] {
        assert_eq!(u2.text, "now push it");
    }
}

#[test]
fn regression_string_content_empty_string_no_block() {
    // Edge case: empty string content should still create a block
    // (the user explicitly sent an empty message -- don't swallow it).
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-empty",
        "message": {"content": ""},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(
        blocks.len(),
        1,
        "empty string user message should still produce a block"
    );
}

#[test]
fn regression_string_content_whitespace_only() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-ws",
        "message": {"content": "   \n  "},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(
        blocks.len(),
        1,
        "whitespace-only user message should still produce a block"
    );
}

#[test]
fn regression_no_message_field_no_crash() {
    // Defensive: malformed entry with no message field should not panic.
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-no-msg",
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 0, "no message field = no block, no crash");
}

#[test]
fn regression_content_is_number_no_crash() {
    // Defensive: content is neither string nor array.
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-num",
        "message": {"content": 42},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(
        blocks.len(),
        0,
        "non-string non-array content = no block, no crash"
    );
}

#[test]
fn regression_string_content_preserves_all_fields() {
    // Ensure string-content path propagates every field identically
    // to the array-content path.
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-full",
        "message": {"content": "test message"},
        "parentUuid": "parent-1",
        "permissionMode": "bypassPermissions",
        "isSidechain": true,
        "agentId": "agent-xyz",
        "timestamp": "2026-04-04T12:30:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    if let ConversationBlock::User(u) = &blocks[0] {
        assert_eq!(u.id, "u-full");
        assert_eq!(u.text, "test message");
        assert_eq!(u.parent_uuid.as_deref(), Some("parent-1"));
        assert_eq!(u.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(u.is_sidechain, Some(true));
        assert_eq!(u.agent_id.as_deref(), Some("agent-xyz"));
        assert!(u.images.is_empty());
        assert!(u.timestamp > 0.0);
    } else {
        panic!("Expected UserBlock");
    }
}

#[test]
fn regression_snapshot_includes_string_content_users() {
    // The snapshot() path (used by terminal WS block-mode) must also
    // include string-content user blocks -- not just finalize().
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-snap",
        "message": {"content": "live prompt"},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));
    let snap = acc.snapshot();
    assert_eq!(
        snap.len(),
        1,
        "snapshot must include string-content user blocks"
    );
    assert!(matches!(&snap[0], ConversationBlock::User(_)));
}

// ── TDD: Zero-gap pipeline tests (Tasks 1-3) ───────────────────────

#[test]
fn scheduled_task_fire_produces_system_block() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "system",
        "uuid": "s-sched-1",
        "subtype": "scheduled_task_fire",
        "timestamp": "2026-04-06T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ConversationBlock::System(sys) => {
            assert_eq!(sys.variant, SystemVariant::ScheduledTaskFire);
        }
        other => panic!("Expected SystemBlock, got {:?}", other),
    }
}

#[test]
fn away_summary_produces_system_block() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "system",
        "uuid": "s-away-1",
        "subtype": "away_summary",
        "content": "Goal was X. Done. Next: Y.",
        "timestamp": "2026-04-28T01:00:00.000Z"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ConversationBlock::System(sys) => {
            assert_eq!(sys.variant, SystemVariant::AwaySummary);
            assert_eq!(
                sys.data.get("content").and_then(|v| v.as_str()),
                Some("Goal was X. Done. Next: Y.")
            );
        }
        other => panic!("Expected SystemBlock, got {:?}", other),
    }
}

#[test]
fn attachment_top_level_type_produces_system_block() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "attachment",
        "uuid": "att-1",
        "timestamp": "2026-04-06T01:00:00.000Z",
        "attachment": {
            "type": "file",
            "addedNames": ["src/main.rs"],
            "removedNames": [],
            "addedLines": 42
        }
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ConversationBlock::System(sys) => {
            assert_eq!(sys.variant, SystemVariant::Attachment);
        }
        other => panic!("Expected SystemBlock, got {:?}", other),
    }
}

#[test]
fn permission_mode_top_level_type_produces_system_block() {
    let mut acc = BlockAccumulator::new();
    acc.process_line(&serde_json::json!({
        "type": "permission-mode",
        "uuid": "pm-1",
        "timestamp": "2026-04-06T01:00:00.000Z",
        "permissionMode": "bypassPermissions"
    }));
    let blocks = acc.finalize();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ConversationBlock::System(sys) => {
            assert_eq!(sys.variant, SystemVariant::PermissionModeChange);
        }
        other => panic!("Expected SystemBlock, got {:?}", other),
    }
}

// ── Team transcript tests ───────────────────────────────────────

#[test]
fn accumulator_builds_team_transcript_from_teammate_messages() {
    let mut acc = BlockAccumulator::new();

    // TeamCreate tool_use
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-tc",
            "content": [{
                "type": "tool_use",
                "id": "tu_tc",
                "name": "TeamCreate",
                "input": { "team_name": "debate", "description": "Tabs vs spaces" }
            }]
        },
        "teamName": "debate"
    }));

    // User with teammate message
    acc.process_line(&serde_json::json!({
        "type": "user",
        "message": {
            "content": "<teammate-message teammate_id=\"tabs\" color=\"blue\" summary=\"Opening\">\nTabs are better.\n</teammate-message>"
        },
        "teamName": "debate"
    }));

    // Moderator narration
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-narr",
            "content": [{ "type": "text", "text": "Strong opening!" }],
            "stop_reason": "end_turn"
        },
        "teamName": "debate"
    }));

    let blocks = acc.finalize();
    let transcript = blocks
        .iter()
        .find(|b| matches!(b, ConversationBlock::TeamTranscript(_)));
    assert!(
        transcript.is_some(),
        "Should produce a TeamTranscript block"
    );

    if let ConversationBlock::TeamTranscript(t) = transcript.unwrap() {
        assert_eq!(t.team_name, "debate");
        assert_eq!(t.description, "Tabs vs spaces");
        assert!(!t.entries.is_empty());
        assert!(!t.speakers.is_empty());
    }
}

#[test]
fn snapshot_includes_in_progress_transcript() {
    let mut acc = BlockAccumulator::new();

    // TeamCreate
    acc.process_line(&serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-tc",
            "content": [{
                "type": "tool_use",
                "id": "tu_tc",
                "name": "TeamCreate",
                "input": { "team_name": "debate", "description": "Test topic" }
            }]
        },
        "teamName": "debate"
    }));

    // User with teammate message (debate in progress)
    acc.process_line(&serde_json::json!({
        "type": "user",
        "message": {
            "content": "<teammate-message teammate_id=\"agent-1\" color=\"blue\" summary=\"Opening\">\nGreat argument.\n</teammate-message>"
        },
        "teamName": "debate"
    }));

    // snapshot() should include the in-progress transcript (not yet finalized)
    let snap = acc.snapshot();
    let transcript = snap
        .iter()
        .find(|b| matches!(b, ConversationBlock::TeamTranscript(_)));
    assert!(
        transcript.is_some(),
        "snapshot() must include in-progress TeamTranscript blocks"
    );

    if let ConversationBlock::TeamTranscript(t) = transcript.unwrap() {
        assert_eq!(t.team_name, "debate");
        assert!(
            !t.entries.is_empty(),
            "snapshot transcript should have entries"
        );
    }
}

#[test]
fn accumulator_no_transcript_without_team_create() {
    let mut acc = BlockAccumulator::new();

    // Just a regular user message -- no TeamCreate
    acc.process_line(&serde_json::json!({
        "type": "user",
        "uuid": "u-1",
        "message": {"content": "hello"},
        "timestamp": "2026-04-04T01:00:00.000Z"
    }));

    let blocks = acc.finalize();
    assert!(
        !blocks
            .iter()
            .any(|b| matches!(b, ConversationBlock::TeamTranscript(_))),
        "Should NOT produce TeamTranscript without TeamCreate"
    );
}

#[test]
fn real_session_produces_team_transcripts() {
    // Reference session with team debates -- discover path dynamically
    let home = std::env::var("HOME").unwrap_or_default();
    let session_id = "4a1005cb-268a-4fe8-a371-11c2440fc28f";
    let claude_projects = format!("{}/.claude/projects", home);

    // Find the session file under any project directory
    let path = std::fs::read_dir(&claude_projects)
        .ok()
        .and_then(|entries| {
            entries.filter_map(|e| e.ok()).find_map(|entry| {
                let candidate = entry.path().join(format!("{}.jsonl", session_id));
                candidate.exists().then_some(candidate)
            })
        });

    let Some(path) = path else {
        eprintln!(
            "Skipping real session test: session {} not found under {}",
            session_id, claude_projects
        );
        return;
    };

    // Skip if reference file doesn't exist (CI, other machines)
    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!(
            "Skipping real session test: reference JSONL not readable at {:?}",
            path
        );
        return;
    };

    let mut acc = BlockAccumulator::new();
    acc.process_all(&content);
    let blocks = acc.finalize();

    // Should have at least one TeamTranscript block
    let transcripts: Vec<_> = blocks
        .iter()
        .filter_map(|b| {
            if let ConversationBlock::TeamTranscript(t) = b {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    assert!(
        !transcripts.is_empty(),
        "Reference session should produce at least 1 TeamTranscript block, got 0. Total blocks: {}",
        blocks.len()
    );

    // Verify structure of first transcript
    let first = &transcripts[0];
    assert!(!first.team_name.is_empty(), "team_name should not be empty");
    assert!(
        !first.description.is_empty(),
        "description should not be empty"
    );
    assert!(!first.speakers.is_empty(), "should have speakers");
    assert!(!first.entries.is_empty(), "should have entries");

    // Verify we have agent messages (the core content)
    let agent_messages: Vec<_> = first
        .entries
        .iter()
        .filter(|e| matches!(e, TranscriptEntry::AgentMessage { .. }))
        .collect();
    assert!(
        !agent_messages.is_empty(),
        "Transcript should contain at least one agent message"
    );

    // Verify protocol messages were classified separately
    let _protocol_messages: Vec<_> = first
        .entries
        .iter()
        .filter(|e| matches!(e, TranscriptEntry::Protocol { .. }))
        .collect();
    // Protocol messages exist in the reference session

    eprintln!("=== Team Transcript E2E Results ===");
    eprintln!("Total transcripts: {}", transcripts.len());
    for (i, t) in transcripts.iter().enumerate() {
        eprintln!(
            "Transcript {}: team={}, desc={}, speakers={}, entries={}",
            i,
            t.team_name,
            t.description,
            t.speakers.len(),
            t.entries.len()
        );
        for s in &t.speakers {
            eprintln!(
                "  Speaker: {} ({}) stance={:?}",
                s.display_name, s.id, s.stance
            );
        }
        let agents = t
            .entries
            .iter()
            .filter(|e| matches!(e, TranscriptEntry::AgentMessage { .. }))
            .count();
        let mods = t
            .entries
            .iter()
            .filter(|e| matches!(e, TranscriptEntry::ModeratorNarration { .. }))
            .count();
        let protocols = t
            .entries
            .iter()
            .filter(|e| matches!(e, TranscriptEntry::Protocol { .. }))
            .count();
        let relays = t
            .entries
            .iter()
            .filter(|e| matches!(e, TranscriptEntry::ModeratorRelay { .. }))
            .count();
        let tasks = t
            .entries
            .iter()
            .filter(|e| matches!(e, TranscriptEntry::TaskEvent { .. }))
            .count();
        eprintln!(
            "  Entries: {} agent, {} mod, {} protocol, {} relay, {} task",
            agents, mods, protocols, relays, tasks
        );
    }
}
