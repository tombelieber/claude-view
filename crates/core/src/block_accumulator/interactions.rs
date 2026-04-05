// crates/core/src/block_accumulator/interactions.rs
//
// Historical InteractionBlock synthesiser.
//
// Live interactive chat produces InteractionBlocks via the Node.js sidecar.
// Historical JSONL replay has no such pathway — the Rust BlockAccumulator
// never constructs InteractionBlocks, so viewing a historical session loses
// plan-approval and user-question context entirely.
//
// This post-processing pass walks the finalised block vec and inserts
// synthesised `ConversationBlock::Interaction` entries for two well-supported
// patterns: Plan (via ExitPlanMode or SystemVariant::PlanContent) and
// Question (via AskUserQuestion tool_use + tool_result).
//
// Permission and Elicitation are explicitly out of scope: JSONL has no
// `decision` field on hook_progress entries, and `SystemVariant::ElicitationComplete`
// is never emitted by the historical pipeline. Per "Trust Over Accuracy":
// we synthesise only from strong signals.
//
// All synthesised blocks carry `historical_source: Some(_)` so the UI (and
// downstream consumers) can distinguish reconstructed data from live data.

use serde_json::json;

use crate::block_types::{
    AssistantBlock, AssistantSegment, ConversationBlock, HistoricalSource, InteractionBlock,
    InteractionVariant, SystemVariant, ToolResult,
};

/// Insert historical InteractionBlock entries into the finalised block vec.
///
/// Walks `blocks` in order, emits an InteractionBlock immediately after the
/// block that triggered detection. The synthesiser is deterministic: same
/// input produces identical IDs (`hist-interaction-0`, `hist-interaction-1`,
/// ...) and the same ordering.
///
/// This function is cheap: one linear scan, small constant-size lookahead.
pub fn synthesize_historical_interactions(blocks: &mut Vec<ConversationBlock>) {
    // First pass: collect the insertions as (original_index, block) pairs.
    // We can't mutate `blocks` during iteration without invalidating indices.
    let mut insertions: Vec<(usize, InteractionBlock)> = Vec::new();
    let mut counter: usize = 0;
    let mut next_id = || {
        let id = format!("hist-interaction-{counter}");
        counter += 1;
        id
    };

    for (i, block) in blocks.iter().enumerate() {
        match block {
            ConversationBlock::Assistant(assistant) => {
                // Scan tool_use segments for plan/question triggers.
                for segment in &assistant.segments {
                    let AssistantSegment::Tool { execution } = segment else {
                        continue;
                    };
                    match execution.tool_name.as_str() {
                        "ExitPlanMode" => {
                            let plan_content = execution
                                .tool_input
                                .get("plan")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            // Approval resolution: the tool_result string is
                            // authoritative — "User has approved your plan"
                            // prefix means accepted; anything else means the
                            // user rejected or the session truncated.
                            let (resolved, approved) = match &execution.result {
                                Some(result) => {
                                    let approved =
                                        result.output.starts_with("User has approved your plan");
                                    (true, approved)
                                }
                                None => {
                                    // No tool_result yet → session ended
                                    // mid-plan. Conservative: resolved=false,
                                    // approved=false (no positive signal).
                                    (false, false)
                                }
                            };
                            let tools_executed_after =
                                count_tools_after(blocks, i, execution.tool_use_id.as_str());
                            insertions.push((
                                i,
                                InteractionBlock {
                                    id: next_id(),
                                    variant: InteractionVariant::Plan,
                                    request_id: None,
                                    resolved,
                                    historical_source: Some(HistoricalSource::SystemVariant),
                                    data: json!({
                                        "planContent": plan_content,
                                        "approved": approved,
                                        "toolsExecutedAfter": tools_executed_after,
                                    }),
                                },
                            ));
                        }
                        "AskUserQuestion" => {
                            let question = extract_question_text(&execution.tool_input);
                            let user_response = execution
                                .result
                                .as_ref()
                                .map(|r: &ToolResult| r.output.clone());
                            let resolved = user_response.is_some();
                            insertions.push((
                                i,
                                InteractionBlock {
                                    id: next_id(),
                                    variant: InteractionVariant::Question,
                                    request_id: None,
                                    resolved,
                                    historical_source: Some(
                                        HistoricalSource::InferredFromToolPattern,
                                    ),
                                    data: json!({
                                        "question": question,
                                        "userResponse": user_response.unwrap_or_default(),
                                        "toolUseName": "AskUserQuestion",
                                    }),
                                },
                            ));
                        }
                        _ => {}
                    }
                }
            }
            ConversationBlock::System(system) if system.variant == SystemVariant::PlanContent => {
                // Secondary path: legacy `system` entry with `planContent`
                // field. Very rare in practice but `block_accumulator/mod.rs`
                // does emit it, so we cover it for completeness. Skip if the
                // previous assistant block already carried an ExitPlanMode
                // tool_use — we'd otherwise double-count. Safe heuristic:
                // if any insertion already targets index i-1, skip.
                if insertions
                    .iter()
                    .any(|(idx, b)| *idx + 1 == i && matches!(b.variant, InteractionVariant::Plan))
                {
                    continue;
                }
                let plan_content = system
                    .data
                    .get("planContent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // Approval resolution: did any tool_use appear in a later
                // AssistantBlock before the next TurnBoundary? This is the
                // weakest signal in the pipeline but tolerable because this
                // path is rarely hit.
                let tools_executed_after = count_tools_before_next_boundary(blocks, i);
                insertions.push((
                    i,
                    InteractionBlock {
                        id: next_id(),
                        variant: InteractionVariant::Plan,
                        request_id: None,
                        // Session may truncate mid-plan without any next
                        // turn at all — but this path is post-processing
                        // so we always know the full stream; resolved=true.
                        resolved: true,
                        historical_source: Some(HistoricalSource::SystemVariant),
                        data: json!({
                            "planContent": plan_content,
                            "approved": tools_executed_after > 0,
                            "toolsExecutedAfter": tools_executed_after,
                        }),
                    },
                ));
            }
            _ => {}
        }
    }

    if insertions.is_empty() {
        return;
    }

    // Apply insertions in reverse order so earlier indices remain valid.
    // Each insertion places the InteractionBlock at index (original+1), i.e.
    // immediately after the triggering block.
    insertions.sort_by(|a, b| b.0.cmp(&a.0));
    for (idx, interaction) in insertions {
        blocks.insert(idx + 1, ConversationBlock::Interaction(interaction));
    }
}

/// Extract user-visible question text from an AskUserQuestion `tool_input`.
///
/// Claude Code's AskUserQuestion tool uses a structured input:
/// `{questions: [{question, header, options, multiSelect}]}`. We join the
/// `question` fields of every entry with a newline separator so the user
/// can see what was asked even for multi-question payloads. Legacy fixtures
/// use a bare string array; we handle both.
fn extract_question_text(tool_input: &serde_json::Value) -> String {
    let Some(questions) = tool_input.get("questions") else {
        return String::new();
    };
    let Some(arr) = questions.as_array() else {
        return String::new();
    };
    let parts: Vec<String> = arr
        .iter()
        .filter_map(|q| {
            // Structured form: {question: "...", ...}
            if let Some(obj_q) = q.get("question").and_then(|v| v.as_str()) {
                return Some(obj_q.to_string());
            }
            // Legacy fixture form: "..."
            q.as_str().map(String::from)
        })
        .collect();
    parts.join("\n")
}

/// Count tool_use segments (excluding the triggering one) that appear after
/// `trigger_index` up to (but not including) the next TurnBoundaryBlock.
/// Used for ExitPlanMode approval-strength: how many tools ran after the
/// plan. When the tool_result string already tells us approved=true, this
/// is diagnostic; when result is missing, it is the best signal we have.
fn count_tools_after(
    blocks: &[ConversationBlock],
    trigger_index: usize,
    trigger_tool_use_id: &str,
) -> usize {
    let mut count = 0;
    for block in blocks.iter().skip(trigger_index + 1) {
        match block {
            ConversationBlock::TurnBoundary(_) => break,
            ConversationBlock::Assistant(a) => {
                count += count_tool_segments(a, Some(trigger_tool_use_id));
            }
            _ => {}
        }
    }
    count
}

/// Count all tool_use segments in AssistantBlocks between `from_index` and
/// the next TurnBoundaryBlock. No exclusion — used for the PlanContent
/// lookahead path where we don't have a single trigger tool_use id.
fn count_tools_before_next_boundary(blocks: &[ConversationBlock], from_index: usize) -> usize {
    let mut count = 0;
    for block in blocks.iter().skip(from_index + 1) {
        match block {
            ConversationBlock::TurnBoundary(_) => break,
            ConversationBlock::Assistant(a) => count += count_tool_segments(a, None),
            _ => {}
        }
    }
    count
}

fn count_tool_segments(block: &AssistantBlock, exclude_tool_use_id: Option<&str>) -> usize {
    block
        .segments
        .iter()
        .filter(|s| matches!(s, AssistantSegment::Tool { .. }))
        .filter(|s| {
            let AssistantSegment::Tool { execution } = s else {
                return true;
            };
            match exclude_tool_use_id {
                Some(excl) => execution.tool_use_id != excl,
                None => true,
            }
        })
        .count()
}

#[cfg(test)]
// These tests compare serde_json::Value against bool/number literals via
// assert_eq! (e.g., `assert_eq!(i.data["approved"], true)`). Clippy's
// `bool_assert_comparison` lint misreads these as comparing a literal bool
// to a literal bool. Suppressed at module scope.
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;
    use crate::block_types::{
        AssistantBlock, AssistantSegment, SystemBlock, SystemVariant, ToolExecution, ToolResult,
        ToolStatus, TurnBoundaryBlock,
    };
    use serde_json::Value;
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
        assert!(i.resolved);
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
        assert!(interactions[0].resolved);
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
        assert!(!interactions[0].resolved);
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
        assert!(i.resolved);
        assert_eq!(i.data["question"], "What is the domain?");
        assert_eq!(i.data["userResponse"], "Web development");
        assert_eq!(i.data["toolUseName"], "AskUserQuestion");
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
        assert!(!interactions[0].resolved);
        assert_eq!(interactions[0].data["userResponse"], "");
    }

    #[test]
    fn question_multi_joins_with_newlines() {
        // Multi-question payload: join all question fields so the user can
        // see what was asked, even when a single tool_use bundles several.
        let mut blocks = vec![assistant_block(
            "a1",
            vec![tool_segment(
                "AskUserQuestion",
                "tu-1",
                json!({
                    "questions": [
                        {"question": "Domain?"},
                        {"question": "Language?"},
                    ]
                }),
                Some("Web / TS"),
            )],
        )];
        synthesize_historical_interactions(&mut blocks);
        let interactions = interactions_only(&blocks);
        assert_eq!(interactions[0].data["question"], "Domain?\nLanguage?");
    }

    #[test]
    fn question_legacy_string_array_format() {
        // Pre-existing fixtures use {questions: ["...", "..."]} rather than
        // the structured form. Handle both so old JSONL keeps working.
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
        assert_eq!(interactions[0].data["question"], "Q1\nQ2");
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
}
