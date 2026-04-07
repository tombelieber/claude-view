// crates/core/src/classification/tests.rs
//! Tests for the classification module.

use super::*;

#[test]
fn test_category_l1_roundtrip() {
    // New canonical form
    assert_eq!(CategoryL1::parse("code_work"), Some(CategoryL1::Code));
    assert_eq!(CategoryL1::parse("support_work"), Some(CategoryL1::Support));
    assert_eq!(
        CategoryL1::parse("thinking_work"),
        Some(CategoryL1::Thinking)
    );
    // Backwards compat: old form still accepted
    assert_eq!(CategoryL1::parse("code"), Some(CategoryL1::Code));
    assert_eq!(CategoryL1::parse("support"), Some(CategoryL1::Support));
    assert_eq!(CategoryL1::parse("thinking"), Some(CategoryL1::Thinking));
    assert_eq!(CategoryL1::parse("invalid"), None);
    // as_str outputs the new canonical form
    assert_eq!(CategoryL1::Code.as_str(), "code_work");
    assert_eq!(CategoryL1::Support.as_str(), "support_work");
    assert_eq!(CategoryL1::Thinking.as_str(), "thinking_work");
}

#[test]
fn test_category_l2_parent() {
    assert_eq!(CategoryL2::Feature.parent_l1(), CategoryL1::Code);
    assert_eq!(CategoryL2::Bugfix.parent_l1(), CategoryL1::Code);
    assert_eq!(CategoryL2::Docs.parent_l1(), CategoryL1::Support);
    assert_eq!(CategoryL2::Planning.parent_l1(), CategoryL1::Thinking);
}

#[test]
fn test_category_l3_parent() {
    assert_eq!(CategoryL3::NewComponent.parent_l2(), CategoryL2::Feature);
    assert_eq!(CategoryL3::ErrorFix.parent_l2(), CategoryL2::Bugfix);
    assert_eq!(CategoryL3::ReadmeGuides.parent_l2(), CategoryL2::Docs);
    assert_eq!(
        CategoryL3::SystemDesign.parent_l2(),
        CategoryL2::Architecture
    );
}

#[test]
fn test_parse_category_string_valid() {
    assert_eq!(
        parse_category_string("code/feature/new-component"),
        Some(("code", "feature", "new-component"))
    );
    assert_eq!(
        parse_category_string("thinking/explanation/debug-investigation"),
        Some(("thinking", "explanation", "debug-investigation"))
    );
    assert_eq!(
        parse_category_string("support/ops/ci-cd"),
        Some(("support", "ops", "ci-cd"))
    );
}

#[test]
fn test_parse_category_string_invalid() {
    assert_eq!(parse_category_string("code/feature"), None);
    assert_eq!(parse_category_string("invalid/feature/new-component"), None);
    assert_eq!(parse_category_string("code/invalid/new-component"), None);
    assert_eq!(parse_category_string("code/feature/invalid-thing"), None);
    assert_eq!(parse_category_string(""), None);
}

#[test]
fn test_truncate_preview_short() {
    let short = "Hello world";
    assert_eq!(truncate_preview(short), short);
}

#[test]
fn test_truncate_preview_long() {
    let long = "a ".repeat(1500);
    let result = truncate_preview(&long);
    assert!(result.len() <= 2000);
    assert!(result.ends_with("... [truncated]"));
}

#[test]
fn test_build_batch_prompt() {
    let inputs = vec![
        ClassificationInput {
            session_id: "sess-1".to_string(),
            preview: "Fix the login bug".to_string(),
            skills_used: vec!["/commit".to_string()],
        },
        ClassificationInput {
            session_id: "sess-2".to_string(),
            preview: "Create a new dashboard".to_string(),
            skills_used: vec![],
        },
    ];

    let prompt = build_batch_prompt(&inputs);
    assert!(prompt.contains("sess-1"));
    assert!(prompt.contains("Fix the login bug"));
    assert!(prompt.contains("/commit"));
    assert!(prompt.contains("sess-2"));
    assert!(prompt.contains("(none)"));
    assert!(prompt.contains("2 sessions"));
}

#[test]
fn test_parse_batch_response_valid() {
    let json = r#"{
        "classifications": [
            {
                "sessionId": "sess-1",
                "category": "code/bugfix/error-fix",
                "confidence": 0.92,
                "reasoning": "User fixing a login bug"
            },
            {
                "sessionId": "sess-2",
                "category": "code/feature/new-component",
                "confidence": 0.85,
                "reasoning": "Creating new dashboard"
            }
        ]
    }"#;

    let results = parse_batch_response(json).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].session_id, "sess-1");
    assert_eq!(results[0].l1, "code");
    assert_eq!(results[0].l2, "bugfix");
    assert_eq!(results[0].l3, "error-fix");
    assert!((results[0].confidence - 0.92).abs() < f64::EPSILON);
}

#[test]
fn test_parse_batch_response_with_markdown() {
    let json = "```json\n{\"classifications\":[{\"sessionId\":\"s1\",\"category\":\"code/feature/integration\",\"confidence\":0.8,\"reasoning\":\"test\"}]}\n```";

    let results = parse_batch_response(json).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].l2, "feature");
}

#[test]
fn test_parse_batch_response_invalid_category() {
    let json = r#"{
        "classifications": [
            {
                "sessionId": "sess-1",
                "category": "invalid/category/path",
                "confidence": 0.5,
                "reasoning": "test"
            }
        ]
    }"#;

    let results = parse_batch_response(json).unwrap();
    assert_eq!(results.len(), 0); // Invalid categories are skipped
}

#[test]
fn test_parse_batch_response_claude_cli_wrapper() {
    let inner = r#"{"classifications":[{"sessionId":"s1","category":"thinking/planning/brainstorming","confidence":0.9,"reasoning":"idea exploration"}]}"#;
    let json = format!(r#"{{"result": "{}"}}"#, inner.replace('"', "\\\""));

    let results = parse_batch_response(&json).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].l1, "thinking");
}

#[test]
fn test_all_30_categories_parse() {
    let categories = [
        "code/feature/new-component",
        "code/feature/add-functionality",
        "code/feature/integration",
        "code/bugfix/error-fix",
        "code/bugfix/logic-fix",
        "code/bugfix/performance-fix",
        "code/refactor/cleanup",
        "code/refactor/pattern-migration",
        "code/refactor/dependency-update",
        "code/testing/unit-tests",
        "code/testing/integration-tests",
        "code/testing/test-fixes",
        "support/docs/code-comments",
        "support/docs/readme-guides",
        "support/docs/api-docs",
        "support/config/env-setup",
        "support/config/build-tooling",
        "support/config/dependencies",
        "support/ops/ci-cd",
        "support/ops/deployment",
        "support/ops/monitoring",
        "thinking/planning/brainstorming",
        "thinking/planning/design-doc",
        "thinking/planning/task-breakdown",
        "thinking/explanation/code-understanding",
        "thinking/explanation/concept-learning",
        "thinking/explanation/debug-investigation",
        "thinking/architecture/system-design",
        "thinking/architecture/data-modeling",
        "thinking/architecture/api-design",
    ];

    for cat in &categories {
        assert!(
            parse_category_string(cat).is_some(),
            "Category '{}' should be valid",
            cat
        );
    }
}

#[test]
fn test_confidence_clamping() {
    let json = r#"{
        "classifications": [
            {
                "sessionId": "s1",
                "category": "code/feature/integration",
                "confidence": 1.5,
                "reasoning": "over-confident"
            }
        ]
    }"#;

    let results = parse_batch_response(json).unwrap();
    assert_eq!(results[0].confidence, 1.0);
}
