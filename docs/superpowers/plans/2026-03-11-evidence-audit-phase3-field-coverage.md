# Evidence Audit Phase 3 — Field Coverage & Phantom Detection

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make it impossible for phantom fields (parser reads a key that doesn't exist in real data) or unextracted fields (real data has a key the parser ignores) to survive an audit pass.

**Architecture:** Add a new serde visitor (`FieldInventoryVisitor`) that collects every JSON key at every nesting depth from real JSONL data, keyed by dotted path (e.g., `message.content[].input.team_name`). Cross-reference this runtime inventory against a manually maintained baseline (`field_extraction_paths` in `evidence-baseline.json`) that declares which paths the parser extracts and which are intentionally ignored. Two new checks: one for phantoms (parser claims to extract a path with 0 occurrences in real data), one for coverage (real paths present in data but absent from both `extracted` and `intentionally_ignored` lists).

**Tech Stack:** Rust (serde `Visitor` + `DeserializeSeed` for recursive key walking), same parallel scan infrastructure as Phase 1+2. Baseline is a JSON manifest — no compile-time static analysis.

**Why this plan exists:** On 2026-03-11, the `teamName` field (present on 71% of JSONL lines after TeamCreate) was completely ignored by the parser because it read `input.team_name` (0 occurrences — phantom). The existing evidence audit passed because it validates type coverage and extraction correctness of fields the parser already knows about, but never asks "what fields exist in real data that the parser doesn't know about?" This plan closes that loop.

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `crates/core/src/field_inventory.rs` | New serde visitor that walks all JSON keys at all depths, collecting `(path, count)` pairs |
| Modify | `crates/core/src/evidence_audit.rs` | Add `FieldInventory` field to `AggregatedSignals`, add `FieldExtractionPaths` struct, add `run_phase3_checks()`, wire into scan |
| Modify | `crates/core/src/pipeline_checks.rs` | Add 2 new checks (field coverage + phantom detection) |
| Modify | `crates/core/src/bin/evidence_audit.rs` | Display results for new checks |
| Modify | `crates/core/src/lib.rs` | Add `pub mod field_inventory;` |
| Modify | `scripts/integrity/evidence-baseline.json` | Add `field_extraction_paths` section |
| Modify | `crates/core/tests/pipeline_invariant_test.rs` | Add test for new checks |

---

## Task 1: Field Inventory Visitor

**Files:**
- Create: `crates/core/src/field_inventory.rs`
- Test: inline `#[cfg(test)]` module

This is the core new component. A serde visitor that walks a JSON object recursively, collecting every key it encounters with its full dotted path and count.

- [ ] **Step 1.1: Write the failing test**

In `crates/core/src/field_inventory.rs`, define the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_level_keys() {
        let line = br#"{"type":"assistant","teamName":"demo","slug":"abc","message":{"role":"assistant"}}"#;
        let inv = extract_field_inventory(line);
        assert!(inv.paths.contains_key("type"));
        assert!(inv.paths.contains_key("teamName"));
        assert!(inv.paths.contains_key("slug"));
        assert!(inv.paths.contains_key("message"));
        assert_eq!(inv.paths["type"], 1);
    }

    #[test]
    fn test_nested_keys() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Agent","input":{"name":"sysinfo","subagent_type":"general-purpose"}}]}}"#;
        let inv = extract_field_inventory(line);
        // Nested paths use dot notation, arrays use []
        assert!(inv.paths.contains_key("message.content[].type"));
        assert!(inv.paths.contains_key("message.content[].name"));
        assert!(inv.paths.contains_key("message.content[].input.name"));
        assert!(inv.paths.contains_key("message.content[].input.subagent_type"));
    }

    #[test]
    fn test_phantom_field_not_present() {
        // Real Agent tool_use input does NOT have team_name
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Agent","input":{"name":"sysinfo","description":"System info","prompt":"..."}}]}}"#;
        let inv = extract_field_inventory(line);
        assert!(!inv.paths.contains_key("message.content[].input.team_name"),
            "team_name must not appear in Agent tool_use input");
    }

    #[test]
    fn test_top_level_team_name_present() {
        let line = br#"{"type":"assistant","teamName":"demo-team","message":{"role":"assistant","content":[]}}"#;
        let inv = extract_field_inventory(line);
        assert!(inv.paths.contains_key("teamName"));
    }

    #[test]
    fn test_does_not_count_string_content_as_keys() {
        // "team_name" appears in text content — must NOT be counted as a structural key
        let line = br#"{"type":"user","message":{"role":"user","content":"I set team_name to demo"}}"#;
        let inv = extract_field_inventory(line);
        assert!(!inv.paths.contains_key("team_name"),
            "text content must not be inventoried as structural keys");
    }
}
```

- [ ] **Step 1.2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core field_inventory`
Expected: compilation error (module doesn't exist yet)

- [ ] **Step 1.3: Implement the field inventory visitor**

Create `crates/core/src/field_inventory.rs`:

```rust
//! Recursive JSON field inventory — collects every structural key with its
//! dotted path from a JSONL line. Used by evidence audit Phase 3 to detect
//! phantom fields and unextracted fields.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::Deserializer;
use std::collections::HashMap;
use std::fmt;

/// Result of inventorying a single JSONL line.
#[derive(Debug, Default)]
pub struct FieldInventory {
    /// Map of dotted path → occurrence count within this line.
    /// e.g., "message.content[].input.name" → 1
    pub paths: HashMap<String, usize>,
}

impl FieldInventory {
    pub fn merge(&mut self, other: &FieldInventory) {
        for (path, count) in &other.paths {
            *self.paths.entry(path.clone()).or_default() += count;
        }
    }
}

/// Extract all structural JSON key paths from a single JSONL line.
pub fn extract_field_inventory(line: &[u8]) -> FieldInventory {
    let mut inv = FieldInventory::default();
    let mut deserializer = Deserializer::from_slice(line);
    let visitor = ObjectVisitor {
        prefix: String::new(),
        inventory: &mut inv,
    };
    // Ignore errors — malformed lines are handled elsewhere
    let _ = deserializer.deserialize_any(visitor);
    inv
}

/// Recursive visitor that collects keys at any depth.
struct ObjectVisitor<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> Visitor<'de> for ObjectVisitor<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            let path = if self.prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", self.prefix, key)
            };
            *self.inventory.paths.entry(path.clone()).or_default() += 1;

            // Recurse into the value — MUST reborrow, not move, because
            // self.inventory is used again on the next loop iteration.
            // `&mut *self.inventory` creates a shorter-lived borrow that expires
            // when `child` is consumed by next_value_seed().
            let child = ValueVisitor {
                prefix: path,
                inventory: &mut *self.inventory,
            };
            map.next_value_seed(child)?;
        }
        Ok(())
    }

    // Non-object roots: do nothing (shouldn't happen for JSONL)
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<(), E> { Ok(()) }
    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<(), E> { Ok(()) }
    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<(), E> { Ok(()) }
    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<(), E> { Ok(()) }
    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<(), E> { Ok(()) }
    fn visit_none<E: de::Error>(self) -> Result<(), E> { Ok(()) }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> { Ok(()) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
        Ok(())
    }
}

/// DeserializeSeed that recurses into values (objects and arrays).
struct ValueVisitor<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> de::DeserializeSeed<'de> for ValueVisitor<'a> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(ValueDispatch {
            prefix: self.prefix,
            inventory: self.inventory,
        })
    }
}

/// Dispatches to ObjectVisitor for maps, array handler for seqs, ignores scalars.
struct ValueDispatch<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> Visitor<'de> for ValueDispatch<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<(), A::Error> {
        let obj = ObjectVisitor {
            prefix: self.prefix,
            inventory: self.inventory,
        };
        obj.visit_map(map)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        // Array: recurse into each element with "[]" suffix on the path.
        // MUST reborrow self.inventory — same reason as ObjectVisitor::visit_map.
        let array_prefix = format!("{}[]", self.prefix);
        while let Some(()) = {
            let child = ValueVisitor {
                prefix: array_prefix.clone(),
                inventory: &mut *self.inventory,
            };
            seq.next_element_seed(child)?
        } {}
        Ok(())
    }

    // Scalars: nothing to recurse into
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<(), E> { Ok(()) }
    fn visit_string<E: de::Error>(self, _v: String) -> Result<(), E> { Ok(()) }
    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<(), E> { Ok(()) }
    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<(), E> { Ok(()) }
    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<(), E> { Ok(()) }
    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<(), E> { Ok(()) }
    fn visit_none<E: de::Error>(self) -> Result<(), E> { Ok(()) }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> { Ok(()) }
}
```

- [ ] **Step 1.4: Register module in `lib.rs`**

Add to `crates/core/src/lib.rs`:
```rust
pub mod field_inventory;
```

- [ ] **Step 1.5: Run tests to verify they pass**

Run: `cargo test -p claude-view-core field_inventory`
Expected: all 5 tests pass

- [ ] **Step 1.6: Commit**

```bash
git add crates/core/src/field_inventory.rs crates/core/src/lib.rs
git commit -m "feat(audit): add field inventory visitor for recursive key path extraction"
```

---

## Task 2: Baseline — Declare Parser Field Paths

**Files:**
- Modify: `scripts/integrity/evidence-baseline.json`

Add two new sections: `field_extraction_paths` (what the parser reads) and `field_intentionally_ignored` (what exists in data but we don't need).

- [ ] **Step 2.1: Build the parser field manifest**

Run this grep to get every `.get("X")` in the parser with its context (to determine nesting depth):

```bash
grep -n '\.get("' crates/core/src/live_parser.rs | grep -v '//' | grep -v '#\[test\]' | grep -v 'fn test_'
```

Cross-reference with the real data key inventory from the audit's scan to categorize each.

- [ ] **Step 2.2: Add field_extraction_paths to baseline**

Add to `evidence-baseline.json`:

```json
"field_extraction_paths": {
    "extracted": [
        "type",
        "message",
        "message.role",
        "message.content",
        "message.content[].type",
        "message.content[].id",
        "message.content[].name",
        "message.content[].input",
        "message.content[].input.name",
        "message.content[].input.description",
        "message.content[].input.subagent_type",
        "message.content[].input.prompt",
        "message.content[].input.skill",
        "message.content[].input.subject",
        "message.content[].input.activeForm",
        "message.content[].input.task",
        "message.content[].input.taskId",
        "message.content[].input.status",
        "message.content[].input.todos",
        "message.content[].text",
        "message.model",
        "message.stop_reason",
        "message.id",
        "message.usage",
        "message.usage.input_tokens",
        "message.usage.output_tokens",
        "message.usage.cache_read_input_tokens",
        "message.usage.cache_creation_input_tokens",
        "message.usage.cache_creation",
        "message.usage.cache_creation.ephemeral_5m_input_tokens",
        "message.usage.cache_creation.ephemeral_1h_input_tokens",
        "timestamp",
        "cwd",
        "gitBranch",
        "slug",
        "teamName",
        "isMeta",
        "requestId",
        "subtype",
        "toolUseResult",
        "toolUseResult.status",
        "toolUseResult.totalDurationMs",
        "toolUseResult.totalToolUseCount",
        "toolUseResult.agentId",
        "toolUseResult.tool_use_id",
        "data",
        "data.type",
        "data.message",
        "content",
        "parentToolUseID",
        "message.content[].tool_use_id"
    ],
    "intentionally_ignored": [
        "sessionId",
        "parentUuid",
        "uuid",
        "version",
        "isSidechain",
        "userType",
        "toolUseID",
        "sourceToolAssistantUUID",
        "agentId",
        "messageId",
        "snapshot",
        "isSnapshotUpdate",
        "operation",
        "level",
        "permissionMode",
        "hookCount",
        "hookInfos",
        "hookErrors",
        "preventedContinuation",
        "stopReason",
        "hasOutput",
        "sourceToolUseID",
        "durationMs",
        "todos",
        "thinkingMetadata",
        "error",
        "retryInMs",
        "retryAttempt",
        "maxRetries",
        "logicalParentUuid",
        "compactMetadata",
        "isVisibleInTranscriptOnly",
        "isCompactSummary",
        "isApiErrorMessage",
        "cause",
        "imagePasteIds",
        "lastPrompt",
        "prNumber",
        "prUrl",
        "prRepository"
    ],
    "intentionally_ignored_note": "Metadata fields used by Claude Code internally. Not needed by our parser for session display, cost tracking, or live monitoring."
}
```

**IMPORTANT:** The `extracted` list must be verified by running the field inventory scan against real data and confirming every path in `extracted` has >0 occurrences. Any path with 0 occurrences is a phantom — remove it from `extracted` and investigate.

**Sustainability note:** This baseline is manually maintained. Adding a `.get("X")` call to the parser without updating the baseline will NOT be caught until the audit runs against data containing that key. To make Phase 3 self-sustaining, run the evidence audit in CI (pre-release gate). This is the same contract-testing pattern used by Stripe for API schema drift detection.

- [ ] **Step 2.3: Commit**

```bash
git add scripts/integrity/evidence-baseline.json
git commit -m "feat(audit): add field extraction paths to evidence baseline"
```

---

## Task 3: Wire Field Inventory Into Scan Pipeline

**Files:**
- Modify: `crates/core/src/evidence_audit.rs`

Add `FieldInventory` aggregation to `AggregatedSignals` and wire into `scan_file_with_pipeline`.

- [ ] **Step 3.1: Add field inventory to AggregatedSignals**

In `evidence_audit.rs`, add to `AggregatedSignals`:

```rust
use crate::field_inventory::FieldInventory;

pub struct AggregatedSignals {
    // ... existing fields ...
    /// Accumulated field path inventory across all scanned lines.
    pub field_inventory: FieldInventory,
}
```

Update `merge()` to handle the new field. (`Default` is derived — `FieldInventory` also derives `Default`, so no manual impl needed. `ingest()` is Phase 1 only — Phase 3 is wired separately in the `process_line` closure.)

Add to `AggregatedSignals::merge()` (after the last `self.*.extend(...)` line):
```rust
self.field_inventory.merge(&other.field_inventory);
```

Add the import at the top of `evidence_audit.rs`:
```rust
use crate::field_inventory::FieldInventory;
```

- [ ] **Step 3.2: Wire into scan_file_with_pipeline**

In the `process_line` closure inside `scan_file_with_pipeline`, add after Phase 2 per-line checks (before storing to `session_lines`):

```rust
// Phase 3: field inventory
let field_inv = crate::field_inventory::extract_field_inventory(line_bytes);
agg.field_inventory.merge(&field_inv);
```

**Performance note:** This adds a 4th deserialization pass per line (Phase 1 serde visitor + Phase 2 parser + Phase 2 `serde_json::from_slice` + Phase 3 serde visitor). On the existing corpus (~1M lines), this adds ~200ms to a 2s scan. Acceptable for a pre-release audit tool. Future optimization: fold Phase 3 key collection into the Phase 1 serde visitor to eliminate the extra pass.

- [ ] **Step 3.3: Run existing tests to verify no regression**

Run: `cargo test -p claude-view-core -- evidence_audit pipeline`
Expected: all existing tests pass unchanged

- [ ] **Step 3.4: Commit**

```bash
git add crates/core/src/evidence_audit.rs
git commit -m "feat(audit): wire field inventory into Phase 1+2 scan pipeline"
```

---

## Task 4: Add Baseline Deserialization for Field Paths

**Files:**
- Modify: `crates/core/src/evidence_audit.rs`

The `Baseline` struct needs to deserialize the new `field_extraction_paths` section.

- [ ] **Step 4.1: Add structs**

```rust
#[derive(Debug, Deserialize)]
pub struct FieldExtractionPaths {
    pub extracted: Vec<String>,
    pub intentionally_ignored: Vec<String>,
}
```

Add to `Baseline`:
```rust
pub struct Baseline {
    // ... existing fields ...
    #[serde(default)]
    pub field_extraction_paths: Option<FieldExtractionPaths>,
}
```

Use `Option` + `#[serde(default)]` so the audit still works with older baseline files that lack this section.

- [ ] **Step 4.2: Run tests**

Run: `cargo test -p claude-view-core -- evidence_audit`
Expected: passes (baseline loads successfully with new field)

- [ ] **Step 4.3: Commit**

```bash
git add crates/core/src/evidence_audit.rs
git commit -m "feat(audit): deserialize field_extraction_paths from baseline"
```

---

## Task 5: Phase 3 Checks — Field Coverage + Phantom Detection

**Files:**
- Modify: `crates/core/src/pipeline_checks.rs`

Two new checks that cross-reference the field inventory against the baseline.

- [ ] **Step 5.1: Write failing tests**

In `crates/core/tests/pipeline_invariant_test.rs`, add the following imports at the top of the file (alongside existing `use` statements):

```rust
use claude_view_core::evidence_audit::FieldExtractionPaths;
use claude_view_core::pipeline_checks::{check_field_coverage, check_phantom_fields};
```

Then add the test functions:

```rust
#[test]
fn test_field_coverage_detects_unknown_path() {
    // Scan fixture, then declare only a subset of its fields as "extracted"
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
    let (agg, _) = scan_file_with_pipeline(&fixture_path);
    let baseline_paths = FieldExtractionPaths {
        extracted: vec!["type".into(), "timestamp".into()],
        intentionally_ignored: vec![],
    };
    let result = check_field_coverage(&agg.field_inventory, &baseline_paths);
    // "type" and "timestamp" exist in data → pass
    // But data also has "uuid", "message", "parentUuid", etc. in neither list → fail
    assert!(!result.passed, "should detect unaccounted paths in data");
}

#[test]
fn test_phantom_detection() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/all_types.jsonl");
    let (agg, _) = scan_file_with_pipeline(&fixture_path);
    let baseline_paths = FieldExtractionPaths {
        extracted: vec![
            "type".into(),
            "message.content[].input.team_name".into(), // phantom — not in fixture data
        ],
        intentionally_ignored: vec![],
    };
    let result = check_phantom_fields(&agg.field_inventory, &baseline_paths);
    assert!(!result.passed, "should detect phantom path");
    assert!(!result.sample_violations.is_empty(), "expected at least one violation");
    assert!(result.sample_violations[0].detail.contains("team_name"));
}
```

- [ ] **Step 5.2: Run to verify they fail**

Run: `cargo test -p claude-view-core --test pipeline_invariant_test`
Expected: compilation error (functions don't exist yet)

- [ ] **Step 5.3: Implement the two checks**

In `pipeline_checks.rs`:

```rust
use crate::evidence_audit::FieldExtractionPaths;
use crate::field_inventory::FieldInventory;

/// Check 19: Field coverage — every path in real data must be in
/// `extracted` or `intentionally_ignored`. Detects new fields Claude Code
/// adds that we don't handle.
pub fn check_field_coverage(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let known: std::collections::HashSet<&str> = baseline
        .extracted
        .iter()
        .chain(&baseline.intentionally_ignored)
        .map(|s| s.as_str())
        .collect();

    let mut accum = CheckAccum::default();

    for (path, count) in &inventory.paths {
        // NOTE: Do NOT manually increment accum.lines_checked here.
        // record_pass() and record_violation() both increment it internally.
        if known.contains(path.as_str()) {
            accum.record_pass();
        } else {
            accum.record_violation(
                "corpus-wide",
                0,
                &format!("unhandled field path: \"{}\" ({} occurrences)", path, count),
            );
        }
    }

    accum.into_result("Field coverage")
}

/// Check 20: Phantom field detection — every path in `extracted` must have
/// >0 occurrences in real data. Detects parser code that reads nonexistent fields.
pub fn check_phantom_fields(
    inventory: &FieldInventory,
    baseline: &FieldExtractionPaths,
) -> PipelineCheckResult {
    let mut accum = CheckAccum::default();

    for path in &baseline.extracted {
        // NOTE: Do NOT manually increment lines_checked — record_pass/record_violation do it.
        match inventory.paths.get(path.as_str()) {
            Some(count) if *count > 0 => {
                accum.record_pass();
            }
            _ => {
                accum.record_violation(
                    "corpus-wide",
                    0,
                    &format!(
                        "PHANTOM: parser claims to extract \"{}\" but 0 occurrences across all scanned data ({} distinct paths seen)",
                        path,
                        inventory.paths.len()
                    ),
                );
            }
        }
    }

    accum.into_result("Phantom field detection")
}
```

- [ ] **Step 5.4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core --test pipeline_invariant_test`
Expected: both new tests pass

- [ ] **Step 5.5: Commit**

```bash
git add crates/core/src/pipeline_checks.rs crates/core/tests/pipeline_invariant_test.rs
git commit -m "feat(audit): add field coverage and phantom detection checks (Phase 3)"
```

---

## Task 6: Wire Checks Into Audit Binary Output

**Files:**
- Modify: `crates/core/src/bin/evidence_audit.rs`
- Modify: `crates/core/src/evidence_audit.rs` (add `run_phase3_checks` public function)

- [ ] **Step 6.1: Add run_phase3_checks function**

In `evidence_audit.rs`, add a public function that runs both Phase 3 checks:

```rust
pub fn run_phase3_checks(
    inventory: &crate::field_inventory::FieldInventory,
    baseline: &Baseline,
) -> Vec<crate::pipeline_checks::PipelineCheckResult> {
    match &baseline.field_extraction_paths {
        Some(paths) => vec![
            crate::pipeline_checks::check_field_coverage(inventory, paths),
            crate::pipeline_checks::check_phantom_fields(inventory, paths),
        ],
        None => vec![
            crate::pipeline_checks::PipelineCheckResult::new_skipped(
                "Field coverage",
                "Baseline lacks field_extraction_paths section"
            ),
            crate::pipeline_checks::PipelineCheckResult::new_skipped(
                "Phantom field detection",
                "Baseline lacks field_extraction_paths section"
            ),
        ],
    }
}
```

- [ ] **Step 6.2: Add Phase 3 output to binary**

First, update the import at the top of `bin/evidence_audit.rs` to include `run_phase3_checks`:
```rust
use claude_view_core::evidence_audit::{
    check_set_diff, load_baseline, run_audit_checks, run_phase3_checks,
    scan_directory_parallel, scan_directory_parallel_with_pipeline, AuditResult,
};
```

Also update the mode label (line ~182) from `"FULL (6 type + 12 pipeline)"` to `"FULL (6 type + 12 pipeline + 2 field)"`.

Then, in `bin/evidence_audit.rs`, **inside** the `if let Some(pipeline) = pipeline_results { ... }` block (after the Phase 2 results loop and the `phase2_passed` check), add:

**Important variable name mapping** (plan names → actual binary names):
- `signals` (not `agg`) — the `AggregatedSignals` from scanning
- `results` (not `phase2_results`) — the Phase 2 `PipelineSignals::into_results()` vec
- `overall_passed` (not `all_passed`) — the final pass/fail flag
- `result` — the Phase 1 `AuditResult` from `run_audit_checks()`

```rust
// Phase 3: Field Coverage (still inside the full-mode block)
println!("\n  {BLUE}── Phase 3: Field Coverage ──{NC}");
let phase3_results = run_phase3_checks(&signals.field_inventory, &baseline);
let phase2_count = results.len();
let phase1_count = result.checks.len();
for (i, check) in phase3_results.iter().enumerate() {
    let idx = phase1_count + phase2_count + i + 1; // continues after Phase 1 + Phase 2
    let grand_total = phase1_count + phase2_count + phase3_results.len();
    if check.skipped {
        println!("  [{idx}/{grand_total}] {}: {YELLOW}SKIPPED{NC}", check.name);
    } else if check.passed {
        println!(
            "  [{idx}/{grand_total}] {}: {GREEN}OK{NC} ({} paths, 0 violations)",
            check.name, check.lines_checked
        );
    } else {
        println!(
            "  [{idx}/{grand_total}] {}: {RED}FAIL{NC} ({} violations)",
            check.name, check.violation_count
        );
        for v in &check.sample_violations {
            println!("         {RED}→{NC} {}", v.detail);
        }
        overall_passed = false;
    }
}
let phase3_passed = phase3_results.iter().all(|r| r.skipped || r.passed);
overall_passed = overall_passed && phase3_passed;
```

- [ ] **Step 6.3: Run the full audit to verify**

Run: `cargo run -p claude-view-core --bin evidence-audit`
Expected: Phase 3 checks appear, field coverage may report unhandled paths (which we then add to `intentionally_ignored` in baseline), phantom detection should show 0 violations (since we already fixed `team_name`).

- [ ] **Step 6.4: Iterate on baseline until Phase 3 passes**

For each unhandled path reported by check 19, decide:
- Is it a field we should extract? → Add to `extracted` and add parser code
- Is it metadata we don't need? → Add to `intentionally_ignored`

Update `evidence-baseline.json` until all 3 phases pass.

- [ ] **Step 6.5: Commit**

```bash
git add crates/core/src/evidence_audit.rs crates/core/src/bin/evidence_audit.rs scripts/integrity/evidence-baseline.json
git commit -m "feat(audit): wire Phase 3 field coverage into audit binary"
```

---

## Task 7: Verify End-to-End

- [ ] **Step 7.1: Run full audit**

```bash
cargo run -p claude-view-core --bin evidence-audit
```

Expected output includes (Phase 1 = 6 checks, Phase 2 = 12 checks, Phase 3 = 2 checks = 20 total):
```
── Phase 3: Field Coverage ──
[19/20] Field coverage: OK (N paths, 0 violations)
[20/20] Phantom field detection: OK (N paths, 0 violations)

ALL CHECKS PASSED — parser matches real data
```

- [ ] **Step 7.2: Regression test — simulate a phantom**

Temporarily add `"message.content[].input.team_name"` to the `extracted` list in baseline. Run the audit again.

Expected: Check 20 FAILS with:
```
PHANTOM: parser claims to extract "message.content[].input.team_name" but 0 occurrences in N files
```

Remove the fake entry after verifying.

- [ ] **Step 7.3: Regression test — simulate unhandled drift**

Temporarily remove `teamName` from both `extracted` and `intentionally_ignored`. Run the audit.

Expected: Check 19 FAILS with:
```
unhandled field path: "teamName" (N occurrences)
```

Restore the entry after verifying.

- [ ] **Step 7.4: Run all workspace tests**

```bash
cargo test -p claude-view-core
cargo test -p claude-view-server
```

Expected: all tests pass, no regressions.

- [ ] **Step 7.5: Final commit**

```bash
git add -A
git commit -m "feat(audit): Phase 3 field coverage complete — all checks pass"
```

---

## Rollback Strategy

If Phase 3 needs to be reverted:
1. Remove `pub mod field_inventory;` from `crates/core/src/lib.rs`
2. Remove the `field_inventory: FieldInventory` field from `AggregatedSignals` and its merge line
3. Remove `field_extraction_paths` section from `evidence-baseline.json`
4. Remove `check_field_coverage` and `check_phantom_fields` from `pipeline_checks.rs`
5. Remove `run_phase3_checks` from `evidence_audit.rs`
6. Remove Phase 3 output block from `bin/evidence_audit.rs`
7. Remove new tests from `pipeline_invariant_test.rs`
8. Delete `crates/core/src/field_inventory.rs`

The changes are fully additive — no existing functions are modified in breaking ways. Reverting means removing new code only. Phase 1 and Phase 2 remain unaffected.

---

## Summary

After this plan is executed, the evidence audit has three phases:

| Phase | What it checks | Bug class it catches |
|-------|---------------|---------------------|
| 1 | Type/variant coverage | New JSONL `type` values, content block types, system subtypes |
| 2 | Parser extraction correctness | Wrong token counts, missing timestamps, broken dedup |
| **3** | **Field coverage + phantom detection** | **Parser reads nonexistent field (phantom), real field ignored by parser (drift)** |

The `teamName` bug would be caught by **both** check 19 (teamName exists in data but not in `extracted`) **and** check 20 (`input.team_name` claimed by parser but 0 occurrences).

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Architecture described "include_str! + regex at compile time" — never implemented | Warning | Rewritten to describe actual baseline JSON approach |
| 2 | `ObjectVisitor::visit_map`: `inventory: self.inventory` moves `&mut` in while loop — won't compile | Blocker | Changed to `&mut *self.inventory` (explicit reborrow) |
| 3 | `ValueDispatch::visit_seq`: same move-in-loop bug | Blocker | Changed to `&mut *self.inventory` |
| 4 | Task 3 said "Update Default, merge(), and ingest()" with no code | Blocker | Added explicit `merge()` code, import, removed misleading "ingest" instruction |
| 5 | Task 3 Step 3.2 had no performance impact note | Warning | Added performance note with optimization path |
| 6 | Test fixture path `Path::new("tests/fixtures/...")` fails from workspace root | Blocker | Changed to `env!("CARGO_MANIFEST_DIR").join(...)` matching existing test pattern |
| 7 | Test code missing imports for `FieldExtractionPaths`, `check_field_coverage`, `check_phantom_fields` | Blocker | Added explicit `use` statements |
| 8 | Test comment referenced "slug" — not present in `all_types.jsonl` fixture | Warning | Fixed to "uuid, message, parentUuid" |
| 9 | `let mut baseline_paths` — mut unnecessary | Minor | Removed `mut` |
| 10 | `check_field_coverage`: manual `lines_checked += 1` + `record_violation()` double-counts | Blocker | Replaced manual increment with `record_pass()` in else branch |
| 11 | `check_phantom_fields`: manual `lines_checked += 1` + `record_pass()/record_violation()` double-counts | Blocker | Removed manual increment |
| 12 | Binary code used `all_passed` — actual variable is `overall_passed` | Blocker | Fixed variable name |
| 13 | Binary code used `agg.field_inventory` — actual variable is `signals` | Blocker | Fixed to `signals.field_inventory` |
| 14 | Binary code used `phase2_results` — actual variable is `results` | Blocker | Fixed variable name |
| 15 | Check numbering `phase2_results.len() + 7`: wrong variable, wrong count (6 not 7), off-by-one | Blocker | Replaced with `result.checks.len() + results.len()` formula with correct indexing |
| 16 | Phase 3 code placement relative to quick-mode block was ambiguous | Warning | Specified: inside `if let Some(pipeline)` block |
| 17 | Missing `run_phase3_checks` import in binary | Blocker | Added import instruction |
| 18 | Mode label not updated for Phase 3 | Minor | Updated to "FULL (6 type + 12 pipeline + 2 field)" |
| 19 | No rollback strategy | Warning | Added Rollback Strategy section |
| 20 | Expected output total not explained | Minor | Added Phase count breakdown in comment |
| 21 | File Structure table said "FieldInventorySignals" — struct is `FieldInventory` | Minor | Fixed table to match actual struct name |
| 22 | `check_phantom_fields` violation message uses `inventory.paths.len()` but says "files" — misleading (paths ≠ files) | Warning | Changed message to "across all scanned data (N distinct paths seen)" |
| 23 | `test_phantom_detection` indexes `sample_violations[0]` without guard — panics with OOB instead of clean test failure | Warning | Added `assert!(!result.sample_violations.is_empty())` before indexing |
