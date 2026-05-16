//! Closed telemetry event taxonomy — the privacy guarantee, in the type system.
//!
//! Every analytics payload carries **only** a variant of these enums plus
//! the existing anonymous id, app version, platform and coarse counts.
//! Because the enums are closed and strict on the wire, a file path,
//! prompt, project name or any free-form user string is *structurally
//! unrepresentable* in an event — there is no variant for it and an
//! arbitrary string fails to deserialize. This is what keeps the public
//! promise ("no code, prompts, paths or session content — ever") literally
//! true regardless of future call sites.
//!
//! Single responsibility: this module is the taxonomy and nothing else —
//! no I/O, no transport, no consent logic.

use serde::{Deserialize, Serialize};

/// Stable PostHog event names. Centralised so the Rust emitters, the
/// `/api/telemetry/event` validator and the dashboards never drift.
pub const EVENT_SERVER_STARTED: &str = "server_started";
pub const EVENT_APP_ACTIVE: &str = "app_active";
pub const EVENT_FIRST_FEATURE_USED: &str = "first_feature_used";
pub const EVENT_FEATURE_OPENED: &str = "feature_opened";
pub const EVENT_SCALE_MILESTONE: &str = "scale_milestone";
pub const EVENT_FEATURE_ACTION: &str = "feature_action";

/// A navigable product surface — one variant per real top-level route in
/// the web app. `feature_opened { surface }` answers BOTH "which features
/// get used" and (via PostHog Paths over the ordered event stream) "what's
/// the user journey" — no separate page-view event needed.
///
/// Closed by design: a route the enum doesn't know about emits nothing
/// rather than leaking a raw path. Keep in lock-step with `router.tsx`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    LiveMonitor,
    Chat,
    History,
    SessionDetail,
    Search,
    Analytics,
    Activity,
    Reports,
    Prompts,
    Teams,
    Workflows,
    Plugins,
    Memory,
    SystemMonitor,
    Insights,
    Settings,
}

/// A high-intent action for `feature_action` — the depth /
/// willingness-to-pay signal that distinguishes "opened a surface" from
/// "actually did the valuable thing". Counts only; the variant itself
/// carries no user content.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ActionId {
    ChatMessageSent,
    SearchRun,
    AnalyticsDashboardOpened,
    SessionOpenedInIde,
    ShareLinkCreated,
    WorkflowRun,
    PlanViewed,
    OnDeviceAiUsed,
    SettingsOpened,
}
