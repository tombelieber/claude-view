//! In-memory analytics rollup feature.
//!
//! Maintains `rollup_daily` — a set of `(date, project_id) → tokens`
//! buckets computed from the JSONL corpus. Rebuilds from scratch on
//! `init` (measured at 7.91 s for 8,564 sessions — see P4 bench).
//! No persistence — on restart the rollup is rebuilt.
//!
//! Query API:
//!   - `daily_sums(filter)` — aggregated token counts per day/project.
//!   - `total_sums(filter)` — grand total across all matching buckets.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::features::{Feature, FeatureContext, FeatureError, SessionEvent};
use crate::session_catalog::{CatalogRow, SessionCatalog};

/// One rollup bucket — accumulated token counts for a (date, project) pair.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RollupBucket {
    pub date: String,
    pub project_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub session_count: u32,
}

/// Query filter for rollup reads.
#[derive(Debug, Default, Clone)]
pub struct RollupFilter {
    pub project_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Grand total across matching buckets.
#[derive(Debug, Default, Clone, Serialize)]
pub struct RollupTotal {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub session_count: u32,
    pub bucket_count: usize,
}

type RollupKey = (String, String); // (date, project_id)

/// The analytics rollup feature. Holds an in-memory rollup that is
/// rebuilt from the full JSONL corpus on init and can be queried.
pub struct AnalyticsRollupFeature {
    inner: Arc<RwLock<HashMap<RollupKey, RollupBucket>>>,
}

impl Default for AnalyticsRollupFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsRollupFeature {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Query daily sums matching `filter`.
    pub fn daily_sums(&self, filter: &RollupFilter) -> Vec<RollupBucket> {
        let read = self.inner.read().unwrap();
        let mut out: Vec<RollupBucket> = read
            .values()
            .filter(|b| {
                if let Some(ref p) = filter.project_id {
                    if &b.project_id != p {
                        return false;
                    }
                }
                if let Some(ref from) = filter.date_from {
                    if b.date.as_str() < from.as_str() {
                        return false;
                    }
                }
                if let Some(ref to) = filter.date_to {
                    if b.date.as_str() > to.as_str() {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        out.sort_unstable_by(|a, b| a.date.cmp(&b.date).then(a.project_id.cmp(&b.project_id)));
        out
    }

    /// Grand total across all matching buckets.
    pub fn total_sums(&self, filter: &RollupFilter) -> RollupTotal {
        let buckets = self.daily_sums(filter);
        let mut t = RollupTotal::default();
        t.bucket_count = buckets.len();
        for b in &buckets {
            t.input_tokens += b.input_tokens;
            t.output_tokens += b.output_tokens;
            t.cache_read_tokens += b.cache_read_tokens;
            t.cache_creation_tokens += b.cache_creation_tokens;
            t.session_count += b.session_count;
        }
        t
    }

    /// Bucket count (number of (date, project) pairs).
    pub fn bucket_count(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// Full rebuild from the session catalog. Walks every session's
    /// JSONL/JSONL.gz and accumulates usage lines into rollup buckets.
    pub fn rebuild(&self, catalog: &SessionCatalog) {
        let rows = catalog.list(
            &crate::session_catalog::Filter::default(),
            crate::session_catalog::Sort::LastTsDesc,
            usize::MAX,
        );
        let mut rollup: HashMap<RollupKey, RollupBucket> = HashMap::new();

        for row in &rows {
            Self::apply_session(row, &mut rollup);
        }

        let mut w = self.inner.write().unwrap();
        *w = rollup;
    }

    fn apply_session(row: &CatalogRow, rollup: &mut HashMap<RollupKey, RollupBucket>) {
        let lines: Vec<MinLine> =
            match crate::jsonl_reader::read_all(&row.file_path, row.is_compressed) {
                Ok(l) => l,
                Err(_) => return,
            };

        let mut seen_msg_ids: HashSet<String> = HashSet::new();
        let mut dates_touched: HashSet<String> = HashSet::new();

        for line in &lines {
            if line.line_type.as_deref() != Some("assistant") {
                continue;
            }
            let Some(ref message) = line.message else {
                continue;
            };
            let Some(ref usage) = message.usage else {
                continue;
            };
            if let Some(ref msg_id) = message.id {
                if !seen_msg_ids.insert(msg_id.clone()) {
                    continue;
                }
            }
            let Some(ref ts) = line.timestamp else {
                continue;
            };
            let Some(date) = date_bucket(ts) else {
                continue;
            };

            let key = (date.clone(), row.project_id.clone());
            let bucket = rollup.entry(key).or_insert_with(|| RollupBucket {
                date: date.clone(),
                project_id: row.project_id.clone(),
                ..Default::default()
            });
            bucket.input_tokens += usage.input_tokens.unwrap_or(0);
            bucket.output_tokens += usage.output_tokens.unwrap_or(0);
            bucket.cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
            bucket.cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
            dates_touched.insert(date);
        }

        for date in dates_touched {
            let key = (date, row.project_id.clone());
            if let Some(bucket) = rollup.get_mut(&key) {
                bucket.session_count += 1;
            }
        }
    }
}

impl Feature for AnalyticsRollupFeature {
    fn name(&self) -> &'static str {
        "analytics-rollup"
    }

    fn init(&self, ctx: &FeatureContext) -> Result<(), FeatureError> {
        self.rebuild(&ctx.catalog);
        Ok(())
    }

    fn on_event(&self, _event: &SessionEvent) -> Result<(), FeatureError> {
        // TODO: incremental update for Added/Updated sessions.
        // For now, the full rebuild on init is the only write path.
        // The design doc says incremental is an "optimisation, not a
        // requirement" — nightly full rebuild handles self-healing.
        Ok(())
    }
}

// ---- Minimal typed JSONL shapes (same as the P4 bench) ----

#[derive(Deserialize)]
struct MinLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<MinMessage>,
}

#[derive(Deserialize)]
struct MinMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    usage: Option<MinUsage>,
}

#[derive(Deserialize)]
struct MinUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

fn date_bucket(ts: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(ts).ok().map(|dt| {
        dt.with_timezone(&chrono::Utc)
            .format("%Y-%m-%d")
            .to_string()
    })
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_catalog::SessionCatalog;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    fn make_jsonl(path: &std::path::Path, lines: &[&str]) {
        let mut f = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    fn assistant_line(ts: &str, input: u64, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"msg-{input}-{output}","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}"#
        )
    }

    #[test]
    fn rebuild_from_fixture() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(live.join("proj-b")).unwrap();

        make_jsonl(
            &live.join("proj-a/s1.jsonl"),
            &[
                &assistant_line("2026-04-10T10:00:00Z", 100, 200),
                &assistant_line("2026-04-10T11:00:00Z", 50, 80),
                &assistant_line("2026-04-11T09:00:00Z", 30, 60),
            ],
        );
        make_jsonl(
            &live.join("proj-b/s2.jsonl"),
            &[&assistant_line("2026-04-10T12:00:00Z", 10, 20)],
        );

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();

        let feature = AnalyticsRollupFeature::new();
        feature.rebuild(&catalog);

        assert_eq!(feature.bucket_count(), 3);

        let all = feature.total_sums(&RollupFilter::default());
        assert_eq!(all.input_tokens, 190);
        assert_eq!(all.output_tokens, 360);
        assert_eq!(all.session_count, 3);

        let proj_a = feature.total_sums(&RollupFilter {
            project_id: Some("proj-a".into()),
            ..Default::default()
        });
        assert_eq!(proj_a.input_tokens, 180);
        assert_eq!(proj_a.output_tokens, 340);
        assert_eq!(proj_a.session_count, 2);
    }

    #[test]
    fn daily_sums_filters_by_date_range() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj")).unwrap();

        make_jsonl(
            &live.join("proj/s1.jsonl"),
            &[
                &assistant_line("2026-04-09T10:00:00Z", 10, 10),
                &assistant_line("2026-04-10T10:00:00Z", 20, 20),
                &assistant_line("2026-04-11T10:00:00Z", 30, 30),
                &assistant_line("2026-04-12T10:00:00Z", 40, 40),
            ],
        );

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();

        let feature = AnalyticsRollupFeature::new();
        feature.rebuild(&catalog);

        let filtered = feature.daily_sums(&RollupFilter {
            date_from: Some("2026-04-10".into()),
            date_to: Some("2026-04-11".into()),
            ..Default::default()
        });
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].date, "2026-04-10");
        assert_eq!(filtered[1].date, "2026-04-11");

        let total = feature.total_sums(&RollupFilter {
            date_from: Some("2026-04-10".into()),
            date_to: Some("2026-04-11".into()),
            ..Default::default()
        });
        assert_eq!(total.input_tokens, 50);
        assert_eq!(total.output_tokens, 50);
    }

    #[test]
    fn feature_trait_init_triggers_rebuild() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj")).unwrap();
        make_jsonl(
            &live.join("proj/s1.jsonl"),
            &[&assistant_line("2026-04-10T10:00:00Z", 100, 200)],
        );

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();

        let feature = AnalyticsRollupFeature::new();
        let ctx = FeatureContext {
            data_dir: tmp.path().to_path_buf(),
            catalog: catalog.clone(),
        };
        feature.init(&ctx).unwrap();

        assert_eq!(feature.bucket_count(), 1);
        let t = feature.total_sums(&RollupFilter::default());
        assert_eq!(t.input_tokens, 100);
        assert_eq!(t.output_tokens, 200);
    }
}
