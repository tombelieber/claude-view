// crates/core/src/discovery/roots.rs
//! Resolution of the Claude config directories claude-view reads from.
//!
//! Claude Code supports running against an alternate config directory via its
//! own `CLAUDE_CONFIG_DIR` environment variable. Users who juggle several
//! clients or contexts commonly wrap the CLI in per-context launchers, each
//! exporting a different `CLAUDE_CONFIG_DIR`, which means their session
//! transcripts are spread across `~/.claude-<name>/projects/` rather than
//! living solely in `~/.claude/projects/`.
//!
//! This module resolves the set of config directories to index. The default is
//! unchanged from historical behaviour: `~/.claude` and nothing else. Extra
//! directories are strictly opt-in, via the environment.
//!
//! | Variable                            | Effect                                                     |
//! |-------------------------------------|------------------------------------------------------------|
//! | `CLAUDE_VIEW_PROJECT_ROOTS`         | `:`-separated *project* dirs. Full override.                |
//! | `CLAUDE_VIEW_CONFIG_DIRS`           | `:`-separated *config* dirs; `projects/` appended to each.  |
//! | `CLAUDE_VIEW_DISCOVER_CONFIG_DIRS`  | Truthy: glob `$HOME/.claude*` for sibling config dirs.      |
//! | *(none set)*                        | `~/.claude` only.                                           |
//!
//! The first variable that is set and yields at least one usable path wins;
//! they are not additive. `~/.claude` is always present and always first, so
//! the primary directory can never be configured away.

use crate::error::DiscoveryError;
use std::path::{Path, PathBuf};

/// Env var holding a `:`-separated list of project directories.
const ENV_PROJECT_ROOTS: &str = "CLAUDE_VIEW_PROJECT_ROOTS";
/// Env var holding a `:`-separated list of Claude config directories.
const ENV_CONFIG_DIRS: &str = "CLAUDE_VIEW_CONFIG_DIRS";
/// Env var enabling `$HOME/.claude*` auto-discovery.
const ENV_DISCOVER: &str = "CLAUDE_VIEW_DISCOVER_CONFIG_DIRS";

/// Directory name of the primary config dir, and the auto-discovery prefix.
const PRIMARY_DIR_NAME: &str = ".claude";
/// Profile name reported for the primary config dir.
pub const DEFAULT_PROFILE: &str = "default";

/// Directories matching `$HOME/.claude*` that are never config dirs.
///
/// `.claude-view` is our own data directory; the rest are well-known tools
/// and caches that sit alongside the real config dirs. Auto-discovery also
/// requires a `projects/` or `settings.json` marker, which excludes most
/// strays on its own — this list is belt-and-braces for the ones that could
/// plausibly grow such a marker.
const DISCOVERY_DENYLIST: &[&str] = &[
    ".claude-view",
    ".claude-backup",
    ".claude-worktrees",
    ".claude-trace",
];

/// Expand a leading `~/` against the home directory.
///
/// Wrapper scripts frequently pass `~/.claude-work` inside single quotes,
/// where the shell never expands it. Handling it here avoids a confusing
/// silent miss.
fn expand_tilde(raw: &str) -> PathBuf {
    match raw.strip_prefix("~/") {
        Some(rest) => match dirs::home_dir() {
            Some(home) => home.join(rest),
            None => PathBuf::from(raw),
        },
        None => PathBuf::from(raw),
    }
}

/// Parse a `:`-separated path list, dropping empty segments.
fn parse_path_list(raw: &str) -> Vec<PathBuf> {
    raw.split(':')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(expand_tilde)
        .collect()
}

/// Whether an env var is set to something other than a falsey value.
fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "" | "0" | "false" | "no" | "off")
        }
        Err(_) => false,
    }
}

/// Push `path` onto `out` unless an equivalent path is already present.
///
/// Comparison is on the canonicalised path where possible, so a config dir
/// that is a symlink to one already in the list collapses rather than being
/// indexed twice. Paths that do not exist cannot be canonicalised and are
/// compared literally.
fn push_deduped(out: &mut Vec<PathBuf>, path: PathBuf) {
    let key = path.canonicalize().unwrap_or_else(|_| path.clone());
    let already_present = out
        .iter()
        .any(|existing| existing.canonicalize().unwrap_or_else(|_| existing.clone()) == key);
    if !already_present {
        out.push(path);
    }
}

/// Does this directory look like a Claude config dir?
fn looks_like_config_dir(path: &Path) -> bool {
    path.join("projects").is_dir() || path.join("settings.json").is_file()
}

/// Glob `$HOME/.claude*` for directories that look like Claude config dirs.
///
/// Returns them sorted by directory name so the ordering is stable across
/// runs; `~/.claude` itself is excluded here and prepended by the caller.
fn discover_sibling_config_dirs(home: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };

    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return false;
            };
            name.starts_with(PRIMARY_DIR_NAME)
                && name != PRIMARY_DIR_NAME
                && !DISCOVERY_DENYLIST.contains(&name)
        })
        .map(|entry| entry.path())
        .filter(|path| looks_like_config_dir(path))
        .collect();

    found.sort();
    found
}

/// Resolve the Claude config directories to read.
///
/// `~/.claude` is always element 0. With no relevant environment variables
/// set, it is the only element, which is exactly the historical behaviour.
///
/// # Errors
/// Returns `DiscoveryError::HomeDirNotFound` if the home directory cannot be
/// determined.
pub fn claude_config_dirs() -> Result<Vec<PathBuf>, DiscoveryError> {
    let home = dirs::home_dir().ok_or(DiscoveryError::HomeDirNotFound)?;
    let primary = home.join(PRIMARY_DIR_NAME);

    let mut dirs_out = vec![primary];

    if let Ok(raw) = std::env::var(ENV_CONFIG_DIRS) {
        for path in parse_path_list(&raw) {
            push_deduped(&mut dirs_out, path);
        }
    } else if env_flag(ENV_DISCOVER) {
        for path in discover_sibling_config_dirs(&home) {
            push_deduped(&mut dirs_out, path);
        }
    }

    Ok(dirs_out)
}

/// Resolve the project directories to index, in priority order.
///
/// `~/.claude/projects` is always element 0 and is returned whether or not it
/// exists, matching the historical contract of [`super::claude_projects_dir`].
/// Additional roots are included only when they exist on disk, so a stale
/// entry in `CLAUDE_VIEW_CONFIG_DIRS` degrades to a warning rather than a
/// stream of failed reads.
///
/// # Errors
/// Returns `DiscoveryError::HomeDirNotFound` if the home directory cannot be
/// determined.
pub fn claude_projects_dirs() -> Result<Vec<PathBuf>, DiscoveryError> {
    if let Ok(raw) = std::env::var(ENV_PROJECT_ROOTS) {
        let explicit = parse_path_list(&raw);
        if !explicit.is_empty() {
            let mut out = Vec::with_capacity(explicit.len());
            for path in explicit {
                push_deduped(&mut out, path);
            }
            return Ok(out);
        }
    }

    let config_dirs = claude_config_dirs()?;
    let mut out: Vec<PathBuf> = Vec::with_capacity(config_dirs.len());

    for (index, config_dir) in config_dirs.iter().enumerate() {
        let projects = config_dir.join("projects");
        // Element 0 is returned unconditionally; see the doc comment.
        if index == 0 || projects.is_dir() {
            push_deduped(&mut out, projects);
        } else {
            tracing::warn!(
                config_dir = %config_dir.display(),
                "claude-view: config dir has no projects/ directory; skipping"
            );
        }
    }

    Ok(out)
}

/// Same as [`claude_projects_dirs`] but never fails: an unresolvable home
/// directory yields an empty list.
///
/// Convenience for the watcher and indexer paths, which already no-op when
/// there is nothing to watch rather than aborting startup.
pub fn claude_projects_dirs_or_empty() -> Vec<PathBuf> {
    claude_projects_dirs().unwrap_or_default()
}

/// Human-readable profile name for a config dir.
///
/// `~/.claude` maps to [`DEFAULT_PROFILE`]; `~/.claude-work` maps to `work`.
/// A directory that does not follow the convention keeps its own name, so the
/// value is always something a user can recognise in the UI.
pub fn profile_name(config_dir: &Path) -> String {
    let Some(name) = config_dir.file_name().and_then(|n| n.to_str()) else {
        return DEFAULT_PROFILE.to_string();
    };
    if name == PRIMARY_DIR_NAME {
        return DEFAULT_PROFILE.to_string();
    }
    match name.strip_prefix(".claude-") {
        Some(suffix) if !suffix.is_empty() => suffix.to_string(),
        _ => name.trim_start_matches('.').to_string(),
    }
}

/// Derive the config dir from a session JSONL path, using structure alone.
///
/// Claude Code always writes sessions as
/// `{config_dir}/projects/{project_encoded}/{session_id}.jsonl`, so the config
/// dir is the great-grandparent, verified by the `projects` component. This
/// makes no syscalls and needs no knowledge of the configured roots, which
/// matters because it runs on every session upsert.
///
/// Returns `None` for any path that does not match that shape, so a caller can
/// distinguish "not a session path" from "the primary config dir".
///
/// The `config_dir` backfill in migration `events::MIGRATIONS` mirrors this
/// derivation in SQL; keep the two in step.
pub fn config_dir_from_session_path(path: &Path) -> Option<PathBuf> {
    let projects_dir = path.parent()?.parent()?;
    if projects_dir.file_name()? != "projects" {
        return None;
    }
    projects_dir.parent().map(Path::to_path_buf)
}

/// Given a session file path, return the config dir it belongs to.
///
/// Matches against `roots` (as returned by [`claude_projects_dirs`]) and
/// returns the parent of the longest matching root, so nested roots resolve
/// to the most specific one. Returns `None` when the path is under none of
/// them.
pub fn config_dir_for_path(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let mut best: Option<&PathBuf> = None;
    for root in roots {
        if path.starts_with(root)
            && best.is_none_or(|current| {
                root.as_os_str().len() > current.as_os_str().len()
            })
        {
            best = Some(root);
        }
    }
    best.and_then(|root| root.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_rewrites_leading_home() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(expand_tilde("~/.claude-x"), home.join(".claude-x"));
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
        // A bare `~` is not a prefix match and is left alone.
        assert_eq!(expand_tilde("~weird"), PathBuf::from("~weird"));
    }

    #[test]
    fn parse_path_list_drops_empty_segments() {
        let parsed = parse_path_list("/a::/b: :/c");
        assert_eq!(
            parsed,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
        assert!(parse_path_list("").is_empty());
        assert!(parse_path_list(":::").is_empty());
    }

    // Mutates process-wide environment; serialised against the other env tests.
    #[test]
    #[serial_test::serial]
    fn env_flag_treats_falsey_values_as_unset() {
        for falsey in ["0", "false", "FALSE", "no", "off", "", "  "] {
            std::env::set_var("CLAUDE_VIEW_TEST_FLAG", falsey);
            assert!(!env_flag("CLAUDE_VIEW_TEST_FLAG"), "{falsey} should be off");
        }
        for truthy in ["1", "true", "yes", "on", "anything"] {
            std::env::set_var("CLAUDE_VIEW_TEST_FLAG", truthy);
            assert!(env_flag("CLAUDE_VIEW_TEST_FLAG"), "{truthy} should be on");
        }
        std::env::remove_var("CLAUDE_VIEW_TEST_FLAG");
        assert!(!env_flag("CLAUDE_VIEW_TEST_FLAG"));
    }

    #[test]
    fn push_deduped_collapses_repeats() {
        let mut out = Vec::new();
        push_deduped(&mut out, PathBuf::from("/tmp/claude-view-dedupe-a"));
        push_deduped(&mut out, PathBuf::from("/tmp/claude-view-dedupe-a"));
        push_deduped(&mut out, PathBuf::from("/tmp/claude-view-dedupe-b"));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn profile_name_maps_convention() {
        assert_eq!(profile_name(Path::new("/home/u/.claude")), "default");
        assert_eq!(profile_name(Path::new("/home/u/.claude-work")), "work");
        assert_eq!(profile_name(Path::new("/home/u/.claude-a-b")), "a-b");
        // Non-conforming names stay recognisable rather than collapsing.
        assert_eq!(profile_name(Path::new("/home/u/.claudex")), "claudex");
        assert_eq!(profile_name(Path::new("/home/u/.claude-")), "claude-");
    }

    #[test]
    fn discovery_requires_a_config_marker() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let home = temp.path();

        // Looks like a config dir: has projects/
        std::fs::create_dir_all(home.join(".claude-alpha").join("projects")).unwrap();
        // Looks like a config dir: has settings.json
        std::fs::create_dir_all(home.join(".claude-beta")).unwrap();
        std::fs::write(home.join(".claude-beta").join("settings.json"), "{}").unwrap();
        // Shared helpers directory with neither marker.
        std::fs::create_dir_all(home.join(".claude-shared").join("skills")).unwrap();
        // Denylisted even though it carries a marker.
        std::fs::create_dir_all(home.join(".claude-view").join("projects")).unwrap();
        // The primary dir is excluded here and prepended by the caller.
        std::fs::create_dir_all(home.join(".claude").join("projects")).unwrap();
        // Unrelated prefix.
        std::fs::create_dir_all(home.join(".config").join("projects")).unwrap();

        let found = discover_sibling_config_dirs(home);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(names, vec![".claude-alpha", ".claude-beta"]);
    }

    #[test]
    fn config_dir_from_session_path_uses_structure() {
        assert_eq!(
            config_dir_from_session_path(Path::new(
                "/home/u/.claude-work/projects/-home-u-repo/abc.jsonl"
            )),
            Some(PathBuf::from("/home/u/.claude-work"))
        );
        assert_eq!(
            config_dir_from_session_path(Path::new("/home/u/.claude/projects/p/abc.jsonl")),
            Some(PathBuf::from("/home/u/.claude"))
        );
        // A path whose grandparent is not `projects` is not a session path.
        assert_eq!(
            config_dir_from_session_path(Path::new("/home/u/.claude/todos/p/abc.jsonl")),
            None
        );
        // Subagent files sit deeper, so the derivation must not claim them.
        assert_eq!(
            config_dir_from_session_path(Path::new(
                "/home/u/.claude/projects/p/abc/subagents/agent.jsonl"
            )),
            None
        );
        // A directory literally named `projects` deeper in the tree still
        // resolves relative to that directory, which is the correct answer.
        assert_eq!(
            config_dir_from_session_path(Path::new("/srv/data/projects/p/abc.jsonl")),
            Some(PathBuf::from("/srv/data"))
        );
        assert_eq!(config_dir_from_session_path(Path::new("abc.jsonl")), None);
    }

    #[test]
    fn config_dir_for_path_picks_longest_root() {
        let roots = vec![
            PathBuf::from("/home/u/.claude/projects"),
            PathBuf::from("/home/u/.claude-work/projects"),
        ];

        assert_eq!(
            config_dir_for_path(Path::new("/home/u/.claude-work/projects/p/s.jsonl"), &roots),
            Some(PathBuf::from("/home/u/.claude-work"))
        );
        assert_eq!(
            config_dir_for_path(Path::new("/home/u/.claude/projects/p/s.jsonl"), &roots),
            Some(PathBuf::from("/home/u/.claude"))
        );
        assert_eq!(
            config_dir_for_path(Path::new("/somewhere/else/s.jsonl"), &roots),
            None
        );
    }
}
