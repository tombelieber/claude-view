//! cc-compat-oracle — the zero-LLM drift detector for the CC-compatibility loop.
//!
//! Claude Code's session JSONL schema changes between releases (new top-level
//! record `type`s, new content-block types). claude-view parses that data, so a
//! release can silently start dropping data the parser doesn't recognize.
//!
//! This bin diffs the **live** `~/.claude` data against the **code's declared
//! coverage** ([`handled_record_types`] / [`handled_content_block_types`]) and
//! against a persisted baseline, and prints a JSON drift report. It spends **no
//! LLM tokens** — it is the cheap gate that decides whether the expensive
//! audit+patch workflow should run at all:
//!
//!   should_run_audit = version_changed  (the skill additionally requires the
//!                                         changelog to be fetched before running)
//!
//! Usage:
//!   cc-compat-oracle                 # print drift report JSON to stdout
//!   cc-compat-oracle --update-baseline  # ALSO persist the current state as the
//!                                         new baseline (run after a clean audit)
//!
//! Baseline: ~/.claude-view/cc-compat/baseline.json

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use claude_view_core::block_accumulator::content::handled_content_block_types;
use claude_view_core::block_accumulator::{
    handled_record_types, intentionally_ignored_record_types,
};
use claude_view_core::claude_projects_dir;
use serde_json::{json, Value};

#[derive(Default)]
struct Scan {
    record_type_counts: BTreeMap<String, u64>,
    content_block_types: BTreeSet<String>,
    newest_version: Option<String>,
    files: u64,
    records: u64,
}

fn main() {
    let update_baseline = std::env::args().any(|a| a == "--update-baseline");

    let projects = match claude_projects_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cc-compat-oracle: cannot resolve ~/.claude/projects: {e}");
            std::process::exit(2);
        }
    };

    let mut scan = Scan::default();
    if projects.is_dir() {
        let mut jsonl = Vec::new();
        collect_jsonl(&projects, &mut jsonl);
        for path in &jsonl {
            scan_file(path, &mut scan);
        }
    }

    let handled_records: BTreeSet<&str> = handled_record_types().iter().copied().collect();
    let ignored_records: BTreeSet<&str> = intentionally_ignored_record_types()
        .iter()
        .copied()
        .collect();
    let handled_blocks: BTreeSet<&str> = handled_content_block_types().iter().copied().collect();

    let data_record_types: BTreeSet<String> = scan.record_type_counts.keys().cloned().collect();

    // Unhandled GAP = in real data, no dispatch arm, AND not deliberately ignored.
    // (`started`/`result` are the tool-result cache — declared-ignored, not a gap.)
    let unhandled_records: Vec<String> = data_record_types
        .iter()
        .filter(|t| !handled_records.contains(t.as_str()) && !ignored_records.contains(t.as_str()))
        .cloned()
        .collect();
    let unhandled_blocks: Vec<String> = scan
        .content_block_types
        .iter()
        .filter(|t| !handled_blocks.contains(t.as_str()))
        .cloned()
        .collect();

    // Baseline diff.
    let baseline = load_baseline();
    let baseline_version = baseline
        .as_ref()
        .and_then(|b| b.get("last_audited_version"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let baseline_records: BTreeSet<String> = baseline
        .as_ref()
        .and_then(|b| b.get("known_record_types"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let baseline_blocks: BTreeSet<String> = baseline
        .as_ref()
        .and_then(|b| b.get("known_content_block_types"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let new_records_since_baseline: Vec<String> = data_record_types
        .iter()
        .filter(|t| !baseline_records.contains(*t))
        .cloned()
        .collect();
    let new_blocks_since_baseline: Vec<String> = scan
        .content_block_types
        .iter()
        .filter(|t| !baseline_blocks.contains(*t))
        .cloned()
        .collect();

    let version_changed = match (&scan.newest_version, &baseline_version) {
        (Some(newest), Some(base)) => version_gt(newest, base),
        (Some(_), None) => true, // never audited
        (None, _) => false,      // no version observed → no signal
    };

    // The token gate: only a real version bump warrants the expensive audit.
    // (The skill additionally requires a successfully-read changelog.)
    let should_run_audit = version_changed;

    let report = json!({
        "newest_version": scan.newest_version,
        "baseline_version": baseline_version,
        "version_changed": version_changed,
        "should_run_audit": should_run_audit,
        "record_types_in_data": scan.record_type_counts,
        "unhandled_record_types": unhandled_records,
        "intentionally_ignored_record_types": intentionally_ignored_record_types(),
        "new_record_types_since_baseline": new_records_since_baseline,
        "content_block_types_in_data": scan.content_block_types,
        "unhandled_content_block_types": unhandled_blocks,
        "new_content_block_types_since_baseline": new_blocks_since_baseline,
        "files_scanned": scan.files,
        "records_scanned": scan.records,
    });

    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    if update_baseline {
        match write_baseline(
            scan.newest_version.as_deref(),
            &data_record_types,
            &scan.content_block_types,
        ) {
            Ok(p) => eprintln!("cc-compat-oracle: baseline updated -> {}", p.display()),
            Err(e) => {
                eprintln!("cc-compat-oracle: failed to write baseline: {e}");
                std::process::exit(2);
            }
        }
    }
}

/// Recursively collect every `*.jsonl` under `root` (parent sessions + subagents).
fn collect_jsonl(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            out.push(path);
        }
    }
}

fn scan_file(path: &Path, scan: &mut Scan) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    scan.files += 1;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        scan.records += 1;

        if let Some(t) = v.get("type").and_then(|t| t.as_str()) {
            *scan.record_type_counts.entry(t.to_string()).or_insert(0) += 1;
        }
        if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
            if scan
                .newest_version
                .as_deref()
                .map(|cur| version_gt(ver, cur))
                .unwrap_or(true)
            {
                scan.newest_version = Some(ver.to_string());
            }
        }
        // Content-block types live under message.content[] (array form only).
        if let Some(arr) = v
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for block in arr {
                let Some(bt) = block.get("type").and_then(|t| t.as_str()) else {
                    continue;
                };
                scan.content_block_types.insert(bt.to_string());

                // `tool_result.content[]` can itself be an array of content blocks
                // (text/image/tool_reference). Some block types — notably
                // `tool_reference` (815x in real data, 0x top-level) — appear ONLY
                // nested here, so a top-level-only census is structurally blind to
                // them and the drift gate can never flag the drop. Descend exactly
                // ONE level into tool_result.content[] (mirroring
                // `extract_tool_results`' traversal) — never a deep walk, which
                // would over-collect junk `type` keys out of `tool_use.input`
                // payloads (base64/url/message/shutdown_request/…).
                if bt == "tool_result" {
                    if let Some(inner) = block.get("content").and_then(|c| c.as_array()) {
                        for nested in inner {
                            if let Some(nbt) = nested.get("type").and_then(|t| t.as_str()) {
                                scan.content_block_types.insert(nbt.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `a > b` for dotted numeric version strings ("2.1.138"). Non-numeric segments
/// compare as 0 so a malformed version never panics the gate.
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| {
                p.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    parse(a) > parse(b)
}

#[cfg(test)]
mod tests {
    use super::version_gt;

    #[test]
    fn version_gt_compares_numerically_not_lexically() {
        assert!(version_gt("2.1.160", "2.1.138"));
        assert!(version_gt("2.1.10", "2.1.9")); // lexical would say 10 < 9
        assert!(version_gt("3.0.0", "2.9.9"));
        assert!(!version_gt("2.1.138", "2.1.138")); // equal is not greater
        assert!(!version_gt("2.1.138", "2.1.160"));
        // shorter prefix vs longer: 2.1 < 2.1.1
        assert!(version_gt("2.1.1", "2.1"));
        // malformed segments degrade to 0, never panic
        assert!(!version_gt("2.1.x", "2.1.0"));
    }
}

fn baseline_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join(".claude-view")
            .join("cc-compat")
            .join("baseline.json")
    })
}

fn load_baseline() -> Option<Value> {
    let p = baseline_path()?;
    let txt = fs::read_to_string(p).ok()?;
    serde_json::from_str(&txt).ok()
}

fn write_baseline(
    version: Option<&str>,
    record_types: &BTreeSet<String>,
    content_block_types: &BTreeSet<String>,
) -> std::io::Result<PathBuf> {
    let path = baseline_path()
        .ok_or_else(|| std::io::Error::other("cannot resolve home dir for baseline"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let doc = json!({
        "last_audited_version": version,
        "known_record_types": record_types,
        "known_content_block_types": content_block_types,
    });
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap())?;
    Ok(path)
}
