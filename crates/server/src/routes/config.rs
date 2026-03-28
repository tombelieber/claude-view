//! Runtime capabilities endpoint.
//!
//! Returns which features are available based on server configuration.
//! Used by the frontend to hide/show auth and sharing UI.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use ts_rs::TS;

use crate::state::AppState;
use claude_view_core::telemetry_config::TelemetryStatus;

#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct ConfigResponse {
    /// Whether Supabase auth is configured (JWKS loaded).
    pub auth: bool,
    /// Whether conversation sharing is configured (Worker + Viewer URLs set).
    pub sharing: bool,
    /// Server version.
    pub version: String,
    /// Current telemetry opt-in status (disabled when no PostHog key compiled in).
    pub telemetry: TelemetryStatus,
    /// PostHog project API key — null when running self-hosted (no compiled key).
    pub posthog_key: Option<String>,
    /// Anonymous device ID for telemetry — null when running self-hosted.
    pub anonymous_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/config",
    tag = "health",
    responses(
        (status = 200, description = "Runtime capabilities", body = ConfigResponse),
    )
)]
pub async fn config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let (telemetry_status, posthog_key, anonymous_id) = match &state.telemetry {
        Some(client) => {
            let config_path = &state.telemetry_config_path;
            let status = claude_view_core::telemetry_config::resolve_telemetry_status(
                Some(&client.api_key),
                config_path,
            );
            let config = claude_view_core::telemetry_config::read_telemetry_config(config_path);
            (
                status,
                Some(client.api_key.clone()),
                Some(config.anonymous_id),
            )
        }
        None => (TelemetryStatus::Disabled, None, None),
    };

    Json(ConfigResponse {
        auth: state.jwks.is_some(),
        sharing: state.share.is_some(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        telemetry: telemetry_status,
        posthog_key,
        anonymous_id,
    })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/config", get(config))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_get(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_config_no_auth_no_sharing() {
        let db = Database::new_in_memory().await.unwrap();
        let app = build_app(db);

        let (status, body) = do_get(app, "/api/config").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["auth"], false);
        assert_eq!(json["sharing"], false);
    }

    #[tokio::test]
    async fn config_includes_telemetry_status() {
        let db = Database::new_in_memory().await.unwrap();
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/config").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.get("telemetry").is_some());
        assert_eq!(json["telemetry"], "disabled");
    }

    #[tokio::test]
    async fn config_returns_null_posthog_key_when_no_key() {
        let db = Database::new_in_memory().await.unwrap();
        let app = build_app(db);
        let (status, body) = do_get(app, "/api/config").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["posthogKey"].is_null());
        assert!(json["anonymousId"].is_null());
    }
}
