//! Install-source detection + install-beacon ping.
//!
//! Extracted from `main.rs` in CQRS Phase 7.c. The beacon is
//! fire-and-forget (3 s timeout, never blocks startup); the detection
//! is a zero-I/O best-effort classification of how the binary was
//! launched.

use std::time::Duration;

/// Pure install-source classifier — no env, no I/O. The complete mapping;
/// [`detect_install_source`] is the impure shell that supplies the real
/// exe path + `CLAUDE_PLUGIN_ROOT`.
///
/// Returns one of: "plugin", "install_sh", "npx".
///
/// Root-cause fix (was swapped): **npx** caches the binary under
/// `~/.cache/claude-view/`, while **install.sh** installs it under
/// `~/.claude-view/bin/`. The npx `.cache` path is checked first so it
/// can never be misread as install.sh.
fn classify_install_source(exe_path: &str, has_plugin_root: bool) -> &'static str {
    if has_plugin_root {
        return "plugin";
    }
    if exe_path.contains("/.cache/claude-view") {
        return "npx";
    }
    if exe_path.contains("/.claude-view/bin") {
        return "install_sh";
    }
    "npx"
}

/// Detect how the server was installed/launched.
///
/// Returns one of: "plugin", "install_sh", "npx".
pub fn detect_install_source() -> &'static str {
    let has_plugin_root = std::env::var("CLAUDE_PLUGIN_ROOT").is_ok();
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    classify_install_source(&exe, has_plugin_root)
}

#[cfg(test)]
mod tests {
    use super::classify_install_source;

    #[test]
    fn npx_cache_path_is_npx_not_install_sh() {
        // The original bug: npx caches under ~/.cache/claude-view and was
        // mislabeled "install_sh".
        assert_eq!(
            classify_install_source("/Users/u/.cache/claude-view/bin/claude-view", false),
            "npx"
        );
    }

    #[test]
    fn install_sh_bin_path_is_install_sh() {
        // install.sh installs under ~/.claude-view/bin — was mislabeled "npx".
        assert_eq!(
            classify_install_source("/Users/u/.claude-view/bin/claude-view", false),
            "install_sh"
        );
    }

    #[test]
    fn plugin_root_wins_over_any_path() {
        assert_eq!(
            classify_install_source("/Users/u/.cache/claude-view/bin/claude-view", true),
            "plugin"
        );
        assert_eq!(
            classify_install_source("/anywhere/claude-view", true),
            "plugin"
        );
    }

    #[test]
    fn unknown_path_defaults_to_npx() {
        assert_eq!(
            classify_install_source("/Users/u/dev/claude-view/target/debug/claude-view", false),
            "npx"
        );
    }
}

/// Fire-and-forget ping to CF Worker for unified install tracking.
pub fn ping_install_beacon(source: &str) {
    let url = format!(
        "https://get.claudeview.ai/ping?source={}&v={}",
        source,
        env!("CARGO_PKG_VERSION"),
    );
    tokio::spawn(async move {
        let _ = reqwest::Client::new()
            .get(&url)
            .timeout(Duration::from_secs(3))
            .send()
            .await;
    });
}
