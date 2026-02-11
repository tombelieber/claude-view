use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFacet {
    pub underlying_goal: Option<String>,
    #[serde(default)]
    pub goal_categories: HashMap<String, i64>,
    pub outcome: Option<String>,
    #[serde(default)]
    pub user_satisfaction_counts: HashMap<String, i64>,
    pub claude_helpfulness: Option<String>,
    pub session_type: Option<String>,
    #[serde(default)]
    pub friction_counts: HashMap<String, i64>,
    #[serde(default)]
    pub friction_detail: Option<String>,
    pub primary_success: Option<String>,
    pub brief_summary: Option<String>,
    pub session_id: Option<String>,
}

impl SessionFacet {
    /// Returns the key with the highest count from `user_satisfaction_counts`,
    /// or `None` if the map is empty.
    pub fn dominant_satisfaction(&self) -> Option<&str> {
        self.user_satisfaction_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| key.as_str())
    }

    /// Returns `true` if `friction_counts` is non-empty.
    pub fn has_friction(&self) -> bool {
        !self.friction_counts.is_empty()
    }

    /// Returns the key with the highest count from `goal_categories`,
    /// or `None` if the map is empty.
    pub fn primary_goal(&self) -> Option<&str> {
        self.goal_categories
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| key.as_str())
    }
}

/// Scan a directory for facet JSON files, returning `(session_id_from_filename, parsed_facet)`.
///
/// - Skips non-`.json` files silently.
/// - Skips malformed JSON files with a `warn!` log.
/// - Returns `Ok(vec![])` if the directory does not exist (not an error).
pub fn scan_facet_cache(dir: &Path) -> std::io::Result<Vec<(String, SessionFacet)>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip non-.json files
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Extract session ID from filename (stem)
        let session_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(stem) => stem.to_string(),
            None => continue,
        };

        // Read and parse the file
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read facet file {}: {}", path.display(), e);
                continue;
            }
        };

        match serde_json::from_str::<SessionFacet>(&contents) {
            Ok(facet) => results.push((session_id, facet)),
            Err(e) => {
                warn!("Failed to parse facet file {}: {}", path.display(), e);
                continue;
            }
        }
    }

    Ok(results)
}

/// Returns the default path to the Claude Code facet cache directory.
pub fn default_facet_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude/usage-data/facets")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_facet_json() -> &'static str {
        r#"{
            "underlying_goal": "Implement a new feature",
            "goal_categories": {"coding": 3, "debugging": 1},
            "outcome": "success",
            "user_satisfaction_counts": {"satisfied": 5, "neutral": 2},
            "claude_helpfulness": "very_helpful",
            "session_type": "development",
            "friction_counts": {"slow_response": 1},
            "friction_detail": "Response was slow on large files",
            "primary_success": "Feature implemented correctly",
            "brief_summary": "User implemented a new caching feature with Claude's help",
            "session_id": "abc-123"
        }"#
    }

    #[test]
    fn test_parse_facet_json() {
        let facet: SessionFacet = serde_json::from_str(sample_facet_json()).unwrap();

        assert_eq!(facet.underlying_goal.as_deref(), Some("Implement a new feature"));
        assert_eq!(facet.goal_categories.get("coding"), Some(&3));
        assert_eq!(facet.goal_categories.get("debugging"), Some(&1));
        assert_eq!(facet.outcome.as_deref(), Some("success"));
        assert_eq!(facet.user_satisfaction_counts.get("satisfied"), Some(&5));
        assert_eq!(facet.user_satisfaction_counts.get("neutral"), Some(&2));
        assert_eq!(facet.claude_helpfulness.as_deref(), Some("very_helpful"));
        assert_eq!(facet.session_type.as_deref(), Some("development"));
        assert_eq!(facet.friction_counts.get("slow_response"), Some(&1));
        assert_eq!(
            facet.friction_detail.as_deref(),
            Some("Response was slow on large files")
        );
        assert_eq!(
            facet.primary_success.as_deref(),
            Some("Feature implemented correctly")
        );
        assert_eq!(
            facet.brief_summary.as_deref(),
            Some("User implemented a new caching feature with Claude's help")
        );
        assert_eq!(facet.session_id.as_deref(), Some("abc-123"));
    }

    #[test]
    fn test_parse_facet_with_empty_satisfaction() {
        let json = r#"{
            "underlying_goal": "Quick question",
            "goal_categories": {},
            "outcome": "success",
            "user_satisfaction_counts": {},
            "claude_helpfulness": "helpful",
            "session_type": "question",
            "friction_counts": {},
            "primary_success": null,
            "brief_summary": "Asked a quick question",
            "session_id": "def-456"
        }"#;

        let facet: SessionFacet = serde_json::from_str(json).unwrap();
        assert_eq!(facet.dominant_satisfaction(), None);
    }

    #[test]
    fn test_dominant_satisfaction() {
        let facet: SessionFacet = serde_json::from_str(sample_facet_json()).unwrap();
        // "satisfied": 5 beats "neutral": 2
        assert_eq!(facet.dominant_satisfaction(), Some("satisfied"));
    }

    #[test]
    fn test_has_friction() {
        let facet: SessionFacet = serde_json::from_str(sample_facet_json()).unwrap();
        assert!(facet.has_friction());

        // Facet with empty friction_counts
        let no_friction_json = r#"{
            "underlying_goal": null,
            "goal_categories": {},
            "outcome": null,
            "user_satisfaction_counts": {},
            "claude_helpfulness": null,
            "session_type": null,
            "friction_counts": {},
            "primary_success": null,
            "brief_summary": null,
            "session_id": null
        }"#;
        let no_friction: SessionFacet = serde_json::from_str(no_friction_json).unwrap();
        assert!(!no_friction.has_friction());
    }

    #[test]
    fn test_primary_goal() {
        let facet: SessionFacet = serde_json::from_str(sample_facet_json()).unwrap();
        // "coding": 3 beats "debugging": 1
        assert_eq!(facet.primary_goal(), Some("coding"));

        // Empty goal_categories
        let empty_json = r#"{
            "goal_categories": {}
        }"#;
        let empty: SessionFacet = serde_json::from_str(empty_json).unwrap();
        assert_eq!(empty.primary_goal(), None);
    }

    #[test]
    fn test_scan_facet_cache_dir() {
        let dir = TempDir::new().unwrap();
        let facet_path = dir.path().join("session-abc-123.json");
        fs::write(&facet_path, sample_facet_json()).unwrap();

        let results = scan_facet_cache(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "session-abc-123");
        assert_eq!(
            results[0].1.underlying_goal.as_deref(),
            Some("Implement a new feature")
        );
    }

    #[test]
    fn test_scan_facet_cache_empty_dir() {
        let dir = TempDir::new().unwrap();
        let results = scan_facet_cache(dir.path()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_facet_cache_missing_dir() {
        let results = scan_facet_cache(Path::new("/nonexistent/path/to/facets")).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_malformed_facet_skipped() {
        let dir = TempDir::new().unwrap();

        // Write a malformed JSON file
        let bad_path = dir.path().join("bad-session.json");
        fs::write(&bad_path, "{ this is not valid json }").unwrap();

        // Write a good JSON file
        let good_path = dir.path().join("good-session.json");
        fs::write(&good_path, sample_facet_json()).unwrap();

        // Also write a non-json file (should be skipped silently)
        let txt_path = dir.path().join("readme.txt");
        fs::write(&txt_path, "not a json file").unwrap();

        let results = scan_facet_cache(dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "good-session");
    }

    #[test]
    fn test_default_facet_cache_path() {
        let path = default_facet_cache_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains(".claude/usage-data/facets"),
            "Expected path to contain '.claude/usage-data/facets', got: {}",
            path_str
        );
    }
}
