//! Conservative filesystem invalidation for the periodic sync. Git remains the
//! authority for history: changed inputs trigger the same bounded full scan, so
//! resets, rebases and branch switches cannot be lost to timestamp watermarks.
use super::types::SessionSyncInfo;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct GitSyncCache {
    pub(super) entries: HashMap<PathBuf, CachedInputs>,
}

#[derive(Debug)]
pub(super) struct CachedInputs {
    pub state: Vec<u8>,
    pub sessions: Vec<SessionSyncInfo>,
    pub checked_at: Instant,
}

impl CachedInputs {
    pub fn unchanged(&self, state: &[u8], sessions: &[SessionSyncInfo]) -> bool {
        // A safety refresh bounds staleness for out-of-band object/config edits.
        // Ordinary ref/session changes invalidate immediately, without waiting.
        self.checked_at.elapsed() < Duration::from_secs(600)
            && self.state == state
            && self.sessions == sessions
    }
}

pub(super) struct RepoLocation {
    pub root: PathBuf,
    // Per-worktree gitdir, deliberately NOT common-dir: each worktree has its
    // own HEAD and can show different history. Aliases/subdirectories share it.
    pub git_dir: PathBuf,
}

pub(super) fn discover(path: &Path) -> Option<RepoLocation> {
    let path = std::fs::canonicalize(path).ok()?;
    if !path.is_dir() {
        return None;
    }
    for root in path.ancestors().take(64) {
        let marker = root.join(".git");
        if marker.is_dir() {
            return Some(RepoLocation {
                root: root.to_owned(),
                git_dir: std::fs::canonicalize(marker).ok()?,
            });
        }
        if marker.is_file() {
            let content = std::fs::read_to_string(&marker).ok()?;
            let dir = content.trim().strip_prefix("gitdir: ")?;
            return Some(RepoLocation {
                root: root.to_owned(),
                git_dir: std::fs::canonicalize(root.join(dir)).ok()?,
            });
        }
        // Bare repositories have no .git marker.
        if root.join("HEAD").is_file()
            && root.join("objects").is_dir()
            && root.join("refs").is_dir()
        {
            return Some(RepoLocation {
                root: root.to_owned(),
                git_dir: root.to_owned(),
            });
        }
    }
    None
}

fn read_optional(path: &Path) -> io::Result<Vec<u8>> {
    match std::fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > 4 * 1024 * 1024 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // Distinguish missing and empty files.
            bytes.insert(0, 1);
            Ok(bytes)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(vec![0]),
        Err(e) => Err(e),
    }
}

/// None means unsupported/unreadable state: scan with git; never skip on doubt.
/// See gitrepository-layout: HEAD and worktree-private refs are per-worktree;
/// ordinary refs, packed refs, shallow boundaries and config are shared.
pub(super) fn fingerprint(repo: &RepoLocation) -> Option<Vec<u8>> {
    let common_file = read_optional(&repo.git_dir.join("commondir")).ok()?;
    let common = if common_file.first() == Some(&1) {
        std::fs::canonicalize(
            repo.git_dir
                .join(std::str::from_utf8(&common_file[1..]).ok()?.trim()),
        )
        .ok()?
    } else {
        repo.git_dir.clone()
    };
    if common.join("reftable").exists() || common.join("refs/replace").exists() {
        return None;
    }
    let mut state = Vec::new();
    let mut add = |value: &[u8]| {
        state.extend_from_slice(&(value.len() as u64).to_le_bytes());
        state.extend_from_slice(value);
    };
    add(common.as_os_str().as_encoded_bytes());
    for file in [
        common.join("config"),
        repo.git_dir.join("config.worktree"),
        common.join("packed-refs"),
        common.join("shallow"),
        common.join("info/grafts"),
        common.join("objects/info/alternates"),
    ] {
        let bytes = read_optional(&file).ok()?;
        if file.ends_with("packed-refs") && bytes.windows(13).any(|w| w == b"refs/replace/") {
            return None;
        }
        add(&bytes);
    }
    let mut head = std::fs::read_to_string(repo.git_dir.join("HEAD")).ok()?;
    for _ in 0..16 {
        add(head.as_bytes());
        match head.trim().strip_prefix("ref: ") {
            Some(reference) => {
                // Do not follow malformed refs out of the metadata directory.
                if !reference.starts_with("refs/") || reference.split('/').any(|p| p == "..") {
                    return None;
                }
                let base = if reference.starts_with("refs/bisect/")
                    || reference.starts_with("refs/worktree/")
                    || reference.starts_with("refs/rewritten/")
                {
                    &repo.git_dir
                } else {
                    &common
                };
                let bytes = read_optional(&base.join(reference)).ok()?;
                add(&bytes);
                if bytes == [0] {
                    return Some(state);
                } // packed or unborn
                head = std::str::from_utf8(&bytes[1..]).ok()?.to_owned();
            }
            None => return Some(state), // detached HEAD or resolved loose ref
        }
    }
    None // symbolic-ref cycle
}
