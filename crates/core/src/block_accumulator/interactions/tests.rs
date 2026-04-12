// crates/core/src/block_accumulator/interactions/tests.rs
//
// Tests for historical InteractionBlock synthesis.

use super::synthesizer::synthesize_historical_interactions;
use crate::block_types::{
    AssistantBlock, AssistantSegment, ConversationBlock, HistoricalSource, InteractionBlock,
    InteractionVariant, SystemBlock, SystemVariant, ToolExecution, ToolResult, ToolStatus,
    TurnBoundaryBlock,
};
use serde_json::{json, Value};
use std::collections::HashMap;

fn assistant_block(id: &str, segments: Vec<AssistantSegment>) -> ConversationBlock {
    ConversationBlock::Assistant(AssistantBlock {
        id: id.into(),
        segments,
        thinking: None,
        streaming: false,
        timestamp: None,
        parent_uuid: None,
        is_sidechain: None,
        agent_id: None,
        raw_json: None,
    })
}

fn tool_segment(
    tool_name: &str,
    tool_use_id: &str,
    input: Value,
    result: Option<&str>,
) -> AssistantSegment {
    AssistantSegment::Tool {
        execution: ToolExecution {
            tool_name: tool_name.into(),
            tool_input: input,
            tool_use_id: tool_use_id.into(),
            parent_tool_use_id: None,
            result: result.map(|r| ToolResult {
                output: r.to_string(),
                is_error: false,
                is_replay: false,
            }),
            progress: None,
            summary: None,
            status: ToolStatus::Complete,
            category: None,
            live_output: None,
            duration: None,
        },
    }
}

fn turn_boundary(id: &str) -> ConversationBlock {
    ConversationBlock::TurnBoundary(TurnBoundaryBlock {
        id: id.into(),
        success: true,
        total_cost_usd: 0.0,
        num_turns: 1,
        duration_ms: 0,
        duration_api_ms: None,
        usage: HashMap::new(),
        model_usage: HashMap::new(),
        permission_denials: Vec::new(),
        result: None,
        structured_output: None,
        stop_reason: None,
        fast_mode_state: None,
        error: None,
        hook_infos: Vec::new(),
        hook_errors: Vec::new(),
        hook_count: None,
        prevented_continuation: None,
    })
}

fn system_plan_content(id: &str, content: &str) -> ConversationBlock {
    ConversationBlock::System(SystemBlock {
        id: id.into(),
        variant: SystemVariant::PlanContent,
        data: json!({"planContent": content}),
        raw_json: None,
    })
}

fn interactions_only(blocks: &[ConversationBlock]) -> Vec<&InteractionBlock> {
    blocks
        .iter()
        .filter_map(|b| {
            if let ConversationBlock::Interaction(i) = b {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn plan_approved_via_exit_plan_mode_result_string() {
    // Primary detection path: ExitPlanMode tool_result starts with
    // "User has approved your plan" → approved=true.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-1",
                json!({"plan": "# My plan\n\nStep 1"}),
                Some("User has approved your plan. Start with step 1."),
            )],
        ),
        assistant_block(
            "a2",
            vec![tool_segment("Bash", "tu-2", json!({}), Some("ok"))],
        ),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    let i = interactions[0];
    assert_eq!(i.variant, InteractionVariant::Plan);
    assert_eq!(i.resolved, true);
    assert_eq!(i.data["approved"], true);
    assert_eq!(i.data["planContent"], "# My plan\n\nStep 1");
    assert_eq!(i.data["toolsExecutedAfter"], 1);
    assert_eq!(
        i.historical_source,
        Some(HistoricalSource::SystemVariant),
        "ExitPlanMode path is a strong signal"
    );
    assert_eq!(i.id, "hist-interaction-0");
}

#[test]
fn plan_rejected_via_exit_plan_mode_result_string() {
    // Non-approval: tool_result exists but doesn't start with the
    // approval prefix → approved=false, resolved=true.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-1",
                json!({"plan": "My plan"}),
                Some("User rejected the plan. Try a different approach."),
            )],
        ),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].resolved, true);
    assert_eq!(interactions[0].data["approved"], false);
}

#[test]
fn plan_truncated_session_mid_exit_plan_mode() {
    // Session ended mid-plan: no tool_result attached → conservative
    // resolved=false, approved=false.
    let mut blocks = vec![assistant_block(
        "a1",
        vec![tool_segment(
            "ExitPlanMode",
            "tu-1",
            json!({"plan": "Truncated"}),
            None,
        )],
    )];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].resolved, false);
    assert_eq!(interactions[0].data["approved"], false);
}

#[test]
fn question_resolved_via_ask_user_question_result() {
    let mut blocks = vec![assistant_block(
        "a1",
        vec![tool_segment(
            "AskUserQuestion",
            "tu-1",
            json!({
                "questions": [
                    {"question": "What is the domain?", "options": []}
                ]
            }),
            Some("Web development"),
        )],
    )];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    let i = interactions[0];
    assert_eq!(i.variant, InteractionVariant::Question);
    assert_eq!(i.resolved, true);
    // Data matches live AskQuestion shape — questions array passed through
    assert_eq!(i.data["type"], "ask_question");
    assert_eq!(i.data["requestId"], "");
    let questions = i.data["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["question"], "What is the domain?");
    assert_eq!(
        i.historical_source,
        Some(HistoricalSource::InferredFromToolPattern)
    );
}

#[test]
fn question_unresolved_when_no_tool_result() {
    let mut blocks = vec![assistant_block(
        "a1",
        vec![tool_segment(
            "AskUserQuestion",
            "tu-1",
            json!({"questions": [{"question": "Hanging?"}]}),
            None,
        )],
    )];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].resolved, false);
    // Data has AskQuestion shape with questions array
    let questions = interactions[0].data["questions"].as_array().unwrap();
    assert_eq!(questions[0]["question"], "Hanging?");
}

#[test]
fn question_multi_passes_through_all_questions() {
    // Multi-question payload: the full questions array is passed through
    // so the frontend can render each question with its options.
    let mut blocks = vec![assistant_block(
        "a1",
        vec![tool_segment(
            "AskUserQuestion",
            "tu-1",
            json!({
                "questions": [
                    {"question": "Domain?", "header": "H1", "options": [], "multiSelect": false},
                    {"question": "Language?", "header": "H2", "options": [], "multiSelect": false},
                ]
            }),
            Some("Web / TS"),
        )],
    )];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    let questions = interactions[0].data["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 2);
    assert_eq!(questions[0]["question"], "Domain?");
    assert_eq!(questions[1]["question"], "Language?");
}

#[test]
fn question_legacy_string_array_format_normalized() {
    // Pre-existing fixtures use {questions: ["...", "..."]} rather than
    // the structured form. The synthesizer normalizes each string into
    // a structured object matching AskQuestion shape.
    let mut blocks = vec![assistant_block(
        "a1",
        vec![tool_segment(
            "AskUserQuestion",
            "tu-1",
            json!({"questions": ["Q1", "Q2"]}),
            Some("A"),
        )],
    )];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    let questions = interactions[0].data["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 2);
    // Each string is normalized into {question, header, options, multiSelect}
    assert_eq!(questions[0]["question"], "Q1");
    assert_eq!(questions[0]["header"], "");
    assert_eq!(questions[0]["options"].as_array().unwrap().len(), 0);
    assert_eq!(questions[1]["question"], "Q2");
}

#[test]
fn plan_content_system_block_lookahead_path() {
    // Secondary detection path: SystemVariant::PlanContent entry with
    // subsequent tool_uses before the next TurnBoundary → approved.
    let mut blocks = vec![
        system_plan_content("s1", "# plan from system"),
        assistant_block(
            "a1",
            vec![tool_segment("Bash", "tu-1", json!({}), Some("ok"))],
        ),
        assistant_block(
            "a2",
            vec![tool_segment("Read", "tu-2", json!({}), Some("ok"))],
        ),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].data["approved"], true);
    assert_eq!(interactions[0].data["toolsExecutedAfter"], 2);
}

#[test]
fn plan_content_system_block_no_tools_means_rejected() {
    let mut blocks = vec![system_plan_content("s1", "# plan"), turn_boundary("tb1")];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].data["approved"], false);
    assert_eq!(interactions[0].data["toolsExecutedAfter"], 0);
}

#[test]
fn no_interactions_synthesised_on_normal_session() {
    // False-positive guard: unrelated tool_uses + no PlanContent entry
    // produce zero InteractionBlocks.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![
                tool_segment("Bash", "tu-1", json!({}), Some("ok")),
                tool_segment("Read", "tu-2", json!({}), Some("ok")),
                tool_segment("Write", "tu-3", json!({}), Some("ok")),
            ],
        ),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    assert!(interactions_only(&blocks).is_empty());
}

#[test]
fn determinism_same_input_same_ids() {
    // Same input produces identical IDs and ordering across runs.
    let build = || {
        vec![
            assistant_block(
                "a1",
                vec![tool_segment(
                    "AskUserQuestion",
                    "tu-1",
                    json!({"questions": [{"question": "Q1"}]}),
                    Some("A1"),
                )],
            ),
            assistant_block(
                "a2",
                vec![tool_segment(
                    "ExitPlanMode",
                    "tu-2",
                    json!({"plan": "P"}),
                    Some("User has approved your plan"),
                )],
            ),
            turn_boundary("tb1"),
        ]
    };
    let mut b1 = build();
    let mut b2 = build();
    synthesize_historical_interactions(&mut b1);
    synthesize_historical_interactions(&mut b2);
    let ids1: Vec<_> = interactions_only(&b1)
        .iter()
        .map(|i| i.id.clone())
        .collect();
    let ids2: Vec<_> = interactions_only(&b2)
        .iter()
        .map(|i| i.id.clone())
        .collect();
    assert_eq!(ids1, ids2);
    assert_eq!(ids1, vec!["hist-interaction-0", "hist-interaction-1"]);
}

#[test]
fn every_synthesised_block_has_historical_source() {
    // Provenance invariant: no synthesised block leaves historical_source
    // unset. Breaks silently otherwise when UI can't tell live from
    // reconstructed data apart.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-1",
                json!({"plan": "p"}),
                Some("User has approved your plan"),
            )],
        ),
        assistant_block(
            "a2",
            vec![tool_segment(
                "AskUserQuestion",
                "tu-2",
                json!({"questions": [{"question": "Q"}]}),
                Some("A"),
            )],
        ),
        system_plan_content("s1", "# legacy plan"),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    for i in &interactions {
        assert!(
            i.historical_source.is_some(),
            "every synthesised block must carry a HistoricalSource"
        );
    }
}

#[test]
fn insertion_positioned_immediately_after_trigger() {
    // Ordering invariant: InteractionBlock lands right after the
    // triggering block, not at the end of the vec.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-1",
                json!({"plan": "p"}),
                Some("User has approved your plan"),
            )],
        ),
        assistant_block("a2", vec![tool_segment("Bash", "tu-2", json!({}), None)]),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    // Expected order: a1, Interaction, a2, tb1
    assert_eq!(blocks.len(), 4);
    assert!(matches!(blocks[0], ConversationBlock::Assistant(_)));
    assert!(matches!(blocks[1], ConversationBlock::Interaction(_)));
    assert!(matches!(blocks[2], ConversationBlock::Assistant(_)));
    assert!(matches!(blocks[3], ConversationBlock::TurnBoundary(_)));
}

#[test]
fn plan_content_skipped_when_exit_plan_mode_preceded_it() {
    // Dedup guard: if an ExitPlanMode tool_use sat immediately before
    // the SystemVariant::PlanContent entry (i.e. Claude Code mirrors
    // the plan into a system entry after exiting plan mode), do not
    // double-emit. Only the ExitPlanMode-derived InteractionBlock
    // should survive.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-1",
                json!({"plan": "p"}),
                Some("User has approved your plan"),
            )],
        ),
        system_plan_content("s1", "# plan"),
        turn_boundary("tb1"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(
        interactions.len(),
        1,
        "dedup: one InteractionBlock from ExitPlanMode, not two"
    );
    // The surviving InteractionBlock should have tu-1's data.
    assert_eq!(interactions[0].data["planContent"], "p");
}

#[test]
fn multiple_interactions_in_sequence() {
    // Two separate turns, each with its own interaction.
    let mut blocks = vec![
        assistant_block(
            "a1",
            vec![tool_segment(
                "AskUserQuestion",
                "tu-1",
                json!({"questions": [{"question": "Q1"}]}),
                Some("A1"),
            )],
        ),
        turn_boundary("tb1"),
        assistant_block(
            "a2",
            vec![tool_segment(
                "ExitPlanMode",
                "tu-2",
                json!({"plan": "p"}),
                Some("User has approved your plan"),
            )],
        ),
        turn_boundary("tb2"),
    ];
    synthesize_historical_interactions(&mut blocks);
    let interactions = interactions_only(&blocks);
    assert_eq!(interactions.len(), 2);
    assert_eq!(interactions[0].variant, InteractionVariant::Question);
    assert_eq!(interactions[1].variant, InteractionVariant::Plan);
    // IDs are monotonic.
    assert_eq!(interactions[0].id, "hist-interaction-0");
    assert_eq!(interactions[1].id, "hist-interaction-1");
}
