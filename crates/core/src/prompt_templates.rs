//! Template detection for prompt history via regex slot replacement + Drain clustering.

use regex_lite::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A detected prompt template with variable slots.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub pattern: String,
    pub frequency: usize,
    pub examples: Vec<String>,
    pub slots: Vec<String>,
    pub projects: Vec<String>,
}

/// A Drain cluster (token-level structural similarity).
#[derive(Debug, Clone)]
pub struct DrainCluster {
    pub template: String,
    pub members: Vec<String>,
}

/// Layer 1: Replace known variable types with named slots.
pub fn normalize_to_template(text: &str) -> String {
    static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            ("<PATH>", Regex::new(r#"/?Users/[^\s'"\\]+"#).unwrap()),
            (
                "<FILE>",
                Regex::new(
                    r"@?[\w./][\w./-]*\.(md|ts|tsx|rs|json|toml|yaml|yml|js|jsx|css|html|py|sh|sql|env)",
                )
                .unwrap(),
            ),
            ("<URL>", Regex::new(r"https?://\S+").unwrap()),
            (
                "<UUID>",
                Regex::new(
                    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
                )
                .unwrap(),
            ),
            ("<HASH>", Regex::new(r"\b[0-9a-f]{7,40}\b").unwrap()),
            ("<NUM>", Regex::new(r"\b\d{2,}\b").unwrap()),
        ]
    });

    let mut result = text.to_string();
    for (slot, re) in patterns.iter() {
        result = re.replace_all(&result, *slot).to_string();
    }
    result
}

/// Layer 2: Drain-style token clustering.
pub fn drain_cluster(prompts: &[&str], threshold: f64) -> Vec<DrainCluster> {
    let mut by_length: HashMap<usize, Vec<(Vec<String>, String)>> = HashMap::new();
    for &p in prompts {
        let tokens: Vec<String> = p.split_whitespace().map(|t| t.to_lowercase()).collect();
        let len = tokens.len();
        if len >= 2 && len <= 40 {
            by_length
                .entry(len)
                .or_default()
                .push((tokens, p.to_string()));
        }
    }

    let mut all_clusters = Vec::new();

    for (_len, items) in &by_length {
        let mut clusters: Vec<(Vec<String>, Vec<String>)> = Vec::new();

        for (tokens, original) in items {
            let mut matched = false;
            for (tmpl, members) in &mut clusters {
                let matching = tokens
                    .iter()
                    .zip(tmpl.iter())
                    .filter(|(a, b)| a == b)
                    .count();
                if matching as f64 / tokens.len() as f64 > threshold {
                    for (t, tmpl_t) in tokens.iter().zip(tmpl.iter_mut()) {
                        if t != tmpl_t && *tmpl_t != "<*>" {
                            *tmpl_t = "<*>".to_string();
                        }
                    }
                    members.push(original.clone());
                    matched = true;
                    break;
                }
            }
            if !matched {
                clusters.push((tokens.clone(), vec![original.clone()]));
            }
        }

        for (tmpl, members) in clusters {
            if members.len() >= 2 {
                all_clusters.push(DrainCluster {
                    template: tmpl.join(" "),
                    members,
                });
            }
        }
    }

    all_clusters.sort_by(|a, b| b.members.len().cmp(&a.members.len()));
    all_clusters
}

/// Combined template detection: regex slots (Layer 1) then Drain (Layer 2).
pub fn detect_templates(prompts: &[&str], min_frequency: usize) -> Vec<PromptTemplate> {
    let mut slot_groups: HashMap<String, Vec<String>> = HashMap::new();
    for &p in prompts {
        let normalized = normalize_to_template(p);
        slot_groups
            .entry(normalized)
            .or_default()
            .push(p.to_string());
    }

    let mut templates = Vec::new();

    for (pattern, examples) in &slot_groups {
        if examples.len() >= min_frequency {
            let slots: Vec<String> = ["<FILE>", "<PATH>", "<URL>", "<UUID>", "<NUM>"]
                .iter()
                .filter(|s| pattern.contains(**s))
                .map(|s| s.trim_matches('<').trim_matches('>').to_string())
                .collect();

            templates.push(PromptTemplate {
                pattern: pattern.clone(),
                frequency: examples.len(),
                examples: examples.iter().take(5).cloned().collect(),
                slots,
                projects: Vec::new(),
            });
        }
    }

    // Layer 2: run Drain on prompts NOT already captured by Layer 1
    let captured: std::collections::HashSet<&str> = slot_groups
        .iter()
        .filter(|(_, v)| v.len() >= min_frequency)
        .flat_map(|(_, v)| v.iter().map(|s| s.as_str()))
        .collect();

    let uncaptured: Vec<&str> = prompts
        .iter()
        .copied()
        .filter(|p| !captured.contains(*p))
        .collect();

    let drain_results = drain_cluster(&uncaptured, 0.5);
    for cluster in drain_results {
        if cluster.members.len() >= min_frequency {
            let has_wildcard = cluster.template.contains("<*>");
            templates.push(PromptTemplate {
                pattern: cluster.template,
                frequency: cluster.members.len(),
                examples: cluster.members.into_iter().take(5).collect(),
                slots: if has_wildcard {
                    vec!["*".to_string()]
                } else {
                    Vec::new()
                },
                projects: Vec::new(),
            });
        }
    }

    templates.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_slot_replaces_file_path() {
        let result = normalize_to_template("review @docs/plan.md is this ready");
        assert!(result.contains("<FILE>"));
        assert!(!result.contains("plan.md"));
    }

    #[test]
    fn regex_slot_replaces_absolute_path() {
        let result = normalize_to_template("'/Users/TBGor/dev/project/file.rs'");
        assert!(result.contains("<PATH>"));
    }

    #[test]
    fn regex_slot_replaces_numbers() {
        let result = normalize_to_template("is the branch 100/100 ready?");
        assert!(result.contains("<NUM>"));
    }

    #[test]
    fn drain_clusters_similar_prompts() {
        let prompts = vec![
            "use uiux pro max skills for UI design",
            "use UIUX pro max skills to enhance it",
            "use uiux pro max skills for this building",
        ];
        let clusters = drain_cluster(&prompts, 0.5);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members.len(), 3);
        assert!(clusters[0].template.contains("<*>"));
    }

    #[test]
    fn drain_separates_different_prompts() {
        let prompts = vec![
            "fix the auth error",
            "fix the auth error",
            "create a new database migration",
            "create a new database migration",
        ];
        let clusters = drain_cluster(&prompts, 0.5);
        assert!(clusters.len() >= 2);
    }

    #[test]
    fn detect_templates_combines_both_layers() {
        let prompts = vec![
            "review @docs/plan-a.md is this ready",
            "review @docs/plan-b.md is this ready",
            "review @docs/plan-c.md is this ready",
        ];
        let templates = detect_templates(&prompts, 2);
        assert!(!templates.is_empty());
        assert!(templates[0].pattern.contains("<FILE>"));
        assert!(templates[0].frequency >= 3);
    }
}
