// crates/core/src/invocation/tests/edge_cases.rs
//
// Additional edge cases, Agent tool, and user-level skill tests.

use super::*;
use crate::registry::InvocableKind;

// -----------------------------------------------------------------------
// Additional edge cases
// -----------------------------------------------------------------------

#[test]
fn test_all_builtin_tools_classify_as_valid() {
    use crate::registry::BUILTIN_TOOLS;

    let registry = builtin_only_registry();
    for &tool in BUILTIN_TOOLS {
        // Skip Task and Agent since they have special handling
        if tool == "Task" || tool == "Agent" {
            continue;
        }
        let result = classify_tool_use(tool, &None, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: format!("builtin:{tool}"),
                kind: InvocableKind::BuiltinTool,
            },
            "Built-in tool '{tool}' should classify as Valid"
        );
    }
}

#[test]
fn test_new_builtin_tools_classify_as_valid() {
    let registry = builtin_only_registry();
    for tool in [
        "TodoWrite",
        "SendMessage",
        "TeamCreate",
        "TeamDelete",
        "CronCreate",
    ] {
        let result = classify_tool_use(tool, &None, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: format!("builtin:{tool}"),
                kind: InvocableKind::BuiltinTool,
            },
            "{tool} should classify as valid builtin"
        );
    }
}

#[test]
fn test_skill_with_bare_name_lookup() {
    let registry = registry_with_skill("superpowers", "brainstorming");
    // Use bare name (without plugin prefix)
    let input = Some(serde_json::json!({"skill": "brainstorming"}));
    let result = classify_tool_use("Skill", &input, &registry);
    assert_eq!(
        result,
        ClassifyResult::Valid {
            invocable_id: "superpowers:brainstorming".into(),
            kind: InvocableKind::Skill,
        }
    );
}

#[test]
fn test_skill_input_missing_skill_field() {
    let registry = builtin_only_registry();
    let input = Some(serde_json::json!({"args": "something"}));
    let result = classify_tool_use("Skill", &input, &registry);
    assert_eq!(result, ClassifyResult::Ignored);
}

#[test]
fn test_is_builtin_agent_known() {
    assert!(is_builtin_agent("Bash"));
    assert!(is_builtin_agent("general-purpose"));
    assert!(is_builtin_agent("Explore"));
    assert!(is_builtin_agent("Plan"));
    assert!(is_builtin_agent("statusline-setup"));
    assert!(is_builtin_agent("claude-code-guide"));
}

#[test]
fn test_is_builtin_agent_unknown() {
    assert!(!is_builtin_agent("feature-dev:code-reviewer"));
    assert!(!is_builtin_agent("custom-agent"));
    assert!(!is_builtin_agent(""));
}

#[test]
fn test_raw_tool_use_struct() {
    let raw = RawToolUse {
        name: "Bash".to_string(),
        input: Some(serde_json::json!({"command": "ls"})),
    };
    assert_eq!(raw.name, "Bash");
    assert!(raw.input.is_some());

    let raw_none = RawToolUse {
        name: "Read".to_string(),
        input: None,
    };
    assert!(raw_none.input.is_none());
}

// -----------------------------------------------------------------------
// Agent tool classification (Task renamed to Agent in ~v0.10)
// -----------------------------------------------------------------------

#[test]
fn test_agent_tool_extracts_subagent_type() {
    let registry = builtin_only_registry();
    let input = Some(serde_json::json!({
        "subagent_type": "Explore",
        "description": "Search codebase",
        "prompt": "Find all uses of..."
    }));
    let result = classify_tool_use("Agent", &input, &registry);
    assert_eq!(
        result,
        ClassifyResult::Valid {
            invocable_id: "builtin:Explore".to_string(),
            kind: InvocableKind::BuiltinTool,
        },
        "Agent tool should extract subagent_type like Task does"
    );
}

#[test]
fn test_agent_tool_no_input_is_ignored() {
    let registry = builtin_only_registry();
    let result = classify_tool_use("Agent", &None, &registry);
    assert_eq!(result, ClassifyResult::Ignored);
}

#[test]
fn test_agent_tool_no_subagent_type_is_ignored() {
    let registry = builtin_only_registry();
    let input = Some(serde_json::json!({"description": "do something"}));
    let result = classify_tool_use("Agent", &input, &registry);
    assert_eq!(result, ClassifyResult::Ignored);
}

#[test]
fn test_agent_tool_plugin_subagent_type() {
    let registry = registry_with_agent("feature-dev", "code-reviewer");
    let input = Some(serde_json::json!({
        "subagent_type": "feature-dev:code-reviewer",
        "description": "Review code",
        "prompt": "Check for bugs"
    }));
    let result = classify_tool_use("Agent", &input, &registry);
    assert!(
        matches!(result, ClassifyResult::Valid { ref invocable_id, .. } if invocable_id.contains("code-reviewer")),
        "Agent tool should route plugin subagent_types through registry"
    );
}

#[test]
fn test_mcp_rejected_when_not_in_registry() {
    let registry = builtin_only_registry();
    let result = classify_tool_use(
        "mcp__plugin_supabase_supabase__execute_sql",
        &None,
        &registry,
    );
    assert_eq!(
        result,
        ClassifyResult::Rejected {
            raw_value: "mcp__plugin_supabase_supabase__execute_sql".into(),
            reason: "not_in_registry".into(),
        }
    );
}

// -----------------------------------------------------------------------
// User-level custom skill classification
// -----------------------------------------------------------------------

#[test]
fn test_skill_with_user_level_lookup() {
    // Build a registry that includes a user-level skill
    let registry = tokio_test::block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create user-level skill
        let skill_dir = claude_dir.join("skills/prove-it");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Prove It\nTest.").unwrap();

        crate::registry::build_registry(claude_dir).await
    });

    // Classify a Skill tool_use with bare name "prove-it"
    let input = Some(serde_json::json!({"skill": "prove-it"}));
    let result = classify_tool_use("Skill", &input, &registry);
    assert_eq!(
        result,
        ClassifyResult::Valid {
            invocable_id: "user:prove-it".into(),
            kind: InvocableKind::Skill,
        }
    );

    // Also works with qualified name
    let input2 = Some(serde_json::json!({"skill": "user:prove-it"}));
    let result2 = classify_tool_use("Skill", &input2, &registry);
    assert_eq!(
        result2,
        ClassifyResult::Valid {
            invocable_id: "user:prove-it".into(),
            kind: InvocableKind::Skill,
        }
    );
}
