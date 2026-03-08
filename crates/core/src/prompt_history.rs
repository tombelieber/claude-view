//! Parser for ~/.claude/history.jsonl — the user's prompt history log.

use chrono::{Datelike, Timelike};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;
use ts_rs::TS;

// ============================================================================
// Core types
// ============================================================================

/// A single entry from history.jsonl.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptEntry {
    pub display: String,
    #[serde(default, rename = "pastedContents")]
    pub pasted_contents: Option<HashMap<String, PasteContent>>,
    #[serde(rename = "timestamp")]
    pub timestamp_ms: u64,
    pub project: String,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasteContent {
    pub id: u64,
    #[serde(rename = "type")]
    pub content_type: String,
    pub content: String,
}

impl PromptEntry {
    /// Extract the last path component as a human-readable project name.
    pub fn project_display_name(&self) -> &str {
        self.project
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.project)
    }

    /// Convert millisecond timestamp to seconds.
    pub fn timestamp_secs(&self) -> i64 {
        (self.timestamp_ms / 1000) as i64
    }

    /// Concatenate all paste contents into a single string, if any exist.
    pub fn paste_text(&self) -> Option<String> {
        let pastes = self.pasted_contents.as_ref()?;
        if pastes.is_empty() {
            return None;
        }
        let text: String = pastes
            .values()
            .map(|p| p.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

// ============================================================================
// JSONL parser
// ============================================================================

/// Parse a history.jsonl file into a vec of prompt entries.
/// Malformed lines are skipped with a debug log.
pub async fn parse_history(path: &Path) -> Result<Vec<PromptEntry>, std::io::Error> {
    let file = tokio::fs::File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut entries = Vec::new();
    let mut line_number = 0u64;

    while let Some(line) = lines.next_line().await? {
        line_number += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<PromptEntry>(trimmed) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                debug!(line = line_number, error = %e, "skipping malformed history line");
            }
        }
    }
    Ok(entries)
}

// ============================================================================
// Intent classification
// ============================================================================

/// Classify a prompt's intent based on keywords in the display text.
pub fn classify_intent(display: &str) -> &'static str {
    if display.starts_with('/') {
        return "command";
    }
    let lower = display.to_lowercase();

    static CONFIRM_RE: OnceLock<Regex> = OnceLock::new();
    let confirm_re = CONFIRM_RE.get_or_init(|| {
        Regex::new(r"^(yes|ok|y|lgtm|go|continue|do it|sure|ys|go ahead)\b").unwrap()
    });
    if confirm_re.is_match(&lower) {
        return "confirm";
    }

    static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            (
                "fix",
                Regex::new(r"\b(fix|debug|error|bug|crash|fail|exception|broken)\b").unwrap(),
            ),
            (
                "create",
                Regex::new(r"\b(create|add|build|implement|make|generate|write|scaffold)\b")
                    .unwrap(),
            ),
            (
                "review",
                Regex::new(r"\b(review|check|verify|audit|ensure|validate|test|shippable)\b")
                    .unwrap(),
            ),
            (
                "explain",
                Regex::new(r"\b(explain|what|why|how|describe|question|understand)\b").unwrap(),
            ),
            (
                "ship",
                Regex::new(r"\b(commit|push|pr|merge|ship|deploy|release)\b").unwrap(),
            ),
            (
                "refactor",
                Regex::new(r"\b(refactor|improve|optimize|clean|simplify|rename)\b").unwrap(),
            ),
        ]
    });

    for (intent, re) in patterns.iter() {
        if re.is_match(&lower) {
            return intent;
        }
    }
    "other"
}

// ============================================================================
// Complexity bucketing
// ============================================================================

/// Bucket a prompt by length into a complexity tier.
pub fn complexity_bucket(display: &str) -> &'static str {
    match display.len() {
        0..=10 => "micro",
        11..=50 => "short",
        51..=200 => "medium",
        201..=500 => "detailed",
        _ => "long",
    }
}

// ============================================================================
// Wire types (Rust -> TypeScript via ts-rs)
// ============================================================================

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PromptStats {
    pub total_prompts: usize,
    pub unique_projects: usize,
    pub days_covered: u64,
    pub prompts_per_day: f64,
    pub night_owl_ratio: f64,
    pub intent_breakdown: HashMap<String, usize>,
    pub hourly_distribution: Vec<usize>,
    pub top_projects: Vec<ProjectCount>,
    pub peak_hour: u8,
    pub peak_day: u8,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProjectCount {
    pub name: String,
    pub count: usize,
}

// ============================================================================
// Stats computation
// ============================================================================

/// Compute aggregate statistics from a slice of prompt entries.
pub fn compute_stats(entries: &[PromptEntry]) -> PromptStats {
    let mut projects = HashSet::new();
    let mut intents: HashMap<String, usize> = HashMap::new();
    let mut hourly = vec![0usize; 24];
    let mut daily = [0usize; 7];
    let mut project_counts: HashMap<String, usize> = HashMap::new();
    let mut min_ts = u64::MAX;
    let mut max_ts = 0u64;

    for entry in entries {
        let proj_name = entry.project_display_name().to_string();
        projects.insert(entry.project.clone());
        *project_counts.entry(proj_name).or_default() += 1;

        let intent = classify_intent(&entry.display);
        *intents.entry(intent.to_string()).or_default() += 1;

        if entry.timestamp_ms > 0 {
            min_ts = min_ts.min(entry.timestamp_ms);
            max_ts = max_ts.max(entry.timestamp_ms);
            let secs = (entry.timestamp_ms / 1000) as i64;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                let hour = dt.hour() as usize;
                let dow = dt.weekday().num_days_from_monday() as usize;
                hourly[hour] += 1;
                daily[dow] += 1;
            }
        }
    }

    let days_covered = if max_ts > min_ts {
        ((max_ts - min_ts) / (1000 * 86400)) + 1
    } else {
        1
    };

    let night_hours = [22, 23, 0, 1, 2, 3, 4];
    let night: usize = night_hours.iter().map(|&h| hourly[h]).sum();
    let day: usize = (8..19).map(|h| hourly[h]).sum();
    let night_owl_ratio = if night + day > 0 {
        night as f64 / (night + day) as f64
    } else {
        0.0
    };

    let peak_hour = hourly
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(h, _)| h as u8)
        .unwrap_or(0);
    let peak_day = daily
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(d, _)| d as u8)
        .unwrap_or(0);

    let mut top_projects: Vec<ProjectCount> = project_counts
        .into_iter()
        .map(|(name, count)| ProjectCount { name, count })
        .collect();
    top_projects.sort_by(|a, b| b.count.cmp(&a.count));
    top_projects.truncate(15);

    PromptStats {
        total_prompts: entries.len(),
        unique_projects: projects.len(),
        days_covered,
        prompts_per_day: entries.len() as f64 / days_covered as f64,
        night_owl_ratio,
        intent_breakdown: intents,
        hourly_distribution: hourly,
        top_projects,
        peak_hour,
        peak_day,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Task 1: PromptEntry parsing --

    #[test]
    fn parse_prompt_entry_with_session_id() {
        let json = r#"{"display":"fix the auth error","pastedContents":null,"timestamp":1772924399103,"project":"/Users/TBGor/dev/claude-view","sessionId":"f9548f02-cd02-4644-a1a7-d1d81347c9b9"}"#;
        let entry: PromptEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.display, "fix the auth error");
        assert_eq!(entry.timestamp_ms, 1772924399103u64);
        assert_eq!(
            entry.session_id.as_deref(),
            Some("f9548f02-cd02-4644-a1a7-d1d81347c9b9")
        );
        assert!(entry.pasted_contents.is_none());
    }

    #[test]
    fn parse_prompt_entry_without_session_id() {
        let json = r#"{"display":"hello","pastedContents":{},"timestamp":1762138954132,"project":"/Users/TBGor/dev/project"}"#;
        let entry: PromptEntry = serde_json::from_str(json).unwrap();
        assert!(entry.session_id.is_none());
        assert_eq!(entry.project, "/Users/TBGor/dev/project");
    }

    #[test]
    fn parse_prompt_entry_with_paste_content() {
        let json = r#"{"display":"[Pasted text #1 +18 lines]","pastedContents":{"1":{"id":1,"type":"text","content":"error log here"}},"timestamp":1772924399103,"project":"/Users/TBGor/dev/proj"}"#;
        let entry: PromptEntry = serde_json::from_str(json).unwrap();
        let pastes = entry.pasted_contents.unwrap();
        assert_eq!(pastes.len(), 1);
        assert_eq!(pastes["1"].content, "error log here");
    }

    // -- Task 2: Intent classification --

    #[test]
    fn classify_intent_fix() {
        assert_eq!(classify_intent("fix the auth middleware error"), "fix");
    }

    #[test]
    fn classify_intent_create() {
        assert_eq!(
            classify_intent("create a new user registration form"),
            "create"
        );
    }

    #[test]
    fn classify_intent_confirm() {
        assert_eq!(classify_intent("yes go ahead"), "confirm");
    }

    #[test]
    fn classify_intent_slash_command() {
        assert_eq!(classify_intent("/clear"), "command");
    }

    #[test]
    fn classify_intent_other() {
        assert_eq!(
            classify_intent("so I was thinking about the design"),
            "other"
        );
    }

    // -- Task 2: Complexity bucketing --

    #[test]
    fn complexity_micro() {
        assert_eq!(complexity_bucket("fix it"), "micro");
    }

    #[test]
    fn complexity_detailed() {
        let long = "a".repeat(300);
        assert_eq!(complexity_bucket(&long), "detailed");
    }

    // -- Task 4: Stats computation --

    #[test]
    fn compute_stats_basic() {
        let entries = vec![
            PromptEntry {
                display: "fix the bug".into(),
                pasted_contents: None,
                timestamp_ms: 1772924399103,
                project: "/Users/TBGor/dev/proj-a".into(),
                session_id: Some("abc".into()),
            },
            PromptEntry {
                display: "/clear".into(),
                pasted_contents: None,
                timestamp_ms: 1772924400000,
                project: "/Users/TBGor/dev/proj-b".into(),
                session_id: None,
            },
        ];
        let stats = compute_stats(&entries);
        assert_eq!(stats.total_prompts, 2);
        assert_eq!(stats.unique_projects, 2);
        assert_eq!(stats.intent_breakdown.get("fix"), Some(&1));
        assert_eq!(stats.intent_breakdown.get("command"), Some(&1));
    }
}
