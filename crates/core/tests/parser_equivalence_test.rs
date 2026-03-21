//! Parser equivalence test: verifies that parser.rs and block_accumulator
//! produce semantically equivalent output for the same JSONL input.
//! Prevents dual-parser divergence from recurring.

use claude_view_core::block_accumulator::parse_session_as_blocks;
use claude_view_core::block_types::*;
use claude_view_core::parser::parse_session;
use claude_view_core::types::{ParsedSession, Role};
use std::path::PathBuf;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/block_accumulator")
}

// ── Counting helpers for parser.rs output ───────────────────────────

/// Count real user prompts (not tool_result messages).
fn count_parser_text_users(session: &ParsedSession) -> usize {
    session
        .messages
        .iter()
        .filter(|m| m.role == Role::User)
        .count()
}

/// Count assistant + tool_use messages (both map to AssistantBlock).
fn count_parser_assistants(session: &ParsedSession) -> usize {
    session
        .messages
        .iter()
        .filter(|m| m.role == Role::Assistant || m.role == Role::ToolUse)
        .count()
}

/// Count individual tool calls across all messages.
fn count_parser_tool_calls(session: &ParsedSession) -> usize {
    session
        .messages
        .iter()
        .filter_map(|m| m.tool_calls.as_ref())
        .map(|tc| tc.len())
        .sum()
}

// ── Counting helpers for block_accumulator output ───────────────────

fn count_block_users(blocks: &[ConversationBlock]) -> usize {
    blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::User(_)))
        .count()
}

fn count_block_assistants(blocks: &[ConversationBlock]) -> usize {
    blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::Assistant(_)))
        .count()
}

fn count_block_tool_executions(blocks: &[ConversationBlock]) -> usize {
    blocks
        .iter()
        .filter_map(|b| {
            if let ConversationBlock::Assistant(a) = b {
                Some(a)
            } else {
                None
            }
        })
        .flat_map(|a| a.segments.iter())
        .filter(|s| matches!(s, AssistantSegment::Tool { .. }))
        .count()
}

// ── Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn simple_turn_equivalence() {
    let path = fixtures_path().join("simple_turn.jsonl");

    // Parse via parser.rs (async)
    let parsed = parse_session(&path).await.unwrap();

    // Parse via block_accumulator (sync)
    let content = std::fs::read_to_string(&path).unwrap();
    let blocks = parse_session_as_blocks(&content);

    // User counts: parser text-users vs block UserBlocks.
    // parser.rs creates separate ToolResult-role messages for tool_result entries,
    // so we only compare real User-role messages.
    let parser_users = count_parser_text_users(&parsed);
    let block_users = count_block_users(&blocks);
    assert_eq!(
        parser_users, block_users,
        "User count mismatch: parser={parser_users} block_acc={block_users}"
    );

    // Both should produce assistant content.
    let parser_assistants = count_parser_assistants(&parsed);
    let block_assistants = count_block_assistants(&blocks);
    assert!(
        block_assistants > 0,
        "block_accumulator should produce AssistantBlocks"
    );
    assert!(
        parser_assistants > 0,
        "parser should produce assistant messages"
    );

    // Tool execution counts should match tool call counts.
    let parser_tools = count_parser_tool_calls(&parsed);
    let block_tools = count_block_tool_executions(&blocks);
    assert_eq!(
        parser_tools, block_tools,
        "Tool count mismatch: parser={parser_tools} block_acc={block_tools}"
    );

    // Field value check: first UserBlock text should match first User message content.
    let first_user_block = blocks.iter().find_map(|b| {
        if let ConversationBlock::User(u) = b {
            Some(u)
        } else {
            None
        }
    });
    let first_user_msg = parsed.messages.iter().find(|m| m.role == Role::User);

    if let (Some(block), Some(msg)) = (first_user_block, first_user_msg) {
        assert_eq!(
            block.text.trim(),
            msg.content.trim(),
            "First user text content mismatch between parser and block_accumulator"
        );
    }
}

#[tokio::test]
async fn multi_turn_equivalence() {
    let path = fixtures_path().join("multi_turn.jsonl");
    if !path.exists() {
        return; // skip if fixture doesn't exist
    }

    let parsed = parse_session(&path).await.unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    let blocks = parse_session_as_blocks(&content);

    // Both should produce content.
    assert!(
        !parsed.messages.is_empty(),
        "parser should produce messages"
    );
    assert!(
        !blocks.is_empty(),
        "block_accumulator should produce blocks"
    );

    // block_accumulator should produce TurnBoundaryBlocks.
    let boundaries = blocks
        .iter()
        .filter(|b| matches!(b, ConversationBlock::TurnBoundary(_)))
        .count();
    assert!(
        boundaries >= 2,
        "Expected at least 2 TurnBoundaryBlocks for multi_turn, got {boundaries}"
    );

    // User and assistant counts should be non-zero in both.
    let parser_users = count_parser_text_users(&parsed);
    let block_users = count_block_users(&blocks);
    assert!(
        parser_users > 0 && block_users > 0,
        "Both parsers should find users"
    );
    assert_eq!(
        parser_users, block_users,
        "Multi-turn user count mismatch: parser={parser_users} block_acc={block_users}"
    );

    let block_assistants = count_block_assistants(&blocks);
    assert!(
        block_assistants > 0,
        "block_accumulator should produce AssistantBlocks for multi_turn"
    );
}
