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
