//! CRUD handlers for the webhooks API.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
    webhook_engine::config::{
        generate_signing_secret, generate_webhook_id, load_config, load_secrets, save_config,
        save_secrets,
    },
};

use super::types::{
    CreateWebhookRequest, CreateWebhookResponse, DeleteWebhookResponse, UpdateWebhookRequest,
    WebhookListResponse,
};

// ============================================================================
// Validation helpers
// ============================================================================

fn validate_create_request(req: &CreateWebhookRequest) -> Result<(), ApiError> {
    if !req.url.starts_with("https://") {
        return Err(ApiError::BadRequest(
            "URL must start with https://".to_string(),
        ));
    }
    if req.name.is_empty() || req.name.len() > 64 {
        return Err(ApiError::BadRequest(
            "Name must be 1-64 characters".to_string(),
        ));
    }
    if req.events.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one event type required".to_string(),
        ));
    }
    Ok(())
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/webhooks — list all webhooks (secrets excluded).
pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<WebhookListResponse>> {
    let config = load_config(&state.webhook_config_path);
    Ok(Json(WebhookListResponse {
        webhooks: config.webhooks,
    }))
}

/// POST /api/webhooks — create a new webhook (returns signing secret once).
pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateWebhookRequest>,
) -> ApiResult<Json<CreateWebhookResponse>> {
    validate_create_request(&req)?;

    let id = generate_webhook_id();
    let signing_secret = generate_signing_secret();
    let created_at = chrono::Utc::now().to_rfc3339();

    let webhook = crate::webhook_engine::config::WebhookConfig {
        id: id.clone(),
        name: req.name,
        url: req.url,
        format: req.format,
        events: req.events,
        enabled: true,
        created_at,
    };

    // Persist to notifications config
    let mut config = load_config(&state.webhook_config_path);
    config.webhooks.push(webhook.clone());
    save_config(&config, &state.webhook_config_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save webhook config: {e}")))?;

    // Persist signing secret
    let mut secrets = load_secrets(&state.webhook_secrets_path);
    secrets.secrets.insert(id.clone(), signing_secret.clone());
    save_secrets(&secrets, &state.webhook_secrets_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save webhook secrets: {e}")))?;

    tracing::info!(id = %id, "Webhook created");

    Ok(Json(CreateWebhookResponse {
        webhook,
        signing_secret,
    }))
}

/// GET /api/webhooks/{id} — get a single webhook by ID.
pub async fn get_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<crate::webhook_engine::config::WebhookConfig>> {
    let config = load_config(&state.webhook_config_path);
    let webhook = config
        .webhooks
        .into_iter()
        .find(|w| w.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Webhook not found: {id}")))?;
    Ok(Json(webhook))
}

/// PUT /api/webhooks/{id} — update an existing webhook (partial update).
pub async fn update_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateWebhookRequest>,
) -> ApiResult<Json<crate::webhook_engine::config::WebhookConfig>> {
    let mut config = load_config(&state.webhook_config_path);
    let webhook = config
        .webhooks
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Webhook not found: {id}")))?;

    if let Some(name) = req.name {
        webhook.name = name;
    }
    if let Some(url) = req.url {
        webhook.url = url;
    }
    if let Some(format) = req.format {
        webhook.format = format;
    }
    if let Some(events) = req.events {
        webhook.events = events;
    }
    if let Some(enabled) = req.enabled {
        webhook.enabled = enabled;
    }

    let updated = webhook.clone();

    save_config(&config, &state.webhook_config_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save webhook config: {e}")))?;

    tracing::info!(id = %id, "Webhook updated");

    Ok(Json(updated))
}

/// DELETE /api/webhooks/{id} — remove a webhook from config and secrets.
pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteWebhookResponse>> {
    let mut config = load_config(&state.webhook_config_path);
    let before = config.webhooks.len();
    config.webhooks.retain(|w| w.id != id);
    if config.webhooks.len() == before {
        return Err(ApiError::NotFound(format!("Webhook not found: {id}")));
    }

    save_config(&config, &state.webhook_config_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save webhook config: {e}")))?;

    // Remove the signing secret as well
    let mut secrets = load_secrets(&state.webhook_secrets_path);
    secrets.secrets.remove(&id);
    save_secrets(&secrets, &state.webhook_secrets_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save webhook secrets: {e}")))?;

    tracing::info!(id = %id, "Webhook deleted");

    Ok(Json(DeleteWebhookResponse { deleted: true, id }))
}

/// POST /api/webhooks/{id}/test — send a synthetic test payload.
pub async fn test_send(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<super::types::TestSendResponse>> {
    let config = load_config(&state.webhook_config_path);
    let webhook = config
        .webhooks
        .iter()
        .find(|w| w.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Webhook not found: {id}")))?
        .clone();

    let secrets = load_secrets(&state.webhook_secrets_path);
    let secret = secrets
        .secrets
        .get(&id)
        .ok_or_else(|| ApiError::Internal("Signing secret not found".into()))?
        .clone();

    // Build a synthetic session.started test payload.
    let test_session = claude_view_server_live_state::core::test_live_session("test-send");
    let payload = crate::webhook_engine::formatters::build_payload(
        &crate::webhook_engine::config::WebhookEventType::SessionStarted,
        &test_session,
        config.base_url.as_deref(),
    );
    let formatted = crate::webhook_engine::formatters::format_payload(&payload, &webhook.format);
    let body = serde_json::to_string(&formatted).unwrap_or_default();

    let client = reqwest::Client::new();
    let result = crate::webhook_engine::delivery::deliver(
        &client,
        &webhook.url,
        &payload.id,
        body,
        &secret,
        1,
    )
    .await;

    Ok(Json(super::types::TestSendResponse {
        success: result.success,
        status_code: result.status_code,
        response_body: None,
        error: result.error,
    }))
}
