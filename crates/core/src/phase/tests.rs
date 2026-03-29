use super::*;

// ============================================================================
// is_shipping_cmd tests
// ============================================================================

#[test]
fn shipping_npm_publish() {
    assert!(is_shipping_cmd("npm publish --access public"));
}

#[test]
fn shipping_cargo_publish() {
    assert!(is_shipping_cmd("cargo publish"));
}

#[test]
fn shipping_fly_deploy() {
    assert!(is_shipping_cmd("fly deploy --app my-app"));
}

#[test]
fn not_shipping_deploy_md() {
    assert!(!is_shipping_cmd("cat deploy.md"));
}

#[test]
fn not_shipping_kubectl_get_deploy() {
    assert!(!is_shipping_cmd("kubectl get deploy"));
}

#[test]
fn not_shipping_grep_deploy() {
    assert!(!is_shipping_cmd("grep deploy config.yaml"));
}

#[test]
fn shipping_gh_release() {
    assert!(is_shipping_cmd("gh release create v1.0.0"));
}

#[test]
fn shipping_vercel_prod() {
    assert!(is_shipping_cmd("vercel --prod"));
}

// ============================================================================
// dominant_phase tests
// ============================================================================

#[test]
fn dominant_building() {
    let labels = vec![
        PhaseLabel {
            phase: SessionPhase::Building,
            confidence: 0.8,
            scope: None,
        },
        PhaseLabel {
            phase: SessionPhase::Building,
            confidence: 0.8,
            scope: None,
        },
        PhaseLabel {
            phase: SessionPhase::Testing,
            confidence: 0.8,
            scope: None,
        },
    ];
    assert_eq!(dominant_phase(&labels), Some(SessionPhase::Building));
}

#[test]
fn dominant_excludes_working() {
    let labels = vec![
        PhaseLabel {
            phase: SessionPhase::Working,
            confidence: 0.5,
            scope: None,
        },
        PhaseLabel {
            phase: SessionPhase::Working,
            confidence: 0.5,
            scope: None,
        },
        PhaseLabel {
            phase: SessionPhase::Building,
            confidence: 0.8,
            scope: None,
        },
    ];
    assert_eq!(dominant_phase(&labels), Some(SessionPhase::Building));
}

#[test]
fn dominant_empty() {
    assert_eq!(dominant_phase(&[]), None);
}

// ============================================================================
// SessionPhase basic tests
// ============================================================================

#[test]
fn session_phase_display() {
    assert_eq!(SessionPhase::Thinking.as_str(), "thinking");
    assert_eq!(SessionPhase::Building.display_label(), "Building");
    assert_eq!(SessionPhase::Working.as_str(), "working");
}

#[test]
fn all_phases_have_six_entries() {
    assert_eq!(SessionPhase::ALL.len(), 6);
}

// ============================================================================
// Golden test for system prompt
// ============================================================================

#[test]
fn system_prompt_golden() {
    let expected = include_str!("../../fixtures/phase_system_prompt.txt");
    assert_eq!(super::client::SYSTEM_PROMPT, expected.trim());
}

// ============================================================================
// Integration tests: parse + stabilizer pipeline
// ============================================================================

#[test]
fn pipeline_parse_and_stabilize() {
    use super::client::parse_classify_response;
    use super::stabilizer::ClassificationStabilizer;

    let mut stabilizer = ClassificationStabilizer::new();

    // Simulate 3 LLM responses (as if from different temperatures)
    let responses = [
        r#"{"phase": "building", "scope": "auth system refactor"}"#,
        r#"{"phase": "building", "scope": "auth system refactoring"}"#,
        r#"{"phase": "building", "scope": "authentication refactor"}"#,
    ];

    for resp in &responses {
        let (phase, scope) = parse_classify_response(resp).expect("valid JSON");
        stabilizer.update(phase, scope);
    }

    assert!(stabilizer.should_emit());
    assert_eq!(stabilizer.displayed_phase(), Some(SessionPhase::Building));
    // Scope should be non-None since 3/3 agreed on building-related scope
    assert!(stabilizer.displayed_scope().is_some());
}

#[test]
fn pipeline_disagreement_holds_first_phase() {
    use super::client::parse_classify_response;
    use super::stabilizer::ClassificationStabilizer;

    let mut stabilizer = ClassificationStabilizer::new();

    let responses = [
        r#"{"phase": "building", "scope": "auth system"}"#,
        r#"{"phase": "reviewing", "scope": "code review"}"#,
        r#"{"phase": "testing", "scope": "unit tests"}"#,
    ];

    for resp in &responses {
        let (phase, scope) = parse_classify_response(resp).expect("valid JSON");
        stabilizer.update(phase, scope);
    }

    // EMA: first call (building) shows immediately; subsequent disagreements
    // don't transition because no 2 consecutive + EMA threshold.
    assert!(stabilizer.should_emit());
    assert_eq!(stabilizer.displayed_phase(), Some(SessionPhase::Building));
}

#[test]
fn pipeline_shipping_override() {
    use super::stabilizer::ClassificationStabilizer;

    let mut stabilizer = ClassificationStabilizer::new();

    // Shipping lock activated
    stabilizer.lock_shipping();

    // LLM says "building" but shipping is locked
    stabilizer.update(SessionPhase::Building, Some("test".into()));
    stabilizer.update(SessionPhase::Building, Some("test".into()));

    // Should NOT emit because shipping lock suppresses
    assert!(!stabilizer.should_emit());
}

#[test]
fn format_conversation_round_trip() {
    use super::client::{format_conversation, ConversationTurn, Role};

    let turns = vec![
        ConversationTurn {
            role: Role::User,
            text: "Fix the auth bug in login.ts".into(),
            tools: vec![],
        },
        ConversationTurn {
            role: Role::Assistant,
            text: "Reading the file".into(),
            tools: vec!["Read".into()],
        },
        ConversationTurn {
            role: Role::Assistant,
            text: "Found the issue".into(),
            tools: vec!["Edit".into()],
        },
        ConversationTurn {
            role: Role::User,
            text: "Add a test too".into(),
            tools: vec![],
        },
        ConversationTurn {
            role: Role::Assistant,
            text: "Writing test".into(),
            tools: vec!["Write".into()],
        },
    ];

    let formatted = format_conversation(&turns);

    // First user turn should be at the top (scope signal)
    assert!(formatted.starts_with("User: Fix the auth bug"));
    // Separator
    assert!(formatted.contains("---"));
    // Tools should be included
    assert!(formatted.contains("[tools: Read]"));
    assert!(formatted.contains("[tools: Edit]"));
    // Last turn should be present
    assert!(formatted.contains("Writing test"));
}
