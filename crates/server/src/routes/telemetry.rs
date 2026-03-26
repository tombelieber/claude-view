use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use claude_view_core::telemetry_config::{
    read_telemetry_config, write_telemetry_config, TelemetryStatus,
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
    request_body = serde_json::Value,
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

    write_telemetry_config(config_path, &config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let status = if req.enabled {
        TelemetryStatus::Enabled
    } else {
        TelemetryStatus::Disabled
    };
    Ok(Json(ConsentResponse { status }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/telemetry/consent", post(set_consent))
}
