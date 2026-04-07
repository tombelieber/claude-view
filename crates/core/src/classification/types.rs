// crates/core/src/classification/types.rs
//! Classification input/output types.

use serde::{Deserialize, Serialize};

/// Input for classification (session ID + preview + skills).
#[derive(Debug, Clone)]
pub struct ClassificationInput {
    pub session_id: String,
    pub preview: String,
    pub skills_used: Vec<String>,
}

/// A single classification result from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationResult {
    pub session_id: String,
    pub category: String,
    pub confidence: f64,
    #[serde(default)]
    pub reasoning: String,
}

/// LLM response for batch classification.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchClassificationResponse {
    pub classifications: Vec<ClassificationResult>,
}

/// Parsed and validated classification result.
#[derive(Debug, Clone)]
pub struct ValidatedClassification {
    pub session_id: String,
    pub l1: String,
    pub l2: String,
    pub l3: String,
    pub confidence: f64,
    pub reasoning: String,
}
