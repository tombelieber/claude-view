//! Install-source detection + install-beacon ping.
//!
//! Extracted from `main.rs` in CQRS Phase 7.c. The beacon is
//! fire-and-forget (3 s timeout, never blocks startup); the detection
//! is a zero-I/O best-effort classification of how the binary was
//! launched.

use std::time::Duration;

/// Detect how the server was installed/launched.
///
/// Returns one of: "plugin", "install_sh", "npx".
pub fn detect_install_source() -> &'static str {
    // Plugin sets CLAUDE_PLUGIN_ROOT when launching via hooks
    if std::env::var("CLAUDE_PLUGIN_ROOT").is_ok() {
        return "plugin";
    }
    // install.sh puts the binary under ~/.cache/claude-view/
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy();
        if exe_str.contains(".cache/claude-view") {
            return "install_sh";
        }
    }
    "npx"
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
