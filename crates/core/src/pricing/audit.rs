//! Boot-time drift detection: find model IDs present in JSONL but missing from pricing.
//!
//! Complements the static `test_all_active_models_priced` test in `loader.rs`:
//! the test catches known-active models, this scan catches newly-observed ones
//! that appeared in the wild between pricing-table updates.

use std::collections::HashSet;
use std::path::Path;

use crate::session_stats::extract_stats;

use super::lookup::{resolve_pricing, MatchKind};
use super::types::PricingTable;

/// Scan a set of JSONL files for distinct model IDs that lack an *exact* price.
///
/// Caller picks which paths to audit — typical use is the N most recent sessions
/// from `SessionCatalog`. Best-effort: files that fail to parse are skipped
/// silently rather than aborting the scan.
///
/// Returns a sorted, deduplicated list of model IDs that are NOT exactly priced —
/// i.e. either served by the family-nearest fallback (cost is estimated from a
/// sibling release; a precise `data/anthropic-pricing.json` entry should be added)
/// or entirely unknown. Exact / alias / prefix matches are considered fully priced
/// and are not reported.
pub fn scan_unpriced_models(paths: &[(&Path, bool)], pricing: &PricingTable) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut unpriced: Vec<String> = Vec::new();

    for (path, is_compressed) in paths {
        let Ok(stats) = extract_stats(path, *is_compressed) else {
            continue;
        };
        for model_id in stats.per_model_tokens.keys() {
            if !seen.insert(model_id.clone()) {
                continue;
            }
            let needs_exact_entry = match resolve_pricing(model_id, pricing) {
                None => true,
                Some(m) => m.kind == MatchKind::FamilyFallback,
            };
            if needs_exact_entry {
                unpriced.push(model_id.clone());
            }
        }
    }

    unpriced.sort();
    unpriced
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::load_pricing;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_jsonl(dir: &Path, name: &str, lines: &[&str]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        path
    }

    #[test]
    fn flags_unknown_model_in_jsonl() {
        let tmp = tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            "s.jsonl",
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-99-future","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            ],
        );

        let pricing = load_pricing();
        let unpriced = scan_unpriced_models(&[(path.as_path(), false)], &pricing);
        assert_eq!(unpriced, vec!["claude-opus-4-99-future".to_string()]);
    }

    #[test]
    fn ignores_known_models() {
        let tmp = tempdir().unwrap();
        let path = write_jsonl(
            tmp.path(),
            "s.jsonl",
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-4-7","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
                r#"{"type":"assistant","message":{"id":"m2","model":"claude-sonnet-4-6","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            ],
        );

        let pricing = load_pricing();
        let unpriced = scan_unpriced_models(&[(path.as_path(), false)], &pricing);
        assert!(
            unpriced.is_empty(),
            "expected no unpriced, got {:?}",
            unpriced
        );
    }

    #[test]
    fn dedups_across_files() {
        let tmp = tempdir().unwrap();
        let p1 = write_jsonl(
            tmp.path(),
            "a.jsonl",
            &[
                r#"{"type":"assistant","message":{"id":"m1","model":"some-unknown-x","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            ],
        );
        let p2 = write_jsonl(
            tmp.path(),
            "b.jsonl",
            &[
                r#"{"type":"assistant","message":{"id":"m2","model":"some-unknown-x","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            ],
        );

        let pricing = load_pricing();
        let unpriced =
            scan_unpriced_models(&[(p1.as_path(), false), (p2.as_path(), false)], &pricing);
        assert_eq!(unpriced, vec!["some-unknown-x".to_string()]);
    }

    #[test]
    fn skips_unreadable_files() {
        let pricing = load_pricing();
        let bogus = Path::new("/nonexistent/path/to/session.jsonl");
        let unpriced = scan_unpriced_models(&[(bogus, false)], &pricing);
        assert!(unpriced.is_empty(), "unreadable file should not crash scan");
    }
}
