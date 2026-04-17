//! 60-second TTL cache over Supabase device lookups.
//!
//! The relay validates device ownership on WS connect AND periodically
//! during the life of the connection (to catch revocations). Without a
//! cache that's one round-trip per message at worst, which we can't afford.
//! 60s is short enough that revocations propagate fast, long enough to
//! mostly hit the cache under load.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::supabase::{DeviceRow, SupabaseClient, SupabaseError};

#[derive(Clone)]
struct CachedEntry {
    row: Option<DeviceRow>,
    cached_at: Instant,
}

pub struct DeviceCache {
    client: Arc<dyn SupabaseClient>,
    ttl: Duration,
    cache: DashMap<String, CachedEntry>,
}

impl DeviceCache {
    pub fn new(client: Arc<dyn SupabaseClient>, ttl: Duration) -> Self {
        Self {
            client,
            ttl,
            cache: DashMap::new(),
        }
    }

    fn key(user_id: &str, device_id: &str) -> String {
        format!("{user_id}::{device_id}")
    }

    /// Get a device row if it exists, is owned by this user, and is not revoked.
    /// Returns `Ok(None)` for the "device not usable" cases so callers can treat
    /// them uniformly. Errors only on transport failures.
    pub async fn get(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceRow>, SupabaseError> {
        let key = Self::key(user_id, device_id);

        if let Some(entry) = self.cache.get(&key) {
            if entry.cached_at.elapsed() < self.ttl {
                return Ok(entry.row.clone());
            }
        }

        let row = self.client.get_device(user_id, device_id).await?;
        self.cache.insert(
            key,
            CachedEntry {
                row: row.clone(),
                cached_at: Instant::now(),
            },
        );
        Ok(row)
    }

    /// Manually invalidate a cached entry. Called when we receive a
    /// `device_revoked` event — we don't want to route another message
    /// to a freshly-revoked device just because the cache hasn't expired.
    pub fn invalidate(&self, user_id: &str, device_id: &str) {
        let key = Self::key(user_id, device_id);
        self.cache.remove(&key);
    }
}
