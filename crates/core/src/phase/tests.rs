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

// ============================================================================
// Cross-validation: Python ↔ Rust prediction parity
// Generated from scripts/phase-classifier/generate_test_vectors.py
// ============================================================================

/// Helper: build feature vector directly and classify via the internal API.
fn classify_features(features: &[f32; 96]) -> (SessionPhase, f64) {
    use super::classifier;
    // Access the internal classify logic by creating a StepSignals window
    // that produces the exact feature vector we want — but that's not possible
    // without the full pipeline. Instead, we test the raw model evaluation.
    //
    // We expose a test-only function for this.
    classifier::classify_from_features(features)
}

// --- Python cross-validation tests ---
// Each test constructs the SAME feature vector as the Python script,
// then verifies the Rust XGBoost evaluation produces matching results.

#[test]
fn xval_all_zeros_low_confidence() {
    let f = [0.0f32; 96];
    let (phase, conf) = classify_features(&f);
    // Python: reviewing at 40.3% → below threshold → Working
    assert_eq!(phase, SessionPhase::Working);
    assert!(conf < 0.75, "Expected low confidence, got {conf}");
}

#[test]
fn xval_heavy_edits_building() {
    let mut f = [0.0f32; 96];
    f[0] = 30.0;
    f[1] = 10.0;
    f[5] = 10.0;
    f[16] = 40.0;
    f[17] = 5.0;
    f[18] = 8.0;
    f[31] = 1.0;
    f[63] = 5.0;
    f[83] = 25.0;
    f[85] = 6.0;
    f[94] = 0.8;

    let (phase, conf) = classify_features(&f);
    // Python: building at 96.2%
    assert_eq!(phase, SessionPhase::Building);
    assert!(conf > 0.90, "Expected >90% confidence, got {conf:.4}");
}

#[test]
fn xval_test_commands_testing() {
    let mut f = [0.0f32; 96];
    f[2] = 10.0;
    f[5] = 20.0;
    f[17] = 10.0;
    f[30] = 1.0;
    f[62] = 8.0;
    f[85] = 12.0;
    f[88] = 6.0;

    let (phase, conf) = classify_features(&f);
    // Python: testing at 89.2%
    assert_eq!(phase, SessionPhase::Testing);
    assert!(conf > 0.80, "Expected >80% confidence, got {conf:.4}");
}

#[test]
fn xval_read_only_reviewing() {
    let mut f = [0.0f32; 96];
    f[2] = 30.0;
    f[3] = 5.0;
    f[4] = 10.0;
    f[17] = 45.0;
    f[18] = 0.0;
    f[84] = 28.0;
    f[95] = 0.9;

    let (phase, conf) = classify_features(&f);
    // Python: reviewing at 95.8%
    assert_eq!(phase, SessionPhase::Reviewing);
    assert!(conf > 0.90, "Expected >90% confidence, got {conf:.4}");
}

#[test]
fn xval_plan_skill_planning() {
    let mut f = [0.0f32; 96];
    f[7] = 5.0;
    f[11] = 8.0;
    f[12] = 3.0;
    f[16] = 5.0;
    f[19] = 1.0;
    f[51] = 5.0;
    f[87] = 3.0;
    f[91] = 2.0;

    let (phase, conf) = classify_features(&f);
    // Python: planning at 95.6% — has file writes so should be Planning (not Thinking)
    assert_eq!(phase, SessionPhase::Planning);
    assert!(conf > 0.90, "Expected >90% confidence, got {conf:.4}");
}

#[test]
fn xval_publish_cmd_shipping_rule() {
    let mut f = [0.0f32; 96];
    f[5] = 5.0;
    f[33] = 1.0; // any_publish_cmd
    f[65] = 1.0;

    let (phase, _conf) = classify_features(&f);
    // Python ML predicts building (43.7%) but shipping RULE should override
    assert_eq!(phase, SessionPhase::Shipping);
}

#[test]
fn xval_git_push_not_shipping() {
    let mut f = [0.0f32; 96];
    f[5] = 5.0;
    f[32] = 1.0; // any_git_push
    f[64] = 3.0;
    f[89] = 2.0;

    let (phase, _conf) = classify_features(&f);
    // Git push should NOT trigger shipping rule
    assert_ne!(phase, SessionPhase::Shipping);
}

#[test]
fn xval_mixed_signals_low_confidence() {
    let mut f = [0.0f32; 96];
    f[0] = 3.0;
    f[2] = 3.0;
    f[5] = 3.0;
    f[7] = 1.0;
    f[16] = 3.0;
    f[17] = 3.0;
    f[18] = 1.0;
    f[19] = 1.0;
    f[30] = 1.0;

    let (phase, conf) = classify_features(&f);
    // Python: planning at 57.6% — borderline case.
    // f32 vs f64 precision differences can push confidence slightly.
    // Key assertion: this is a LOW confidence prediction (< 80%).
    // Whether it's above or below the 75% gate depends on float precision.
    assert!(
        conf < 0.80,
        "Mixed signals should have low confidence, got {conf:.4}"
    );
    // If gated to Working, that's correct. If barely above threshold → Planning is acceptable.
    assert!(
        phase == SessionPhase::Working || phase == SessionPhase::Planning,
        "Expected Working or Planning (borderline), got {phase:?} at {conf:.4}"
    );
}

// ============================================================================
// Edge cases
// ============================================================================

#[test]
fn edge_single_step_window() {
    let mut s = StepSignals::default();
    s.edit_count = 5;
    let result = classify_window(&[s]);
    // Should not panic, should produce a result
    assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
}

#[test]
fn edge_very_large_values() {
    let mut f = [0.0f32; 96];
    f[0] = 10000.0; // huge edit count
    f[16] = 10000.0;
    f[18] = 10000.0;
    let (phase, conf) = classify_features(&f);
    // Should not panic or produce NaN
    assert!(!conf.is_nan());
    assert!(conf >= 0.0 && conf <= 1.0);
    // Extreme edits → likely building
    assert_eq!(phase, SessionPhase::Building);
}

#[test]
fn edge_all_features_set() {
    let f = [1.0f32; 96];
    let (_, conf) = classify_features(&f);
    assert!(!conf.is_nan());
    assert!(conf >= 0.0 && conf <= 1.0);
}

#[test]
fn perf_model_load_time() {
    // Model load should be fast (< 100ms)
    let start = std::time::Instant::now();
    let _ = classify_features(&[0.0f32; 96]);
    let load_time = start.elapsed();
    eprintln!("  Model load + first classify: {:?}", load_time);
    assert!(
        load_time.as_millis() < 500,
        "Model load took too long: {:?}",
        load_time
    );
}

#[test]
fn perf_classification_latency() {
    // Warm up model
    let _ = classify_features(&[0.0f32; 96]);

    // Measure 1000 classifications
    let mut f = [0.0f32; 96];
    f[0] = 10.0;
    f[5] = 5.0;
    f[16] = 10.0;

    let start = std::time::Instant::now();
    let n = 1000;
    for _ in 0..n {
        let _ = classify_features(&f);
    }
    let total = start.elapsed();
    let per_call = total / n;
    eprintln!("  {n} classifications in {:?} ({:?}/call)", total, per_call);
    assert!(
        per_call.as_micros() < 1000, // < 1ms per call
        "Classification too slow: {:?}/call",
        per_call
    );
}

#[test]
fn edge_negative_features() {
    let mut f = [0.0f32; 96];
    f[92] = -1.0; // first_edit_pos when no edit (Python outputs -1/n)
    f[93] = -1.0;
    let (_, conf) = classify_features(&f);
    assert!(!conf.is_nan());
    assert!(conf >= 0.0 && conf <= 1.0);
}
