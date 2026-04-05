//! Integration test for historical InteractionBlock synthesis.
//!
//! Runs a realistic JSONL fixture (ExitPlanMode approval followed by tool
//! execution) through the full BlockAccumulator pipeline and asserts the
//! synthesizer produces exactly one InteractionBlock::Plan with correct
//! provenance, approval status, and positioning.

use claude_view_core::block_accumulator::BlockAccumulator;
use claude_view_core::block_types::{ConversationBlock, HistoricalSource, InteractionVariant};
use std::path::PathBuf;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("block_accumulator")
}

fn parse_session(fixture_name: &str) -> Vec<ConversationBlock> {
    let fixture =
        std::fs::read_to_string(fixtures_path().join(fixture_name)).expect("fixture exists");
    let mut acc = BlockAccumulator::new();
    acc.process_all(&fixture);
    acc.finalize()
}

#[test]
fn plan_interaction_synthesised_for_exit_plan_mode_turn() {
    let blocks = parse_session("with_plan_interaction.jsonl");

    let interactions: Vec<_> = blocks
        .iter()
        .filter_map(|b| match b {
            ConversationBlock::Interaction(i) => Some(i),
            _ => None,
        })
        .collect();

    assert_eq!(
        interactions.len(),
        1,
        "expected exactly one historical InteractionBlock from the plan turn, got {}: {:#?}",
        interactions.len(),
        interactions
    );

    let plan = interactions[0];
    assert_eq!(plan.variant, InteractionVariant::Plan);
    assert!(plan.resolved);
    assert_eq!(plan.data["approved"], true);
    assert_eq!(
        plan.data["toolsExecutedAfter"], 1,
        "one Bash tool_use ran after the approved plan"
    );
    assert_eq!(
        plan.historical_source,
        Some(HistoricalSource::SystemVariant),
        "ExitPlanMode is a strong detection signal"
    );
    assert!(plan.id.starts_with("hist-interaction-"));
    assert!(
        plan.request_id.is_none(),
        "historical → no sidecar requestId"
    );

    let plan_content = plan.data["planContent"].as_str().unwrap_or("");
    assert!(
        plan_content.contains("Auth Refactor Plan"),
        "plan content from tool_input should flow through, got: {plan_content}"
    );
}

#[test]
fn plan_interaction_positioned_immediately_after_assistant_block() {
    let blocks = parse_session("with_plan_interaction.jsonl");

    // Find the assistant block that carried the ExitPlanMode tool_use.
    let (assistant_idx, _) = blocks
        .iter()
        .enumerate()
        .find(|(_, b)| {
            let ConversationBlock::Assistant(a) = b else {
                return false;
            };
            a.segments.iter().any(|s| {
                let claude_view_core::block_types::AssistantSegment::Tool { execution } = s else {
                    return false;
                };
                execution.tool_name == "ExitPlanMode"
            })
        })
        .expect("fixture has an ExitPlanMode assistant block");

    // The next block should be the synthesised InteractionBlock.
    assert!(
        matches!(
            blocks.get(assistant_idx + 1),
            Some(ConversationBlock::Interaction(_))
        ),
        "InteractionBlock must follow the triggering AssistantBlock directly"
    );
}

#[test]
fn existing_fixture_with_ask_user_question_gets_question_interaction() {
    // Reuse existing fixture with an AskUserQuestion turn to validate the
    // Question detection path end-to-end through the full accumulator.
    let blocks = parse_session("with_interactions.jsonl");

    let interactions: Vec<_> = blocks
        .iter()
        .filter_map(|b| match b {
            ConversationBlock::Interaction(i) => Some(i),
            _ => None,
        })
        .collect();

    assert!(
        !interactions.is_empty(),
        "AskUserQuestion fixture should synthesise at least one Question interaction"
    );
    let q = interactions
        .iter()
        .find(|i| i.variant == InteractionVariant::Question)
        .expect("fixture contains AskUserQuestion → expect Question");

    assert!(q.resolved, "user response attached as tool_result");
    assert_eq!(
        q.historical_source,
        Some(HistoricalSource::InferredFromToolPattern)
    );
    // The fixture uses legacy string-array format: ["What domain...", "What language..."]
    // joined with newline.
    let question = q.data["question"].as_str().unwrap_or("");
    assert!(
        question.contains("domain") || question.contains("Domain"),
        "question text should mention domain, got: {question}"
    );
}
