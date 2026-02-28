use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use ed25519_dalek::VerifyingKey;
use tokio::sync::mpsc;

use crate::auth::SupabaseAuth;
use crate::rate_limit::RateLimiter;

/// A connected device's WebSocket sender.
pub struct DeviceConnection {
    pub device_id: String,
    pub tx: mpsc::UnboundedSender<String>,
    pub connected_at: Instant,
}

/// A pending pairing offer (created by Mac, claimed by phone).
pub struct PairingOffer {
    pub device_id: String,
    pub pubkey: Vec<u8>,
    pub created_at: Instant,
}

/// A registered device (stored after pairing completes).
pub struct RegisteredDevice {
    pub device_id: String,
    pub verifying_key: VerifyingKey,
    pub paired_devices: HashSet<String>,
}

/// Shared relay server state.
#[derive(Clone)]
pub struct RelayState {
    /// Active WebSocket connections, keyed by device_id.
    pub connections: Arc<DashMap<String, DeviceConnection>>,
    /// Pending pairing offers, keyed by one_time_token.
    pub pairing_offers: Arc<DashMap<String, PairingOffer>>,
    /// Registered devices, keyed by device_id.
    pub devices: Arc<DashMap<String, RegisteredDevice>>,
    /// Expo push tokens, keyed by device_id.
    pub push_tokens: Arc<DashMap<String, String>>,
    /// Supabase JWT validator (None = JWT auth disabled).
    pub supabase_auth: Option<Arc<SupabaseAuth>>,
    /// Rate limiter for POST /pair.
    pub pair_rate_limiter: Arc<RateLimiter>,
    /// Rate limiter for POST /pair/claim.
    pub claim_rate_limiter: Arc<RateLimiter>,
    /// Rate limiter for POST /push-tokens (10 req/min per device_id).
    pub push_rate_limiter: Arc<RateLimiter>,
    /// PostHog HTTP client (None = tracking disabled).
    pub posthog_client: Option<reqwest::Client>,
    /// PostHog API key.
    pub posthog_api_key: String,
}

impl RelayState {
    pub fn new(
        supabase_auth: Option<Arc<SupabaseAuth>>,
        pair_rate_limiter: Arc<RateLimiter>,
        claim_rate_limiter: Arc<RateLimiter>,
        push_rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        let posthog_key = std::env::var("POSTHOG_API_KEY").unwrap_or_default();
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
            push_tokens: Arc::new(DashMap::new()),
            supabase_auth,
            pair_rate_limiter,
            claim_rate_limiter,
            push_rate_limiter,
            posthog_client: if posthog_key.is_empty() {
                None
            } else {
                Some(reqwest::Client::new())
            },
            posthog_api_key: posthog_key,
        }
    }
}
