//! Background task: keep the stored AuthSession fresh by calling Supabase's
//! `POST /auth/v1/token?grant_type=refresh_token` ~500 s before `expires_at`.
//!
//! Single responsibility: orchestrate sleep → refresh → write-back. The HTTP
//! call itself lives behind `supabase_proxy::refresh_access_token()` so this
//! file stays a thin loop.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::auth::session_store::{AuthSession, SessionStore};
use crate::supabase_proxy::{refresh_access_token, SupabaseProxyError};

/// How long before `expires_at` we trigger a refresh. 500 s = ~8.3 min
/// earlier than the 1 h default Supabase expiry — wide enough to absorb
/// 30 s of clock skew plus the HTTP round-trip.
const REFRESH_LEAD: Duration = Duration::from_secs(500);

/// Min sleep between refresh ticks, to guard against a misconfigured token
/// with already-past `expires_at` that would otherwise busy-loop.
const MIN_SLEEP: Duration = Duration::from_secs(30);

/// Spawn the refresh loop. Reads/writes `state` through its `RwLock`, never
/// holding the guard across `.await` (§3.4.1 snapshot-clone pattern).
pub fn spawn(
    state: Arc<RwLock<Option<AuthSession>>>,
    store: Arc<SessionStore>,
    http: reqwest::Client,
    supabase_url: String,
    publishable_key: String,
) {
    tokio::spawn(async move {
        loop {
            // Snapshot the session — never hold the lock across HTTP.
            let session = {
                let guard = state.read().await;
                guard.clone()
            };
            let Some(session) = session else {
                // No session → nothing to refresh. Sleep long and check again.
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            };

            // Compute how long to sleep before refreshing.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let seconds_until_expiry = session.expires_at_unix.saturating_sub(now);
            let sleep_for = seconds_until_expiry
                .saturating_sub(REFRESH_LEAD.as_secs())
                .max(MIN_SLEEP.as_secs());

            tokio::time::sleep(Duration::from_secs(sleep_for)).await;

            // Refresh.
            match refresh_access_token(
                &http,
                &supabase_url,
                &publishable_key,
                &session.refresh_token,
            )
            .await
            {
                Ok(new_session) => {
                    info!(
                        user_id = %new_session.user_id,
                        new_exp = new_session.expires_at_unix,
                        "Supabase access token refreshed"
                    );
                    // Update in-memory first, then disk. Readers see the new one
                    // even if the disk write fails.
                    {
                        let mut guard = state.write().await;
                        *guard = Some(new_session.clone());
                    }
                    if let Err(e) = store.save(&new_session).await {
                        error!("Failed to persist refreshed session: {e}");
                    }
                }
                Err(SupabaseProxyError::Unauthorized) => {
                    warn!("Refresh token rejected — clearing session. User must sign in again.");
                    {
                        let mut guard = state.write().await;
                        *guard = None;
                    }
                    let _ = store.clear().await;
                }
                Err(e) => {
                    // Transient: log and back off; we'll re-enter the loop and try again.
                    warn!(error = %e, "Session refresh failed transiently — will retry");
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    });
}
