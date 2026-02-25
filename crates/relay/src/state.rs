use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use ed25519_dalek::VerifyingKey;
use tokio::sync::mpsc;

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
}

impl RelayState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
        }
    }
}
