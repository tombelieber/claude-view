//! Static-asset directory discovery.
//!
//! Extracted from `main.rs` in CQRS Phase 7.c. Same four-level
//! precedence the runtime has relied on since Phase 1: explicit env
//! var → binary-relative → monorepo layout → flat layout.

use std::path::PathBuf;

/// Get the static directory for serving frontend files.
///
/// Priority:
/// 1. `STATIC_DIR` environment variable (explicit override)
/// 2. Binary-relative `./dist` (npx distribution: binary + dist/ are siblings)
/// 3. CWD-relative `./apps/web/dist` (monorepo dev layout via cargo run)
/// 4. CWD-relative `./dist` (legacy flat layout)
/// 5. `None` (API-only mode)
pub fn get_static_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("STATIC_DIR") {
        let p = PathBuf::from(&dir);
        if p.exists() {
            return Some(p);
        }
        tracing::warn!(static_dir = %dir, "STATIC_DIR set but directory does not exist");
        return None;
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            if let Some(exe_dir) = canonical.parent() {
                let bin_dist = exe_dir.join("dist");
                if bin_dist.exists() {
                    return Some(bin_dist);
                }
            }
        }
    }

    let monorepo_dist = PathBuf::from("apps/web/dist");
    if monorepo_dist.exists() {
        return Some(monorepo_dist);
    }

    let dist = PathBuf::from("dist");
    dist.exists().then_some(dist)
}
