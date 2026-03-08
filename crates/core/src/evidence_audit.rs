//! Evidence audit: validates parser/indexer type coverage against real JSONL data.
//!
//! Scans Claude Code JSONL files and compares structural inventories
//! against `evidence-baseline.json`. Catches drift before release.

use serde::de::{self, Deserializer as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

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

// ─── Line Signal Extraction ──────────────────────────────────────

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

/// Extract structural signals from a single JSONL line using streaming visitors.
///
/// Uses `serde_json::Deserializer` + `IgnoredAny` to correctly distinguish
/// top-level "type" fields from nested ones (e.g. "type":"message" inside content).
pub fn extract_line_signals(line: &[u8]) -> LineSignals {
    let mut deserializer = serde_json::Deserializer::from_slice(line);
    match deserializer.deserialize_map(TopLevelVisitor) {
        Ok(mut signals) => {
            // Only assistant messages have meaningful content block types
            if signals.top_level_type.as_deref() != Some("assistant") {
                signals.content_block_types.clear();
                signals.thinking_key_sets.clear();
            }
            signals
        }
        Err(_) => LineSignals::default(),
    }
}

// ─── TopLevelVisitor ─────────────────────────────────────────────

struct TopLevelVisitor;

impl<'de> Visitor<'de> for TopLevelVisitor {
    type Value = LineSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object (JSONL line)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<LineSignals, A::Error> {
        let mut signals = LineSignals::default();
        let mut message_signals: Option<MessageSignals> = None;
        let mut data_signals: Option<DataSignals> = None;

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "type" => {
                    signals.top_level_type = Some(map.next_value::<String>()?);
                }
                "subtype" => {
                    signals.subtype = Some(map.next_value::<String>()?);
                }
                "message" => {
                    message_signals = Some(map.next_value::<MessageSignals>()?);
                }
                "data" => {
                    data_signals = Some(map.next_value::<DataSignals>()?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        // Transfer message signals
        if let Some(ms) = message_signals {
            signals.content_block_types = ms.content_block_types;
            signals.thinking_key_sets = ms.thinking_key_sets;
        }

        // Transfer data signals
        if let Some(ds) = data_signals {
            signals.data_type = ds.data_type;
            signals.nesting_direct = ds.nesting_direct;
            signals.nesting_nested = ds.nesting_nested;
        }

        Ok(signals)
    }
}

// ─── MessageSignals ──────────────────────────────────────────────

#[derive(Debug, Default)]
struct MessageSignals {
    content_block_types: Vec<String>,
    thinking_key_sets: Vec<BTreeSet<String>>,
}

impl<'de> Deserialize<'de> for MessageSignals {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(MessageVisitor)
    }
}

struct MessageVisitor;

impl<'de> Visitor<'de> for MessageVisitor {
    type Value = MessageSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a message object with optional content array")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<MessageSignals, A::Error> {
        let mut signals = MessageSignals::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "content" => {
                    let content = map.next_value::<ContentArrayOrString>()?;
                    match content {
                        ContentArrayOrString::Blocks(blocks) => {
                            for block in blocks {
                                if let Some(t) = block.block_type {
                                    if t == "thinking" {
                                        signals.thinking_key_sets.push(block.all_keys);
                                    }
                                    signals.content_block_types.push(t);
                                }
                            }
                        }
                        ContentArrayOrString::Other => {}
                    }
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(signals)
    }
}

// ─── ContentArrayOrString ────────────────────────────────────────

enum ContentArrayOrString {
    Blocks(Vec<ContentBlock>),
    Other,
}

impl<'de> Deserialize<'de> for ContentArrayOrString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ContentArrayOrStringVisitor)
    }
}

struct ContentArrayOrStringVisitor;

impl<'de> Visitor<'de> for ContentArrayOrStringVisitor {
    type Value = ContentArrayOrString;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a content array, string, or null")
    }

    fn visit_str<E: de::Error>(self, _v: &str) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_none<E: de::Error>(self) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_unit<E: de::Error>(self) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ContentArrayOrString, A::Error> {
        let mut blocks = Vec::new();
        while let Some(block) = seq.next_element::<ContentBlock>()? {
            blocks.push(block);
        }
        Ok(ContentArrayOrString::Blocks(blocks))
    }
}

// ─── ContentBlock ────────────────────────────────────────────────

#[derive(Debug)]
struct ContentBlock {
    block_type: Option<String>,
    all_keys: BTreeSet<String>,
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ContentBlockVisitor)
    }
}

struct ContentBlockVisitor;

impl<'de> Visitor<'de> for ContentBlockVisitor {
    type Value = ContentBlock;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a content block object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ContentBlock, A::Error> {
        let mut block_type = None;
        let mut all_keys = BTreeSet::new();

        while let Some(key) = map.next_key::<String>()? {
            all_keys.insert(key.clone());
            if key == "type" {
                block_type = Some(map.next_value::<String>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }

        Ok(ContentBlock {
            block_type,
            all_keys,
        })
    }
}

// ─── DataSignals ─────────────────────────────────────────────────

#[derive(Debug, Default)]
struct DataSignals {
    data_type: Option<String>,
    nesting_direct: bool,
    nesting_nested: bool,
}

impl<'de> Deserialize<'de> for DataSignals {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(DataVisitor)
    }
}

struct DataVisitor;

impl<'de> Visitor<'de> for DataVisitor {
    type Value = DataSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a data object with optional type and message")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<DataSignals, A::Error> {
        let mut signals = DataSignals::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "type" => {
                    let t = map.next_value::<String>()?;
                    if t == "agent_progress" {
                        signals.nesting_direct = true;
                    }
                    signals.data_type = Some(t);
                }
                "message" => {
                    // Check for nested agent_progress: data.message.message.content
                    let nesting = map.next_value::<NestingCheck>()?;
                    if nesting.has_nested_content {
                        signals.nesting_nested = true;
                    }
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(signals)
    }
}

// ─── NestingCheck ────────────────────────────────────────────────

#[derive(Debug, Default)]
struct NestingCheck {
    has_nested_content: bool,
}

impl<'de> Deserialize<'de> for NestingCheck {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NestingVisitor)
    }
}

struct NestingVisitor;

impl<'de> Visitor<'de> for NestingVisitor {
    type Value = NestingCheck;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a message envelope that may contain nested message.content")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NestingCheck, A::Error> {
        let mut result = NestingCheck::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "message" => {
                    // This is data.message.message — check if it has a content array
                    let inner = map.next_value::<InnerMessageHasContentArray>()?;
                    result.has_nested_content = inner.has_content_array;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(result)
    }

    // data.message might not be an object
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }
}

// ─── InnerMessageHasContentArray ─────────────────────────────────

#[derive(Debug, Default)]
struct InnerMessageHasContentArray {
    has_content_array: bool,
}

impl<'de> Deserialize<'de> for InnerMessageHasContentArray {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(InnerMessageVisitor)
    }
}

struct InnerMessageVisitor;

impl<'de> Visitor<'de> for InnerMessageVisitor {
    type Value = InnerMessageHasContentArray;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an inner message object")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<InnerMessageHasContentArray, A::Error> {
        let mut result = InnerMessageHasContentArray::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "content" => {
                    let is_arr = map.next_value::<IsArray>()?;
                    result.has_content_array = is_arr.0;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(result)
    }

    // Inner message might not be an object
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }
}

// ─── IsArray ─────────────────────────────────────────────────────

/// Zero-allocation check: is this JSON value an array?
struct IsArray(bool);

impl<'de> Deserialize<'de> for IsArray {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(IsArrayVisitor)
    }
}

struct IsArrayVisitor;

impl<'de> Visitor<'de> for IsArrayVisitor {
    type Value = IsArray;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<IsArray, A::Error> {
        // Consume the sequence without allocating
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(IsArray(true))
    }

    fn visit_str<E: de::Error>(self, _v: &str) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<IsArray, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(IsArray(false))
    }

    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_none<E: de::Error>(self) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_unit<E: de::Error>(self) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }
}

pub fn load_baseline(path: &Path) -> Result<Baseline, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read baseline {}: {}", path.display(), e))?;
    serde_json::from_slice(&data).map_err(|e| format!("Failed to parse baseline JSON: {}", e))
}

// ─── Aggregated Signals ──────────────────────────────────────────

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

/// Trim leading/trailing ASCII whitespace from a byte slice (MSRV-safe).
fn trim_ascii_bytes(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|p| p + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// Scan a single JSONL file and return aggregated signals.
pub(crate) fn scan_file(path: &Path) -> AggregatedSignals {
    let mut agg = AggregatedSignals {
        files_scanned: 1,
        ..Default::default()
    };

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            agg.errors += 1;
            return agg;
        }
    };

    let newline = b'\n';
    let mut start = 0;
    for pos in memchr::memchr_iter(newline, &data) {
        let line = trim_ascii_bytes(&data[start..pos]);
        start = pos + 1;
        if line.is_empty() {
            continue;
        }
        agg.lines_scanned += 1;
        let signals = extract_line_signals(line);
        if signals.top_level_type.is_none() {
            agg.errors += 1;
        }
        agg.ingest(signals);
    }

    // Handle last line (no trailing newline)
    let last = trim_ascii_bytes(&data[start..]);
    if !last.is_empty() {
        agg.lines_scanned += 1;
        let signals = extract_line_signals(last);
        if signals.top_level_type.is_none() {
            agg.errors += 1;
        }
        agg.ingest(signals);
    }

    agg
}

/// Discover JSONL files in a Claude Code data directory (3-level walk).
///
/// Structure: `data_dir/<project>/<session>/*.jsonl`
pub fn discover_jsonl_files(data_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let projects = match std::fs::read_dir(data_dir) {
        Ok(rd) => rd,
        Err(_) => return files,
    };

    for project_entry in projects.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let sessions = match std::fs::read_dir(&project_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for session_entry in sessions.flatten() {
            let session_path = session_entry.path();
            if !session_path.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(&session_path) {
                Ok(rd) => rd,
                Err(_) => continue,
            };

            for file_entry in entries.flatten() {
                let file_path = file_entry.path();
                if file_path.extension().is_some_and(|ext| ext == "jsonl") {
                    files.push(file_path);
                }
            }
        }
    }

    files
}

/// Scan all JSONL files in a data directory in parallel using scoped threads.
pub fn scan_directory_parallel(data_dir: &Path) -> AggregatedSignals {
    let files = discover_jsonl_files(data_dir);
    if files.is_empty() {
        return AggregatedSignals::default();
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_size = files.len().div_ceil(num_threads);
    let chunks: Vec<&[PathBuf]> = files.chunks(chunk_size).collect();

    let mut final_agg = AggregatedSignals::default();

    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    let mut local = AggregatedSignals::default();
                    for path in chunk {
                        local.merge(scan_file(path));
                    }
                    local
                })
            })
            .collect();

        for handle in handles {
            if let Ok(partial) = handle.join() {
                final_agg.merge(partial);
            }
        }
    });

    final_agg
}

// ─── Sorted Trait ────────────────────────────────────────────────

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

// ─── Audit Comparison ────────────────────────────────────────────

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

/// Compare actual vs expected sets, reporting new (drift) and absent items.
///
/// `new_items` = in actual but not expected (potential drift).
/// `absent_items` = in expected but not actual (may be fine — not all types appear in every corpus).
/// Only `new_items` being non-empty means `passed = false`.
pub fn check_set_diff(
    name: &str,
    actual: &HashSet<String>,
    expected: &HashSet<String>,
) -> CheckResult {
    let new_items: Vec<String> = actual.difference(expected).cloned().sorted();
    let absent_items: Vec<String> = expected.difference(actual).cloned().sorted();
    let passed = new_items.is_empty();

    CheckResult {
        name: name.to_string(),
        passed,
        new_items,
        absent_items,
    }
}

/// Run all 6 audit checks comparing scanned signals against the baseline.
pub fn run_audit_checks(signals: &AggregatedSignals, baseline: &Baseline) -> AuditResult {
    let mut checks = Vec::new();

    // 1. Top-level types
    let expected_top = baseline.top_level_types.all_known();
    checks.push(check_set_diff(
        "Top-level types",
        &signals.top_level_types,
        &expected_top,
    ));

    // 2. Assistant content block types
    let expected_content: HashSet<String> = baseline
        .content_block_types
        .assistant
        .iter()
        .cloned()
        .collect();
    checks.push(check_set_diff(
        "Assistant content block types",
        &signals.assistant_content_block_types,
        &expected_content,
    ));

    // 3. System subtypes
    let expected_sys: HashSet<String> = baseline.system_subtypes.known.iter().cloned().collect();
    checks.push(check_set_diff(
        "System subtypes",
        &signals.system_subtypes,
        &expected_sys,
    ));

    // 4. Progress data.type values
    let expected_prog: HashSet<String> =
        baseline.progress_data_types.known.iter().cloned().collect();
    checks.push(check_set_diff(
        "Progress data types",
        &signals.progress_data_types,
        &expected_prog,
    ));

    // 5. Thinking block keys — check each unique key-shape variant
    let expected_thinking: BTreeSet<String> = baseline
        .thinking_block_keys
        .required
        .iter()
        .cloned()
        .collect();

    // Empty corpus = pass with note (not drift)
    let thinking_check = if signals.thinking_key_sets.is_empty() {
        let absent: Vec<String> = expected_thinking.iter().cloned().collect();
        CheckResult {
            name: "Thinking block keys".to_string(),
            passed: true,
            new_items: vec![],
            absent_items: absent,
        }
    } else {
        // Every observed key-shape must exactly match expected
        let all_match = signals
            .thinking_key_sets
            .iter()
            .all(|ks| *ks == expected_thinking);
        if all_match {
            CheckResult {
                name: "Thinking block keys".to_string(),
                passed: true,
                new_items: vec![],
                absent_items: vec![],
            }
        } else {
            // Collect deviating variants
            let mut new_items = Vec::new();
            for ks in &signals.thinking_key_sets {
                if *ks != expected_thinking {
                    let extra: Vec<String> = ks.difference(&expected_thinking).cloned().collect();
                    let missing: Vec<String> = expected_thinking.difference(ks).cloned().collect();
                    new_items.push(format!(
                        "variant {{{}}} — extra: {:?}, missing: {:?}",
                        ks.iter().cloned().collect::<Vec<_>>().join(", "),
                        extra,
                        missing,
                    ));
                }
            }
            CheckResult {
                name: "Thinking block keys".to_string(),
                passed: false,
                new_items,
                absent_items: vec![],
            }
        }
    };
    checks.push(thinking_check);

    // 6. Agent progress nesting
    // Fail if we see direct agent_progress but never the nested content path
    let nesting_passed = !(signals.nesting_direct_count > 0 && signals.nesting_nested_count == 0);
    checks.push(CheckResult {
        name: "Agent progress nesting".to_string(),
        passed: nesting_passed,
        new_items: if nesting_passed {
            vec![]
        } else {
            vec![format!(
                "direct={} but nested=0 — double-nesting path not validated",
                signals.nesting_direct_count
            )]
        },
        absent_items: vec![],
    });

    let passed = checks.iter().all(|c| c.passed);

    AuditResult {
        passed,
        checks,
        nesting_direct_count: signals.nesting_direct_count,
        nesting_nested_count: signals.nesting_nested_count,
        files_scanned: signals.files_scanned,
        lines_scanned: signals.lines_scanned,
        errors: signals.errors,
    }
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

    #[test]
    fn test_extract_signals_assistant_with_thinking() {
        let line = br#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-28T10:01:00Z","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"thinking","thinking":"hmm","signature":"sig1"},{"type":"text","text":"hello"},{"type":"tool_use","id":"tu1","name":"Read","input":{}}]}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("assistant"));
        assert_eq!(signals.subtype, None);
        assert_eq!(signals.data_type, None);

        // Content block types
        assert_eq!(signals.content_block_types.len(), 3);
        assert!(signals
            .content_block_types
            .contains(&"thinking".to_string()));
        assert!(signals.content_block_types.contains(&"text".to_string()));
        assert!(signals
            .content_block_types
            .contains(&"tool_use".to_string()));

        // Thinking key sets
        assert_eq!(signals.thinking_key_sets.len(), 1);
        let keys = &signals.thinking_key_sets[0];
        assert!(keys.contains("type"));
        assert!(keys.contains("thinking"));
        assert!(keys.contains("signature"));
    }

    #[test]
    fn test_extract_signals_system() {
        let line = br#"{"type":"system","uuid":"s1","timestamp":"2026-01-28T10:03:05Z","subtype":"turn_duration","durationMs":5000}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("system"));
        assert_eq!(signals.subtype.as_deref(), Some("turn_duration"));
        assert!(signals.content_block_types.is_empty());
    }

    #[test]
    fn test_extract_signals_progress() {
        let line = br#"{"type":"progress","uuid":"p1","timestamp":"2026-01-28T10:03:10Z","data":{"type":"hook_progress","hookEvent":"PreToolUse"}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("progress"));
        assert_eq!(signals.data_type.as_deref(), Some("hook_progress"));
    }

    #[test]
    fn test_extract_signals_agent_progress_nesting() {
        // agent_progress with double-nested message.message.content[]
        let line = br#"{"type":"progress","uuid":"p2","data":{"type":"agent_progress","message":{"uuid":"x","type":"message","timestamp":"t","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]}}}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("progress"));
        assert_eq!(signals.data_type.as_deref(), Some("agent_progress"));
        assert!(
            signals.nesting_direct,
            "should detect direct agent_progress"
        );
        assert!(
            signals.nesting_nested,
            "should detect nested message.message.content[]"
        );
    }

    #[test]
    fn test_no_misclassify_string_content_as_type() {
        // User message with string content containing "type":"assistant" — must NOT extract content blocks
        let line = br#"{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"role":"user","content":"The type:assistant message was sent"}}"#;
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type.as_deref(), Some("user"));
        // Content blocks should be empty because: (1) string content yields Other, (2) non-assistant clears them
        assert!(
            signals.content_block_types.is_empty(),
            "user message should not have content block types"
        );
    }

    #[test]
    fn test_extract_signals_malformed_line() {
        let line = b"not valid json at all {{{";
        let signals = extract_line_signals(line);
        assert_eq!(signals.top_level_type, None);
        assert!(signals.content_block_types.is_empty());
    }

    #[test]
    fn test_scan_file_aggregates_signals() {
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
        let agg = scan_file(&fixture_path);

        assert_eq!(agg.files_scanned, 1);
        assert!(agg.lines_scanned >= 10, "should scan multiple lines");
        assert_eq!(agg.errors, 0, "fixture should have no parse errors");

        // Top-level types from fixture
        assert!(
            agg.top_level_types.contains("user"),
            "should find 'user' type"
        );
        assert!(
            agg.top_level_types.contains("assistant"),
            "should find 'assistant' type"
        );
        assert!(
            agg.top_level_types.contains("system"),
            "should find 'system' type"
        );
        assert!(
            agg.top_level_types.contains("progress"),
            "should find 'progress' type"
        );
        assert!(
            agg.top_level_types.contains("queue-operation"),
            "should find 'queue-operation' type"
        );
        assert!(
            agg.top_level_types.contains("summary"),
            "should find 'summary' type"
        );
        assert!(
            agg.top_level_types.contains("file-history-snapshot"),
            "should find 'file-history-snapshot' type"
        );

        // Content block types from assistant messages
        assert!(
            agg.assistant_content_block_types.contains("thinking"),
            "should find 'thinking' content block"
        );
        assert!(
            agg.assistant_content_block_types.contains("text"),
            "should find 'text' content block"
        );
        assert!(
            agg.assistant_content_block_types.contains("tool_use"),
            "should find 'tool_use' content block"
        );

        // System subtypes
        assert!(
            agg.system_subtypes.contains("turn_duration"),
            "should find 'turn_duration' subtype"
        );

        // Progress data types
        assert!(
            agg.progress_data_types.contains("hook_progress"),
            "should find 'hook_progress' data type"
        );

        // Thinking block key audit
        assert!(
            !agg.thinking_key_sets.is_empty(),
            "should have thinking key sets"
        );
        let first_keys = agg.thinking_key_sets.iter().next().unwrap();
        assert!(
            first_keys.contains("type"),
            "thinking keys should include 'type'"
        );
        assert!(
            first_keys.contains("thinking"),
            "thinking keys should include 'thinking'"
        );
        assert!(
            first_keys.contains("signature"),
            "thinking keys should include 'signature'"
        );
    }

    #[test]
    fn test_check_result_no_drift() {
        let actual: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let expected: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let result = check_set_diff("test", &actual, &expected);

        assert!(result.passed, "no new items means pass");
        assert!(result.new_items.is_empty());
        assert_eq!(result.absent_items, vec!["c".to_string()]);
    }

    #[test]
    fn test_check_result_with_drift() {
        let actual: HashSet<String> = ["a", "b", "new_type"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let expected: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let result = check_set_diff("test", &actual, &expected);

        assert!(!result.passed, "new items means fail");
        assert!(result.new_items.contains(&"new_type".to_string()));
        assert!(result.absent_items.is_empty());
    }

    #[test]
    fn test_full_audit_pass() {
        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/integrity/evidence-baseline.json");
        let baseline = load_baseline(&baseline_path).expect("should load baseline");

        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
        let signals = scan_file(&fixture_path);

        let result = run_audit_checks(&signals, &baseline);

        // The fixture covers the basic types — should pass
        assert!(
            result.passed,
            "audit should pass for fixture. Failed checks: {:?}",
            result
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| format!("{}: new={:?}", c.name, c.new_items))
                .collect::<Vec<_>>()
        );

        // Verify stats are populated
        assert_eq!(result.files_scanned, 1);
        assert!(result.lines_scanned > 0);
        assert_eq!(result.errors, 0);
    }
}
