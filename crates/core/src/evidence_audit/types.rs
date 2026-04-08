//! Baseline structs, line signals, aggregated signals, and audit result types.

use crate::field_inventory::FieldInventory;
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};

// ─── Baseline Structs ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Baseline {
    pub top_level_types: TopLevelTypes,
    pub content_block_types: ContentBlockTypes,
    pub system_subtypes: SystemSubtypes,
    pub progress_data_types: ProgressDataTypes,
    pub thinking_block_keys: ThinkingBlockKeys,
    #[serde(default)]
    pub field_extraction_paths: Option<FieldExtractionPaths>,
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

#[derive(Debug, Deserialize)]
pub struct FieldExtractionPaths {
    pub extracted: Vec<String>,
    pub intentionally_ignored: Vec<String>,
}

// ─── Line Signals ───────────────────────────────────────────────

/// Signals extracted from a single JSONL line.
#[derive(Debug, Default)]
pub struct LineSignals {
    pub top_level_type: Option<String>,
    pub subtype: Option<String>,
    pub data_type: Option<String>,
    pub content_block_types: Vec<String>,
    pub thinking_key_sets: Vec<BTreeSet<String>>,
    pub nesting_direct: bool,
    pub nesting_nested: bool,
}

// ─── Aggregated Signals ─────────────────────────────────────────

/// Accumulated signals across many JSONL lines/files.
#[derive(Debug, Default)]
pub struct AggregatedSignals {
    pub top_level_types: HashSet<String>,
    pub system_subtypes: HashSet<String>,
    pub progress_data_types: HashSet<String>,
    pub assistant_content_block_types: HashSet<String>,
    pub thinking_key_sets: HashSet<BTreeSet<String>>,
    pub nesting_direct_count: usize,
    pub nesting_nested_count: usize,
    pub field_inventory: FieldInventory,
    pub files_scanned: usize,
    pub lines_scanned: usize,
    pub errors: usize,
}

impl AggregatedSignals {
    pub fn merge(&mut self, other: AggregatedSignals) {
        self.top_level_types.extend(other.top_level_types);
        self.system_subtypes.extend(other.system_subtypes);
        self.progress_data_types.extend(other.progress_data_types);
        self.assistant_content_block_types
            .extend(other.assistant_content_block_types);
        self.thinking_key_sets.extend(other.thinking_key_sets);
        self.nesting_direct_count += other.nesting_direct_count;
        self.nesting_nested_count += other.nesting_nested_count;
        self.field_inventory.merge(&other.field_inventory);
        self.files_scanned += other.files_scanned;
        self.lines_scanned += other.lines_scanned;
        self.errors += other.errors;
    }

    pub fn ingest(&mut self, signals: LineSignals) {
        if let Some(ref t) = signals.top_level_type {
            self.top_level_types.insert(t.clone());

            match t.as_str() {
                "system" => {
                    if let Some(sub) = signals.subtype {
                        self.system_subtypes.insert(sub);
                    }
                }
                "progress" => {
                    if let Some(dt) = signals.data_type {
                        self.progress_data_types.insert(dt);
                    }
                    if signals.nesting_direct {
                        self.nesting_direct_count += 1;
                    }
                    if signals.nesting_nested {
                        self.nesting_nested_count += 1;
                    }
                }
                "assistant" => {
                    for bt in &signals.content_block_types {
                        self.assistant_content_block_types.insert(bt.clone());
                    }
                    for ks in signals.thinking_key_sets {
                        self.thinking_key_sets.insert(ks);
                    }
                }
                _ => {}
            }
        }
    }
}

// ─── Audit Results ──────────────────────────────────────────────

/// Result of a single set-difference check.
#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub new_items: Vec<String>,
    pub absent_items: Vec<String>,
}

/// Overall audit result across all checks.
#[derive(Debug)]
pub struct AuditResult {
    pub passed: bool,
    pub checks: Vec<CheckResult>,
    pub nesting_direct_count: usize,
    pub nesting_nested_count: usize,
    pub files_scanned: usize,
    pub lines_scanned: usize,
    pub errors: usize,
}

// ─── Sorted Trait ───────────────────────────────────────────────

pub(crate) trait Sorted: Iterator {
    fn sorted(self) -> Vec<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        let mut v: Vec<Self::Item> = self.collect();
        v.sort();
        v
    }
}

impl<I: Iterator> Sorted for I {}
