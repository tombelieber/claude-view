// crates/core/src/phase/tests.rs
use super::*;

fn default_config() -> PhaseClassifierConfig {
    PhaseClassifierConfig::default()
}

fn make_step() -> StepSignals {
    StepSignals::default()
}

#[test]
fn test_empty_window() {
    let result = classify_window(&[], &default_config());
    assert_eq!(result.phase, SessionPhase::Unknown);
    assert_eq!(result.confidence, 0.0);
}

#[test]
fn test_pure_edit_window() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.edit_count = 3;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Implementing);
    assert!(result.confidence > 0.3, "confidence was {}", result.confidence);
}

#[test]
fn test_pure_read_window() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.read_count = 2;
            s.grep_count = 1;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Exploring);
}

#[test]
fn test_ship_skill_overrides() {
    let mut steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.edit_count = 2;
            s
        })
        .collect();
    steps[9].has_ship_skill = true;
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Releasing);
}

#[test]
fn test_debug_skill() {
    let mut steps = vec![make_step(); 10];
    steps[0].has_debug_skill = true;
    steps[3].read_count = 2;
    steps[5].edit_count = 1;
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Debugging);
}

#[test]
fn test_test_commands() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.bash_count = 1;
            s.has_test_cmd = true;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Testing);
}

#[test]
fn test_plan_skill_with_low_edits() {
    let mut steps = vec![make_step(); 10];
    steps[0].has_plan_skill = true;
    steps[1].read_count = 3;
    steps[2].read_count = 2;
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Planning);
}

#[test]
fn test_config_files_edited() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.edit_count = 1;
            s.config_files_edited = 1;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Configuring);
}

#[test]
fn test_impl_skill_absorbs_explore() {
    let mut steps = vec![make_step(); 10];
    steps[0].has_impl_skill = true;
    for s in &mut steps[1..5] {
        s.has_explore_agent = true;
        s.read_count = 3;
    }
    for s in &mut steps[5..10] {
        s.edit_count = 2;
    }
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Implementing);
}

#[test]
fn test_user_prompt_keywords() {
    let mut steps = vec![make_step(); 5];
    steps[0].is_user_prompt = true;
    steps[0].prompt_fix_kw = true;
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Debugging);
}

#[test]
fn test_phase_parse_roundtrip() {
    for phase in SessionPhase::ALL {
        let s = phase.as_str();
        let parsed = SessionPhase::parse(s).expect(&format!("Failed to parse {}", s));
        assert_eq!(parsed, phase);
    }
}

#[test]
fn test_dominant_phase() {
    let labels = vec![
        PhaseLabel { phase: SessionPhase::Implementing, confidence: 0.8, secondary: None, step_index: 10 },
        PhaseLabel { phase: SessionPhase::Implementing, confidence: 0.9, secondary: None, step_index: 20 },
        PhaseLabel { phase: SessionPhase::Testing, confidence: 0.7, secondary: None, step_index: 30 },
    ];
    assert_eq!(dominant_phase(&labels), Some(SessionPhase::Implementing));
}

#[test]
fn test_dominant_phase_empty() {
    assert_eq!(dominant_phase(&[]), None);
}

#[test]
fn test_git_push_is_releasing() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.bash_count = 1;
            s.has_git_push = true;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Releasing);
}

#[test]
fn test_secondary_phase_when_close() {
    let mut steps = vec![make_step(); 10];
    for s in &mut steps[0..5] {
        s.bash_count = 1;
        s.has_test_cmd = true;
    }
    for s in &mut steps[5..10] {
        s.edit_count = 2;
    }
    let result = classify_window(&steps, &default_config());
    assert!(result.secondary.is_some());
}

#[test]
fn test_decay_favors_recent() {
    let mut steps = vec![make_step(); 10];
    for s in &mut steps[0..8] {
        s.read_count = 5;
    }
    for s in &mut steps[8..10] {
        s.edit_count = 5;
        s.write_count = 2;
    }
    let result = classify_window(&steps, &default_config());
    assert!(result.confidence > 0.0);
}

// TDD-specific test: edit-heavy window with test commands should be implementing
#[test]
fn test_tdd_is_implementing() {
    let steps: Vec<StepSignals> = (0..10)
        .map(|_| {
            let mut s = make_step();
            s.edit_count = 3;
            s.bash_count = 1;
            s.has_test_cmd = true;
            s
        })
        .collect();
    let result = classify_window(&steps, &default_config());
    assert_eq!(result.phase, SessionPhase::Implementing);
}
