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

#[test]
fn malformed_json_returns_other_without_panic() {
    let finders = TailFinders::new();
    let line = parse(b"this is not json {{{", &finders);
    assert_eq!(line.line_type, LineType::Other);
    assert!(line.content_preview.is_empty());
    assert!(line.model.is_none());
}

#[test]
fn content_byte_len_tracks_original_size() {
    let finders = TailFinders::new();
    // Short content — no truncation
    let short = parse(
        br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello world"}]}}"#,
        &finders,
    );
    assert_eq!(short.content_byte_len, Some(11));

    // No content — None
    let no_content = parse(
        br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","id":"t1","input":{}}]}}"#,
        &finders,
    );
    assert_eq!(no_content.content_byte_len, None);
}

#[test]
fn task_notification_regex_fallback_handles_whitespace() {
    let finders = TailFinders::new();
    // Whitespace inside tags — strict XML parse fails, regex succeeds
    let line = parse(
        br#"{"type":"user","message":{"role":"user","content":"<task-notification>\n  <task-id> agent_123 </task-id>\n  <status> completed </status>\n</task-notification>"}}"#,
        &finders,
    );
    let notif = line
        .sub_agent_notification
        .expect("should parse via regex fallback");
    assert_eq!(notif.agent_id, "agent_123");
    assert_eq!(notif.status, "completed");
}

#[test]
fn truncated_json_returns_other() {
    let finders = TailFinders::new();
    let line = parse(br#"{"type":"assistant","message":{"role":"assis"#, &finders);
    assert_eq!(line.line_type, LineType::Other);
}

#[test]
fn todo_write_skips_items_without_content() {
    let finders = TailFinders::new();
    let json = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"TodoWrite","id":"toolu_1","input":{"todos":[{"content":"valid task","status":"pending"},{"status":"in_progress"},{"content":"another valid","status":"completed"}]}}]}}"#;
    let line = parse(json, &finders);
    let todos = line.todo_write.expect("should have todo_write");
    assert_eq!(
        todos.len(),
        2,
        "should keep 2 valid items, skip 1 without content"
    );
    assert_eq!(todos[0].content, "valid task");
    assert_eq!(todos[1].content, "another valid");
}

#[test]
fn task_id_assignment_empty_when_no_tool_result_block() {
    let finders = TailFinders::new();
    // Has toolUseResult.task.id but content is plain text (no tool_result block)
    let line = parse(
        br#"{"type":"user","message":{"role":"user","content":"plain text"},"toolUseResult":{"task":{"id":"42"},"status":"completed"}}"#,
        &finders,
    );
    assert!(
        line.task_id_assignments.is_empty(),
        "should skip assignment when no tool_result block matches"
    );
}
