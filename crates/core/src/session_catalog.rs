//! In-memory catalog of every session JSONL known to claude-view.
//!
//! This module is the **core** layer of the JSONL-first architecture
//! described in `docs/plans/2026-04-16-hardcut-jsonl-first-design.md`.
//!
//! Responsibilities:
//! - Walk `~/.claude/projects/` for live `.jsonl` files.
//! - Walk `~/.claude-backup/machines/*/projects/` for archived
//!   `.jsonl.gz` files.
//! - Deduplicate by session id (live wins on collision — it is the
//!   more recent copy).
//! - Expose filtered, sorted reads via `SessionCatalog::list`.
//!
//! What this module deliberately does NOT do:
//! - Read the contents of any JSONL file. That is the job of the
//!   `jsonl_reader` module.
//! - Persist anything to SQLite. The catalog is pure in-memory state.
//! - Parse Claude Code's `sessions-index.json` metadata files. That
//!   concern lives in the `session_index` module (different data
//!   source, different problem).
//!
//! # Example
//!
//! ```ignore
//! use claude_view_core::session_catalog::{SessionCatalog, Filter, Sort};
//! use std::path::PathBuf;
//!
//! let catalog = SessionCatalog::new();
//! catalog.rebuild_from_filesystem(
//!     &PathBuf::from("/home/user/.claude/projects"),
//!     &PathBuf::from("/home/user/.claude-backup/machines"),
//! )?;
//!
//! let recent = catalog.list(&Filter::default(), Sort::LastTsDesc, 50);
//! # Ok::<(), std::io::Error>(())
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub type SessionId = String;
pub type ProjectId = String;

/// One row in the in-memory session catalog.
///
/// Everything expensive (total tokens, turn list, cost) is computed
/// on read from the JSONL file pointed to by `file_path`. Only
/// filesystem-cheap metadata lives in the row itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogRow {
    pub id: SessionId,
    pub file_path: PathBuf,
    pub is_compressed: bool,
    pub bytes: u64,
    pub mtime: i64,
    pub project_id: ProjectId,
    /// Optional — computed lazily the first time a caller needs
    /// "when did this session start". Populated from the first
    /// line of the JSONL.
    pub first_ts: Option<i64>,
    /// Optional — defaults to `mtime`. Can be refined by reading
    /// the last line of the JSONL.
    pub last_ts: Option<i64>,
}

impl CatalogRow {
    /// Effective timestamp for sorting: use `last_ts` if populated,
    /// else fall back to `mtime`.
    pub fn sort_ts(&self) -> i64 {
        self.last_ts.unwrap_or(self.mtime)
    }
}

/// Filter predicate applied during `SessionCatalog::list`.
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub project_id: Option<ProjectId>,
    pub min_last_ts: Option<i64>,
}

impl Filter {
    pub fn by_project(project_id: impl Into<ProjectId>) -> Self {
        Self {
            project_id: Some(project_id.into()),
            ..Default::default()
        }
    }

    fn matches(&self, row: &CatalogRow) -> bool {
        if let Some(ref proj) = self.project_id {
            if &row.project_id != proj {
                return false;
            }
        }
        if let Some(min_ts) = self.min_last_ts {
            if row.sort_ts() < min_ts {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Sort {
    LastTsDesc,
    LastTsAsc,
}

/// Stats returned from a filesystem walk.
#[derive(Debug, Default, Clone, Serialize)]
pub struct WalkStats {
    pub live_found: usize,
    pub backup_found: usize,
    pub backup_unique: usize,
    pub total_after_dedup: usize,
}

/// Thread-safe handle to the in-memory session catalog. Cloneable —
/// all clones share the same underlying `RwLock`.
#[derive(Debug, Clone)]
pub struct SessionCatalog {
    inner: Arc<RwLock<HashMap<SessionId, CatalogRow>>>,
}

impl Default for SessionCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCatalog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, id: &str) -> Option<CatalogRow> {
        self.inner.read().unwrap().get(id).cloned()
    }

    /// Replace the catalog contents with `rows`. Callers using the
    /// walk helpers should prefer `rebuild_from_filesystem`.
    pub fn replace_all(&self, rows: Vec<CatalogRow>) {
        let mut w = self.inner.write().unwrap();
        w.clear();
        for row in rows {
            w.insert(row.id.clone(), row);
        }
    }

    /// List rows matching `filter`, sorted, limited.
    pub fn list(&self, filter: &Filter, sort: Sort, limit: usize) -> Vec<CatalogRow> {
        let read = self.inner.read().unwrap();
        let mut matched: Vec<CatalogRow> = read
            .values()
            .filter(|r| filter.matches(r))
            .cloned()
            .collect();
        match sort {
            Sort::LastTsDesc => matched.sort_unstable_by_key(|r| std::cmp::Reverse(r.sort_ts())),
            Sort::LastTsAsc => matched.sort_unstable_by_key(|r| r.sort_ts()),
        }
        matched.truncate(limit);
        matched
    }

    /// Distinct project ids with session counts.
    pub fn projects(&self) -> HashMap<ProjectId, usize> {
        let read = self.inner.read().unwrap();
        let mut out: HashMap<ProjectId, usize> = HashMap::new();
        for r in read.values() {
            *out.entry(r.project_id.clone()).or_default() += 1;
        }
        out
    }

    /// Walk live + backup roots, dedup by session id (live wins),
    /// replace the catalog contents, return stats.
    pub fn rebuild_from_filesystem(
        &self,
        live_root: &Path,
        backup_machines_root: &Path,
    ) -> std::io::Result<WalkStats> {
        let mut stats = WalkStats::default();
        let mut by_id: HashMap<SessionId, CatalogRow> = HashMap::new();

        // Pass 1 — live
        if live_root.is_dir() {
            for row in walk_root(live_root, ".jsonl", false) {
                stats.live_found += 1;
                by_id.insert(row.id.clone(), row);
            }
        }

        // Pass 2 — backup (one project dir per machine)
        if backup_machines_root.is_dir() {
            for entry in std::fs::read_dir(backup_machines_root)? {
                let entry = entry?;
                let projects = entry.path().join("projects");
                if !projects.is_dir() {
                    continue;
                }
                for row in walk_root(&projects, ".jsonl.gz", true) {
                    stats.backup_found += 1;
                    if !by_id.contains_key(&row.id) {
                        stats.backup_unique += 1;
                        by_id.insert(row.id.clone(), row);
                    }
                }
            }
        }

        stats.total_after_dedup = by_id.len();
        self.replace_all(by_id.into_values().collect());
        Ok(stats)
    }
}

/// Walk a single root looking for files whose name ends with
/// `suffix`. Parent sessions live at `<root>/<project>/<sid><suffix>`.
/// Subagent sidecar files (depth > 2) are excluded.
fn walk_root(root: &Path, suffix: &str, is_compressed: bool) -> Vec<CatalogRow> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.ends_with(suffix))
        })
        .filter_map(|e| {
            let path = e.path();
            let rel = path.strip_prefix(root).ok()?;
            let components: Vec<_> = rel.components().collect();
            if components.len() != 2 {
                return None;
            }
            let project_id = components[0].as_os_str().to_str()?.to_string();
            let filename = path.file_name()?.to_str()?;
            let id = filename.strip_suffix(suffix)?.to_string();
            let meta = e.metadata().ok()?;
            let mtime = meta
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            Some(CatalogRow {
                id,
                file_path: path.to_path_buf(),
                is_compressed,
                bytes: meta.len(),
                mtime,
                project_id,
                first_ts: None,
                last_ts: None,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::Write;

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tempfile::tempdir;

    fn make_jsonl(path: &Path, lines: &[&str]) {
        let mut f = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
    }

    fn make_gz(path: &Path, lines: &[&str]) {
        let f = fs::File::create(path).unwrap();
        let mut enc = GzEncoder::new(f, Compression::default());
        for line in lines {
            writeln!(enc, "{}", line).unwrap();
        }
        enc.finish().unwrap();
    }

    #[test]
    fn catalog_rebuilds_from_live_only() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(live.join("proj-b")).unwrap();
        make_jsonl(
            &live.join("proj-a/sess-1.jsonl"),
            &[r#"{"type":"assistant"}"#],
        );
        make_jsonl(&live.join("proj-a/sess-2.jsonl"), &[r#"{"type":"user"}"#]);
        make_jsonl(
            &live.join("proj-b/sess-3.jsonl"),
            &[r#"{"type":"assistant"}"#],
        );

        let catalog = SessionCatalog::new();
        let backup_root = tmp.path().join("nonexistent");
        let stats = catalog
            .rebuild_from_filesystem(&live, &backup_root)
            .unwrap();
        assert_eq!(stats.live_found, 3);
        assert_eq!(stats.backup_found, 0);
        assert_eq!(stats.total_after_dedup, 3);
        assert_eq!(catalog.len(), 3);
    }

    #[test]
    fn catalog_dedups_live_and_backup() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        let backup_machines = tmp.path().join("machines");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(backup_machines.join("m1/projects/proj-a")).unwrap();
        fs::create_dir_all(backup_machines.join("m1/projects/proj-z")).unwrap();

        // Live has sess-1 and sess-2.
        make_jsonl(
            &live.join("proj-a/sess-1.jsonl"),
            &[r#"{"type":"assistant"}"#],
        );
        make_jsonl(
            &live.join("proj-a/sess-2.jsonl"),
            &[r#"{"type":"assistant"}"#],
        );
        // Backup has sess-1 (collision, live wins) and sess-3 (unique).
        make_gz(
            &backup_machines.join("m1/projects/proj-a/sess-1.jsonl.gz"),
            &[r#"{"type":"assistant"}"#],
        );
        make_gz(
            &backup_machines.join("m1/projects/proj-z/sess-3.jsonl.gz"),
            &[r#"{"type":"assistant"}"#],
        );

        let catalog = SessionCatalog::new();
        let stats = catalog
            .rebuild_from_filesystem(&live, &backup_machines)
            .unwrap();
        assert_eq!(stats.live_found, 2);
        assert_eq!(stats.backup_found, 2);
        assert_eq!(stats.backup_unique, 1, "only sess-3 is new");
        assert_eq!(stats.total_after_dedup, 3);

        // sess-1 should come from live (not compressed)
        let sess1 = catalog.get("sess-1").unwrap();
        assert!(!sess1.is_compressed);

        // sess-3 should come from backup (compressed)
        let sess3 = catalog.get("sess-3").unwrap();
        assert!(sess3.is_compressed);
    }

    #[test]
    fn catalog_filter_by_project() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(live.join("proj-b")).unwrap();
        for i in 0..5 {
            make_jsonl(
                &live.join(format!("proj-a/a-{i}.jsonl")),
                &[r#"{"type":"assistant"}"#],
            );
        }
        for i in 0..3 {
            make_jsonl(
                &live.join(format!("proj-b/b-{i}.jsonl")),
                &[r#"{"type":"assistant"}"#],
            );
        }

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();

        let all = catalog.list(&Filter::default(), Sort::LastTsDesc, 100);
        assert_eq!(all.len(), 8);

        let only_a = catalog.list(&Filter::by_project("proj-a"), Sort::LastTsDesc, 100);
        assert_eq!(only_a.len(), 5);
        assert!(only_a.iter().all(|r| r.project_id == "proj-a"));
    }

    #[test]
    fn catalog_projects_counts() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(live.join("proj-b")).unwrap();
        make_jsonl(&live.join("proj-a/x.jsonl"), &["{}"]);
        make_jsonl(&live.join("proj-a/y.jsonl"), &["{}"]);
        make_jsonl(&live.join("proj-b/z.jsonl"), &["{}"]);

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();
        let projects = catalog.projects();
        assert_eq!(projects.get("proj-a").copied(), Some(2));
        assert_eq!(projects.get("proj-b").copied(), Some(1));
    }

    #[test]
    fn catalog_excludes_subagent_sidecar() {
        let tmp = tempdir().unwrap();
        let live = tmp.path().join("live");
        fs::create_dir_all(live.join("proj-a")).unwrap();
        fs::create_dir_all(live.join("proj-a/sess-1/subagents")).unwrap();
        make_jsonl(&live.join("proj-a/sess-1.jsonl"), &["{}"]);
        make_jsonl(&live.join("proj-a/sess-1/subagents/sub-a.jsonl"), &["{}"]);
        make_jsonl(&live.join("proj-a/sess-1/subagents/sub-b.jsonl"), &["{}"]);

        let catalog = SessionCatalog::new();
        catalog
            .rebuild_from_filesystem(&live, &tmp.path().join("nope"))
            .unwrap();
        assert_eq!(catalog.len(), 1, "subagent sidecars must be excluded");
    }
}
