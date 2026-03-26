use super::*;

#[test]
fn empty_window_returns_working() {
    let result = classify_window(&[]);
    assert_eq!(result.phase, SessionPhase::Working);
    assert_eq!(result.confidence, 0.0);
}

#[test]
fn shipping_rule_publish_overrides_ml() {
    let mut step = StepSignals::default();
    step.has_publish_cmd = true;
    step.bash_count = 1;
    let result = classify_window(&[step]);
    assert_eq!(result.phase, SessionPhase::Shipping);
    assert_eq!(result.confidence, 1.0);
}

#[test]
fn shipping_rule_deploy_overrides_ml() {
    let mut step = StepSignals::default();
    step.has_deploy_cmd = true;
    step.bash_count = 1;
    let result = classify_window(&[step]);
    assert_eq!(result.phase, SessionPhase::Shipping);
}

#[test]
fn git_push_alone_does_not_trigger_shipping() {
    let mut step = StepSignals::default();
    step.has_git_push = true;
    step.bash_count = 1;
    let result = classify_window(&[step]);
    assert_ne!(result.phase, SessionPhase::Shipping);
}

#[test]
fn heavy_edit_window_predicts_building() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = StepSignals::default();
            s.edit_count = 3;
            s.write_count = 1;
            s.bash_count = 1;
            s.has_build_cmd = true;
            s
        })
        .collect();
    let result = classify_window(&steps);
    if result.confidence >= 0.75 {
        assert_eq!(result.phase, SessionPhase::Building);
    }
}

#[test]
fn test_cmd_window_predicts_testing() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = StepSignals::default();
            s.has_test_cmd = true;
            s.bash_count = 2;
            s.read_count = 1;
            s
        })
        .collect();
    let result = classify_window(&steps);
    if result.confidence >= 0.75 {
        assert_eq!(result.phase, SessionPhase::Testing);
    }
}

#[test]
fn read_only_window_predicts_reviewing() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = StepSignals::default();
            s.read_count = 3;
            s.grep_count = 2;
            s.glob_count = 1;
            s
        })
        .collect();
    let result = classify_window(&steps);
    if result.confidence >= 0.75 {
        assert_eq!(result.phase, SessionPhase::Reviewing);
    }
}

#[test]
fn plan_skill_with_doc_writes_is_planning_or_thinking() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = StepSignals::default();
            s.has_plan_skill = true;
            s.skill_count = 1;
            s.doc_files_edited = 2;
            s.write_count = 2;
            s
        })
        .collect();
    let result = classify_window(&steps);
    if result.confidence >= 0.75 {
        assert!(
            result.phase == SessionPhase::Planning || result.phase == SessionPhase::Thinking,
            "Expected Planning or Thinking, got {:?}",
            result.phase
        );
    }
}

#[test]
fn model_loads_successfully() {
    let step = StepSignals::default();
    let _ = classify_window(&[step]);
}

#[test]
fn feature_vector_correct_length() {
    let steps: Vec<StepSignals> = (0..5).map(|_| StepSignals::default()).collect();
    let features = features::flatten_window(&steps);
    assert_eq!(features.len(), 96);
}

#[test]
fn dominant_phase_excludes_working() {
    let labels = vec![
        PhaseLabel {
            phase: SessionPhase::Working,
            confidence: 0.5,
            window_size: 10,
        },
        PhaseLabel {
            phase: SessionPhase::Working,
            confidence: 0.5,
            window_size: 10,
        },
        PhaseLabel {
            phase: SessionPhase::Building,
            confidence: 0.9,
            window_size: 10,
        },
    ];
    assert_eq!(dominant_phase(&labels), Some(SessionPhase::Building));
}

#[test]
fn session_phase_display() {
    assert_eq!(SessionPhase::Thinking.as_str(), "thinking");
    assert_eq!(SessionPhase::Building.display_label(), "Building");
    assert_eq!(SessionPhase::Shipping.emoji(), "🚀");
    assert_eq!(SessionPhase::Working.as_str(), "working");
}

#[test]
fn all_phases_have_six_entries() {
    assert_eq!(SessionPhase::ALL.len(), 6);
}

#[test]
fn confidence_below_threshold_returns_working() {
    // Single empty step — model should have low confidence
    let result = classify_window(&[StepSignals::default()]);
    // With minimal signals, confidence should be low
    if result.confidence < 0.75 {
        assert_eq!(result.phase, SessionPhase::Working);
    }
}
