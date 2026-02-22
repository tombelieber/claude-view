# Mobile Remote M1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship end-to-end remote session monitoring from phone — relay server, E2E encryption, QR pairing, WSS client, and PWA status monitor.

**Architecture:** Zero-knowledge relay forwards NaCl-encrypted blobs between Mac (daemon WSS client in existing server process) and phone (PWA at `/mobile` route). QR code pairing bootstraps X25519 key exchange. All state flows through existing Phase A broadcast channel.

**Tech Stack:** Rust/Axum (relay + WSS client), `crypto_box`/`ed25519-dalek`/`security-framework` (crypto), React/TypeScript (PWA), `tweetnacl`/`jsQR` (phone-side JS)

**Design doc:** `docs/plans/2026-02-23-mobile-remote-m1-design.md`

---

## Task 1: Relay Server — Crate Scaffold

**Files:**
- Create: `crates/relay/Cargo.toml`
- Create: `crates/relay/src/main.rs`
- Create: `crates/relay/src/lib.rs`
- Create: `crates/relay/src/state.rs`
- Modify: `Cargo.toml` (workspace members)

**Step 1: Add relay crate to workspace**

In root `Cargo.toml`, the workspace members is `crates/*` which auto-discovers. Create the crate directory:

```toml
# crates/relay/Cargo.toml
[package]
name = "claude-view-relay"
version.workspace = true
edition.workspace = true

[dependencies]
axum = { workspace = true }
tokio = { workspace = true }
tokio-tungstenite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
dashmap = "6"
uuid = { version = "1", features = ["v4"] }
base64 = { workspace = true }
ed25519-dalek = { version = "2", features = ["rand_core"] }
rand = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
```

**Step 2: Write the relay state types**

```rust
// crates/relay/src/state.rs
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
```

**Step 3: Write minimal relay main.rs**

```rust
// crates/relay/src/main.rs
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod state;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "warn,claude_view_relay=info".into()))
        .init();

    let state = state::RelayState::new();
    let app = claude_view_relay::app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 47893));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind relay port");
    info!("Relay server listening on {addr}");
    axum::serve(listener, app).await.expect("relay server");
}
```

```rust
// crates/relay/src/lib.rs
pub mod state;

use axum::{routing::get, Router};
use state::RelayState;

pub fn app(state: RelayState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p claude-view-relay`
Expected: compiles with 0 errors

**Step 5: Commit**

```
feat(relay): scaffold relay server crate with state types
```

---

## Task 2: Relay Server — WebSocket Handler

**Files:**
- Create: `crates/relay/src/ws.rs`
- Create: `crates/relay/src/auth.rs`
- Modify: `crates/relay/src/lib.rs`

**Step 1: Write auth verification**

```rust
// crates/relay/src/auth.rs
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
pub struct AuthMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub device_id: String,
    pub timestamp: u64,
    #[serde(with = "base64_bytes")]
    pub signature: Vec<u8>,
}

/// Verify an Ed25519 auth challenge. Returns true if valid.
pub fn verify_auth(msg: &AuthMessage, verifying_key: &VerifyingKey) -> bool {
    // Check timestamp freshness (60s window)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.abs_diff(msg.timestamp) > 60 {
        return false;
    }

    // Verify signature over "timestamp:device_id"
    let payload = format!("{}:{}", msg.timestamp, msg.device_id);
    let Ok(signature) = Signature::from_slice(&msg.signature) else {
        return false;
    };
    verifying_key.verify(payload.as_bytes(), &signature).is_ok()
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}
```

**Step 2: Write WebSocket handler**

```rust
// crates/relay/src/ws.rs
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::auth::{verify_auth, AuthMessage};
use crate::state::{DeviceConnection, RelayState};

#[derive(Deserialize)]
struct RelayEnvelope {
    to: String,
    payload: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<RelayState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: RelayState) {
    let (mut sink, mut stream) = socket.split();

    // First message must be auth
    let Some(Ok(Message::Text(first_msg))) = stream.next().await else {
        return;
    };
    let Ok(auth): Result<AuthMessage, _> = serde_json::from_str(&first_msg) else {
        let _ = sink.send(Message::Text(r#"{"error":"invalid auth format"}"#.into())).await;
        return;
    };
    if auth.msg_type != "auth" {
        let _ = sink.send(Message::Text(r#"{"error":"first message must be auth"}"#.into())).await;
        return;
    }

    // Verify against registered device
    let device_id = auth.device_id.clone();
    let verified = state.devices.get(&device_id)
        .map(|d| verify_auth(&auth, &d.verifying_key))
        .unwrap_or(false);

    if !verified {
        let _ = sink.send(Message::Text(r#"{"error":"auth failed"}"#.into())).await;
        return;
    }

    info!(device_id = %device_id, "device authenticated");
    let _ = sink.send(Message::Text(r#"{"type":"auth_ok"}"#.into())).await;

    // Create channel for forwarding messages TO this device
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    state.connections.insert(device_id.clone(), DeviceConnection {
        device_id: device_id.clone(),
        tx,
        connected_at: std::time::Instant::now(),
    });

    // Spawn task to forward queued messages to WS sink
    let device_id_clone = device_id.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Read incoming messages and forward to recipients
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(envelope) = serde_json::from_str::<RelayEnvelope>(&text) {
                    // Check if sender and recipient are paired
                    let is_paired = state.devices.get(&device_id)
                        .map(|d| d.paired_devices.contains(&envelope.to))
                        .unwrap_or(false);

                    if is_paired {
                        if let Some(conn) = state.connections.get(&envelope.to) {
                            let _ = conn.tx.send(text.to_string());
                        }
                        // If recipient offline, message is dropped (M1: no queuing)
                    } else {
                        warn!(from = %device_id, to = %envelope.to, "unpaired device, dropping");
                    }
                }
            }
            Message::Ping(data) => {
                // Pong is handled automatically by axum
                let _ = data;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup
    state.connections.remove(&device_id);
    forward_task.abort();
    info!(device_id = %device_id_clone, "device disconnected");
}
```

**Step 3: Wire into router**

Update `crates/relay/src/lib.rs`:

```rust
pub mod state;
pub mod ws;
pub mod auth;

use axum::{routing::get, Router};
use state::RelayState;

pub fn app(state: RelayState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p claude-view-relay`
Expected: compiles with 0 errors

**Step 5: Commit**

```
feat(relay): WebSocket handler with Ed25519 auth and message forwarding
```

---

## Task 3: Relay Server — Pairing Endpoints

**Files:**
- Create: `crates/relay/src/pairing.rs`
- Modify: `crates/relay/src/lib.rs`
- Modify: `crates/relay/src/state.rs`

**Step 1: Write pairing handlers**

```rust
// crates/relay/src/pairing.rs
use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;
use tracing::info;

use crate::state::{PairingOffer, RegisteredDevice, RelayState};

#[derive(Deserialize)]
pub struct PairRequest {
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    pub one_time_token: String,
}

#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    /// Phone's X25519 pubkey encrypted with Mac's X25519 pubkey (NaCl box).
    pub pubkey_encrypted_blob: String,
}

#[derive(Serialize)]
pub struct PairResponse {
    pub ok: bool,
}

/// POST /pair — Mac creates a pairing offer.
pub async fn create_pair(
    State(state): State<RelayState>,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // Validate Ed25519 pubkey
    let verifying_key = VerifyingKey::from_bytes(
        req.pubkey.as_slice().try_into().map_err(|_| StatusCode::BAD_REQUEST)?
    ).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Register device if not already registered
    if !state.devices.contains_key(&req.device_id) {
        state.devices.insert(req.device_id.clone(), RegisteredDevice {
            device_id: req.device_id.clone(),
            verifying_key,
            paired_devices: HashSet::new(),
        });
    }

    // Store pairing offer with TTL (cleanup handled by background task)
    state.pairing_offers.insert(req.one_time_token.clone(), PairingOffer {
        device_id: req.device_id,
        pubkey: req.pubkey,
        created_at: Instant::now(),
    });

    info!(token = %req.one_time_token, "pairing offer created");
    Ok(Json(PairResponse { ok: true }))
}

/// POST /pair/claim — Phone claims a pairing offer.
pub async fn claim_pair(
    State(state): State<RelayState>,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // Look up and consume the one-time token
    let (_, offer) = state.pairing_offers
        .remove(&req.one_time_token)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check TTL (5 minutes)
    if offer.created_at.elapsed().as_secs() > 300 {
        return Err(StatusCode::GONE);
    }

    // Register phone device
    let phone_verifying_key = VerifyingKey::from_bytes(
        req.pubkey.as_slice().try_into().map_err(|_| StatusCode::BAD_REQUEST)?
    ).map_err(|_| StatusCode::BAD_REQUEST)?;

    state.devices.insert(req.device_id.clone(), RegisteredDevice {
        device_id: req.device_id.clone(),
        verifying_key: phone_verifying_key,
        paired_devices: {
            let mut s = HashSet::new();
            s.insert(offer.device_id.clone());
            s
        },
    });

    // Update Mac's paired devices
    if let Some(mut mac_device) = state.devices.get_mut(&offer.device_id) {
        mac_device.paired_devices.insert(req.device_id.clone());
    }

    // Forward encrypted phone pubkey blob to Mac via WS (if connected)
    if let Some(mac_conn) = state.connections.get(&offer.device_id) {
        let msg = serde_json::json!({
            "type": "pair_complete",
            "device_id": req.device_id,
            "pubkey_encrypted_blob": req.pubkey_encrypted_blob,
        });
        let _ = mac_conn.tx.send(msg.to_string());
    }

    info!(mac = %offer.device_id, phone = %req.device_id, "pairing complete");
    Ok(Json(PairResponse { ok: true }))
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}
```

**Step 2: Add pairing offer cleanup task**

Add to `crates/relay/src/lib.rs`:

```rust
pub mod state;
pub mod ws;
pub mod auth;
pub mod pairing;

use axum::{routing::{get, post}, Router};
use state::RelayState;
use std::time::Duration;
use tracing::debug;

pub fn app(state: RelayState) -> Router {
    // Spawn background cleanup for expired pairing offers
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            cleanup_state.pairing_offers.retain(|_, offer| {
                offer.created_at.elapsed().as_secs() < 300
            });
            debug!("cleaned expired pairing offers");
        }
    });

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws::ws_handler))
        .route("/pair", post(pairing::create_pair))
        .route("/pair/claim", post(pairing::claim_pair))
        .with_state(state)
}
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-relay`

**Step 4: Commit**

```
feat(relay): pairing endpoints with one-time token and device registry
```

---

## Task 4: Relay Server — Tests

**Files:**
- Create: `crates/relay/tests/integration.rs`

**Step 1: Write integration tests**

```rust
// crates/relay/tests/integration.rs
use axum::http::StatusCode;
use axum_test::TestServer;

#[tokio::test]
async fn health_check() {
    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state);
    let server = TestServer::new(app).unwrap();

    let resp = server.get("/health").await;
    resp.assert_status_ok();
    resp.assert_text("ok");
}

#[tokio::test]
async fn pair_creates_offer() {
    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state.clone());
    let server = TestServer::new(app).unwrap();

    // Generate a test Ed25519 keypair
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut OsRng);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();

    use base64::{engine::general_purpose::STANDARD, Engine};
    let resp = server.post("/pair").json(&serde_json::json!({
        "device_id": "mac-test-001",
        "pubkey": STANDARD.encode(pubkey_bytes),
        "one_time_token": "test-token-123",
    })).await;

    resp.assert_status_ok();
    assert!(state.pairing_offers.contains_key("test-token-123"));
    assert!(state.devices.contains_key("mac-test-001"));
}

#[tokio::test]
async fn claim_consumes_token() {
    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state.clone());
    let server = TestServer::new(app).unwrap();

    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use base64::{engine::general_purpose::STANDARD, Engine};

    // Mac creates offer
    let mac_key = SigningKey::generate(&mut OsRng);
    server.post("/pair").json(&serde_json::json!({
        "device_id": "mac-test-001",
        "pubkey": STANDARD.encode(mac_key.verifying_key().to_bytes()),
        "one_time_token": "claim-token-123",
    })).await.assert_status_ok();

    // Phone claims
    let phone_key = SigningKey::generate(&mut OsRng);
    let resp = server.post("/pair/claim").json(&serde_json::json!({
        "one_time_token": "claim-token-123",
        "device_id": "phone-test-001",
        "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
        "pubkey_encrypted_blob": "encrypted-x25519-pubkey-placeholder",
    })).await;
    resp.assert_status_ok();

    // Token consumed
    assert!(!state.pairing_offers.contains_key("claim-token-123"));
    // Devices are paired
    assert!(state.devices.get("mac-test-001").unwrap().paired_devices.contains("phone-test-001"));
    assert!(state.devices.get("phone-test-001").unwrap().paired_devices.contains("mac-test-001"));
}

#[tokio::test]
async fn claim_expired_token_returns_gone() {
    let state = claude_view_relay::state::RelayState::new();

    // Insert an expired offer directly
    state.pairing_offers.insert("expired-token".into(), claude_view_relay::state::PairingOffer {
        device_id: "mac-old".into(),
        pubkey: vec![0u8; 32],
        created_at: std::time::Instant::now() - std::time::Duration::from_secs(600),
    });

    let app = claude_view_relay::app(state);
    let server = TestServer::new(app).unwrap();

    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use base64::{engine::general_purpose::STANDARD, Engine};
    let phone_key = SigningKey::generate(&mut OsRng);

    let resp = server.post("/pair/claim").json(&serde_json::json!({
        "one_time_token": "expired-token",
        "device_id": "phone-late",
        "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
        "pubkey_encrypted_blob": "doesnt-matter",
    })).await;
    resp.assert_status(StatusCode::GONE);
}

#[tokio::test]
async fn claim_nonexistent_token_returns_404() {
    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state);
    let server = TestServer::new(app).unwrap();

    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use base64::{engine::general_purpose::STANDARD, Engine};
    let phone_key = SigningKey::generate(&mut OsRng);

    let resp = server.post("/pair/claim").json(&serde_json::json!({
        "one_time_token": "nonexistent",
        "device_id": "phone-lost",
        "pubkey": STANDARD.encode(phone_key.verifying_key().to_bytes()),
        "pubkey_encrypted_blob": "doesnt-matter",
    })).await;
    resp.assert_status(StatusCode::NOT_FOUND);
}
```

**Step 2: Add test dependency**

Add to `crates/relay/Cargo.toml`:

```toml
[dev-dependencies]
axum-test = "16"
```

**Step 3: Run tests**

Run: `cargo test -p claude-view-relay`
Expected: 5 tests pass

**Step 4: Commit**

```
test(relay): integration tests for health, pairing, and token expiry
```

---

## Task 5: Crypto Module — Keychain + NaCl Box

**Files:**
- Create: `crates/server/src/crypto.rs`
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/Cargo.toml`

**Step 1: Add crypto dependencies to server crate**

Add to `crates/server/Cargo.toml` under `[dependencies]`:

```toml
crypto_box = "0.9"
ed25519-dalek = { version = "2", features = ["rand_core"] }
security-framework = "3"
x25519-dalek = { version = "2", features = ["static_secrets"] }
rand = "0.8"
```

**Step 2: Write the crypto module**

```rust
// crates/server/src/crypto.rs
//! NaCl box encryption, Ed25519 signing, and macOS Keychain key storage.

use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::{
    aead::{Aead, OsRng},
    SalsaBox, SecretKey as BoxSecretKey, PublicKey as BoxPublicKey,
};
use ed25519_dalek::{SigningKey, Signer};
use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
use security_framework::passwords::{delete_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

const KEYCHAIN_SERVICE: &str = "com.claude-view";
const KEYCHAIN_ACCOUNT_IDENTITY: &str = "identity-keys";
const KEYCHAIN_ACCOUNT_DEVICES: &str = "paired-devices";

/// All keys for this device.
#[derive(Serialize, Deserialize)]
pub struct DeviceIdentity {
    /// Ed25519 signing key (32 bytes, base64).
    pub signing_key: String,
    /// X25519 encryption secret key (32 bytes, base64).
    pub encryption_key: String,
    /// Unique device ID.
    pub device_id: String,
}

/// A paired remote device.
#[derive(Serialize, Deserialize, Clone)]
pub struct PairedDevice {
    pub device_id: String,
    pub x25519_pubkey: String, // base64
    pub name: String,
    pub paired_at: u64, // unix timestamp
}

/// Load or create device identity from macOS Keychain.
pub fn load_or_create_identity() -> Result<DeviceIdentity, String> {
    // Try to load existing
    if let Ok(data) = security_framework::passwords::get_generic_password(
        KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_IDENTITY,
    ) {
        if let Ok(identity) = serde_json::from_slice::<DeviceIdentity>(&data) {
            info!("loaded device identity from Keychain");
            return Ok(identity);
        }
    }

    // Generate fresh keys
    let signing_key = SigningKey::generate(&mut OsRng);
    let box_secret = BoxSecretKey::generate(&mut OsRng);
    let device_id = format!("mac-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap());

    let identity = DeviceIdentity {
        signing_key: STANDARD.encode(signing_key.to_bytes()),
        encryption_key: STANDARD.encode(box_secret.to_bytes()),
        device_id,
    };

    let json = serde_json::to_vec(&identity).map_err(|e| e.to_string())?;
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_IDENTITY, &json)
        .map_err(|e| format!("Keychain write failed: {e}"))?;

    info!(device_id = %identity.device_id, "created new device identity in Keychain");
    Ok(identity)
}

/// Load paired devices from Keychain.
pub fn load_paired_devices() -> Vec<PairedDevice> {
    match security_framework::passwords::get_generic_password(
        KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_DEVICES,
    ) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Save paired devices to Keychain.
pub fn save_paired_devices(devices: &[PairedDevice]) -> Result<(), String> {
    let json = serde_json::to_vec(devices).map_err(|e| e.to_string())?;
    // Delete then set (Keychain doesn't have upsert)
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_DEVICES);
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_DEVICES, &json)
        .map_err(|e| format!("Keychain write failed: {e}"))?;
    Ok(())
}

/// Add a newly paired device.
pub fn add_paired_device(device: PairedDevice) -> Result<(), String> {
    let mut devices = load_paired_devices();
    // Replace if already exists
    devices.retain(|d| d.device_id != device.device_id);
    devices.push(device);
    save_paired_devices(&devices)
}

/// Remove a paired device by ID.
pub fn remove_paired_device(device_id: &str) -> Result<(), String> {
    let mut devices = load_paired_devices();
    devices.retain(|d| d.device_id != device_id);
    save_paired_devices(&devices)
}

/// Encrypt a message for a paired device using NaCl box.
pub fn encrypt_for_device(
    plaintext: &[u8],
    recipient_pubkey_b64: &str,
    sender_secret: &BoxSecretKey,
) -> Result<String, String> {
    let recipient_pubkey_bytes = STANDARD.decode(recipient_pubkey_b64)
        .map_err(|e| format!("bad pubkey base64: {e}"))?;
    let recipient_pubkey = BoxPublicKey::from(
        <[u8; 32]>::try_from(recipient_pubkey_bytes.as_slice())
            .map_err(|_| "pubkey must be 32 bytes")?
    );

    let salsa_box = SalsaBox::new(&recipient_pubkey, sender_secret);
    let nonce = crypto_box::generate_nonce(&mut OsRng);
    let ciphertext = salsa_box.encrypt(&nonce, plaintext)
        .map_err(|e| format!("encryption failed: {e}"))?;

    // Wire format: nonce (24 bytes) || ciphertext
    let mut wire = nonce.to_vec();
    wire.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(wire))
}

/// Sign an auth challenge for relay authentication.
pub fn sign_auth_challenge(identity: &DeviceIdentity) -> Result<(u64, String), String> {
    let signing_bytes = STANDARD.decode(&identity.signing_key)
        .map_err(|e| format!("bad signing key: {e}"))?;
    let signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(signing_bytes.as_slice())
            .map_err(|_| "signing key must be 32 bytes")?
    );

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let payload = format!("{}:{}", timestamp, identity.device_id);
    let signature = signing_key.sign(payload.as_bytes());

    Ok((timestamp, STANDARD.encode(signature.to_bytes())))
}

/// Get the X25519 BoxSecretKey from the identity.
pub fn box_secret_key(identity: &DeviceIdentity) -> Result<BoxSecretKey, String> {
    let bytes = STANDARD.decode(&identity.encryption_key)
        .map_err(|e| format!("bad encryption key: {e}"))?;
    Ok(BoxSecretKey::from(
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| "encryption key must be 32 bytes")?
    ))
}

/// Get the Ed25519 verifying (public) key bytes from the identity.
pub fn verifying_key_bytes(identity: &DeviceIdentity) -> Result<Vec<u8>, String> {
    let signing_bytes = STANDARD.decode(&identity.signing_key)
        .map_err(|e| format!("bad signing key: {e}"))?;
    let signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(signing_bytes.as_slice())
            .map_err(|_| "signing key must be 32 bytes")?
    );
    Ok(signing_key.verifying_key().to_bytes().to_vec())
}
```

**Step 3: Export from lib.rs**

Add `pub mod crypto;` to `crates/server/src/lib.rs`.

**Step 4: Verify it compiles**

Run: `cargo check -p claude-view-server`

**Step 5: Commit**

```
feat(server): crypto module with NaCl box encryption and Keychain key storage
```

---

## Task 6: Relay WSS Client

**Files:**
- Create: `crates/server/src/live/relay_client.rs`
- Modify: `crates/server/src/live/mod.rs`
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Write the relay client**

```rust
// crates/server/src/live/relay_client.rs
//! WebSocket client that connects to the relay server and forwards
//! encrypted session updates to paired mobile devices.

use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use super::state::{LiveSession, SessionEvent};
use crate::crypto::{
    self, box_secret_key, encrypt_for_device, load_or_create_identity, load_paired_devices,
    sign_auth_challenge, DeviceIdentity, PairedDevice,
};

/// Configuration for the relay client.
pub struct RelayClientConfig {
    pub relay_url: String,
    pub heartbeat_interval: Duration,
    pub max_reconnect_delay: Duration,
}

impl Default for RelayClientConfig {
    fn default() -> Self {
        Self {
            relay_url: "ws://localhost:47893/ws".into(),
            heartbeat_interval: Duration::from_secs(30),
            max_reconnect_delay: Duration::from_secs(30),
        }
    }
}

/// Spawn the relay client as a background task.
/// Subscribes to the broadcast channel and forwards encrypted session updates.
pub fn spawn_relay_client(
    tx: broadcast::Sender<SessionEvent>,
    sessions: super::manager::LiveSessionMap,
    config: RelayClientConfig,
) {
    tokio::spawn(async move {
        // Load identity (or create on first run)
        let identity = match load_or_create_identity() {
            Ok(id) => id,
            Err(e) => {
                error!("failed to load device identity: {e}");
                return;
            }
        };

        info!(device_id = %identity.device_id, "relay client starting");

        let mut backoff = Duration::from_secs(1);

        loop {
            let paired_devices = load_paired_devices();
            if paired_devices.is_empty() {
                // No devices paired — sleep and check again
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            match connect_and_stream(&identity, &paired_devices, &tx, &sessions, &config).await {
                Ok(()) => {
                    info!("relay connection closed cleanly");
                    backoff = Duration::from_secs(1); // Reset backoff
                }
                Err(e) => {
                    warn!(backoff_secs = backoff.as_secs(), "relay connection failed: {e}");
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(config.max_reconnect_delay);
        }
    });
}

async fn connect_and_stream(
    identity: &DeviceIdentity,
    paired_devices: &[PairedDevice],
    tx: &broadcast::Sender<SessionEvent>,
    sessions: &super::manager::LiveSessionMap,
    config: &RelayClientConfig,
) -> Result<(), String> {
    // Connect to relay
    let (ws_stream, _) = connect_async(&config.relay_url)
        .await
        .map_err(|e| format!("WS connect failed: {e}"))?;

    let (mut sink, mut stream) = ws_stream.split();

    // Authenticate
    let (timestamp, signature) = sign_auth_challenge(identity)?;
    let auth_msg = serde_json::json!({
        "type": "auth",
        "device_id": identity.device_id,
        "timestamp": timestamp,
        "signature": signature,
    });
    sink.send(Message::Text(auth_msg.to_string()))
        .await
        .map_err(|e| format!("auth send failed: {e}"))?;

    // Wait for auth_ok
    match stream.next().await {
        Some(Ok(Message::Text(text))) => {
            if text.contains("error") {
                return Err(format!("auth rejected: {text}"));
            }
        }
        other => return Err(format!("unexpected auth response: {other:?}")),
    }

    info!("relay authenticated, sending initial snapshot");

    let box_secret = box_secret_key(identity)?;

    // Send initial snapshot of all current sessions
    {
        let sessions_map = sessions.read().await;
        for session in sessions_map.values() {
            let json = serde_json::to_vec(session).unwrap_or_default();
            for device in paired_devices {
                if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                    let envelope = serde_json::json!({
                        "to": device.device_id,
                        "payload": encrypted,
                    });
                    let _ = sink.send(Message::Text(envelope.to_string())).await;
                }
            }
        }
    }

    // Subscribe to broadcast and forward events
    let mut rx = tx.subscribe();
    let heartbeat_interval = config.heartbeat_interval;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(SessionEvent::SessionDiscovered { session } |
                       SessionEvent::SessionUpdated { session }) => {
                        let json = serde_json::to_vec(&session).unwrap_or_default();
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = serde_json::json!({
                                    "to": device.device_id,
                                    "payload": encrypted,
                                });
                                if sink.send(Message::Text(envelope.to_string())).await.is_err() {
                                    return Ok(()); // Connection closed
                                }
                            }
                        }
                    }
                    Ok(SessionEvent::SessionCompleted { session_id }) => {
                        let msg = serde_json::json!({"type": "session_completed", "session_id": session_id});
                        let json = serde_json::to_vec(&msg).unwrap_or_default();
                        for device in paired_devices {
                            if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                let envelope = serde_json::json!({
                                    "to": device.device_id,
                                    "payload": encrypted,
                                });
                                let _ = sink.send(Message::Text(envelope.to_string())).await;
                            }
                        }
                    }
                    Ok(SessionEvent::Summary { .. }) => {
                        // Skip summary events for mobile (phone computes locally)
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "relay client lagged, will resync");
                        // Resync: send all current sessions
                        let sessions_map = sessions.read().await;
                        for session in sessions_map.values() {
                            let json = serde_json::to_vec(session).unwrap_or_default();
                            for device in paired_devices {
                                if let Ok(encrypted) = encrypt_for_device(&json, &device.x25519_pubkey, &box_secret) {
                                    let envelope = serde_json::json!({
                                        "to": device.device_id,
                                        "payload": encrypted,
                                    });
                                    let _ = sink.send(Message::Text(envelope.to_string())).await;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Ok(()); // Server shutting down
                    }
                }
            }
            _ = tokio::time::sleep(heartbeat_interval) => {
                if sink.send(Message::Ping(vec![])).await.is_err() {
                    return Ok(()); // Connection closed
                }
            }
            // Read incoming messages (pair_complete events from relay)
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle pair_complete from relay
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
                                info!("received pair_complete from relay");
                                // TODO: decrypt phone pubkey and store in Keychain
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}
```

**Step 2: Wire into LiveSessionManager**

Add `pub mod relay_client;` to `crates/server/src/live/mod.rs`.

In `crates/server/src/live/manager.rs`, after the existing `spawn_cleanup_task()` call in `start()`, add:

```rust
// Spawn relay client for mobile remote access
relay_client::spawn_relay_client(
    tx.clone(),
    sessions.clone(),
    relay_client::RelayClientConfig::default(),
);
```

Update the log line to say `4 background tasks`.

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`

**Step 4: Commit**

```
feat(server): relay WSS client with encrypted session forwarding
```

---

## Task 7: Desktop Pairing Routes

**Files:**
- Create: `crates/server/src/routes/pairing.rs`
- Modify: `crates/server/src/routes/mod.rs`
- Modify: `crates/server/src/state.rs`

**Step 1: Write pairing API routes**

Endpoints for the desktop slide-over panel:

```rust
// crates/server/src/routes/pairing.rs
use axum::{extract::State, http::StatusCode, routing::{delete, get, post}, Json, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::crypto::{
    load_or_create_identity, load_paired_devices, remove_paired_device, verifying_key_bytes,
};
use crate::state::AppState;

#[derive(Serialize)]
struct QrPayload {
    /// Relay WebSocket URL.
    r: String,
    /// Mac X25519 public key (base64).
    k: String,
    /// One-time pairing token.
    t: String,
    /// Protocol version.
    v: u8,
}

#[derive(Serialize)]
struct PairedDeviceResponse {
    device_id: String,
    name: String,
    paired_at: u64,
}

/// GET /api/pairing/qr — Generate QR payload for mobile pairing.
async fn generate_qr(
    State(state): State<Arc<AppState>>,
) -> Result<Json<QrPayload>, StatusCode> {
    let identity = load_or_create_identity().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get X25519 public key from the encryption secret key
    let box_secret = crate::crypto::box_secret_key(&identity)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let box_public = box_secret.public_key();

    // Generate one-time token
    let token = uuid::Uuid::new_v4().to_string();

    // Register pairing offer with relay
    // (In local dev, relay runs on localhost:47893)
    let relay_url = "ws://localhost:47893/ws";

    // POST to relay /pair endpoint
    let ed25519_pubkey = verifying_key_bytes(&identity)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let client = reqwest::Client::new();
    let _ = client.post("http://localhost:47893/pair")
        .json(&serde_json::json!({
            "device_id": identity.device_id,
            "pubkey": base64::engine::general_purpose::STANDARD.encode(&ed25519_pubkey),
            "one_time_token": &token,
        }))
        .send()
        .await;

    Ok(Json(QrPayload {
        r: relay_url.to_string(),
        k: base64::engine::general_purpose::STANDARD.encode(box_public.as_bytes()),
        t: token,
        v: 1,
    }))
}

/// GET /api/pairing/devices — List paired devices.
async fn list_devices() -> Json<Vec<PairedDeviceResponse>> {
    let devices = load_paired_devices();
    Json(devices.into_iter().map(|d| PairedDeviceResponse {
        device_id: d.device_id,
        name: d.name,
        paired_at: d.paired_at,
    }).collect())
}

/// DELETE /api/pairing/devices/:id — Unpair a device.
async fn unpair_device(
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> StatusCode {
    match remove_paired_device(&device_id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/pairing/qr", get(generate_qr))
        .route("/pairing/devices", get(list_devices))
        .route("/pairing/devices/{id}", delete(unpair_device))
}
```

**Step 2: Wire into routes/mod.rs**

Add `pub mod pairing;` to the module list.
Add `.nest("/api", pairing::router())` to the router chain.

**Step 3: Add `reqwest` dependency**

Add to `crates/server/Cargo.toml`:

```toml
reqwest = { version = "0.12", features = ["json"] }
uuid = { version = "1", features = ["v4"] }
```

**Step 4: Verify it compiles**

Run: `cargo check -p claude-view-server`

**Step 5: Commit**

```
feat(server): desktop pairing API — QR generation, device list, unpair
```

---

## Task 8: Frontend — Mobile Route + QR Scanner Page

**Files:**
- Create: `src/pages/MobilePairingPage.tsx`
- Create: `src/pages/MobileMonitorPage.tsx`
- Create: `src/pages/MobileLayout.tsx`
- Modify: `src/router.tsx`

**Step 1: Create mobile layout (no Header/Sidebar)**

```tsx
// src/pages/MobileLayout.tsx
import { Outlet } from 'react-router-dom'

/** Minimal layout for /mobile — no desktop header, sidebar, or status bar. */
export function MobileLayout() {
  return (
    <div className="h-screen flex flex-col bg-gray-950 text-gray-100">
      <Outlet />
    </div>
  )
}
```

**Step 2: Create pairing page stub**

```tsx
// src/pages/MobilePairingPage.tsx
import { Smartphone } from 'lucide-react'

/** Shown when phone has no paired keys in IndexedDB. */
export function MobilePairingPage() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6">
      <Smartphone className="w-16 h-16 text-gray-500 mb-6" />
      <h1 className="text-xl font-semibold mb-2">Pair with Desktop</h1>
      <p className="text-gray-400 text-center mb-8 max-w-xs">
        Scan the QR code from your desktop claude-view to monitor sessions remotely.
      </p>
      <button
        className="px-6 py-3 bg-green-600 hover:bg-green-500 text-white rounded-lg font-medium transition-colors cursor-pointer min-h-[44px]"
        onClick={() => {
          // TODO: Task 11 — open camera QR scanner
          alert('QR scanner coming in Task 11')
        }}
      >
        Scan QR Code
      </button>
    </div>
  )
}
```

**Step 3: Create monitor page stub**

```tsx
// src/pages/MobileMonitorPage.tsx (mobile version, separate from desktop)
/** Shown when phone is paired and connected to relay. */
export function MobileMonitorPageMobile() {
  return (
    <div className="flex-1 flex flex-col">
      <header className="h-12 flex items-center justify-between px-4 border-b border-gray-800">
        <h1 className="text-lg font-semibold">Claude Sessions</h1>
        <div className="w-2 h-2 rounded-full bg-green-500" title="Connected" />
      </header>
      <div className="flex-1 flex items-center justify-center text-gray-500">
        Connecting to relay...
      </div>
    </div>
  )
}
```

**Step 4: Add /mobile route to router**

In `src/router.tsx`, add:

```tsx
import { MobileLayout } from './pages/MobileLayout'
import { MobilePairingPage } from './pages/MobilePairingPage'
import { MobileMonitorPageMobile } from './pages/MobileMonitorPage'
```

Add a second top-level route (sibling to the existing `path: '/'`):

```tsx
{
  path: '/mobile',
  element: <MobileLayout />,
  children: [
    { index: true, element: <MobilePairingPage /> },
    { path: 'monitor', element: <MobileMonitorPageMobile /> },
  ],
},
```

**Step 5: Verify**

Run: `bunx --bun vitest run --passWithNoTests` (type check only)
Navigate to `http://localhost:5173/mobile` in dev mode — should see pairing page.

**Step 6: Commit**

```
feat(frontend): /mobile route with pairing and monitor page stubs
```

---

## Task 9: Frontend — Desktop QR Pairing Slide-Over

**Files:**
- Create: `src/components/PairingPanel.tsx`
- Create: `src/hooks/use-pairing.ts`
- Modify: `src/components/Header.tsx`

**Step 1: Create pairing hook**

```tsx
// src/hooks/use-pairing.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'

interface QrPayload {
  r: string  // relay URL
  k: string  // X25519 pubkey
  t: string  // one-time token
  v: number
}

interface PairedDevice {
  device_id: string
  name: string
  paired_at: number
}

export function useQrCode(enabled: boolean) {
  return useQuery<QrPayload>({
    queryKey: ['pairing', 'qr'],
    queryFn: () => fetch('/api/pairing/qr').then(r => r.json()),
    enabled,
    refetchInterval: 4 * 60 * 1000, // Refresh every 4 min (token expires at 5)
    staleTime: 0,
  })
}

export function usePairedDevices() {
  return useQuery<PairedDevice[]>({
    queryKey: ['pairing', 'devices'],
    queryFn: () => fetch('/api/pairing/devices').then(r => r.json()),
  })
}

export function useUnpairDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (deviceId: string) =>
      fetch(`/api/pairing/devices/${deviceId}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['pairing'] }),
  })
}
```

**Step 2: Create PairingPanel component**

```tsx
// src/components/PairingPanel.tsx
import { useState } from 'react'
import { Smartphone, X, Trash2, QrCode, Loader2 } from 'lucide-react'
import * as Popover from '@radix-ui/react-popover'
import { useQrCode, usePairedDevices, useUnpairDevice } from '../hooks/use-pairing'

export function PairingPanel() {
  const [open, setOpen] = useState(false)
  const [showQr, setShowQr] = useState(false)
  const { data: devices = [] } = usePairedDevices()
  const { data: qr, isLoading: qrLoading } = useQrCode(open && (devices.length === 0 || showQr))
  const unpair = useUnpairDevice()

  const hasPairedDevices = devices.length > 0

  return (
    <Popover.Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) setShowQr(false) }}>
      <Popover.Trigger asChild>
        <button
          aria-label="Mobile devices"
          className="relative p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 rounded-md"
        >
          <Smartphone className="w-5 h-5" />
          {!hasPairedDevices && (
            <span className="absolute top-1 right-1 w-2 h-2 bg-green-500 rounded-full" />
          )}
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="end"
          sideOffset={8}
          className="w-72 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-4 z-50"
        >
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Mobile Access</h3>
            <Popover.Close asChild>
              <button className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-pointer rounded">
                <X className="w-4 h-4" />
              </button>
            </Popover.Close>
          </div>

          {/* QR Code Section */}
          {(!hasPairedDevices || showQr) && (
            <div className="mb-4">
              <div className="bg-white rounded-lg p-3 flex items-center justify-center min-h-[160px]">
                {qrLoading ? (
                  <Loader2 className="w-8 h-8 text-gray-400 animate-spin" />
                ) : qr ? (
                  <div className="text-center">
                    <QrCode className="w-32 h-32 text-gray-800 mx-auto" />
                    <p className="text-xs text-gray-500 mt-2">
                      Scan with your phone camera
                    </p>
                  </div>
                ) : (
                  <p className="text-sm text-gray-500">Failed to generate QR</p>
                )}
              </div>
            </div>
          )}

          {/* Paired Devices */}
          {hasPairedDevices && (
            <div>
              <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
                Paired Devices
              </h4>
              {devices.map((d) => (
                <div key={d.device_id} className="flex items-center justify-between py-2 border-t border-gray-100 dark:border-gray-800">
                  <div>
                    <p className="text-sm text-gray-900 dark:text-gray-100">{d.name || d.device_id}</p>
                    <p className="text-xs text-gray-500">
                      {new Date(d.paired_at * 1000).toLocaleDateString()}
                    </p>
                  </div>
                  <button
                    onClick={() => unpair.mutate(d.device_id)}
                    className="p-1.5 text-gray-400 hover:text-red-500 cursor-pointer rounded transition-colors"
                    aria-label={`Remove ${d.name || d.device_id}`}
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
              {!showQr && (
                <button
                  onClick={() => setShowQr(true)}
                  className="mt-2 text-xs text-blue-500 hover:text-blue-400 cursor-pointer"
                >
                  + Pair another device
                </button>
              )}
            </div>
          )}

          {!hasPairedDevices && (
            <p className="text-xs text-gray-500 text-center">No devices paired</p>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
```

**Step 3: Add PairingPanel to Header**

In `src/components/Header.tsx`, import `PairingPanel` and add it to the right nav section, before the `NotificationSoundPopover`:

```tsx
<PairingPanel />
<NotificationSoundPopover ... />
```

**Step 4: Verify**

Run: `bun run typecheck`
Visual check: phone icon appears in header.

**Step 5: Commit**

```
feat(frontend): desktop QR pairing panel in header with device management
```

---

## Task 10: Frontend — Mobile WebSocket Hook

**Files:**
- Create: `src/hooks/use-mobile-relay.ts`
- Create: `src/lib/mobile-crypto.ts`
- Create: `src/lib/mobile-storage.ts`

**Step 1: Create IndexedDB storage helpers**

```tsx
// src/lib/mobile-storage.ts
/** IndexedDB helpers for storing device keys on mobile. */

const DB_NAME = 'claude-view-mobile'
const STORE_NAME = 'keys'

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1)
    req.onupgradeneeded = () => req.result.createObjectStore(STORE_NAME)
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export async function getItem(key: string): Promise<string | null> {
  const db = await openDb()
  return new Promise((resolve) => {
    const tx = db.transaction(STORE_NAME, 'readonly')
    const req = tx.objectStore(STORE_NAME).get(key)
    req.onsuccess = () => resolve(req.result ?? null)
    req.onerror = () => resolve(null)
  })
}

export async function setItem(key: string, value: string): Promise<void> {
  const db = await openDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite')
    tx.objectStore(STORE_NAME).put(value, key)
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
}

export async function removeItem(key: string): Promise<void> {
  const db = await openDb()
  return new Promise((resolve) => {
    const tx = db.transaction(STORE_NAME, 'readwrite')
    tx.objectStore(STORE_NAME).delete(key)
    tx.oncomplete = () => resolve()
    tx.onerror = () => resolve()
  })
}
```

**Step 2: Create crypto helpers for phone**

Install tweetnacl: `bun add tweetnacl tweetnacl-util`

```tsx
// src/lib/mobile-crypto.ts
import nacl from 'tweetnacl'
import { decodeBase64, encodeBase64, decodeUTF8, encodeUTF8 } from 'tweetnacl-util'
import { getItem, setItem } from './mobile-storage'

/** Generate and store phone keypairs in IndexedDB. */
export async function generatePhoneKeys(): Promise<{
  encryptionPublicKey: Uint8Array
  signingPublicKey: Uint8Array
}> {
  const encKp = nacl.box.keyPair()
  const signKp = nacl.sign.keyPair()

  await setItem('enc_secret', encodeBase64(encKp.secretKey))
  await setItem('enc_public', encodeBase64(encKp.publicKey))
  await setItem('sign_secret', encodeBase64(signKp.secretKey))
  await setItem('sign_public', encodeBase64(signKp.publicKey))

  return {
    encryptionPublicKey: encKp.publicKey,
    signingPublicKey: signKp.publicKey,
  }
}

/** Decrypt a NaCl box message from Mac. */
export async function decryptMessage(
  encryptedBase64: string,
  macPublicKeyBase64: string,
): Promise<string | null> {
  const secretKeyB64 = await getItem('enc_secret')
  if (!secretKeyB64) return null

  const secretKey = decodeBase64(secretKeyB64)
  const macPublicKey = decodeBase64(macPublicKeyBase64)
  const wire = decodeBase64(encryptedBase64)

  // Wire format: nonce (24 bytes) || ciphertext
  const nonce = wire.slice(0, 24)
  const ciphertext = wire.slice(24)

  const plaintext = nacl.box.open(ciphertext, nonce, macPublicKey, secretKey)
  if (!plaintext) return null

  return encodeUTF8(plaintext)
}

/** Sign an auth challenge for relay. */
export async function signAuthChallenge(deviceId: string): Promise<{
  timestamp: number
  signature: string
} | null> {
  const secretKeyB64 = await getItem('sign_secret')
  if (!secretKeyB64) return null

  const secretKey = decodeBase64(secretKeyB64)
  const timestamp = Math.floor(Date.now() / 1000)
  const payload = `${timestamp}:${deviceId}`
  const signature = nacl.sign.detached(decodeUTF8(payload), secretKey)

  return { timestamp, signature: encodeBase64(signature) }
}

/** Check if phone has stored keys (= is paired). */
export async function isPaired(): Promise<boolean> {
  const key = await getItem('enc_secret')
  return key !== null
}

/** Get stored Mac public key (set during pairing). */
export async function getMacPublicKey(): Promise<string | null> {
  return getItem('mac_enc_public')
}

/** Store Mac public key after QR scan. */
export async function storeMacPublicKey(pubkeyBase64: string): Promise<void> {
  await setItem('mac_enc_public', pubkeyBase64)
}
```

**Step 3: Create mobile relay hook**

```tsx
// src/hooks/use-mobile-relay.ts
import { useCallback, useEffect, useRef, useState } from 'react'
import type { LiveSession } from '../components/live/types'
import { decryptMessage, getMacPublicKey, signAuthChallenge } from '../lib/mobile-crypto'
import { getItem } from '../lib/mobile-storage'

interface UseMobileRelayResult {
  sessions: Map<string, LiveSession>
  isConnected: boolean
  error: string | null
}

export function useMobileRelay(relayUrl: string | null): UseMobileRelayResult {
  const [sessions, setSessions] = useState<Map<string, LiveSession>>(new Map())
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    if (!relayUrl) return

    let cancelled = false

    async function connect() {
      const deviceId = await getItem('device_id')
      const macPubKey = await getMacPublicKey()
      if (!deviceId || !macPubKey) {
        setError('Not paired')
        return
      }

      const ws = new WebSocket(relayUrl!)
      wsRef.current = ws

      ws.onopen = async () => {
        // Send auth
        const auth = await signAuthChallenge(deviceId)
        if (!auth) { ws.close(); return }
        ws.send(JSON.stringify({
          type: 'auth',
          device_id: deviceId,
          timestamp: auth.timestamp,
          signature: auth.signature,
        }))
      }

      ws.onmessage = async (event) => {
        if (wsRef.current !== ws) return // Stale guard

        const data = JSON.parse(event.data)

        if (data.type === 'auth_ok') {
          setIsConnected(true)
          setError(null)
          return
        }

        if (data.error) {
          setError(data.error)
          return
        }

        // Decrypt payload
        if (data.payload) {
          const json = await decryptMessage(data.payload, macPubKey!)
          if (!json) return

          const parsed = JSON.parse(json)

          if (parsed.type === 'session_completed') {
            setSessions((prev) => {
              const next = new Map(prev)
              next.delete(parsed.session_id)
              return next
            })
          } else {
            // LiveSession update
            const session = parsed as LiveSession
            setSessions((prev) => {
              const next = new Map(prev)
              next.set(session.id, session)
              return next
            })
          }
        }
      }

      ws.onclose = () => {
        if (wsRef.current === ws) {
          setIsConnected(false)
          if (!cancelled) {
            // Reconnect with backoff
            setTimeout(connect, 3000)
          }
        }
      }

      ws.onerror = () => {
        setError('Connection failed')
      }
    }

    connect()

    return () => {
      cancelled = true
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [relayUrl])

  return { sessions, isConnected, error }
}
```

**Step 4: Verify**

Run: `bun run typecheck`

**Step 5: Commit**

```
feat(frontend): mobile relay WebSocket hook with NaCl decryption
```

---

## Task 11: Frontend — QR Scanner + Full Mobile Monitor

**Files:**
- Modify: `src/pages/MobilePairingPage.tsx`
- Modify: `src/pages/MobileMonitorPage.tsx`
- Create: `src/components/mobile/MobileSessionCard.tsx`
- Create: `src/components/mobile/MobileSessionDetail.tsx`

**Step 1: Install jsQR**

Run: `bun add jsqr`

**Step 2: Implement QR scanner in pairing page**

Update `src/pages/MobilePairingPage.tsx` with camera-based QR scanning. Uses `getUserMedia` + canvas + jsQR for decoding. On successful scan, stores Mac pubkey in IndexedDB, generates phone keys, POSTs to relay `/pair/claim`, and navigates to `/mobile/monitor`.

Key interactions:
- Camera preview in a `<video>` element
- Canvas offscreen for frame capture
- `requestAnimationFrame` loop calling `jsQR()`
- On decode: parse QR payload (`r`, `k`, `t`, `v`), call `generatePhoneKeys()`, store relay URL + Mac pubkey

**Step 3: Implement mobile session list**

Update `src/pages/MobileMonitorPage.tsx` to use `useMobileRelay()` hook and render `MobileSessionCard` components.

**Step 4: Create MobileSessionCard**

`src/components/mobile/MobileSessionCard.tsx` — compact card with:
- StatusDot (reuse from desktop)
- Project name + title
- Agent state label
- Cost badge
- Context bar (thin progress bar)
- 44px minimum touch target
- `onClick` opens MobileSessionDetail

**Step 5: Create MobileSessionDetail**

`src/components/mobile/MobileSessionDetail.tsx` — bottom sheet (Radix Dialog):
- Full agent state with icon + label
- Token breakdown (input, output, cache)
- Cost breakdown
- Sub-agent pills (reuse)
- Progress items list (reuse)
- Swipe-down to dismiss

**Step 6: Verify**

Run: `bun run typecheck`
Manual test: open `/mobile` on phone browser, scan QR from desktop.

**Step 7: Commit**

```
feat(frontend): QR scanner, mobile session list, and detail bottom sheet
```

---

## Task 12: PWA Manifest + Vite Config

**Files:**
- Create: `public/manifest.json`
- Create: `public/icon-192.png`
- Create: `public/icon-512.png`
- Modify: `index.html`
- Modify: `vite.config.ts`

**Step 1: Create PWA manifest**

```json
{
  "name": "Claude View",
  "short_name": "Claude",
  "start_url": "/mobile",
  "display": "standalone",
  "background_color": "#0F172A",
  "theme_color": "#0F172A",
  "icons": [
    { "src": "/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/icon-512.png", "sizes": "512x512", "type": "image/png" }
  ]
}
```

**Step 2: Add manifest link to index.html**

```html
<link rel="manifest" href="/manifest.json" />
<meta name="theme-color" content="#0F172A" />
<meta name="apple-mobile-web-app-capable" content="yes" />
```

**Step 3: Update Vite proxy for relay**

Add relay proxy to `vite.config.ts`:

```typescript
'/relay': {
  target: 'http://localhost:47893',
  ws: true,
},
```

**Step 4: Generate placeholder icons**

Create simple placeholder PNG icons (192x192 and 512x512) — solid dark background with "CV" text. Can be replaced with real icons later.

**Step 5: Commit**

```
feat: PWA manifest and Vite relay proxy configuration
```

---

## Task 13: End-to-End Integration Test

**Files:**
- No new files — manual verification

**Step 1: Start relay server**

Run: `cargo run -p claude-view-relay`
Expected: "Relay server listening on 0.0.0.0:47893"

**Step 2: Start main server**

Run: `cargo run -p claude-view-server`
Expected: "LiveSessionManager started with 4 background tasks"
Expected: "relay client starting" in logs

**Step 3: Start frontend dev**

Run: `bun dev`

**Step 4: Test desktop pairing panel**

- Open `http://localhost:5173`
- Click phone icon in header
- QR code should appear
- Paired devices list should be empty

**Step 5: Test mobile route**

- Open `http://localhost:5173/mobile` on phone (same WiFi)
- Should see pairing page with "Scan QR Code" button
- Scan QR from desktop → should pair and navigate to monitor view

**Step 6: Test session forwarding**

- Open a Claude Code session in terminal
- Desktop Mission Control should show the session
- Phone should receive encrypted updates and show the session card

**Step 7: Commit (if any fixes needed)**

```
fix: integration fixes for end-to-end mobile relay pipeline
```

---

## Task Summary

| # | Task | Crate | Scope |
|---|------|-------|-------|
| 1 | Relay server scaffold | `crates/relay/` | New crate, state types, health endpoint |
| 2 | Relay WebSocket handler | `crates/relay/` | WS auth, message forwarding |
| 3 | Relay pairing endpoints | `crates/relay/` | POST /pair, /pair/claim, TTL cleanup |
| 4 | Relay tests | `crates/relay/` | Integration tests (health, pair, claim, expiry) |
| 5 | Crypto module | `crates/server/` | NaCl box, Ed25519, Keychain |
| 6 | Relay WSS client | `crates/server/` | Connect, auth, encrypt, forward |
| 7 | Desktop pairing routes | `crates/server/` | QR generation, device list, unpair |
| 8 | Mobile route + stubs | Frontend | /mobile route, layout, page stubs |
| 9 | Desktop QR panel | Frontend | Header icon, Radix popover, device list |
| 10 | Mobile relay hook | Frontend | WebSocket, NaCl decrypt, IndexedDB |
| 11 | QR scanner + monitor UI | Frontend | Camera, jsQR, session cards, detail sheet |
| 12 | PWA manifest + config | Frontend | manifest.json, icons, Vite proxy |
| 13 | E2E integration test | Full stack | Manual verification of complete pipeline |

---

## Dependencies Between Tasks

```
Task 1 → Task 2 → Task 3 → Task 4
Task 5 → Task 6 → Task 7
Task 8 (independent)
Task 9 (depends on Task 7 for API)
Task 10 (depends on Task 5 concepts)
Task 11 (depends on Task 8 + Task 10)
Task 12 (depends on Task 8)
Task 13 (depends on all)
```

**Parallelizable:**
- Tasks 1-4 (relay) and Task 5 (crypto) can run in parallel
- Task 8 (mobile stubs) can start anytime
- Tasks 9 (desktop panel) and 10 (mobile hook) can run in parallel after their deps
