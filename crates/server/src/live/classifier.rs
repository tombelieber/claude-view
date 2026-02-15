// crates/server/src/live/classifier.rs
//! Session state classifier for intelligent pause classification.
//!
//! Structural-only system:
//! - Tier 1: Structural signals (free, instant, high confidence)
//! - Fallback: Basic heuristic when no structural signal matches

use serde::{Serialize, Deserialize};

/// Classifier that determines *why* a session paused.
#[derive(Clone)]
pub struct SessionStateClassifier;

/// Context for classification — recent messages, tools, and session state.
#[derive(Clone)]
pub struct SessionStateContext {
    /// Last 3-5 messages with role, content_preview, tool_names.
    pub recent_messages: Vec<MessageSummary>,
    /// Stop reason of last assistant message.
    pub last_stop_reason: Option<String>,
    /// Last tool name used.
    pub last_tool: Option<String>,
    /// Whether Claude process is running.
    pub has_running_process: bool,
    /// Seconds since last file modification.
    pub seconds_since_modified: u64,
    /// Number of user turns.
    pub turn_count: u32,
}

/// Summary of a single message for classification context.
#[derive(Debug, Clone)]
pub struct MessageSummary {
    pub role: String,
    pub content_preview: String,
    pub tool_names: Vec<String>,
}

/// Classification result for a paused session.
///
/// Uses camelCase to match LiveSession's `#[serde(rename_all = "camelCase")]`
/// so the entire API is consistent. Frontend expects camelCase everywhere.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseClassification {
    pub reason: PauseReason,
    pub label: String,
    pub confidence: f32,
    pub source: ClassificationSource,
}

/// Why a session is paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauseReason {
    #[serde(rename = "needsInput")]
    NeedsInput,
    #[serde(rename = "taskComplete")]
    TaskComplete,
    #[serde(rename = "midWork")]
    MidWork,
    #[serde(rename = "error")]
    Error,
}

/// How the classification was determined.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClassificationSource {
    #[serde(rename = "structural")]
    Structural,
    #[serde(rename = "fallback")]
    Fallback,
}

impl SessionStateClassifier {
    /// Create a new classifier (structural-only, no AI provider).
    pub fn new() -> Self {
        Self
    }

    /// Tier 1: Structural signal classification (sync, instant, free).
    ///
    /// Returns `Some(classification)` when a known pattern is detected.
    /// Returns `None` when no structural signal matches (ambiguous).
    pub fn structural_classify(&self, ctx: &SessionStateContext) -> Option<PauseClassification> {
        // Check last tool for explicit signals
        if let Some(tool) = &ctx.last_tool {
            match tool.as_str() {
                "AskUserQuestion" => return Some(PauseClassification {
                    reason: PauseReason::NeedsInput,
                    label: "Asked you a question".into(),
                    confidence: 0.99,
                    source: ClassificationSource::Structural,
                }),
                "ExitPlanMode" => return Some(PauseClassification {
                    reason: PauseReason::NeedsInput,
                    label: "Plan ready for review".into(),
                    confidence: 0.99,
                    source: ClassificationSource::Structural,
                }),
                _ => {}
            }
        }

        // Process exited = session ended
        if !ctx.has_running_process && ctx.seconds_since_modified > 30 {
            return Some(PauseClassification {
                reason: PauseReason::TaskComplete,
                label: "Session ended".into(),
                confidence: 0.95,
                source: ClassificationSource::Structural,
            });
        }

        // Check content patterns in last message
        if let Some(last_msg) = ctx.recent_messages.last() {
            if last_msg.role == "assistant" {
                let content = last_msg.content_preview.to_lowercase();

                // Commit/push pattern
                if last_msg.tool_names.iter().any(|t| t == "Bash")
                    && (content.contains("commit") || content.contains("pushed"))
                    && ctx.last_stop_reason.as_deref() == Some("end_turn")
                {
                    return Some(PauseClassification {
                        reason: PauseReason::TaskComplete,
                        label: "Committed changes".into(),
                        confidence: 0.85,
                        source: ClassificationSource::Structural,
                    });
                }

                // Single-turn Q&A
                if ctx.turn_count <= 2
                    && ctx.last_stop_reason.as_deref() == Some("end_turn")
                {
                    return Some(PauseClassification {
                        reason: PauseReason::TaskComplete,
                        label: "Answered your question".into(),
                        confidence: 0.80,
                        source: ClassificationSource::Structural,
                    });
                }
            }
        }

        None // Ambiguous -- fall through to fallback
    }

    /// Fallback classification when structural signals don't match.
    pub fn fallback_classify(&self, ctx: &SessionStateContext) -> PauseClassification {
        if ctx.last_stop_reason.as_deref() == Some("end_turn") {
            PauseClassification {
                reason: PauseReason::MidWork,
                label: "Waiting for your next message".into(),
                confidence: 0.5,
                source: ClassificationSource::Fallback,
            }
        } else {
            PauseClassification {
                reason: PauseReason::MidWork,
                label: "Session paused".into(),
                confidence: 0.3,
                source: ClassificationSource::Fallback,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_classifier() -> SessionStateClassifier {
        SessionStateClassifier::new()
    }

    fn make_context() -> SessionStateContext {
        SessionStateContext {
            recent_messages: vec![],
            last_stop_reason: None,
            last_tool: None,
            has_running_process: true,
            seconds_since_modified: 10,
            turn_count: 5,
        }
    }

    #[test]
    fn test_structural_ask_user_question() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.last_tool = Some("AskUserQuestion".to_string());

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.reason, PauseReason::NeedsInput);
        assert!(c.confidence >= 0.99);
    }

    #[test]
    fn test_structural_exit_plan_mode() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.last_tool = Some("ExitPlanMode".to_string());

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.reason, PauseReason::NeedsInput);
        assert_eq!(c.label, "Plan ready for review");
    }

    #[test]
    fn test_structural_process_exited() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.has_running_process = false;
        ctx.seconds_since_modified = 45;

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.reason, PauseReason::TaskComplete);
        assert_eq!(c.label, "Session ended");
    }

    #[test]
    fn test_structural_commit_pattern() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.last_stop_reason = Some("end_turn".to_string());
        ctx.recent_messages.push(MessageSummary {
            role: "assistant".to_string(),
            content_preview: "Done! I've committed the changes.".to_string(),
            tool_names: vec!["Bash".to_string()],
        });

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.reason, PauseReason::TaskComplete);
        assert_eq!(c.label, "Committed changes");
    }

    #[test]
    fn test_structural_single_turn_qa() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.turn_count = 1;
        ctx.last_stop_reason = Some("end_turn".to_string());
        ctx.recent_messages.push(MessageSummary {
            role: "assistant".to_string(),
            content_preview: "The answer is 42.".to_string(),
            tool_names: vec![],
        });

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.reason, PauseReason::TaskComplete);
        assert_eq!(c.label, "Answered your question");
    }

    #[test]
    fn test_structural_no_match() {
        let classifier = make_classifier();
        let ctx = make_context();

        let result = classifier.structural_classify(&ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_fallback_end_turn() {
        let classifier = make_classifier();
        let mut ctx = make_context();
        ctx.last_stop_reason = Some("end_turn".to_string());

        let result = classifier.fallback_classify(&ctx);
        assert_eq!(result.reason, PauseReason::MidWork);
        assert_eq!(result.label, "Waiting for your next message");
        assert!((result.confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fallback_no_stop_reason() {
        let classifier = make_classifier();
        let ctx = make_context();

        let result = classifier.fallback_classify(&ctx);
        assert_eq!(result.reason, PauseReason::MidWork);
        assert_eq!(result.label, "Session paused");
        assert!((result.confidence - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pause_classification_serialize() {
        let pc = PauseClassification {
            reason: PauseReason::NeedsInput,
            label: "Asked a question".into(),
            confidence: 0.99,
            source: ClassificationSource::Structural,
        };
        let json = serde_json::to_string(&pc).unwrap();
        assert!(json.contains("\"reason\":\"needsInput\""));
        assert!(json.contains("\"source\":\"structural\""));
        // camelCase field names
        assert!(json.contains("\"confidence\""));
        assert!(json.contains("\"label\""));
    }
}
