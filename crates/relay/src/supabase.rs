//! Thin Supabase REST client for device ownership verification.
//!
//! The relay validates on WebSocket connect that the device_id the client
//! claims really belongs to the user_id in the JWT. We hit Supabase over
//! HTTPS with the secret key (the relay is server-side, trusted — bypasses RLS).

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Minimal device row shape the relay cares about. We don't need ed25519/x25519
/// keys here — the relay doesn't encrypt/decrypt, it just routes opaque blobs.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceRow {
    pub device_id: String,
    pub user_id: String,
    pub platform: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SupabaseError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("supabase returned status {0}")]
    Status(StatusCode),
    #[error("missing SUPABASE_URL or SUPABASE_SECRET_KEY")]
    MissingConfig,
}

/// Abstract Supabase client trait — makes testing with a mock trivial.
#[async_trait]
pub trait SupabaseClient: Send + Sync {
    /// Look up a device by (user_id, device_id). Returns None if:
    /// - row doesn't exist
    /// - row exists but user_id doesn't match
    /// - row is revoked
    async fn get_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceRow>, SupabaseError>;
}

pub struct HttpSupabaseClient {
    http: Client,
    url: String,
    secret_key: String,
}

impl HttpSupabaseClient {
    /// Construct from env. Accepts either SUPABASE_SECRET_KEY (new) or
    /// SUPABASE_SERVICE_ROLE_KEY (legacy). Both identify the secret key
    /// per the 2026 Supabase rename.
    pub fn from_env(http: Client) -> Result<Self, SupabaseError> {
        let url = std::env::var("SUPABASE_URL").map_err(|_| SupabaseError::MissingConfig)?;
        let secret_key = std::env::var("SUPABASE_SECRET_KEY")
            .or_else(|_| std::env::var("SUPABASE_SERVICE_ROLE_KEY"))
            .map_err(|_| SupabaseError::MissingConfig)?;
        Ok(Self {
            http,
            url,
            secret_key,
        })
    }
}

#[async_trait]
impl SupabaseClient for HttpSupabaseClient {
    async fn get_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceRow>, SupabaseError> {
        let endpoint = format!(
            "{}/rest/v1/devices?select=device_id,user_id,platform,revoked_at&device_id=eq.{}",
            self.url, device_id
        );
        let resp = self
            .http
            .get(&endpoint)
            .header("apikey", &self.secret_key)
            .header("authorization", format!("Bearer {}", self.secret_key))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(SupabaseError::Status(resp.status()));
        }
        let rows: Vec<DeviceRow> = resp.json().await?;
        let device = rows.into_iter().next();
        match device {
            None => Ok(None),
            Some(d) if d.user_id != user_id => Ok(None),
            Some(d) if d.revoked_at.is_some() => Ok(None),
            Some(d) => Ok(Some(d)),
        }
    }
}

/// Test mock that records how many times `get_device` was called.
#[derive(Default)]
pub struct MockSupabaseClient {
    devices: std::sync::Mutex<Vec<DeviceRow>>,
    calls: std::sync::atomic::AtomicUsize,
}

impl MockSupabaseClient {
    pub fn insert(&self, device: DeviceRow) {
        self.devices.lock().unwrap().push(device);
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl SupabaseClient for MockSupabaseClient {
    async fn get_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceRow>, SupabaseError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let devices = self.devices.lock().unwrap();
        let row = devices.iter().find(|d| d.device_id == device_id).cloned();
        match row {
            None => Ok(None),
            Some(d) if d.user_id != user_id => Ok(None),
            Some(d) if d.revoked_at.is_some() => Ok(None),
            Some(d) => Ok(Some(d)),
        }
    }
}
