//! XGBoost-based phase classifier with confidence gating.
//!
//! Architecture:
//!   1. Feature extraction (features.rs) → 96-element f32 vector
//!   2. XGBoost 4-class evaluation → raw scores per class
//!   3. Softmax → probabilities
//!   4. Shipping rule check (overrides ML)
//!   5. Confidence gate (below threshold → Working)
//!   6. Thinking/Planning split (post-ML heuristic)

use std::sync::OnceLock;

use serde::Deserialize;

use super::features::{flatten_window, N_FEATURES};
use super::{PhaseLabel, SessionPhase, StepSignals};

/// Minimum confidence to show a phase badge (below → Working).
const CONFIDENCE_THRESHOLD: f64 = 0.75;

/// XGBoost model classes (alphabetical order, matching LabelEncoder).
const ML_CLASSES: [SessionPhase; 4] = [
    SessionPhase::Building,  // 0
    SessionPhase::Planning,  // 1
    SessionPhase::Reviewing, // 2
    SessionPhase::Testing,   // 3
];

/// Feature indices for post-ML heuristics.
const TOTAL_EDIT_IDX: usize = 16;
const SUM_WRITE_IDX: usize = 1;
const SUM_DOC_FILES_IDX: usize = 11;
const SUM_PLAN_FILES_IDX: usize = 12;

// Shipping signal feature indices.
const ANY_PUBLISH_CMD_IDX: usize = 33; // 19 + 14
const ANY_DEPLOY_CMD_IDX: usize = 34; // 19 + 15
const _ANY_SHIP_SKILL_IDX: usize = 22; // 19 + 3 (reserved for future shipping rule refinement)

// ============================================================================
// Model data structures
// ============================================================================

#[derive(Deserialize)]
struct ModelData {
    n_classes: usize,
    trees: Vec<TreeData>,
}

#[derive(Deserialize)]
struct TreeData {
    class_idx: usize,
    nodes: Vec<NodeData>,
}

#[derive(Deserialize)]
struct NodeData {
    id: usize,
    is_leaf: bool,
    #[serde(default)]
    feature: usize,
    #[serde(default)]
    threshold: f32,
    #[serde(default)]
    yes: usize,
    #[serde(default)]
    no: usize,
    #[serde(default)]
    value: f32,
}

/// Compiled tree for fast evaluation — flat array indexed by node ID.
struct CompiledTree {
    class_idx: usize,
    nodes: Vec<CompiledNode>,
}

#[derive(Clone, Copy)]
struct CompiledNode {
    is_leaf: bool,
    feature: u16,
    threshold: f32,
    yes: u16,
    no: u16,
    value: f32,
}

impl Default for CompiledNode {
    fn default() -> Self {
        Self {
            is_leaf: true,
            feature: 0,
            threshold: 0.0,
            yes: 0,
            no: 0,
            value: 0.0,
        }
    }
}

struct CompiledModel {
    trees: Vec<CompiledTree>,
}

// ============================================================================
// Model loading
// ============================================================================

static MODEL: OnceLock<CompiledModel> = OnceLock::new();

fn load_model() -> CompiledModel {
    let json_str = include_str!("model/trees.json");
    let raw: ModelData = serde_json::from_str(json_str).expect("embedded model is valid JSON");

    let trees = raw
        .trees
        .into_iter()
        .map(|t| {
            let max_id = t.nodes.iter().map(|n| n.id).max().unwrap_or(0);
            let mut nodes = vec![CompiledNode::default(); max_id + 1];
            for n in &t.nodes {
                nodes[n.id] = CompiledNode {
                    is_leaf: n.is_leaf,
                    feature: n.feature as u16,
                    threshold: n.threshold,
                    yes: n.yes as u16,
                    no: n.no as u16,
                    value: n.value,
                };
            }
            CompiledTree {
                class_idx: t.class_idx,
                nodes,
            }
        })
        .collect();

    let _ = raw.n_classes; // validated by ML_CLASSES.len()
    CompiledModel { trees }
}

fn get_model() -> &'static CompiledModel {
    MODEL.get_or_init(load_model)
}

// ============================================================================
// Tree evaluation
// ============================================================================

/// Traverse a single tree to its leaf, return the leaf value.
fn eval_tree(features: &[f32; N_FEATURES], tree: &CompiledTree) -> f32 {
    let mut node_id = 0u16;
    loop {
        let node = &tree.nodes[node_id as usize];
        if node.is_leaf {
            return node.value;
        }
        let feat_val = features[node.feature as usize];
        // XGBoost convention: yes = left (condition true), no = right
        node_id = if feat_val < node.threshold {
            node.yes
        } else {
            node.no
        };
    }
}

/// Evaluate all trees and return raw scores per class.
fn eval_xgboost(features: &[f32; N_FEATURES]) -> [f64; 4] {
    let model = get_model();
    let mut scores = [0.0f64; 4];
    for tree in &model.trees {
        scores[tree.class_idx] += eval_tree(features, tree) as f64;
    }
    scores
}

/// Softmax: convert raw scores to probabilities.
fn softmax(scores: &[f64; 4]) -> [f64; 4] {
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut exps = [0.0f64; 4];
    let mut sum = 0.0f64;
    for (i, &s) in scores.iter().enumerate() {
        exps[i] = (s - max_score).exp();
        sum += exps[i];
    }
    for e in &mut exps {
        *e /= sum;
    }
    exps
}

// ============================================================================
// Shipping rule
// ============================================================================

/// Check if the feature vector indicates shipping activity.
/// Uses strict signals only (publish, deploy) — NOT git push.
fn is_shipping(features: &[f32; N_FEATURES]) -> bool {
    features[ANY_PUBLISH_CMD_IDX] > 0.0 || features[ANY_DEPLOY_CMD_IDX] > 0.0
}

// ============================================================================
// Public API
// ============================================================================

/// Phase classifier with configurable confidence threshold.
pub struct PhaseClassifier {
    pub confidence_threshold: f64,
}

impl Default for PhaseClassifier {
    fn default() -> Self {
        Self {
            confidence_threshold: CONFIDENCE_THRESHOLD,
        }
    }
}

impl PhaseClassifier {
    pub fn classify(&self, steps: &[StepSignals]) -> PhaseLabel {
        classify_window_with_threshold(steps, self.confidence_threshold)
    }
}

/// Classify a window of steps into a phase using the default threshold.
pub fn classify_window(steps: &[StepSignals]) -> PhaseLabel {
    classify_window_with_threshold(steps, CONFIDENCE_THRESHOLD)
}

fn classify_window_with_threshold(steps: &[StepSignals], threshold: f64) -> PhaseLabel {
    let n = steps.len();
    if n == 0 {
        return PhaseLabel {
            phase: SessionPhase::Working,
            confidence: 0.0,
            window_size: 0,
        };
    }

    let features = flatten_window(steps);

    // 1. Check shipping rule first (deterministic, overrides ML)
    if is_shipping(&features) {
        return PhaseLabel {
            phase: SessionPhase::Shipping,
            confidence: 1.0,
            window_size: n as u32,
        };
    }

    // 2. Run XGBoost 4-class model
    let scores = eval_xgboost(&features);
    let probs = softmax(&scores);

    // Find best class
    let (best_idx, best_prob) = probs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    // 3. Confidence gate
    if *best_prob < threshold {
        return PhaseLabel {
            phase: SessionPhase::Working,
            confidence: *best_prob,
            window_size: n as u32,
        };
    }

    // 4. Map ML class to product phase
    let ml_phase = ML_CLASSES[best_idx];

    // 5. Thinking/Planning post-ML split
    let phase = if ml_phase == SessionPhase::Planning {
        let has_writes = features[TOTAL_EDIT_IDX] + features[SUM_WRITE_IDX] > 0.0
            || features[SUM_DOC_FILES_IDX] + features[SUM_PLAN_FILES_IDX] > 0.0;
        if has_writes {
            SessionPhase::Planning
        } else {
            SessionPhase::Thinking
        }
    } else {
        ml_phase
    };

    PhaseLabel {
        phase,
        confidence: *best_prob,
        window_size: n as u32,
    }
}
