use claude_view_core::live_parser::{
    parse_single_line, progress_message_content_fallback_count,
    reset_progress_message_content_fallback_count, LineType, TailFinders,
};

fn parse(raw: &[u8], finders: &TailFinders) -> claude_view_core::live_parser::LiveLine {
    parse_single_line(raw, finders)
}

#[test]
fn exact_type_classification_never_uses_substring_inference() {
    let finders = TailFinders::new();

    let non_exact = parse(
        br#"{"type":"assistant_progress","message":{"role":"assistant","content":[{"type":"text","text":"mentions user progress result system assistant"}]},"toolUseResult":{"status":"completed"}}"#,
        &finders,
    );
    assert_eq!(non_exact.line_type, LineType::Other);

    let missing_type = parse(
        br#"{"message":{"role":"user","content":"hello"},"toolUseResult":{"status":"completed"}}"#,
        &finders,
    );
    assert_eq!(missing_type.line_type, LineType::Other);

    let non_string_type = parse(
        br#"{"type":{"kind":"user"},"message":{"role":"user","content":"hello"}}"#,
        &finders,
    );
    assert_eq!(non_string_type.line_type, LineType::Other);

    let exact_user = parse(
        br#"{"type":"user","message":{"role":"assistant","content":"hello"}}"#,
        &finders,
    );
    assert_eq!(exact_user.line_type, LineType::User);
}

#[test]
fn progress_extraction_uses_primary_path_then_fallback_with_counter() {
    let finders = TailFinders::new();

    reset_progress_message_content_fallback_count();
    let before = progress_message_content_fallback_count();

    let primary = parse(
        br#"{"type":"progress","parentToolUseID":"toolu_primary","data":{"type":"agent_progress","agentId":"agent_primary","message":{"message":{"content":[{"type":"tool_use","name":"PrimaryTool","input":{}}]},"content":[{"type":"tool_use","name":"FallbackToolShouldNotWin","input":{}}]}}}"#,
        &finders,
    );
    assert_eq!(primary.line_type, LineType::Progress);
    let primary_progress = primary
        .sub_agent_progress
        .expect("primary path should parse");
    assert_eq!(
        primary_progress.current_tool.as_deref(),
        Some("PrimaryTool")
    );

    let fallback = parse(
        br#"{"type":"progress","parentToolUseID":"toolu_fallback","data":{"type":"agent_progress","agentId":"agent_fallback","message":{"content":[{"type":"tool_use","name":"FallbackTool","input":{}}]}}}"#,
        &finders,
    );
    let fallback_progress = fallback
        .sub_agent_progress
        .expect("fallback path should parse");
    assert_eq!(
        fallback_progress.current_tool.as_deref(),
        Some("FallbackTool")
    );

    let after = progress_message_content_fallback_count();
    assert!(
        after >= before + 1,
        "fallback counter must increment when fallback path is used"
    );
}

#[test]
fn tool_use_result_polymorphism_is_handled_without_silent_invalid_coercion() {
    let finders = TailFinders::new();

    let object_variant = parse(
        br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_obj","content":"done"}]},"toolUseResult":{"status":"completed","agentId":"agent_obj","totalDurationMs":42,"totalToolUseCount":3,"usage":{"input_tokens":10,"output_tokens":5}}}"#,
        &finders,
    );
    let object_result = object_variant
        .sub_agent_result
        .expect("object variant should parse");
    assert_eq!(object_result.tool_use_id, "toolu_obj");
    assert_eq!(object_result.status, "completed");
    assert_eq!(object_result.agent_id.as_deref(), Some("agent_obj"));
    assert_eq!(object_result.total_duration_ms, Some(42));
    assert_eq!(object_result.total_tool_use_count, Some(3));
    assert_eq!(object_result.usage_input_tokens, Some(10));
    assert_eq!(object_result.usage_output_tokens, Some(5));

    let string_variant = parse(
        br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_str","content":"done"}]},"toolUseResult":"failed"}"#,
        &finders,
    );
    let string_result = string_variant
        .sub_agent_result
        .expect("string variant should parse");
    assert_eq!(string_result.tool_use_id, "toolu_str");
    assert_eq!(string_result.status, "failed");
    assert!(string_result.agent_id.is_none());

    let array_variant = parse(
        br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_arr","content":"done"}]},"toolUseResult":[{"agentId":"agent_arr"},{"status":"killed","totalToolUseCount":7}]}"#,
        &finders,
    );
    let array_result = array_variant
        .sub_agent_result
        .expect("array variant should parse");
    assert_eq!(array_result.tool_use_id, "toolu_arr");
    assert_eq!(array_result.agent_id.as_deref(), Some("agent_arr"));
    assert_eq!(array_result.status, "killed");
    assert_eq!(array_result.total_tool_use_count, Some(7));

    let invalid_number = parse(
        br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_num","content":"done"}]},"toolUseResult":123}"#,
        &finders,
    );
    assert!(
        invalid_number.sub_agent_result.is_none(),
        "invalid numeric shape must not be silently coerced"
    );

    let invalid_object = parse(
        br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_unknown","content":"done"}]},"toolUseResult":{"unexpected":"value"}}"#,
        &finders,
    );
    assert!(
        invalid_object.sub_agent_result.is_none(),
        "unknown object shape must not be silently coerced"
    );
}
