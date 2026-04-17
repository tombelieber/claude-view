//! RelayState — live connection registry and shared config for the relay.
//!
//! Contrast with the pre-Phase-1 version: we used to hold DashMaps of
//! `devices`, `pairing_offers`, and `connections`. Now only `connections`
//! exists — everything else lives in Supabase Postgres (see Phase 0).
//!
//! Connections are keyed by `(user_id, device_id)` so message routing is
//! a direct map lookup, and per-user fanout is a prefix scan. DashMap
//! gives us lock-free-ish concurrent access from the tokio runtime.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use reqwest::Client;
use tokio::sync::mpsc;

use crate::auth::SupabaseAuth;
use crate::device_cache::DeviceCache;
use crate::rate_limit::RateLimiter;

/// A single live WebSocket connection on the relay.
pub struct DeviceConnection {
    pub user_id: String,
    pub device_id: String,
    pub platform: String,
    /// Send queue for frames going OUT to this device's WS. The WS forwarder
    /// task awaits on this channel and writes to the socket.
    pub tx: mpsc::UnboundedSender<String>,
    pub connected_at: Instant,
}

impl DeviceConnection {
    /// Composite key for the connections map: `{user_id}::{device_id}`.
    pub fn key(user_id: &str, device_id: &str) -> String {
        format!("{user_id}::{device_id}")
    }

    pub fn self_key(&self) -> String {
        Self::key(&self.user_id, &self.device_id)
    }
}

/// Shared relay state passed to every handler via `axum::State`.
#[derive(Clone)]
pub struct RelayState {
    /// Live WebSocket connections. Keyed by `"{user_id}::{device_id}"`.
    /// Insertion on auth success, removal on disconnect.
    pub connections: Arc<DashMap<String, Arc<DeviceConnection>>>,

    /// Supabase JWT validator + JWKS cache. Optional — if SUPABASE_URL is
    /// unset, the relay runs in a no-auth dev mode that rejects all WS
    /// connections (safer than silently allowing them).
    pub supabase_auth: Option<Arc<SupabaseAuth>>,

    /// HTTP client for one-shot Supabase REST calls and OneSignal.
    pub http: Client,

    /// Cached device lookups. 60-second TTL. Bypass by calling Supabase
    /// directly when freshness matters (e.g., right after a pair).
    pub device_cache: Arc<DeviceCache>,

    /// OneSignal push config (unchanged from pre-Phase-1).
    pub onesignal_app_id: Option<String>,
    pub onesignal_rest_api_key: Option<String>,
    pub onesignal_http: Option<Client>,

    /// PostHog telemetry (unchanged).
    pub posthog_api_key: Option<String>,
    pub posthog_http: Option<Client>,

    /// Rate limiters (dropped the /pair limiter; kept push + WS).
    pub ws_rate_limiter: Arc<RateLimiter>,
    pub push_rate_limiter: Arc<RateLimiter>,
}

impl RelayState {
    /// Look up all connections for a user (for fanout, device list push, etc.).
    pub fn connections_for_user(&self, user_id: &str) -> Vec<Arc<DeviceConnection>> {
        let prefix = format!("{user_id}::");
        self.connections
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Look up a specific device connection by (user_id, device_id).
    pub fn connection_for(&self, user_id: &str, device_id: &str) -> Option<Arc<DeviceConnection>> {
        let key = DeviceConnection::key(user_id, device_id);
        self.connections
            .get(&key)
            .map(|entry| entry.value().clone())
    }

    /// Remove a connection by its composite key. Called on WS close.
    pub fn remove_connection(&self, user_id: &str, device_id: &str) {
        let key = DeviceConnection::key(user_id, device_id);
        self.connections.remove(&key);
    }
}
