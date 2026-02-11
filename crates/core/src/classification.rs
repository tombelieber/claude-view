// crates/core/src/classification.rs
//! Classification taxonomy types, prompt templates, and response parsing.
//!
//! This module defines the 30-category taxonomy for session classification,
//! builds batch classification prompts, and parses LLM responses.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Taxonomy Enums
// ============================================================================

/// Top-level category (L1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum CategoryL1 {
    Code,
    Support,
    Thinking,
}

impl CategoryL1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code_work",
            Self::Support => "support_work",
            Self::Thinking => "thinking_work",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "code_work" | "code" => Some(Self::Code),
            "support_work" | "support" => Some(Self::Support),
            "thinking_work" | "thinking" => Some(Self::Thinking),
            _ => None,
        }
    }
}

/// Second-level category (L2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum CategoryL2 {
    Feature,
    Bugfix,
    Refactor,
    Testing,
    Docs,
    Config,
    Ops,
    Planning,
    Explanation,
    Architecture,
}

impl CategoryL2 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Bugfix => "bug_fix",
            Self::Refactor => "refactor",
            Self::Testing => "testing",
            Self::Docs => "docs",
            Self::Config => "config",
            Self::Ops => "ops",
            Self::Planning => "planning",
            Self::Explanation => "explanation",
            Self::Architecture => "architecture",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "feature" => Some(Self::Feature),
            "bug_fix" | "bugfix" => Some(Self::Bugfix),
            "refactor" => Some(Self::Refactor),
            "testing" => Some(Self::Testing),
            "docs" => Some(Self::Docs),
            "config" => Some(Self::Config),
            "ops" => Some(Self::Ops),
            "planning" => Some(Self::Planning),
            "explanation" => Some(Self::Explanation),
            "architecture" => Some(Self::Architecture),
            _ => None,
        }
    }

    /// Get the parent L1 category for this L2 category.
    pub fn parent_l1(&self) -> CategoryL1 {
        match self {
            Self::Feature | Self::Bugfix | Self::Refactor | Self::Testing => CategoryL1::Code,
            Self::Docs | Self::Config | Self::Ops => CategoryL1::Support,
            Self::Planning | Self::Explanation | Self::Architecture => CategoryL1::Thinking,
        }
    }
}

/// Third-level category (L3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "kebab-case")]
pub enum CategoryL3 {
    // Feature
    NewComponent,
    AddFunctionality,
    Integration,
    // Bugfix
    ErrorFix,
    LogicFix,
    PerformanceFix,
    // Refactor
    Cleanup,
    PatternMigration,
    DependencyUpdate,
    // Testing
    UnitTests,
    IntegrationTests,
    TestFixes,
    // Docs
    CodeComments,
    ReadmeGuides,
    ApiDocs,
    // Config
    EnvSetup,
    BuildTooling,
    Dependencies,
    // Ops
    CiCd,
    Deployment,
    Monitoring,
    // Planning
    Brainstorming,
    DesignDoc,
    TaskBreakdown,
    // Explanation
    CodeUnderstanding,
    ConceptLearning,
    DebugInvestigation,
    // Architecture
    SystemDesign,
    DataModeling,
    ApiDesign,
}

impl CategoryL3 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewComponent => "new-component",
            Self::AddFunctionality => "add-functionality",
            Self::Integration => "integration",
            Self::ErrorFix => "error-fix",
            Self::LogicFix => "logic-fix",
            Self::PerformanceFix => "performance-fix",
            Self::Cleanup => "cleanup",
            Self::PatternMigration => "pattern-migration",
            Self::DependencyUpdate => "dependency-update",
            Self::UnitTests => "unit-tests",
            Self::IntegrationTests => "integration-tests",
            Self::TestFixes => "test-fixes",
            Self::CodeComments => "code-comments",
            Self::ReadmeGuides => "readme-guides",
            Self::ApiDocs => "api-docs",
            Self::EnvSetup => "env-setup",
            Self::BuildTooling => "build-tooling",
            Self::Dependencies => "dependencies",
            Self::CiCd => "ci-cd",
            Self::Deployment => "deployment",
            Self::Monitoring => "monitoring",
            Self::Brainstorming => "brainstorming",
            Self::DesignDoc => "design-doc",
            Self::TaskBreakdown => "task-breakdown",
            Self::CodeUnderstanding => "code-understanding",
            Self::ConceptLearning => "concept-learning",
            Self::DebugInvestigation => "debug-investigation",
            Self::SystemDesign => "system-design",
            Self::DataModeling => "data-modeling",
            Self::ApiDesign => "api-design",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "new-component" => Some(Self::NewComponent),
            "add-functionality" => Some(Self::AddFunctionality),
            "integration" => Some(Self::Integration),
            "error-fix" => Some(Self::ErrorFix),
            "logic-fix" => Some(Self::LogicFix),
            "performance-fix" => Some(Self::PerformanceFix),
            "cleanup" => Some(Self::Cleanup),
            "pattern-migration" => Some(Self::PatternMigration),
            "dependency-update" => Some(Self::DependencyUpdate),
            "unit-tests" => Some(Self::UnitTests),
            "integration-tests" => Some(Self::IntegrationTests),
            "test-fixes" => Some(Self::TestFixes),
            "code-comments" => Some(Self::CodeComments),
            "readme-guides" => Some(Self::ReadmeGuides),
            "api-docs" => Some(Self::ApiDocs),
            "env-setup" => Some(Self::EnvSetup),
            "build-tooling" => Some(Self::BuildTooling),
            "dependencies" => Some(Self::Dependencies),
            "ci-cd" => Some(Self::CiCd),
            "deployment" => Some(Self::Deployment),
            "monitoring" => Some(Self::Monitoring),
            "brainstorming" => Some(Self::Brainstorming),
            "design-doc" => Some(Self::DesignDoc),
            "task-breakdown" => Some(Self::TaskBreakdown),
            "code-understanding" => Some(Self::CodeUnderstanding),
            "concept-learning" => Some(Self::ConceptLearning),
            "debug-investigation" => Some(Self::DebugInvestigation),
            "system-design" => Some(Self::SystemDesign),
            "data-modeling" => Some(Self::DataModeling),
            "api-design" => Some(Self::ApiDesign),
            _ => None,
        }
    }

    /// Get the parent L2 category for this L3 category.
    pub fn parent_l2(&self) -> CategoryL2 {
        match self {
            Self::NewComponent | Self::AddFunctionality | Self::Integration => CategoryL2::Feature,
            Self::ErrorFix | Self::LogicFix | Self::PerformanceFix => CategoryL2::Bugfix,
            Self::Cleanup | Self::PatternMigration | Self::DependencyUpdate => CategoryL2::Refactor,
            Self::UnitTests | Self::IntegrationTests | Self::TestFixes => CategoryL2::Testing,
            Self::CodeComments | Self::ReadmeGuides | Self::ApiDocs => CategoryL2::Docs,
            Self::EnvSetup | Self::BuildTooling | Self::Dependencies => CategoryL2::Config,
            Self::CiCd | Self::Deployment | Self::Monitoring => CategoryL2::Ops,
            Self::Brainstorming | Self::DesignDoc | Self::TaskBreakdown => CategoryL2::Planning,
            Self::CodeUnderstanding | Self::ConceptLearning | Self::DebugInvestigation => CategoryL2::Explanation,
            Self::SystemDesign | Self::DataModeling | Self::ApiDesign => CategoryL2::Architecture,
        }
    }
}

// ============================================================================
// Classification Input / Output
// ============================================================================

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

// ============================================================================
// Prompt Building
// ============================================================================

/// The system prompt with full taxonomy reference.
pub const SYSTEM_PROMPT: &str = r#"You are a session classifier for a coding assistant analytics tool. Your job is to categorize AI coding sessions based on the user's first prompt (the initial question or request).

## Taxonomy

Classify each session into exactly ONE category from this hierarchy:

### Code Work
- **feature/new-component**: Creating new UI components, pages, modules from scratch
- **feature/add-functionality**: Adding capabilities to existing code
- **feature/integration**: Connecting systems, APIs, external services
- **bugfix/error-fix**: Fixing errors, exceptions, crashes
- **bugfix/logic-fix**: Correcting business logic, wrong behavior
- **bugfix/performance-fix**: Resolving slowness, memory issues, optimization
- **refactor/cleanup**: Code style, formatting, removing dead code
- **refactor/pattern-migration**: Moving to new patterns, architectures
- **refactor/dependency-update**: Updating packages, libraries, versions
- **testing/unit-tests**: Adding or fixing unit tests
- **testing/integration-tests**: E2E, API, browser tests
- **testing/test-fixes**: Fixing flaky or broken tests

### Support Work
- **docs/code-comments**: Inline documentation, JSDoc, docstrings
- **docs/readme-guides**: README, tutorials, user guides
- **docs/api-docs**: API documentation, OpenAPI specs
- **config/env-setup**: Environment variables, local dev setup
- **config/build-tooling**: Webpack, Vite, tsconfig, build configs
- **config/dependencies**: package.json, Cargo.toml, requirements.txt
- **ops/ci-cd**: GitHub Actions, CI pipelines, automation
- **ops/deployment**: Docker, K8s, hosting, infrastructure
- **ops/monitoring**: Logging, alerting, metrics, observability

### Thinking Work
- **planning/brainstorming**: Exploring ideas, comparing options
- **planning/design-doc**: Writing technical specs, PRDs
- **planning/task-breakdown**: Breaking work into subtasks
- **explanation/code-understanding**: "How does X work?", code walkthroughs
- **explanation/concept-learning**: Learning new technologies, concepts
- **explanation/debug-investigation**: Diagnosing issues, finding root cause
- **architecture/system-design**: High-level system architecture
- **architecture/data-modeling**: Database schemas, data structures
- **architecture/api-design**: API contracts, protocols, interfaces

## Classification Rules

1. Use ONLY the user's first prompt to classify (ignore later context)
2. If a prompt spans multiple categories, pick the PRIMARY intent
3. For ambiguous prompts, prefer the more specific category
4. If truly unclear, use the closest match with confidence < 0.7
5. Non-English prompts: Classify based on detected intent, or use closest match with low confidence

## Response Format

Return ONLY a JSON object (no markdown, no explanation):
{
  "classifications": [
    {
      "sessionId": "abc123",
      "category": "code/feature/new-component",
      "confidence": 0.92,
      "reasoning": "User asks to create a new React component"
    }
  ]
}

Confidence levels:
- 0.9-1.0: Very confident, clear intent
- 0.7-0.9: Confident, but some ambiguity
- 0.5-0.7: Uncertain, best guess
- <0.5: Very uncertain"#;

/// Truncate a preview string to a maximum length, at a word boundary if possible.
pub fn truncate_preview(preview: &str) -> String {
    const MAX_LEN: usize = 2000;
    const SUFFIX: &str = "... [truncated]";

    if preview.len() <= MAX_LEN {
        preview.to_string()
    } else {
        let truncate_at = MAX_LEN - SUFFIX.len();
        let boundary = preview[..truncate_at]
            .rfind(char::is_whitespace)
            .unwrap_or(truncate_at);
        format!("{}{}", &preview[..boundary], SUFFIX)
    }
}

/// Build the user prompt for batch classification.
pub fn build_batch_prompt(inputs: &[ClassificationInput]) -> String {
    let mut prompt = String::from("Classify these coding sessions:\n\n");

    for input in inputs {
        let preview = truncate_preview(&input.preview);
        let skills = if input.skills_used.is_empty() {
            "(none)".to_string()
        } else {
            // Limit to 10 skills
            let limited: Vec<&str> = input.skills_used.iter().take(10).map(|s| s.as_str()).collect();
            limited.join(", ")
        };

        prompt.push_str("---\n");
        prompt.push_str(&format!("Session ID: {}\n", input.session_id));
        prompt.push_str(&format!("First Prompt: {}\n", preview));
        prompt.push_str(&format!("Skills Used: {}\n", skills));
        prompt.push_str("---\n\n");
    }

    prompt.push_str(&format!("Return JSON with classifications for all {} sessions.", inputs.len()));
    prompt
}

// ============================================================================
// Response Parsing
// ============================================================================

/// Parse and validate a batch classification response from the LLM.
///
/// Handles:
/// - Direct JSON objects
/// - Claude CLI `{ "result": "..." }` wrapper
/// - Markdown code blocks around JSON
/// - Category strings like "code/feature/new-component"
pub fn parse_batch_response(raw: &str) -> Result<Vec<ValidatedClassification>, String> {
    // Strip markdown code blocks if present
    let cleaned = strip_markdown_json(raw);

    // Try to parse as the batch response format
    let response: BatchClassificationResponse = serde_json::from_str(&cleaned)
        .or_else(|_| {
            // Try Claude CLI wrapper format: { "result": "..." }
            let wrapper: serde_json::Value = serde_json::from_str(&cleaned)
                .map_err(|e| format!("JSON parse failed: {}", e))?;
            if let Some(result_str) = wrapper.get("result").and_then(|v| v.as_str()) {
                let inner_cleaned = strip_markdown_json(result_str);
                serde_json::from_str(&inner_cleaned)
                    .map_err(|e| format!("Inner JSON parse failed: {}", e))
            } else {
                Err("Not a valid classification response".to_string())
            }
        })
        .map_err(|e| format!("Failed to parse classification response: {}", e))?;

    let mut results = Vec::new();
    for item in response.classifications {
        match parse_category_string(&item.category) {
            Some((l1, l2, l3)) => {
                let confidence = item.confidence.clamp(0.0, 1.0);
                results.push(ValidatedClassification {
                    session_id: item.session_id,
                    l1: l1.to_string(),
                    l2: l2.to_string(),
                    l3: l3.to_string(),
                    confidence,
                    reasoning: item.reasoning,
                });
            }
            None => {
                // Skip invalid categories but log them
                tracing::warn!(
                    session_id = %item.session_id,
                    category = %item.category,
                    "Skipping invalid category string"
                );
            }
        }
    }

    Ok(results)
}

/// Parse a category string like "code/feature/new-component" into (l1, l2, l3) components.
pub fn parse_category_string(category: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = category.split('/').collect();
    if parts.len() != 3 {
        return None;
    }

    let l1 = parts[0];
    let l2 = parts[1];
    let l3 = parts[2];

    // Validate L1
    CategoryL1::parse(l1)?;

    // Validate L2
    CategoryL2::parse(l2)?;

    // Validate L3
    CategoryL3::parse(l3)?;

    Some((l1, l2, l3))
}

/// Strip markdown code block fences from a string containing JSON.
fn strip_markdown_json(s: &str) -> String {
    let trimmed = s.trim();

    // Strip ```json ... ``` or ``` ... ```
    if trimmed.starts_with("```") {
        let start = if let Some(newline_pos) = trimmed.find('\n') {
            newline_pos + 1
        } else {
            return trimmed.to_string();
        };

        let end = trimmed.rfind("```").unwrap_or(trimmed.len());
        if end > start {
            return trimmed[start..end].trim().to_string();
        }
    }

    trimmed.to_string()
}

// ============================================================================
// Cost Estimation
// ============================================================================

/// Estimate the classification cost in cents.
///
/// Uses approximate token counts for Claude Haiku:
/// - System prompt: ~800 tokens
/// - Per session: ~200 tokens input, ~50 tokens output
/// - Haiku pricing: $0.25/MTok input, $1.25/MTok output
pub fn estimate_cost_cents(session_count: i64, batch_size: i64) -> i64 {
    let num_batches = (session_count + batch_size - 1) / batch_size;
    let system_tokens = 800; // per batch
    let per_session_input = 200;
    let per_session_output = 50;

    let total_input = num_batches * system_tokens + session_count * per_session_input;
    let total_output = session_count * per_session_output;

    // Haiku: $0.25/MTok input, $1.25/MTok output
    // Convert to cents: $0.25/1M = 0.025 cents/1K tokens
    let input_cost_cents = total_input as f64 * 0.025 / 1000.0;
    let output_cost_cents = total_output as f64 * 0.125 / 1000.0;

    (input_cost_cents + output_cost_cents).ceil() as i64
}

/// Estimate duration in seconds for classification.
///
/// Assumes ~2 seconds per batch (Claude CLI overhead + Haiku response time).
pub fn estimate_duration_secs(session_count: i64, batch_size: i64) -> i64 {
    let num_batches = (session_count + batch_size - 1) / batch_size;
    num_batches * 2 // ~2 seconds per batch
}

// ============================================================================
// Constants
// ============================================================================

/// Default batch size for classification (sessions per LLM call).
pub const BATCH_SIZE: usize = 5;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_l1_roundtrip() {
        // New canonical form
        assert_eq!(CategoryL1::parse("code_work"), Some(CategoryL1::Code));
        assert_eq!(CategoryL1::parse("support_work"), Some(CategoryL1::Support));
        assert_eq!(CategoryL1::parse("thinking_work"), Some(CategoryL1::Thinking));
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
        assert_eq!(CategoryL3::SystemDesign.parent_l2(), CategoryL2::Architecture);
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
    fn test_estimate_cost_cents() {
        let cost = estimate_cost_cents(100, 5);
        assert!(cost > 0);
        assert!(cost < 100); // Should be well under $1 for 100 sessions
    }

    #[test]
    fn test_estimate_duration_secs() {
        let duration = estimate_duration_secs(100, 5);
        assert_eq!(duration, 40); // 20 batches * 2 sec
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
}
