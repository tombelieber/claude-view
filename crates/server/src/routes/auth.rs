//! POST /api/auth/session — web UI pushes Supabase session to the Mac.
//! DELETE /api/auth/session — sign-out.
//! GET /api/auth/status — is the daemon authenticated?
//!
//! After persisting a new session via POST, we `tokio::spawn` a non-blocking
//! call to Supabase's `devices-register-self` edge function so the Mac's row
//! in `public.devices` is idempotently upserted. Failures are logged and
//! surface as DEVICE_NOT_OWNED on the next relay connect (user retries).

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::auth::{AuthSession, SessionStore};
use crate::state::AppState;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PostSessionBody {
    /// Supabase JWT (short-lived access token).
    #[serde(default)]
    pub access_token: String,
    /// Supabase opaque refresh token (long-lived).
    #[serde(default)]
    pub refresh_token: String,
    /// Seconds until `access_token` expires (Supabase returns `expires_in`).
    #[serde(default)]
    pub expires_in: u64,
    /// UUID from the JWT's `sub` claim. Web UI reads this from
    /// supabase.auth.getUser() and forwards it so we don't double-parse.
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PostSessionResponse {
    pub user_id: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthStatusResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<u64>,
}

#[utoipa::path(post, path = "/api/auth/session", tag = "auth",
    request_body = PostSessionBody,
    responses(
        (status = 200, description = "Session stored", body = PostSessionResponse),
        (status = 400, description = "Malformed body"),
        (status = 500, description = "Disk write error"),
    )
)]
pub async fn post_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PostSessionBody>,
) -> Result<Json<PostSessionResponse>, StatusCode> {
    if body.access_token.is_empty() || body.refresh_token.is_empty() || body.user_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let session = AuthSession {
        user_id: body.user_id.clone(),
        email: body.email.clone(),
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        expires_at_unix: now + body.expires_in,
    };

    let store = SessionStore::new();
    if let Err(e) = store.save(&session).await {
        tracing::error!("failed to persist auth session: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    {
        let mut guard = state.auth_session.write().await;
        *guard = Some(session.clone());
    }

    // Bootstrap the Mac's row in public.devices via devices-register-self
    // (2026-04-17). Non-blocking for sign-in success — a failed bootstrap
    // surfaces as DEVICE_NOT_OWNED on the next relay connect; retrying
    // sign-in fixes it. Supabase URL/key optional for tests.
    let supabase_url = std::env::var("SUPABASE_URL")
        .ok()
        .or_else(|| option_env!("SUPABASE_URL").map(str::to_string));
    if let Some(url) = supabase_url {
        let access = session.access_token.clone();
        tokio::spawn(async move {
            let identity = match crate::crypto::load_or_create_identity() {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("devices-register-self skipped: identity load failed: {e}");
                    return;
                }
            };
            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("build http client");
            if let Err(e) =
                crate::supabase_proxy::bootstrap_device_row(&http, &url, &access, &identity).await
            {
                tracing::warn!("devices-register-self failed (will retry on next sign-in): {e}");
            }
        });
    }

    Ok(Json(PostSessionResponse {
        user_id: session.user_id,
        email: session.email,
    }))
}

#[utoipa::path(delete, path = "/api/auth/session", tag = "auth",
    responses(
        (status = 204, description = "Session cleared"),
        (status = 500, description = "Disk error"),
    )
)]
pub async fn delete_session(State(state): State<Arc<AppState>>) -> StatusCode {
    {
        let mut guard = state.auth_session.write().await;
        *guard = None;
    }
    let store = SessionStore::new();
    if let Err(e) = store.clear().await {
        tracing::error!("failed to clear auth session: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::NO_CONTENT
}

#[utoipa::path(get, path = "/api/auth/status", tag = "auth",
    responses((status = 200, description = "Current auth state", body = AuthStatusResponse))
)]
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<AuthStatusResponse> {
    let snapshot = {
        let guard = state.auth_session.read().await;
        guard.clone()
    };
    match snapshot {
        Some(s) => Json(AuthStatusResponse {
            authenticated: true,
            user_id: Some(s.user_id),
            email: s.email,
            expires_at_unix: Some(s.expires_at_unix),
        }),
        None => Json(AuthStatusResponse {
            authenticated: false,
            user_id: None,
            email: None,
            expires_at_unix: None,
        }),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/session", post(post_session))
        .route("/auth/session", delete(delete_session))
        .route("/auth/status", get(get_status))
}
