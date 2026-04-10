//! API key management endpoints.
//!
//! - POST   /api/auth/generate-key  — Generate a new API key (returns raw key once)
//! - DELETE  /api/auth/keys/{id}    — Revoke an API key

use axum::{
    extract::{Path, State},
    routing::{delete, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::api_key;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateKeyResponse {
    pub key: String,
    pub id: String,
    pub prefix: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeKeyResponse {
    pub revoked: bool,
    pub id: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/generate-key", post(generate_key))
        .route("/auth/keys/{id}", delete(revoke_key))
}

async fn generate_key(State(state): State<Arc<AppState>>) -> ApiResult<Json<GenerateKeyResponse>> {
    let (raw_key, entry) = api_key::generate_key();
    let resp = GenerateKeyResponse {
        key: raw_key,
        id: entry.id.clone(),
        prefix: entry.prefix.clone(),
    };
    let mut store = state.api_key_store.write().await;
    store.keys.push(entry);
    api_key::save_store(&store, &state.api_key_store_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save key store: {e}")))?;
    Ok(Json(resp))
}

async fn revoke_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<RevokeKeyResponse>> {
    let mut store = state.api_key_store.write().await;
    let before = store.keys.len();
    store.keys.retain(|k| k.id != id);
    if store.keys.len() == before {
        return Err(ApiError::NotFound(format!("API key not found: {id}")));
    }
    api_key::save_store(&store, &state.api_key_store_path)
        .map_err(|e| ApiError::Internal(format!("Failed to save key store: {e}")))?;
    Ok(Json(RevokeKeyResponse { revoked: true, id }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        Router,
    };
    use claude_view_db::Database;
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn build_test_app(tmp: &TempDir) -> (Router, Arc<AppState>) {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let mut state = AppState::builder(db).build();
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.api_key_store_path = tmp.path().join("api-keys.json");
        }
        let app = Router::new()
            .nest("/api", router())
            .with_state(state.clone());
        (app, state)
    }

    async fn do_request(app: Router, method: Method, uri: &str) -> (StatusCode, String) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_generate_key_returns_raw_key() {
        let tmp = TempDir::new().unwrap();
        let (app, _state) = build_test_app(&tmp).await;
        let (status, body) = do_request(app, Method::POST, "/api/auth/generate-key").await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let key = json["key"].as_str().expect("key field");
        assert!(
            key.starts_with("cv_live_"),
            "key must start with cv_live_, got: {key}"
        );
        assert!(!json["id"].as_str().unwrap_or("").is_empty());
        assert!(!json["prefix"].as_str().unwrap_or("").is_empty());
    }

    #[tokio::test]
    async fn test_generate_key_persists_to_disk() {
        let tmp = TempDir::new().unwrap();
        let key_path = tmp.path().join("api-keys.json");
        let (app, _state) = build_test_app(&tmp).await;
        let (status, _body) = do_request(app, Method::POST, "/api/auth/generate-key").await;
        assert_eq!(status, StatusCode::OK);
        assert!(key_path.exists(), "api-keys.json should be written to disk");
        let content = std::fs::read_to_string(&key_path).unwrap();
        let store: api_key::ApiKeyStore = serde_json::from_str(&content).unwrap();
        assert_eq!(store.keys.len(), 1, "store should have one key");
    }

    #[tokio::test]
    async fn test_revoke_key() {
        let tmp = TempDir::new().unwrap();
        let (app_gen, state) = build_test_app(&tmp).await;

        // Generate a key using the first app instance
        let (status, body) = do_request(app_gen, Method::POST, "/api/auth/generate-key").await;
        assert_eq!(status, StatusCode::OK, "generate failed: {body}");
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let id = json["id"].as_str().expect("id field").to_string();

        // Build a second app instance from the same state, revoke the key
        let app_del = Router::new()
            .nest("/api", router())
            .with_state(state.clone());
        let (status, body) =
            do_request(app_del, Method::DELETE, &format!("/api/auth/keys/{id}")).await;
        assert_eq!(status, StatusCode::OK, "revoke failed: {body}");
        let revoke_json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(revoke_json["revoked"], true);
        assert_eq!(revoke_json["id"].as_str().unwrap(), id);

        // Verify the store is now empty
        let store = state.api_key_store.read().await;
        assert!(store.keys.is_empty(), "store should be empty after revoke");
    }

    #[tokio::test]
    async fn test_revoke_nonexistent_key_returns_404() {
        let tmp = TempDir::new().unwrap();
        let (app, _state) = build_test_app(&tmp).await;
        let (status, body) = do_request(app, Method::DELETE, "/api/auth/keys/nonexistent-id").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "expected 404, body: {body}");
    }
}
