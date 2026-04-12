// crates/core/src/block_accumulator/interactions/synthesizer.rs
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
                            let user_response = execution
                                .result
                                .as_ref()
                                .map(|r: &ToolResult| r.output.clone());
                            let resolved = user_response.is_some();
                            // Build data matching the live AskQuestion shape so the
                            // frontend renderer doesn't crash on `.questions.map()`.
                            // Normalize to [{question, header, options, multiSelect}].
                            let questions = normalize_questions_array(&execution.tool_input);
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
                                        "type": "ask_question",
                                        "requestId": "",
                                        "questions": questions,
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

/// Normalize tool_input.questions into the frontend-compatible shape:
/// `[{question, header, options, multiSelect}]`.
///
/// Handles three cases:
/// 1. Structured objects `[{question: "...", header: "...", options: [...]}]` → pass through
/// 2. Legacy string arrays `["Q1", "Q2"]` → wrap each into a structured object
/// 3. Missing/empty → single entry with empty question text
fn normalize_questions_array(tool_input: &serde_json::Value) -> serde_json::Value {
    let Some(questions) = tool_input.get("questions") else {
        return json!([{
            "question": "",
            "header": "",
            "options": [],
            "multiSelect": false,
        }]);
    };
    let Some(arr) = questions.as_array() else {
        return json!([{
            "question": "",
            "header": "",
            "options": [],
            "multiSelect": false,
        }]);
    };
    let normalized: Vec<serde_json::Value> = arr
        .iter()
        .map(|entry| {
            if entry.is_object() {
                // Already structured — ensure all fields exist with defaults
                let mut obj = entry.clone();
                let map = obj.as_object_mut().unwrap();
                map.entry("header").or_insert_with(|| json!(""));
                map.entry("options").or_insert_with(|| json!([]));
                map.entry("multiSelect").or_insert_with(|| json!(false));
                obj
            } else if let Some(text) = entry.as_str() {
                // Legacy string entry → wrap
                json!({
                    "question": text,
                    "header": "",
                    "options": [],
                    "multiSelect": false,
                })
            } else {
                json!({
                    "question": "",
                    "header": "",
                    "options": [],
                    "multiSelect": false,
                })
            }
        })
        .collect();
    json!(normalized)
}
