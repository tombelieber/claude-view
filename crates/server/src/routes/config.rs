//! Runtime capabilities endpoint.
//!
//! Returns which features are available based on server configuration.
//! Used by the frontend to hide/show auth and sharing UI.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use ts_rs::TS;

use crate::state::AppState;

#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct ConfigResponse {
    /// Whether Supabase auth is configured (JWKS loaded).
    pub auth: bool,
    /// Whether conversation sharing is configured (Worker + Viewer URLs set).
    pub sharing: bool,
    /// Server version.
    pub version: String,
}

pub async fn config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        auth: state.jwks.is_some(),
        sharing: state.share.is_some(),
        version: env!("CARGO_PKG_VERSION").to_string(),
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
}
