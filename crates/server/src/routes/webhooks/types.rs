//! Request and response types for the webhooks CRUD API.

use serde::{Deserialize, Serialize};

use crate::webhook_engine::config::{WebhookConfig, WebhookEventType, WebhookFormat};

// ============================================================================
// Request types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub format: WebhookFormat,
    pub events: Vec<WebhookEventType>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub format: Option<WebhookFormat>,
    pub events: Option<Vec<WebhookEventType>>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhookResponse {
    pub webhook: WebhookConfig,
    pub signing_secret: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookListResponse {
    pub webhooks: Vec<WebhookConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWebhookResponse {
    pub deleted: bool,
    pub id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSendResponse {
    pub success: bool,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub error: Option<String>,
}
