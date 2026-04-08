// crates/core/src/llm/claude_cli/prompt.rs
//! Classification prompt construction for the Claude CLI provider.

use crate::llm::types::ClassificationRequest;

/// Build the classification prompt for a session.
pub fn build_classification_prompt(request: &ClassificationRequest) -> String {
    format!(
        r#"You are a JSON classifier. Output ONLY a JSON object, no other text.

Classify this Claude Code session based on the available information. The prompt may be truncated — classify using your best judgment from whatever is provided. Never refuse or ask for more context.

First prompt: {}
Files touched: {}
Skills used: {}

L1: code_work | support_work | thinking_work
L2 (code_work): feature | bugfix | refactor | testing
L2 (support_work): docs | config | ops
L2 (thinking_work): planning | explanation | architecture
L3: feature→new-component|add-functionality|integration, bugfix→error-fix|logic-fix|performance-fix, refactor→cleanup|pattern-migration|dependency-update, testing→unit-tests|integration-tests|test-fixes, docs→code-comments|readme-guides|api-docs, config→env-setup|build-tooling|dependencies, ops→ci-cd|deployment|monitoring, planning→brainstorming|design-doc|task-breakdown, explanation→code-understanding|concept-learning|debug-investigation, architecture→system-design|data-modeling|api-design

{{"category_l1":"...","category_l2":"...","category_l3":"...","confidence":0.0}}"#,
        request.first_prompt,
        request.files_touched.join(", "),
        request.skills_used.join(", ")
    )
}
