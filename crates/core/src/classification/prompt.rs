// crates/core/src/classification/prompt.rs
//! System prompt template and batch prompt builder for classification.

use super::types::ClassificationInput;

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
            let limited: Vec<&str> = input
                .skills_used
                .iter()
                .take(10)
                .map(|s| s.as_str())
                .collect();
            limited.join(", ")
        };

        prompt.push_str("---\n");
        prompt.push_str(&format!("Session ID: {}\n", input.session_id));
        prompt.push_str(&format!("First Prompt: {}\n", preview));
        prompt.push_str(&format!("Skills Used: {}\n", skills));
        prompt.push_str("---\n\n");
    }

    prompt.push_str(&format!(
        "Return JSON with classifications for all {} sessions.",
        inputs.len()
    ));
    prompt
}
