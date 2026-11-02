//! Resolve PATH in-process once, rather than making posix_spawn try every entry.
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::process::Command;

static GIT: Mutex<Option<PathBuf>> = Mutex::new(None);

fn is_executable_file(path: &std::path::Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    true
}

fn resolve_git(path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| {
        let candidate = dir.join(if cfg!(windows) { "git.exe" } else { "git" });
        if !is_executable_file(&candidate) {
            return None;
        }
        std::fs::canonicalize(candidate).ok()
    })
}

fn cached_git(cache: &Mutex<Option<PathBuf>>, path: &std::ffi::OsStr) -> io::Result<PathBuf> {
    let mut cached = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Homebrew upgrades can delete the canonical Cellar target. Resolve again
    // when that happens, and never keep a negative lookup across sync ticks.
    if cached.as_ref().is_none_or(|path| !is_executable_file(path)) {
        *cached = resolve_git(path);
    }
    cached
        .clone()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "git not found in PATH"))
}

pub(super) fn git_command() -> io::Result<Command> {
    let path = cached_git(&GIT, &std::env::var_os("PATH").unwrap_or_default())?;
    let mut command = Command::new(path);
    // Git hooks export these; they must not redirect scans to the caller's repo.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(key);
    }
    command.kill_on_drop(true);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_absolute_executable_after_missing_and_non_executable_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");
        let real = tmp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let git = real.join(if cfg!(windows) { "git.exe" } else { "git" });
        std::fs::write(&git, "fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(resolve_git(&std::env::join_paths([&missing, &real]).unwrap()).is_none());
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::join_paths([&missing, &real]).unwrap();
        assert_eq!(
            resolve_git(&path),
            Some(std::fs::canonicalize(git).unwrap())
        );
        assert!(resolve_git(missing.as_os_str()).is_none());
    }
    #[test]
    fn missing_git_and_deleted_upgrade_target_recover_without_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        std::fs::create_dir(&dir).unwrap();
        let path = dir.as_os_str();
        let cache = Mutex::new(None);
        assert_eq!(
            cached_git(&cache, path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(cache.lock().unwrap().is_none());
        let git = dir.join(if cfg!(windows) { "git.exe" } else { "git" });
        std::fs::write(&git, "fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let resolved = cached_git(&cache, path).unwrap();
        assert!(resolved.is_absolute());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(cached_git(&cache, path).is_err());
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(cached_git(&cache, path).unwrap(), resolved);
        }

        std::fs::remove_file(&git).unwrap();
        assert_eq!(
            cached_git(&cache, path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        std::fs::write(&git, "upgraded fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(cached_git(&cache, path).unwrap(), resolved);
    }
}
