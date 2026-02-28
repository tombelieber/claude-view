use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::{
    auth::supabase::{extract_bearer, validate_jwt_with_rotation, AuthUser},
    error::{ApiError, ApiResult},
    share_serializer::{key_to_base64url, serialize_and_encrypt},
    state::AppState,
};

fn extract_raw_jwt(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

#[derive(Serialize)]
pub struct ShareResponse {
    pub token: String,
    pub url: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{session_id}/share", post(create_share))
        .route("/sessions/{session_id}/share", delete(revoke_share))
        .route("/shares", get(list_shares))
}

async fn require_auth(headers: &HeaderMap, state: &AppState) -> ApiResult<AuthUser> {
    let jwks_lock = state
        .jwks
        .as_ref()
        .ok_or_else(|| ApiError::Unauthorized("Auth not configured".into()))?;

    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let token = extract_bearer(auth_header)
        .ok_or_else(|| ApiError::Unauthorized("Expected Bearer token".into()))?;

    let jwks = jwks_lock.read().await;
    match validate_jwt_with_rotation(token, &jwks).await {
        Ok((user, None)) => Ok(user),
        Ok((user, Some(new_cache))) => {
            drop(jwks);
            let mut jwks_write = jwks_lock.write().await;
            *jwks_write = new_cache;
            tracing::info!("JWKS cache updated after key rotation");
            Ok(user)
        }
        Err(e) => Err(ApiError::Unauthorized(format!("Invalid token: {e}"))),
    }
}

pub async fn create_share(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<ShareResponse>> {
    let _user = require_auth(&headers, &state).await?;
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;

    let share_cfg = state
        .share
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    let file_path = state
        .db
        .get_session_file_path(&session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;

    let session = state
        .db
        .get_session_by_id(&session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;
    let title = session
        .summary
        .clone()
        .unwrap_or_else(|| session.preview.chars().take(80).collect::<String>());

    let path = std::path::PathBuf::from(&file_path);
    let encrypted = serialize_and_encrypt(&path).await?;
    let size_bytes = encrypted.blob.len();

    let token_resp: serde_json::Value = share_cfg
        .http_client
        .post(format!("{}/api/share", share_cfg.worker_url))
        .bearer_auth(&raw_jwt)
        .json(&serde_json::json!({
            "session_id": session_id,
            "title": title,
            "size_bytes": size_bytes,
        }))
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Worker POST failed: {e}")))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Worker response: {e}")))?;

    let token = token_resp["token"]
        .as_str()
        .ok_or_else(|| ApiError::Internal("Missing token in Worker response".into()))?
        .to_string();

    let blob_resp = share_cfg
        .http_client
        .put(format!("{}/api/share/{}/blob", share_cfg.worker_url, token))
        .body(encrypted.blob)
        .header("Content-Type", "application/octet-stream")
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Blob upload failed: {e}")))?;
    if !blob_resp.status().is_success() {
        return Err(ApiError::Internal(format!(
            "Blob upload returned {}",
            blob_resp.status()
        )));
    }

    let key_b64 = key_to_base64url(&encrypted.key);
    let url = format!("{}/s/{}#k={}", share_cfg.viewer_url, token, key_b64);

    Ok(Json(ShareResponse { token, url }))
}

pub async fn revoke_share(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let _user = require_auth(&headers, &state).await?;
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;
    let share_cfg = state
        .share
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    let resp = share_cfg
        .http_client
        .delete(format!(
            "{}/api/shares/by-session/{}",
            share_cfg.worker_url, session_id
        ))
        .bearer_auth(&raw_jwt)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Worker DELETE failed: {e}")))?;

    if !resp.status().is_success() && resp.status().as_u16() != 404 {
        return Err(ApiError::Internal(format!(
            "Worker returned {}",
            resp.status()
        )));
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn list_shares(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let _user = require_auth(&headers, &state).await?;
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;
    let share_cfg = state
        .share
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    let resp: serde_json::Value = share_cfg
        .http_client
        .get(format!("{}/api/shares", share_cfg.worker_url))
        .bearer_auth(&raw_jwt)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Worker GET failed: {e}")))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Worker response: {e}")))?;

    Ok(Json(resp))
}
