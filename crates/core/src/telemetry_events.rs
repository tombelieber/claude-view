//! Closed telemetry event taxonomy — the privacy guarantee, in the type system.
//!
//! Every analytics payload carries **only** a variant of these enums plus
//! the existing anonymous id, app version, platform and coarse counts.
//! Because the enums are closed and `#[serde(deny_unknown)]`-strict on the
//! wire, a file path, prompt, project name or any free-form user string is
//! *structurally unrepresentable* in an event — there is no variant for it
//! and an arbitrary string fails to deserialize. This is what keeps the
//! public promise ("no code, prompts, paths or session content — ever")
//! literally true regardless of future call sites.
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
pub const EVENT_PAGE_VIEWED: &str = "page_viewed";
pub const EVENT_FEATURE_ACTION: &str = "feature_action";

/// A product surface. Covers every major feature in the README so the
/// long-tail ("are workflows/teams/on-device-AI worth keeping?") is
/// answerable, not just the headline features.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum FeatureId {
    LiveMonitor,
    Chat,
    Search,
    Analytics,
    AgentInternals,
    Plans,
    Prompts,
    Teams,
    SystemMonitor,
    OnDeviceAi,
    Workflows,
    OpenInIde,
    Share,
    Settings,
}

/// A web-app route for `page_viewed`. A fixed enum — never a URL — so a
/// session id, project name or path can never ride along in the route.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum RouteId {
    Monitor,
    SessionDetail,
    Search,
    Analytics,
    Settings,
    Plans,
    Prompts,
    Teams,
    AgentInternals,
    SystemMonitor,
    Workflows,
    OnDeviceAi,
    Share,
}

/// A high-intent action for `feature_action` — the depth/willingness-to-pay
/// signal. Counts only; the variant itself carries no user content.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ActionId {
    ChatMessageSent,
    SearchRun,
    AnalyticsDashboardOpened,
    MonitorGridOpened,
    SessionOpenedInIde,
    ShareLinkCreated,
    WorkflowRun,
    PlanViewed,
    OnDeviceAiUsed,
    SettingsOpened,
}
