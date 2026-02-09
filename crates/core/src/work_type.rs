// crates/core/src/work_type.rs
//! Work type classification for sessions.
//!
//! Classifies sessions into work types based on duration, edit counts, skills, etc.
//! This is rule-based classification - no LLM needed.
//!
//! ## Work Types
//!
//! | Type | Heuristic | Badge |
//! |------|-----------|-------|
//! | Deep Work | duration > 30min, files_edited > 5, LOC > 200 | Blue |
//! | Quick Ask | duration < 5min, turn_count < 3, no edits | Lightning |
//! | Planning | skills contain "brainstorming" or "plan", low edits | Clipboard |
//! | Bug Fix | skills contain "debugging", moderate edits | Bug |
//! | Standard | Everything else | None |

use serde::{Deserialize, Serialize};

/// Work type classification for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkType {
    /// Deep focused work: duration > 30min, files_edited > 5, LOC > 200
    DeepWork,
    /// Quick question/answer: duration < 5min, turn_count < 3, no edits
    QuickAsk,
    /// Planning or brainstorming session: planning skills, low edits
    Planning,
    /// Bug fixing session: debugging skills, moderate edits
    BugFix,
    /// Standard work session (default)
    Standard,
}

impl WorkType {
    /// Get the string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkType::DeepWork => "deep_work",
            WorkType::QuickAsk => "quick_ask",
            WorkType::Planning => "planning",
            WorkType::BugFix => "bug_fix",
            WorkType::Standard => "standard",
        }
    }

    /// Parse from database string.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "deep_work" => Some(WorkType::DeepWork),
            "quick_ask" => Some(WorkType::QuickAsk),
            "planning" => Some(WorkType::Planning),
            "bug_fix" => Some(WorkType::BugFix),
            "standard" => Some(WorkType::Standard),
            _ => None,
        }
    }

    /// Get display label for UI.
    pub fn display_label(&self) -> &'static str {
        match self {
            WorkType::DeepWork => "Deep Work",
            WorkType::QuickAsk => "Quick Ask",
            WorkType::Planning => "Planning",
            WorkType::BugFix => "Bug Fix",
            WorkType::Standard => "Standard",
        }
    }
}

impl std::fmt::Display for WorkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Input data needed for work type classification.
#[derive(Debug, Clone, Default)]
pub struct ClassificationInput {
    /// Session duration in seconds
    pub duration_seconds: u32,
    /// Number of user-assistant turn pairs
    pub turn_count: u32,
    /// Number of unique files edited
    pub files_edited_count: u32,
    /// Total lines of code added by AI
    pub ai_lines_added: u32,
    /// Skills used in the session
    pub skills_used: Vec<String>,
}

impl ClassificationInput {
    /// Create a new classification input with explicit values.
    pub fn new(
        duration_seconds: u32,
        turn_count: u32,
        files_edited_count: u32,
        ai_lines_added: u32,
        skills_used: Vec<String>,
    ) -> Self {
        Self {
            duration_seconds,
            turn_count,
            files_edited_count,
            ai_lines_added,
            skills_used,
        }
    }
}

/// Classification thresholds (can be tuned).
pub mod thresholds {
    /// Deep work: minimum duration in seconds (30 minutes)
    pub const DEEP_WORK_MIN_DURATION_SECS: u32 = 30 * 60;
    /// Deep work: minimum files edited
    pub const DEEP_WORK_MIN_FILES_EDITED: u32 = 5;
    /// Deep work: minimum lines of code added
    pub const DEEP_WORK_MIN_LOC: u32 = 200;

    /// Quick ask: maximum duration in seconds (5 minutes)
    pub const QUICK_ASK_MAX_DURATION_SECS: u32 = 5 * 60;
    /// Quick ask: maximum turn count
    pub const QUICK_ASK_MAX_TURNS: u32 = 3;

    /// Planning: maximum files edited (low edits)
    pub const PLANNING_MAX_FILES_EDITED: u32 = 2;

    /// Bug fix: maximum files edited (moderate edits)
    pub const BUG_FIX_MAX_FILES_EDITED: u32 = 10;
}

/// Skills that indicate planning/brainstorming work.
const PLANNING_SKILLS: &[&str] = &[
    "brainstorming",
    "brainstorm",
    "plan",
    "planning",
    "design",
    "architect",
    "architecture",
];

/// Skills that indicate debugging/bug fixing work.
const DEBUGGING_SKILLS: &[&str] = &[
    "debugging",
    "debug",
    "bugfix",
    "bug-fix",
    "troubleshoot",
    "troubleshooting",
];

/// Check if skills contain any of the given keywords.
fn skills_contain_any(skills: &[String], keywords: &[&str]) -> bool {
    skills.iter().any(|skill| {
        let skill_lower = skill.to_lowercase();
        keywords.iter().any(|kw| skill_lower.contains(kw))
    })
}

/// Classify a session into a work type based on its characteristics.
///
/// Classification order (first match wins):
/// 1. Quick Ask - very short sessions with minimal interaction
/// 2. Planning - sessions using planning/brainstorming skills
/// 3. Bug Fix - sessions using debugging skills
/// 4. Deep Work - long sessions with significant output
/// 5. Standard - everything else
///
/// # Arguments
/// * `input` - Classification input data
///
/// # Returns
/// The classified `WorkType`
pub fn classify_work_type(input: &ClassificationInput) -> WorkType {
    use thresholds::*;

    // 1. Quick Ask: short duration, few turns, no edits
    if input.duration_seconds <= QUICK_ASK_MAX_DURATION_SECS
        && input.turn_count <= QUICK_ASK_MAX_TURNS
        && input.files_edited_count == 0
    {
        return WorkType::QuickAsk;
    }

    // 2. Planning: planning skills with low edits
    if skills_contain_any(&input.skills_used, PLANNING_SKILLS)
        && input.files_edited_count <= PLANNING_MAX_FILES_EDITED
    {
        return WorkType::Planning;
    }

    // 3. Bug Fix: debugging skills with moderate edits
    if skills_contain_any(&input.skills_used, DEBUGGING_SKILLS)
        && input.files_edited_count <= BUG_FIX_MAX_FILES_EDITED
    {
        return WorkType::BugFix;
    }

    // 4. Deep Work: long duration, many files, significant LOC
    if input.duration_seconds >= DEEP_WORK_MIN_DURATION_SECS
        && input.files_edited_count >= DEEP_WORK_MIN_FILES_EDITED
        && input.ai_lines_added >= DEEP_WORK_MIN_LOC
    {
        return WorkType::DeepWork;
    }

    // 5. Standard: everything else
    WorkType::Standard
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WorkType tests
    // ========================================================================

    #[test]
    fn test_work_type_as_str() {
        assert_eq!(WorkType::DeepWork.as_str(), "deep_work");
        assert_eq!(WorkType::QuickAsk.as_str(), "quick_ask");
        assert_eq!(WorkType::Planning.as_str(), "planning");
        assert_eq!(WorkType::BugFix.as_str(), "bug_fix");
        assert_eq!(WorkType::Standard.as_str(), "standard");
    }

    #[test]
    fn test_work_type_from_str() {
        assert_eq!(WorkType::parse_str("deep_work"), Some(WorkType::DeepWork));
        assert_eq!(WorkType::parse_str("quick_ask"), Some(WorkType::QuickAsk));
        assert_eq!(WorkType::parse_str("planning"), Some(WorkType::Planning));
        assert_eq!(WorkType::parse_str("bug_fix"), Some(WorkType::BugFix));
        assert_eq!(WorkType::parse_str("standard"), Some(WorkType::Standard));
        assert_eq!(WorkType::parse_str("unknown"), None);
        assert_eq!(WorkType::parse_str(""), None);
    }

    #[test]
    fn test_work_type_roundtrip() {
        let types = [
            WorkType::DeepWork,
            WorkType::QuickAsk,
            WorkType::Planning,
            WorkType::BugFix,
            WorkType::Standard,
        ];
        for wt in types {
            assert_eq!(WorkType::parse_str(wt.as_str()), Some(wt));
        }
    }

    #[test]
    fn test_work_type_display() {
        assert_eq!(format!("{}", WorkType::DeepWork), "deep_work");
    }

    #[test]
    fn test_work_type_serialize() {
        let json = serde_json::to_string(&WorkType::DeepWork).unwrap();
        assert_eq!(json, "\"deep_work\"");
    }

    #[test]
    fn test_work_type_deserialize() {
        let wt: WorkType = serde_json::from_str("\"deep_work\"").unwrap();
        assert_eq!(wt, WorkType::DeepWork);
    }

    // ========================================================================
    // classify_work_type tests
    // ========================================================================

    #[test]
    fn test_classify_quick_ask() {
        let input = ClassificationInput {
            duration_seconds: 2 * 60, // 2 minutes
            turn_count: 2,
            files_edited_count: 0,
            ai_lines_added: 0,
            skills_used: vec![],
        };
        assert_eq!(classify_work_type(&input), WorkType::QuickAsk);
    }

    #[test]
    fn test_classify_quick_ask_at_threshold() {
        let input = ClassificationInput {
            duration_seconds: 5 * 60, // exactly 5 minutes
            turn_count: 3,             // exactly 3 turns
            files_edited_count: 0,
            ai_lines_added: 0,
            skills_used: vec![],
        };
        assert_eq!(classify_work_type(&input), WorkType::QuickAsk);
    }

    #[test]
    fn test_classify_quick_ask_with_edits_becomes_standard() {
        let input = ClassificationInput {
            duration_seconds: 2 * 60,
            turn_count: 2,
            files_edited_count: 1, // has edits, not a quick ask
            ai_lines_added: 10,
            skills_used: vec![],
        };
        assert_eq!(classify_work_type(&input), WorkType::Standard);
    }

    #[test]
    fn test_classify_planning() {
        let input = ClassificationInput {
            duration_seconds: 20 * 60,
            turn_count: 10,
            files_edited_count: 1, // low edits
            ai_lines_added: 50,
            skills_used: vec!["brainstorming".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_classify_planning_with_plan_skill() {
        let input = ClassificationInput {
            duration_seconds: 15 * 60,
            turn_count: 8,
            files_edited_count: 2,
            ai_lines_added: 30,
            skills_used: vec!["plan".to_string(), "outline".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_classify_planning_high_edits_becomes_standard() {
        let input = ClassificationInput {
            duration_seconds: 20 * 60,
            turn_count: 10,
            files_edited_count: 5, // too many edits for planning
            ai_lines_added: 200,
            skills_used: vec!["brainstorming".to_string()],
        };
        // Falls through to deep work or standard
        assert_ne!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_classify_bug_fix() {
        let input = ClassificationInput {
            duration_seconds: 25 * 60,
            turn_count: 15,
            files_edited_count: 3, // moderate edits
            ai_lines_added: 50,
            skills_used: vec!["debugging".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::BugFix);
    }

    #[test]
    fn test_classify_bug_fix_with_debug_skill() {
        let input = ClassificationInput {
            duration_seconds: 10 * 60,
            turn_count: 5,
            files_edited_count: 2,
            ai_lines_added: 20,
            skills_used: vec!["debug".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::BugFix);
    }

    #[test]
    fn test_classify_deep_work() {
        let input = ClassificationInput {
            duration_seconds: 45 * 60, // 45 minutes
            turn_count: 50,
            files_edited_count: 10,
            ai_lines_added: 500,
            skills_used: vec!["tdd".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::DeepWork);
    }

    #[test]
    fn test_classify_deep_work_at_threshold() {
        let input = ClassificationInput {
            duration_seconds: 30 * 60, // exactly 30 minutes
            turn_count: 20,
            files_edited_count: 5,     // exactly 5 files
            ai_lines_added: 200,       // exactly 200 lines
            skills_used: vec![],
        };
        assert_eq!(classify_work_type(&input), WorkType::DeepWork);
    }

    #[test]
    fn test_classify_deep_work_below_threshold() {
        let input = ClassificationInput {
            duration_seconds: 29 * 60, // just under 30 minutes
            turn_count: 20,
            files_edited_count: 5,
            ai_lines_added: 200,
            skills_used: vec![],
        };
        // Missing duration threshold
        assert_eq!(classify_work_type(&input), WorkType::Standard);
    }

    #[test]
    fn test_classify_standard() {
        let input = ClassificationInput {
            duration_seconds: 15 * 60,
            turn_count: 10,
            files_edited_count: 3,
            ai_lines_added: 50,
            skills_used: vec!["commit".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Standard);
    }

    #[test]
    fn test_classify_empty_session() {
        let input = ClassificationInput::default();
        // Zero duration, zero turns, no edits = quick ask
        assert_eq!(classify_work_type(&input), WorkType::QuickAsk);
    }

    // ========================================================================
    // Classification priority tests
    // ========================================================================

    #[test]
    fn test_quick_ask_takes_priority_over_planning() {
        // Very short session with planning skill - should be quick ask
        let input = ClassificationInput {
            duration_seconds: 2 * 60,
            turn_count: 2,
            files_edited_count: 0,
            ai_lines_added: 0,
            skills_used: vec!["brainstorming".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::QuickAsk);
    }

    #[test]
    fn test_planning_takes_priority_over_deep_work() {
        // Long session with planning skills but low edits - should be planning
        let input = ClassificationInput {
            duration_seconds: 60 * 60, // 1 hour
            turn_count: 30,
            files_edited_count: 1, // low edits
            ai_lines_added: 50,
            skills_used: vec!["architecture".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_bug_fix_takes_priority_over_deep_work() {
        // Long session with debugging skills - should be bug fix
        let input = ClassificationInput {
            duration_seconds: 45 * 60,
            turn_count: 30,
            files_edited_count: 8, // moderate edits
            ai_lines_added: 150,
            skills_used: vec!["troubleshooting".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::BugFix);
    }

    // ========================================================================
    // Skills matching tests
    // ========================================================================

    #[test]
    fn test_skills_case_insensitive() {
        let input = ClassificationInput {
            duration_seconds: 15 * 60,
            turn_count: 8,
            files_edited_count: 1,
            ai_lines_added: 20,
            skills_used: vec!["BRAINSTORMING".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_skills_partial_match() {
        let input = ClassificationInput {
            duration_seconds: 15 * 60,
            turn_count: 8,
            files_edited_count: 1,
            ai_lines_added: 20,
            skills_used: vec!["my-brainstorming-skill".to_string()],
        };
        assert_eq!(classify_work_type(&input), WorkType::Planning);
    }

    #[test]
    fn test_skills_multiple_skills_one_match() {
        let input = ClassificationInput {
            duration_seconds: 15 * 60,
            turn_count: 8,
            files_edited_count: 3,
            ai_lines_added: 40,
            skills_used: vec![
                "commit".to_string(),
                "review".to_string(),
                "debug".to_string(), // matches debugging
            ],
        };
        assert_eq!(classify_work_type(&input), WorkType::BugFix);
    }
}
