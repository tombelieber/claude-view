//! Context digest builder and report prompt template for AI work summaries.

use serde::{Deserialize, Serialize};

/// Type of report to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportType {
    Daily,
    Weekly,
    Custom,
}

impl ReportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for ReportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured context digest fed to the AI prompt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextDigest {
    pub report_type: String,
    pub date_range: String,
    pub projects: Vec<ProjectDigest>,
    pub top_tools: Vec<String>,
    pub top_skills: Vec<String>,
    pub summary_line: String,
}

/// Per-project context within the digest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDigest {
    pub name: String,
    pub branches: Vec<BranchDigest>,
    pub session_count: usize,
    pub commit_count: usize,
    pub total_duration_secs: i64,
}

/// Per-branch context within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDigest {
    pub name: String,
    pub sessions: Vec<SessionDigest>,
}

/// Minimal session context for the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDigest {
    pub first_prompt: String,
    pub category: Option<String>,
    pub duration_secs: i64,
}

/// Maximum character length for session first_prompt in the digest.
const MAX_PROMPT_LEN: usize = 100;

impl ContextDigest {
    /// Format the digest as structured text for the AI prompt.
    pub fn to_prompt_text(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("Report: {} ({})\n", self.report_type, self.date_range));
        out.push_str(&format!("Summary: {}\n\n", self.summary_line));

        for project in &self.projects {
            out.push_str(&format!(
                "## {} — {} sessions, {} commits, {}s total\n",
                project.name, project.session_count, project.commit_count, project.total_duration_secs
            ));

            for branch in &project.branches {
                out.push_str(&format!("  Branch: {}\n", branch.name));
                for session in &branch.sessions {
                    let prompt = truncate_prompt(&session.first_prompt, MAX_PROMPT_LEN);
                    let cat = session.category.as_deref().unwrap_or("uncategorized");
                    out.push_str(&format!("    - [{}] {} ({}s)\n", cat, prompt, session.duration_secs));
                }
            }
            out.push('\n');
        }

        if !self.top_tools.is_empty() {
            out.push_str(&format!("Top tools: {}\n", self.top_tools.join(", ")));
        }
        if !self.top_skills.is_empty() {
            out.push_str(&format!("Top skills: {}\n", self.top_skills.join(", ")));
        }

        out
    }
}

/// Truncate a prompt string to `max_len` chars, appending "..." if truncated.
fn truncate_prompt(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Build the full report generation prompt from a context digest.
///
/// Returns the system+user prompt to send to the LLM.
pub fn build_report_prompt(digest: &ContextDigest) -> String {
    let context = digest.to_prompt_text();

    format!(
        r#"You are a concise technical writer summarizing AI-assisted development work.

Given the session data below, produce a work report as markdown bullet points.

Rules:
- Write 5-8 bullet points summarizing what was accomplished
- Group by theme/project, not chronologically
- Use active voice ("Implemented X", "Fixed Y", "Refactored Z")
- Include specific technical details (file names, feature names, bug descriptions)
- Mention tools and skills used when relevant
- End with a "Key metrics" line: sessions, duration, projects
- Do NOT invent work not present in the data
- Do NOT include any preamble or closing — just the bullets

---

{context}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_digest() -> ContextDigest {
        ContextDigest {
            report_type: "daily".to_string(),
            date_range: "2026-02-21".to_string(),
            projects: vec![
                ProjectDigest {
                    name: "claude-view".to_string(),
                    branches: vec![BranchDigest {
                        name: "feat/reports".to_string(),
                        sessions: vec![
                            SessionDigest {
                                first_prompt: "Add reports DB migration".to_string(),
                                category: Some("feature".to_string()),
                                duration_secs: 1800,
                            },
                            SessionDigest {
                                first_prompt: "Fix search indexer bug where queries with special chars crash".to_string(),
                                category: Some("bugfix".to_string()),
                                duration_secs: 900,
                            },
                        ],
                    }],
                    session_count: 2,
                    commit_count: 4,
                    total_duration_secs: 2700,
                },
                ProjectDigest {
                    name: "vicky-wiki".to_string(),
                    branches: vec![BranchDigest {
                        name: "main".to_string(),
                        sessions: vec![SessionDigest {
                            first_prompt: "Update README".to_string(),
                            category: Some("docs".to_string()),
                            duration_secs: 600,
                        }],
                    }],
                    session_count: 1,
                    commit_count: 1,
                    total_duration_secs: 600,
                },
            ],
            top_tools: vec!["Read".to_string(), "Edit".to_string(), "Bash".to_string()],
            top_skills: vec!["/commit".to_string(), "/review-pr".to_string()],
            summary_line: "3 sessions across 2 projects".to_string(),
        }
    }

    #[test]
    fn test_to_prompt_text_contains_project_names() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("claude-view"), "Should contain project name");
        assert!(text.contains("vicky-wiki"), "Should contain second project");
    }

    #[test]
    fn test_to_prompt_text_contains_branches() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("feat/reports"), "Should contain branch name");
        assert!(text.contains("main"), "Should contain main branch");
    }

    #[test]
    fn test_to_prompt_text_contains_session_prompts() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("Add reports DB migration"));
        assert!(text.contains("Update README"));
    }

    #[test]
    fn test_to_prompt_text_contains_tools_and_skills() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("Read, Edit, Bash"));
        assert!(text.contains("/commit, /review-pr"));
    }

    #[test]
    fn test_build_report_prompt_contains_instructions() {
        let digest = sample_digest();
        let prompt = build_report_prompt(&digest);
        assert!(prompt.contains("5-8 bullet points"), "Should contain bullet point instruction");
        assert!(prompt.contains("active voice"), "Should contain active voice instruction");
        assert!(prompt.contains("Do NOT invent work"), "Should contain guardrail");
    }

    #[test]
    fn test_build_report_prompt_contains_context() {
        let digest = sample_digest();
        let prompt = build_report_prompt(&digest);
        assert!(prompt.contains("claude-view"), "Prompt should include project data");
        assert!(prompt.contains("feat/reports"), "Prompt should include branch data");
    }

    #[test]
    fn test_empty_digest_produces_valid_prompt() {
        let digest = ContextDigest::default();
        let prompt = build_report_prompt(&digest);
        assert!(prompt.contains("5-8 bullet points"), "Even empty digest should have instructions");
        assert!(!prompt.contains("Top tools:"), "Empty digest should not have tools section");
    }

    #[test]
    fn test_long_prompt_truncation() {
        let long_prompt = "a".repeat(200);
        let truncated = truncate_prompt(&long_prompt, MAX_PROMPT_LEN);
        assert_eq!(truncated.len(), MAX_PROMPT_LEN + 3); // +3 for "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_short_prompt_not_truncated() {
        let short = "Fix bug";
        let result = truncate_prompt(short, MAX_PROMPT_LEN);
        assert_eq!(result, "Fix bug");
        assert!(!result.contains("..."));
    }

    #[test]
    fn test_report_type_as_str() {
        assert_eq!(ReportType::Daily.as_str(), "daily");
        assert_eq!(ReportType::Weekly.as_str(), "weekly");
        assert_eq!(ReportType::Custom.as_str(), "custom");
    }

    #[test]
    fn test_report_type_display() {
        assert_eq!(format!("{}", ReportType::Daily), "daily");
        assert_eq!(format!("{}", ReportType::Weekly), "weekly");
    }

    #[test]
    fn test_to_prompt_text_contains_categories() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("[feature]"), "Should show category");
        assert!(text.contains("[bugfix]"), "Should show bugfix category");
        assert!(text.contains("[docs]"), "Should show docs category");
    }

    #[test]
    fn test_to_prompt_text_contains_durations() {
        let digest = sample_digest();
        let text = digest.to_prompt_text();
        assert!(text.contains("1800s"), "Should show session duration");
        assert!(text.contains("2700s"), "Should show project total duration");
    }
}
