use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use claude_view_core::telemetry_config::{
    read_telemetry_config, write_telemetry_config, TelemetryStatus,
};
use claude_view_core::telemetry_events::{
    ActionId, FeatureId, RouteId, EVENT_FEATURE_ACTION, EVENT_FEATURE_OPENED,
    EVENT_FIRST_FEATURE_USED, EVENT_PAGE_VIEWED,
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ConsentRequest {
    enabled: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ConsentResponse {
    status: TelemetryStatus,
}

/// POST /api/telemetry/consent — Set telemetry consent preference.
#[utoipa::path(post, path = "/api/telemetry/consent", tag = "telemetry",
    request_body = ConsentRequest,
    responses(
        (status = 200, description = "Telemetry consent updated", body = crate::routes::telemetry::ConsentResponse),
    )
)]
pub async fn set_consent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConsentRequest>,
) -> Result<Json<ConsentResponse>, StatusCode> {
    let config_path = &state.telemetry_config_path;
    let mut config = read_telemetry_config(config_path);
    config.enabled = Some(req.enabled);

    if let Some(ref client) = state.telemetry {
        client.set_enabled(req.enabled);
    }

    if req.enabled && config.consent_given_at.is_none() {
        config.consent_given_at = Some(chrono::Utc::now().to_rfc3339());
        if let Some(ref client) = state.telemetry {
            client.track(
                "telemetry_consent_given",
                serde_json::json!({
                    "consent_given_at": config.consent_given_at,
                    "$set_once": { "first_consent_at": config.consent_given_at },
                }),
            );
        }
    }

    // One-time `installed` ("acquired") event. Under an opt-in model the
    // first server start is `Undecided`, so `track()` is dropped there
    // (telemetry.rs) — consent is the first moment this machine becomes
    // countable, so it doubles as the install signal. Keyed on the
    // persistent `anonymous_id` (PostHog distinct_id) and deduped by the
    // persisted `install_reported` flag so toggling consent off/on never
    // re-fires it.
    if req.enabled && !config.install_reported {
        config.install_reported = true;
        if let Some(ref client) = state.telemetry {
            client.track(
                "installed",
                serde_json::json!({
                    "install_source": crate::startup::install::detect_install_source(),
                    "version": env!("CARGO_PKG_VERSION"),
                    "platform": std::env::consts::OS,
                    "$set_once": { "installed_at": chrono::Utc::now().to_rfc3339() },
                }),
            );
        }
    }

    write_telemetry_config(config_path, &config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let status = if req.enabled {
        TelemetryStatus::Enabled
    } else {
        TelemetryStatus::Disabled
    };
    Ok(Json(ConsentResponse { status }))
}

/// Web journey event. The closed enums (`RouteId`/`ActionId`/`FeatureId`)
/// ARE the privacy boundary: any free-form string (a path, a prompt, a
/// project name) fails to deserialize, so axum rejects the request with
/// 422 instead of forwarding invented data.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TelemetryEventRequest {
    PageViewed { route: RouteId },
    FeatureAction { action: ActionId },
    FeatureOpened { feature: FeatureId },
}

/// POST /api/telemetry/event — ingress for web journey events.
///
/// Routed through the server (not posthog-js directly) so ad-blockers on
/// the PostHog domain can't blind us and the closed-enum guarantee is
/// enforced server-side. Always 204: a disabled/absent client accepts and
/// drops silently (the browser must not care whether telemetry is on).
#[utoipa::path(post, path = "/api/telemetry/event", tag = "telemetry",
    request_body = TelemetryEventRequest,
    responses(
        (status = 204, description = "Accepted (forwarded only when telemetry is enabled)"),
    )
)]
pub async fn ingest_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TelemetryEventRequest>,
) -> StatusCode {
    let Some(client) = state.telemetry.as_ref() else {
        return StatusCode::NO_CONTENT; // source/dev build — no client
    };
    if !client.is_enabled() {
        return StatusCode::NO_CONTENT; // opted out / CI — accept, drop
    }
    match req {
        TelemetryEventRequest::PageViewed { route } => {
            client.track(EVENT_PAGE_VIEWED, serde_json::json!({ "route": route }));
        }
        TelemetryEventRequest::FeatureAction { action } => {
            client.track(
                EVENT_FEATURE_ACTION,
                serde_json::json!({ "action": action }),
            );
        }
        TelemetryEventRequest::FeatureOpened { feature } => {
            client.track(
                EVENT_FEATURE_OPENED,
                serde_json::json!({ "feature": feature }),
            );
            // Activation: the first feature EVER opened on this install —
            // emitted exactly once, guarded by the persisted field.
            let path = &state.telemetry_config_path;
            let mut cfg = read_telemetry_config(path);
            if cfg.first_feature_used.is_none() {
                if let Some(name) = serde_json::to_value(feature)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                {
                    cfg.first_feature_used = Some(name.clone());
                    let _ = write_telemetry_config(path, &cfg);
                    client.track(
                        EVENT_FIRST_FEATURE_USED,
                        serde_json::json!({ "feature": name }),
                    );
                }
            }
        }
    }
    StatusCode::NO_CONTENT
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/telemetry/consent", post(set_consent))
        .route("/telemetry/event", post(ingest_event))
}
