//! Search index migration coordinator.
//!
//! Handles the transition from the legacy flat layout to the versioned
//! `v{N}/` layout, and orchestrates blue-green schema migrations by
//! returning a `PendingMigration` that the caller can use to spawn a
//! background rebuild.

use std::path::{Path, PathBuf};

use crate::version_layout::{
    current_versioned_path, is_complete_versioned_index, is_legacy_flat_layout,
    list_versioned_dirs, read_schema_version_file, versioned_path,
};
use crate::{SearchError, SEARCH_SCHEMA_VERSION};

/// Migrate a legacy flat-layout search index into a versioned subdirectory.
///
/// Preconditions: `base` contains a `schema_version` file at root AND tantivy
/// data files at root (verified by `is_legacy_flat_layout`).
///
/// Behavior:
/// 1. Reads `base/schema_version` to learn the legacy version L.
/// 2. Creates `base/v{L}/` (deleting any half-built `v{L}/` first).
/// 3. Moves every tantivy data file (and `meta.json`, lock files) from `base`
///    to `base/v{L}/` via `fs::rename` (atomic on same filesystem).
/// 4. Moves `schema_version` LAST. Until this final move completes, a crash
///    leaves the legacy file at root, so the next startup retries the migration.
/// 5. Returns the path of the new `v{L}/` directory.
///
/// Errors:
/// - `SearchError::Io` if reading the version file fails or any rename fails.
pub fn migrate_legacy_flat_layout(base: &Path) -> Result<PathBuf, SearchError> {
    let legacy_version = read_schema_version_file(base).ok_or_else(|| {
        SearchError::Io(std::io::Error::other(
            "legacy migration: schema_version file missing or unreadable",
        ))
    })?;

    let target_dir = versioned_path(base, legacy_version);

    // Idempotent retry: if a previous migration crashed mid-way, scrap the
    // half-built v{L}/ and start over.
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)?;
    }
    std::fs::create_dir_all(&target_dir)?;

    // Walk root, move every entry that is not a `v*` directory and not the
    // `schema_version` file. Move `schema_version` LAST for crash safety.
    let entries = std::fs::read_dir(base)?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "schema_version" {
            continue;
        }
        if path.is_dir() && name.starts_with('v') {
            // Existing versioned directory — leave alone.
            continue;
        }
        let dest = target_dir.join(name);
        std::fs::rename(&path, &dest)?;
    }

    // Final atomic step: move schema_version into the versioned dir.
    let src_version = base.join("schema_version");
    let dst_version = target_dir.join("schema_version");
    std::fs::rename(&src_version, &dst_version)?;

    Ok(target_dir)
}

/// Outcome of `open_versioned()`. The caller MUST install `index` into the
/// holder immediately. If `pending_migration` is `Some`, the caller MUST
/// spawn a background task to build the new version and atomically swap.
pub struct VersionedOpenResult {
    /// The index to use right now. May be the previous-version fallback if
    /// a rebuild is in progress.
    pub index: crate::SearchIndex,
    /// If present, the caller must build a new index at `target_path` and
    /// then atomically swap it into the holder, then call `cleanup_old_version`.
    pub pending_migration: Option<PendingMigration>,
}

/// Instructions for completing a blue-green migration.
#[derive(Debug, Clone)]
pub struct PendingMigration {
    /// Where to build the new index (e.g. `<base>/v7/`). The directory does
    /// NOT exist yet — the rebuild orchestrator must create it.
    pub target_path: PathBuf,
    /// The schema version being migrated to.
    pub target_version: u32,
    /// The currently-serving previous-version directory, to be deleted after
    /// the new index is committed and swapped. `None` if there was no
    /// fallback (first-ever startup or all previous versions corrupt).
    pub old_version_path: Option<PathBuf>,
}

/// Delete an old versioned index directory after a successful swap.
pub fn cleanup_old_version(path: &Path) -> Result<(), SearchError> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
        tracing::info!(path = %path.display(), "removed old search index version");
    }
    Ok(())
}

/// Resolve which search index to open and whether a background rebuild is
/// needed. See `SearchIndex::open_versioned` for the user-facing entry point.
///
/// Decision tree:
/// 1. If `<base>/v{CURRENT}/` exists AND is complete → open it, no migration.
/// 2. Else if legacy flat layout at `<base>/` → migrate it in-place to
///    `<base>/v{L}/`. If L == CURRENT, open it directly. If L < CURRENT,
///    open it as the fallback and return a pending migration to v{CURRENT}.
/// 3. Else if `<base>/v{N}/` exists for some N < CURRENT → open the highest
///    valid one as the fallback, return a pending migration to v{CURRENT}.
/// 4. Else → first-ever startup. Create `<base>/v{CURRENT}/` and return it
///    with no pending migration (the caller's normal indexing flow will
///    populate it; no fallback exists so there is no "blue" to keep serving).
pub(crate) fn resolve_open_plan(base: &Path) -> Result<ResolvedPlan, SearchError> {
    std::fs::create_dir_all(base)?;

    let current_path = current_versioned_path(base);

    // Case 1: current versioned index is complete and ready.
    if is_complete_versioned_index(&current_path, SEARCH_SCHEMA_VERSION) {
        return Ok(ResolvedPlan {
            open_path: current_path,
            pending_migration: None,
        });
    }

    // Case 1b: current versioned dir exists but is incomplete (interrupted
    // rebuild). Discard it; we'll either find a fallback or build fresh.
    if current_path.exists() {
        std::fs::remove_dir_all(&current_path)?;
    }

    // Case 2: legacy flat layout at root. Migrate in-place.
    if is_legacy_flat_layout(base) {
        let migrated = migrate_legacy_flat_layout(base)?;
        let migrated_version = read_schema_version_file(&migrated).ok_or_else(|| {
            SearchError::Io(std::io::Error::other(
                "post-migration: schema_version file missing in target",
            ))
        })?;

        if migrated_version == SEARCH_SCHEMA_VERSION {
            return Ok(ResolvedPlan {
                open_path: migrated,
                pending_migration: None,
            });
        } else {
            return Ok(ResolvedPlan {
                open_path: migrated.clone(),
                pending_migration: Some(PendingMigration {
                    target_path: current_versioned_path(base),
                    target_version: SEARCH_SCHEMA_VERSION,
                    old_version_path: Some(migrated),
                }),
            });
        }
    }

    // Case 3: existing versioned dirs for older versions only.
    let existing = list_versioned_dirs(base);
    if let Some((found_version, found_path)) = existing
        .into_iter()
        .find(|(v, p)| is_complete_versioned_index(p, *v))
    {
        tracing::info!(
            version = found_version,
            path = %found_path.display(),
            "opening older search index version as fallback during blue-green migration"
        );
        return Ok(ResolvedPlan {
            open_path: found_path.clone(),
            pending_migration: Some(PendingMigration {
                target_path: current_versioned_path(base),
                target_version: SEARCH_SCHEMA_VERSION,
                old_version_path: Some(found_path),
            }),
        });
    }

    // Case 4: nothing usable. Build fresh at v{CURRENT}/, no fallback.
    std::fs::create_dir_all(&current_path)?;
    Ok(ResolvedPlan {
        open_path: current_path,
        pending_migration: None,
    })
}

pub(crate) struct ResolvedPlan {
    pub open_path: PathBuf,
    pub pending_migration: Option<PendingMigration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn make_legacy_flat(base: &Path, version: u32) {
        write_file(&base.join("schema_version"), version.to_string().as_bytes());
        write_file(&base.join("abc123.idx"), b"fake idx");
        write_file(&base.join("abc123.store"), b"fake store");
        write_file(&base.join("meta.json"), b"{}");
    }

    fn make_complete_versioned(base: &Path, version: u32) {
        let dir = base.join(format!("v{version}"));
        std::fs::create_dir_all(&dir).unwrap();
        write_file(&dir.join("schema_version"), version.to_string().as_bytes());
        write_file(&dir.join("xyz.idx"), b"fake idx");
        write_file(&dir.join("xyz.store"), b"fake store");
        write_file(&dir.join("meta.json"), b"{}");
    }

    #[test]
    fn migrate_legacy_moves_files_into_versioned_subdir() {
        let dir = tempdir().unwrap();
        make_legacy_flat(dir.path(), 7);

        let result = migrate_legacy_flat_layout(dir.path()).unwrap();

        assert_eq!(result, dir.path().join("v7"));
        assert!(result.join("schema_version").exists());
        assert!(result.join("abc123.idx").exists());
        assert!(result.join("abc123.store").exists());
        assert!(result.join("meta.json").exists());
        // Root must no longer have those files
        assert!(!dir.path().join("schema_version").exists());
        assert!(!dir.path().join("abc123.idx").exists());
    }

    #[test]
    fn migrate_legacy_recovers_from_half_built_target() {
        let dir = tempdir().unwrap();
        make_legacy_flat(dir.path(), 7);
        // Simulate a previously-crashed migration: half-built v7 dir
        let v7 = dir.path().join("v7");
        std::fs::create_dir_all(&v7).unwrap();
        write_file(&v7.join("garbage.idx"), b"stale");

        let result = migrate_legacy_flat_layout(dir.path()).unwrap();

        // Stale file must be gone, new files must be present
        assert!(!result.join("garbage.idx").exists());
        assert!(result.join("abc123.idx").exists());
    }

    #[test]
    fn resolve_plan_first_ever_startup() {
        let dir = tempdir().unwrap();
        let plan = resolve_open_plan(dir.path()).unwrap();
        assert_eq!(plan.open_path, current_versioned_path(dir.path()));
        assert!(plan.pending_migration.is_none());
        // The directory must have been created
        assert!(plan.open_path.exists());
    }

    #[test]
    fn resolve_plan_existing_current_version() {
        let dir = tempdir().unwrap();
        make_complete_versioned(dir.path(), SEARCH_SCHEMA_VERSION);

        let plan = resolve_open_plan(dir.path()).unwrap();
        assert_eq!(plan.open_path, current_versioned_path(dir.path()));
        assert!(plan.pending_migration.is_none());
    }

    #[test]
    fn resolve_plan_legacy_matching_version_migrates_in_place() {
        let dir = tempdir().unwrap();
        make_legacy_flat(dir.path(), SEARCH_SCHEMA_VERSION);

        let plan = resolve_open_plan(dir.path()).unwrap();
        assert_eq!(plan.open_path, current_versioned_path(dir.path()));
        assert!(plan.pending_migration.is_none(), "no rebuild needed");
        // Files moved
        assert!(plan.open_path.join("abc123.idx").exists());
    }

    #[test]
    fn resolve_plan_legacy_old_version_returns_pending() {
        let dir = tempdir().unwrap();
        let old_version = SEARCH_SCHEMA_VERSION - 1;
        make_legacy_flat(dir.path(), old_version);

        let plan = resolve_open_plan(dir.path()).unwrap();
        // Opens the migrated old version as fallback
        assert_eq!(plan.open_path, dir.path().join(format!("v{old_version}")));
        // Pending migration to current version
        let pending = plan.pending_migration.expect("should have pending");
        assert_eq!(pending.target_version, SEARCH_SCHEMA_VERSION);
        assert_eq!(pending.target_path, current_versioned_path(dir.path()));
        assert_eq!(
            pending.old_version_path,
            Some(dir.path().join(format!("v{old_version}")))
        );
    }

    #[test]
    fn resolve_plan_existing_old_versioned_dir_returns_pending() {
        let dir = tempdir().unwrap();
        let old_version = SEARCH_SCHEMA_VERSION - 1;
        make_complete_versioned(dir.path(), old_version);

        let plan = resolve_open_plan(dir.path()).unwrap();
        assert_eq!(plan.open_path, dir.path().join(format!("v{old_version}")));
        let pending = plan.pending_migration.expect("should have pending");
        assert_eq!(pending.target_version, SEARCH_SCHEMA_VERSION);
        assert_eq!(
            pending.old_version_path,
            Some(dir.path().join(format!("v{old_version}")))
        );
    }

    #[test]
    fn resolve_plan_picks_highest_complete_old_version() {
        let dir = tempdir().unwrap();
        if SEARCH_SCHEMA_VERSION < 3 {
            return; // not enough room for this test
        }
        make_complete_versioned(dir.path(), SEARCH_SCHEMA_VERSION - 1);
        make_complete_versioned(dir.path(), SEARCH_SCHEMA_VERSION - 2);

        let plan = resolve_open_plan(dir.path()).unwrap();
        assert_eq!(
            plan.open_path,
            dir.path().join(format!("v{}", SEARCH_SCHEMA_VERSION - 1))
        );
    }

    #[test]
    fn resolve_plan_discards_incomplete_current_version() {
        let dir = tempdir().unwrap();
        // Half-built current: directory exists, but no schema_version file
        let current = current_versioned_path(dir.path());
        std::fs::create_dir_all(&current).unwrap();
        write_file(&current.join("garbage.idx"), b"junk");
        // Also a complete old version as fallback
        let old_version = SEARCH_SCHEMA_VERSION - 1;
        make_complete_versioned(dir.path(), old_version);

        let plan = resolve_open_plan(dir.path()).unwrap();
        // Should fall back to old_version, with pending rebuild of current
        assert_eq!(plan.open_path, dir.path().join(format!("v{old_version}")));
        assert!(plan.pending_migration.is_some());
        // The garbage v{CURRENT}/ must have been removed
        assert!(!current.exists() || current.read_dir().unwrap().next().is_none());
    }

    #[test]
    fn cleanup_old_version_removes_directory() {
        let dir = tempdir().unwrap();
        let old = dir.path().join("v5");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("foo.idx"), b"x").unwrap();

        cleanup_old_version(&old).unwrap();
        assert!(!old.exists());
    }

    #[test]
    fn cleanup_old_version_idempotent_on_missing_dir() {
        let dir = tempdir().unwrap();
        let nonexistent = dir.path().join("v99");
        // Should not error
        cleanup_old_version(&nonexistent).unwrap();
    }
}
