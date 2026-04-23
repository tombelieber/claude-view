//! Platform gate — macOS and Linux only, with `CLAUDE_VIEW_SKIP_PLATFORM_CHECK`
//! as an escape hatch.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Behaviour and env-var contract
//! are unchanged.

/// Returns `true` if the given platform is supported or the check is bypassed.
pub fn is_platform_supported(os: &str, skip_check: bool) -> bool {
    os == "macos" || os == "linux" || skip_check
}

/// Check the current platform. If unsupported and the escape hatch is not set,
/// print a helpful message and exit with status 1.
pub fn ensure_supported() {
    let skip = std::env::var("CLAUDE_VIEW_SKIP_PLATFORM_CHECK").as_deref() == Ok("1");
    if !is_platform_supported(std::env::consts::OS, skip) {
        eprintln!(
            "\n\u{26a0}\u{fe0f}  claude-view supports macOS and Linux. \
             Your platform ({}) is not officially supported.",
            std::env::consts::OS
        );
        eprintln!("   Set CLAUDE_VIEW_SKIP_PLATFORM_CHECK=1 to try anyway.");
        eprintln!("   Report issues: https://github.com/tombelieber/claude-view/issues\n");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_macos_allowed() {
        assert!(is_platform_supported("macos", false));
    }

    #[test]
    fn platform_linux_allowed() {
        assert!(is_platform_supported("linux", false));
    }

    #[test]
    fn platform_windows_blocked() {
        assert!(!is_platform_supported("windows", false));
    }

    #[test]
    fn platform_unknown_blocked() {
        assert!(!is_platform_supported("freebsd", false));
    }

    #[test]
    fn platform_windows_allowed_with_skip() {
        assert!(is_platform_supported("windows", true));
    }

    #[test]
    fn platform_unknown_allowed_with_skip() {
        assert!(is_platform_supported("freebsd", true));
    }
}
