//! Request/response types for live monitoring endpoints.

use serde::Deserialize;

/// Query parameters for the messages endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MessagesQuery {
    /// Maximum number of messages to return (default: 20).
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Request body for binding sidecar control to a session.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BindControlRequest {
    pub control_id: String,
}

/// Request body for unbinding sidecar control from a session.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnbindControlRequest {
    pub control_id: String,
}
