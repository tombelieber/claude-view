//! Startup telemetry decisions + the one-time privacy notice.
//!
//! Pure policy (`plan_startup_telemetry`) decides what fires; the impure
//! `server_bind::fire_startup_events` orchestrates I/O. The notice is shown
//! exactly once, only when telemetry is on *by default* (the user has made
//! no explicit choice) — an explicit opt-in needs no disclosure and an
//! opt-out / source build / CI never reaches `enabled == true`.

/// What startup should do, derived purely from telemetry state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupTelemetryPlan {
    /// First time this install is counted → emit the one-shot `installed`
    /// acquisition event and set the persistent guard.
    pub fire_installed: bool,
    /// First launch this UTC day → emit `app_active` (the DAU heartbeat).
    pub fire_app_active: bool,
    /// Telemetry is on by default and the one-time notice has not been
    /// shown → print it once, then stamp `notice_shown_at`.
    pub show_notice: bool,
}

/// Pure startup-telemetry policy. No I/O, no clock, no env — `today` and
/// the persisted state are injected so every branch is unit-tested.
pub fn plan_startup_telemetry(
    enabled: bool,
    consent: Option<bool>,
    install_reported: bool,
    last_active_date: Option<&str>,
    notice_shown_at: Option<&str>,
    today: &str,
) -> StartupTelemetryPlan {
    if !enabled {
        return StartupTelemetryPlan {
            fire_installed: false,
            fire_app_active: false,
            show_notice: false,
        };
    }
    StartupTelemetryPlan {
        fire_installed: !install_reported,
        fire_app_active: last_active_date != Some(today),
        // Disclose only when telemetry is on *by default* (no explicit
        // choice) and the notice has never been shown.
        show_notice: consent.is_none() && notice_shown_at.is_none(),
    }
}

/// The one-time terminal notice. Honest framing for default-on: states
/// what is collected, what is not, and the one-command opt-out. Styled to
/// match the existing startup output (2-space indent, Unicode, stderr).
pub fn notice_text() -> &'static str {
    "  \u{2139} Anonymous usage analytics are on to help improve claude-view.\n    \
     No code, prompts, file paths, or session content \u{2014} ever. Only feature counts.\n    \
     Opt out anytime:  CLAUDE_VIEW_TELEMETRY=0  \u{00b7}  details: https://claudeview.ai/privacy"
}

/// Print the one-time notice to stderr (mirrors the banner's spacing).
pub fn print_privacy_notice() {
    eprintln!("\n{}\n", notice_text());
}

#[cfg(test)]
mod tests {
    use super::{notice_text, plan_startup_telemetry};

    fn plan(
        enabled: bool,
        consent: Option<bool>,
        install_reported: bool,
        last_active: Option<&str>,
        notice_shown_at: Option<&str>,
    ) -> super::StartupTelemetryPlan {
        plan_startup_telemetry(
            enabled,
            consent,
            install_reported,
            last_active,
            notice_shown_at,
            "2026-05-16",
        )
    }

    #[test]
    fn telemetry_off_fires_nothing_and_shows_no_notice() {
        let p = plan(false, None, false, None, None);
        assert!(!p.fire_installed);
        assert!(!p.fire_app_active);
        assert!(!p.show_notice, "no notice when telemetry resolved off");
    }

    #[test]
    fn first_counted_run_fires_installed_once() {
        assert!(plan(true, None, false, None, None).fire_installed);
        assert!(
            !plan(true, None, true, None, None).fire_installed,
            "install_reported guard prevents re-fire"
        );
    }

    #[test]
    fn app_active_fires_once_per_utc_day() {
        assert!(
            plan(true, None, true, None, None).fire_app_active,
            "never active before"
        );
        assert!(
            plan(true, None, true, Some("2026-05-15"), None).fire_app_active,
            "new day → fire"
        );
        assert!(
            !plan(true, None, true, Some("2026-05-16"), None).fire_app_active,
            "same day → skip"
        );
    }

    #[test]
    fn notice_shown_once_only_when_defaulted_on() {
        assert!(
            plan(true, None, true, Some("2026-05-16"), None).show_notice,
            "default-on (consent None), not yet shown → show"
        );
        assert!(
            !plan(true, Some(true), true, Some("2026-05-16"), None).show_notice,
            "explicit opt-in needs no disclosure"
        );
        assert!(
            !plan(
                true,
                None,
                true,
                Some("2026-05-16"),
                Some("2026-05-16T00:00:00Z")
            )
            .show_notice,
            "already shown → never again"
        );
    }

    #[test]
    fn notice_text_is_honest_and_actionable() {
        let t = notice_text();
        assert!(
            t.contains("CLAUDE_VIEW_TELEMETRY=0"),
            "must show the opt-out"
        );
        assert!(
            t.contains("claudeview.ai/privacy"),
            "must link the published-collection page"
        );
        assert!(
            t.to_lowercase().contains("no code") || t.to_lowercase().contains("session content"),
            "must state what is NOT collected"
        );
        // Must not perpetuate the now-false 'off by default / opt-in' claim.
        assert!(!t.to_lowercase().contains("opt-in"));
        assert!(!t.to_lowercase().contains("off by default"));
    }
}
