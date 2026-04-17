//! Persisted Supabase auth session for the Mac daemon.
//!
//! Lives at `~/.claude-view/auth-session.json` (MANDATORY — per
//! project_app_data_dir.md, never XDG, never Library/Application Support).
//!
//! Owns ONE thing: serializing / deserializing / wiping the session blob.
//! The refresh loop is a separate module (see session_refresh.rs) — single
//! responsibility per CLAUDE.md Code Design rule.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

/// Supabase auth session stored on disk. Mirrors Supabase-JS's
/// `supabase.auth.getSession()` return shape, trimmed to the fields we use.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuthSession {
    /// UUID from `auth.users.id`. Matches the `sub` claim in the access token.
    #[serde(default)]
    pub user_id: String,

    /// Email (may be None for phone-first accounts; Supabase still accepts).
    #[serde(default)]
    pub email: Option<String>,

    /// Short-lived JWT (~1h lifetime). Presented on every request.
    #[serde(default)]
    pub access_token: String,

    /// Long-lived opaque refresh token. Used to mint new access tokens.
    #[serde(default)]
    pub refresh_token: String,

    /// Unix seconds at which `access_token` expires. NOT a monotonic clock —
    /// treat as advisory; the refresh loop also swallows 401s as a fallback.
    #[serde(default)]
    pub expires_at_unix: u64,
}

impl AuthSession {
    /// Returns true if the access token is within `window` of expiring.
    /// Used by the refresh loop (500s) and by `relay_client` (120s sanity check).
    pub fn is_near_expiry(&self, window: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.expires_at_unix.saturating_sub(now) <= window.as_secs()
    }
}

#[derive(Debug, Error)]
pub enum SessionStoreError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "auth-session.json is corrupt ({path}): {source}. Delete the file to re-authenticate."
    )]
    Corrupt {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Owns read/write/delete of `auth-session.json`. No locking, no cache —
/// callers hold the `Arc<RwLock<Option<AuthSession>>>` in AppState and only
/// touch disk on save/clear/initial load.
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new() -> Self {
        let path = claude_view_core::paths::config_dir().join("auth-session.json");
        Self { path }
    }

    /// Construct a store pointing at an explicit path. Used by tests (and by
    /// any callers that need to inject a non-default location).
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub async fn load(&self) -> Result<Option<AuthSession>, SessionStoreError> {
        let bytes = match fs::read(&self.path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SessionStoreError::Io {
                    path: self.path.clone(),
                    source,
                })
            }
        };
        match serde_json::from_slice::<AuthSession>(&bytes) {
            Ok(s) => Ok(Some(s)),
            Err(source) => Err(SessionStoreError::Corrupt {
                path: self.path.clone(),
                source,
            }),
        }
    }

    pub async fn save(&self, session: &AuthSession) -> Result<(), SessionStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| SessionStoreError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }
        let bytes = serde_json::to_vec_pretty(session)?;
        // Atomic write: write to tmp, rename.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, &bytes)
            .await
            .map_err(|source| SessionStoreError::Io {
                path: tmp.clone(),
                source,
            })?;
        fs::rename(&tmp, &self.path)
            .await
            .map_err(|source| SessionStoreError::Io {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    pub async fn clear(&self) -> Result<(), SessionStoreError> {
        match fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SessionStoreError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
