//! /api/devices — list + revoke + terminate-others.
//!
//! Thin proxies to Supabase. Every handler pulls the current AuthSession
//! snapshot (clone-and-drop), injects it into the supabase_proxy call,
//! and maps the typed proxy error to an HTTP status.
//!
//! See design spec §5.2.5.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;

use crate::auth::AuthSession;
use crate::state::AppState;
use crate::supabase_proxy::{
    list_devices, revoke_device, terminate_others, DeviceRow, SupabaseProxyError,
};

#[derive(Serialize, utoipa::ToSchema)]
pub struct TerminateOthersResponse {
    pub revoked_count: u32,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DeviceListItem {
    pub device_id: String,
    pub user_id: String,
    pub platform: String,
    pub display_name: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
    pub revoked_reason: Option<String>,
}

impl From<DeviceRow> for DeviceListItem {
    fn from(r: DeviceRow) -> Self {
        Self {
            device_id: r.device_id,
            user_id: r.user_id,
            platform: r.platform,
            display_name: r.display_name,
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            revoked_at: r.revoked_at,
            revoked_reason: r.revoked_reason,
        }
    }
}

async fn require_session(state: &AppState) -> Result<AuthSession, StatusCode> {
    let snapshot = {
        let guard = state.auth_session.read().await;
        guard.clone()
    };
    snapshot.ok_or(StatusCode::UNAUTHORIZED)
}

fn supabase_env() -> Result<(String, String), StatusCode> {
    let url = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let publishable = std::env::var("SUPABASE_PUBLISHABLE_KEY")
        .ok()
        .or_else(|| std::env::var("SUPABASE_ANON_KEY").ok())
        .or_else(|| option_env!("SUPABASE_PUBLISHABLE_KEY").map(str::to_string))
        .or_else(|| option_env!("SUPABASE_ANON_KEY").map(str::to_string))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((url, publishable))
}

fn map_err(err: SupabaseProxyError) -> StatusCode {
    match err {
        SupabaseProxyError::Unauthorized => StatusCode::UNAUTHORIZED,
        SupabaseProxyError::Forbidden => StatusCode::FORBIDDEN,
        SupabaseProxyError::Business { .. } => StatusCode::BAD_REQUEST,
        SupabaseProxyError::MissingConfig => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_GATEWAY,
    }
}

#[utoipa::path(get, path = "/api/devices", tag = "devices",
    responses(
        (status = 200, description = "List of devices", body = [DeviceListItem]),
        (status = 401, description = "Not signed in"),
    )
)]
pub async fn list_devices_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DeviceListItem>>, StatusCode> {
    let session = require_session(&state).await?;
    let (url, publishable) = supabase_env()?;
    let http = reqwest::Client::new();
    let rows = list_devices(&http, &url, &publishable, &session.access_token)
        .await
        .map_err(map_err)?;
    Ok(Json(rows.into_iter().map(DeviceListItem::from).collect()))
}

#[utoipa::path(delete, path = "/api/devices/{device_id}", tag = "devices",
    params(("device_id" = String, Path, description = "Device ID")),
    responses((status = 200, description = "Device revoked"))
)]
pub async fn delete_device_handler(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> Result<Json<DeviceListItem>, StatusCode> {
    let session = require_session(&state).await?;
    let (url, publishable) = supabase_env()?;
    let http = reqwest::Client::new();
    let row = revoke_device(
        &http,
        &url,
        &publishable,
        &session.access_token,
        &device_id,
        "user_action",
    )
    .await
    .map_err(map_err)?;
    Ok(Json(DeviceListItem::from(row)))
}

#[utoipa::path(post, path = "/api/devices/terminate-others", tag = "devices",
    responses((status = 200, description = "Others revoked", body = TerminateOthersResponse))
)]
pub async fn terminate_others_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TerminateOthersResponse>, StatusCode> {
    let session = require_session(&state).await?;
    let (url, publishable) = supabase_env()?;
    let http = reqwest::Client::new();
    // We need the Mac's own device_id. Read from the cached DeviceIdentity.
    let identity =
        crate::crypto::load_or_create_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let count = terminate_others(
        &http,
        &url,
        &publishable,
        &session.access_token,
        &identity.device_id,
    )
    .await
    .map_err(map_err)?;
    Ok(Json(TerminateOthersResponse {
        revoked_count: count,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/devices", get(list_devices_handler))
        .route("/devices/{device_id}", delete(delete_device_handler))
        .route("/devices/terminate-others", post(terminate_others_handler))
}
