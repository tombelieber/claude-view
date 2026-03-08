//! Evidence audit: validates parser/indexer type coverage against real JSONL data.
//!
//! Scans Claude Code JSONL files and compares structural inventories
//! against `evidence-baseline.json`. Catches drift before release.

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::Path;

// ─── Baseline Structs ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Baseline {
    pub top_level_types: TopLevelTypes,
    pub content_block_types: ContentBlockTypes,
    pub system_subtypes: SystemSubtypes,
    pub progress_data_types: ProgressDataTypes,
    pub thinking_block_keys: ThinkingBlockKeys,
}

#[derive(Debug, Deserialize)]
pub struct TopLevelTypes {
    pub handled: Vec<String>,
    pub handled_as_progress: Vec<String>,
    pub silently_ignored: Vec<String>,
    #[serde(default)]
    pub zero_occurrence_but_parser_has_arm: Vec<String>,
    #[serde(default)]
    pub zero_occurrence_not_in_parser: Vec<String>,
}

impl TopLevelTypes {
    pub fn all_known(&self) -> HashSet<String> {
        self.handled
            .iter()
            .chain(&self.handled_as_progress)
            .chain(&self.silently_ignored)
            .chain(&self.zero_occurrence_but_parser_has_arm)
            .chain(&self.zero_occurrence_not_in_parser)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockTypes {
    pub assistant: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SystemSubtypes {
    pub known: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProgressDataTypes {
    pub known: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ThinkingBlockKeys {
    pub required: Vec<String>,
}

pub fn load_baseline(path: &Path) -> Result<Baseline, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read baseline {}: {}", path.display(), e))?;
    serde_json::from_slice(&data).map_err(|e| format!("Failed to parse baseline JSON: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_deserializes_from_real_file() {
        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/integrity/evidence-baseline.json");
        let baseline = load_baseline(&baseline_path).expect("should deserialize baseline");

        // Verify top-level types
        assert!(
            baseline
                .top_level_types
                .handled
                .contains(&"assistant".to_string()),
            "handled should contain 'assistant'"
        );
        assert!(
            baseline
                .top_level_types
                .handled
                .contains(&"user".to_string()),
            "handled should contain 'user'"
        );
        assert!(
            baseline
                .top_level_types
                .handled_as_progress
                .contains(&"progress".to_string()),
            "handled_as_progress should contain 'progress'"
        );
        assert!(
            baseline
                .top_level_types
                .silently_ignored
                .contains(&"pr-link".to_string()),
            "silently_ignored should contain 'pr-link'"
        );

        // Verify all_known includes everything
        let all = baseline.top_level_types.all_known();
        assert!(all.contains("assistant"));
        assert!(all.contains("progress"));
        assert!(all.contains("pr-link"));
        assert!(all.contains("hook_event")); // zero_occurrence_not_in_parser

        // Verify content block types
        assert!(
            baseline
                .content_block_types
                .assistant
                .contains(&"thinking".to_string()),
            "assistant content blocks should contain 'thinking'"
        );

        // Verify system subtypes
        assert!(
            baseline
                .system_subtypes
                .known
                .contains(&"turn_duration".to_string()),
            "system subtypes should contain 'turn_duration'"
        );

        // Verify progress data types
        assert!(
            baseline
                .progress_data_types
                .known
                .contains(&"agent_progress".to_string()),
            "progress data types should contain 'agent_progress'"
        );

        // Verify thinking block keys
        assert!(
            baseline
                .thinking_block_keys
                .required
                .contains(&"signature".to_string()),
            "thinking block keys should contain 'signature'"
        );
    }
}
