//! Auth identity endpoint — wraps `claude auth status --json`.

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Public response shape for `GET /api/oauth/identity`.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthIdentityResponse {
    pub has_auth: bool,
    pub email: Option<String>,
    pub org_name: Option<String>,
    pub subscription_type: Option<String>,
    pub auth_method: Option<String>,
}

/// Parsed output of `claude auth status --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeAuthStatusOutput {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    subscription_type: Option<String>,
    #[serde(default)]
    auth_method: Option<String>,
}

/// Timeout for the `claude auth status` subprocess.
const AUTH_STATUS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Run `claude auth status --json` and parse the result.
/// Returns `None` on any failure (CLI missing, timeout, parse error).
pub fn fetch_auth_identity() -> Option<crate::state::AuthIdentity> {
    let cli_path = claude_view_core::resolved_cli_path()?;

    // Strip CLAUDE* env vars to prevent SIGKILL inside Claude Code sessions.
    let claude_vars: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("CLAUDE"))
        .map(|(k, _)| k)
        .collect();

    let mut cmd = std::process::Command::new(cli_path);
    cmd.args(["auth", "status", "--json"]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    for var in &claude_vars {
        cmd.env_remove(var);
    }

    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    tracing::debug!("claude auth status exited with {}", status);
                    return None;
                }
                break;
            }
            Ok(None) => {
                if start.elapsed() > AUTH_STATUS_TIMEOUT {
                    let _ = child.kill();
                    tracing::warn!(
                        "claude auth status timed out after {:?}",
                        AUTH_STATUS_TIMEOUT
                    );
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                tracing::debug!("claude auth status wait error: {e}");
                return None;
            }
        }
    }

    let output = child.wait_with_output().ok()?;

    let parsed: ClaudeAuthStatusOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                error = %e,
                stdout = %String::from_utf8_lossy(&output.stdout),
                "Failed to parse claude auth status JSON — command may not support --json"
            );
            return None;
        }
    };

    if !parsed.logged_in {
        return None;
    }

    Some(crate::state::AuthIdentity {
        email: parsed.email,
        org_name: parsed.org_name,
        subscription_type: parsed.subscription_type,
        auth_method: parsed.auth_method,
    })
}

/// `GET /api/oauth/identity`
///
/// Returns cached auth identity (email, org, plan).
/// Calls `claude auth status` on first request only, caches forever.
#[utoipa::path(
    get,
    path = "/api/oauth/identity",
    tag = "oauth",
    responses(
        (status = 200, description = "Auth identity (email, org, plan)", body = AuthIdentityResponse),
    )
)]
pub async fn get_auth_identity(State(state): State<Arc<AppState>>) -> Json<AuthIdentityResponse> {
    let identity = state
        .auth_identity
        .get_or_init(|| async {
            // Run subprocess in blocking task to avoid blocking the tokio runtime.
            match tokio::task::spawn_blocking(fetch_auth_identity).await {
                Ok(result) => result,
                Err(e) => {
                    tracing::error!("fetch_auth_identity spawn_blocking failed: {e}");
                    None
                }
            }
        })
        .await;

    match identity {
        Some(id) => Json(AuthIdentityResponse {
            has_auth: true,
            email: id.email.clone(),
            org_name: id.org_name.clone(),
            subscription_type: id.subscription_type.clone(),
            auth_method: id.auth_method.clone(),
        }),
        None => Json(AuthIdentityResponse {
            has_auth: false,
            email: None,
            org_name: None,
            subscription_type: None,
            auth_method: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    #[tokio::test]
    async fn identity_endpoint_returns_cached_identity() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        state
            .auth_identity
            .get_or_init(|| async {
                Some(crate::state::AuthIdentity {
                    email: Some("test@example.com".into()),
                    org_name: Some("Test Corp".into()),
                    subscription_type: Some("max".into()),
                    auth_method: Some("claude.ai".into()),
                })
            })
            .await;

        let app = Router::new()
            .route("/api/oauth/identity", axum::routing::get(get_auth_identity))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oauth/identity")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: AuthIdentityResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(body.has_auth);
        assert_eq!(body.email.as_deref(), Some("test@example.com"));
        assert_eq!(body.org_name.as_deref(), Some("Test Corp"));
    }

    #[tokio::test]
    async fn identity_endpoint_returns_no_auth_when_empty() {
        let db = claude_view_db::Database::new_in_memory().await.unwrap();
        let state = crate::state::AppState::new(db);

        state.auth_identity.get_or_init(|| async { None }).await;

        let app = Router::new()
            .route("/api/oauth/identity", axum::routing::get(get_auth_identity))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oauth/identity")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: AuthIdentityResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!body.has_auth);
        assert!(body.email.is_none());
    }
}
