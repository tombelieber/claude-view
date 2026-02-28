# M1 Phase A: Fix the 3 Pairing Bugs

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the broken pairing flow so Mac and phone can connect end-to-end.

**Architecture:** Phone sends X25519 pubkey in clear → relay forwards in pair_complete → Mac always connected, receives and stores pubkey in Keychain → sessions start flowing.

**Tech Stack:** Rust/Axum (relay + server), TypeScript (mobile pages), NaCl box (crypto)

**Test locally with:** `cargo run -p claude-view-relay` + `bun run dev`

**Parent epic:** [PROGRESS.md](./PROGRESS.md)
**Design doc:** [design.md](./design.md)
**Bug analysis:** [analysis-pairing-bugs.md](./analysis-pairing-bugs.md)

---

### Task 1: Add `x25519_pubkey` field to relay ClaimRequest

**Why:** The phone encrypts its X25519 pubkey inside a NaCl box, but the Mac needs the phone's X25519 pubkey to *decrypt* it — circular dependency. Fix: send the pubkey in the clear alongside the encrypted blob. Public keys are public by definition.

**Files:**
- Modify: `crates/relay/src/pairing.rs:18-26` (ClaimRequest struct)
- Modify: `crates/relay/src/pairing.rs:116-124` (pair_complete forwarding)

**Step 1: Add `x25519_pubkey` to ClaimRequest**

In `crates/relay/src/pairing.rs`, the ClaimRequest struct (line 18-26) currently has:

```rust
#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    pub pubkey_encrypted_blob: String,
}
```

Add optional `x25519_pubkey` field:

```rust
#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    pub pubkey_encrypted_blob: String,
    #[serde(default)]
    pub x25519_pubkey: Option<String>,
}
```

**Step 2: Forward `x25519_pubkey` in pair_complete message**

In the same file, the pair_complete JSON (around line 116-124) currently sends:

```rust
let msg = serde_json::json!({
    "type": "pair_complete",
    "device_id": claim.device_id,
    "pubkey_encrypted_blob": claim.pubkey_encrypted_blob,
});
```

Add the x25519_pubkey field:

```rust
let msg = serde_json::json!({
    "type": "pair_complete",
    "device_id": claim.device_id,
    "pubkey_encrypted_blob": claim.pubkey_encrypted_blob,
    "x25519_pubkey": claim.x25519_pubkey,
});
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-relay`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/relay/src/pairing.rs
git commit -m "feat(relay): add x25519_pubkey field to ClaimRequest and pair_complete"
```

---

### Task 2: Send `x25519_pubkey` from phone side

**Why:** Both the static mobile.html and the React MobilePairingPage need to include the phone's X25519 public key in the claim POST body.

**Files:**
- Modify: `crates/relay/static/mobile.html:240-260` (claim POST body)
- Modify: `src/pages/MobilePairingPage.tsx:74-83` (claim POST body)

**Step 1: Update mobile.html claim POST**

In `crates/relay/static/mobile.html`, the claim POST body (around line 248-253) currently sends:

```javascript
body: JSON.stringify({
    one_time_token: token,
    device_id: deviceId,
    pubkey: naclUtil.encodeBase64(signKp.publicKey),
    pubkey_encrypted_blob: naclUtil.encodeBase64(encrypted),
})
```

Add `x25519_pubkey`:

```javascript
body: JSON.stringify({
    one_time_token: token,
    device_id: deviceId,
    pubkey: naclUtil.encodeBase64(signKp.publicKey),
    pubkey_encrypted_blob: naclUtil.encodeBase64(encrypted),
    x25519_pubkey: naclUtil.encodeBase64(encKp.publicKey),
})
```

**Step 2: Update MobilePairingPage.tsx claim POST**

In `src/pages/MobilePairingPage.tsx`, the claim POST body (around line 74-83) currently sends:

```typescript
body: JSON.stringify({
    one_time_token: payload.t,
    device_id: deviceId,
    pubkey: naclUtil.encodeBase64(signingPublicKey),
    pubkey_encrypted_blob: encryptedBlob,
})
```

Add `x25519_pubkey`:

```typescript
body: JSON.stringify({
    one_time_token: payload.t,
    device_id: deviceId,
    pubkey: naclUtil.encodeBase64(signingPublicKey),
    pubkey_encrypted_blob: encryptedBlob,
    x25519_pubkey: naclUtil.encodeBase64(encryptionPublicKey),
})
```

Note: `encryptionPublicKey` is already available — it's the return value from `generatePhoneKeys()` called on line 52.

**Step 3: Verify frontend compiles**

Run: `bun run typecheck`
Expected: No errors.

**Step 4: Commit**

```bash
git add crates/relay/static/mobile.html src/pages/MobilePairingPage.tsx
git commit -m "feat(mobile): send x25519_pubkey in clear with pair claim request"
```

---

### Task 3: Always connect relay_client to relay

**Why:** The relay_client only connects when paired devices exist. But `pair_complete` arrives via WebSocket — so the Mac must be connected *before* any devices are paired. Chicken-and-egg.

**Files:**
- Modify: `crates/server/src/live/relay_client.rs:55-87` (main loop)
- Modify: `crates/server/src/live/relay_client.rs:89-147` (connect_and_stream initial snapshot)

**Step 1: Remove the "skip if no devices" guard**

In `relay_client.rs`, the main loop (lines 55-87) currently has:

```rust
loop {
    let paired_devices = load_paired_devices();
    if paired_devices.is_empty() {
        tokio::time::sleep(Duration::from_secs(10)).await;
        continue;
    }
    match connect_and_stream(&identity, &paired_devices, &tx, &sessions, &relay_url, &config).await {
        // ...
    }
    tokio::time::sleep(backoff).await;
    backoff = std::cmp::min(backoff * 2, config.max_reconnect_delay);
}
```

Replace with always-connect:

```rust
loop {
    let paired_devices = load_paired_devices();
    // Always connect — must receive pair_complete even with 0 devices
    match connect_and_stream(&identity, &paired_devices, &tx, &sessions, &relay_url, &config).await {
        Ok(()) => {
            info!("relay connection closed cleanly, reconnecting");
            backoff = Duration::from_secs(1);
        }
        Err(e) => {
            warn!(error = %e, "relay connection failed");
        }
    }
    tokio::time::sleep(backoff).await;
    backoff = std::cmp::min(backoff * 2, config.max_reconnect_delay);
}
```

**Step 2: Skip initial snapshot when no paired devices**

In `connect_and_stream()`, the initial snapshot (around lines 131-147) sends all sessions to all paired devices on connect. Guard this:

```rust
if !paired_devices.is_empty() {
    // Send initial snapshot of all current sessions
    // ... existing snapshot code ...
}
```

When connected with 0 devices, the client just listens for incoming messages (like `pair_complete`) without trying to encrypt/send anything.

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/server/src/live/relay_client.rs
git commit -m "fix(relay-client): always connect to relay, even with 0 paired devices"
```

---

### Task 4: Implement `pair_complete` handler

**Why:** When the phone claims pairing, the relay sends `pair_complete` to the Mac via WebSocket. Currently this is a TODO stub. Need to extract the phone's X25519 pubkey and store it in Keychain.

**Files:**
- Modify: `crates/server/src/live/relay_client.rs:218-221` (pair_complete handler)

**Step 1: Add `add_paired_device` to imports**

At the top of `relay_client.rs`, find the import from `crate::crypto` (around line 5-10) and ensure `add_paired_device` is included:

```rust
use crate::crypto::{
    add_paired_device, box_secret_key, encrypt_for_device, load_or_create_identity,
    load_paired_devices, sign_auth_challenge, DeviceIdentity, PairedDevice,
};
```

**Step 2: Replace the TODO stub**

The current stub (lines 218-221):

```rust
if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
    info!("received pair_complete from relay");
    // TODO: decrypt phone pubkey and store in Keychain
}
```

Replace with:

```rust
if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
    let device_id = val.get("device_id").and_then(|v| v.as_str()).unwrap_or_default();
    let x25519_pubkey = val.get("x25519_pubkey").and_then(|v| v.as_str()).unwrap_or_default();
    if !device_id.is_empty() && !x25519_pubkey.is_empty() {
        let device = PairedDevice {
            device_id: device_id.to_string(),
            x25519_pubkey: x25519_pubkey.to_string(),
            name: device_id.to_string(),
            paired_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        match add_paired_device(device) {
            Ok(()) => {
                info!(device_id, "paired device stored in Keychain");
                // Return Ok to reconnect with updated paired devices list
                return Ok(());
            }
            Err(e) => {
                error!(device_id, error = %e, "failed to store paired device");
            }
        }
    } else {
        warn!("pair_complete missing device_id or x25519_pubkey");
    }
}
```

The `return Ok(())` is key — it exits `connect_and_stream`, which causes the main loop to reconnect. On reconnect, `load_paired_devices()` will include the new device, and session data will start flowing.

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/server/src/live/relay_client.rs
git commit -m "feat(relay-client): implement pair_complete handler, store phone pubkey in Keychain"
```

---

### Task 5: Redeploy relay to Fly.io

**Why:** Tasks 1-2 changed relay code. Need to deploy for the static mobile.html and ClaimRequest changes to take effect.

**Step 1: Deploy from workspace root**

```bash
fly deploy --config crates/relay/fly.toml
```

**Important:** Must deploy from workspace root, not from `crates/relay/`. The Dockerfile copies workspace-level Cargo.toml.

**Step 2: Verify health**

```bash
curl https://claude-view-relay.fly.dev/health
```

Expected: `ok`

**Step 3: Verify mobile page loads**

```bash
curl -s https://claude-view-relay.fly.dev/mobile | head -5
```

Expected: HTML content with `<title>` tag.

---

### Task 6: Local end-to-end test

**Why:** Verify the 3 bug fixes work together before adding auth.

**Prerequisites:**
- Relay deployed (Task 5) OR running locally (`cargo run -p claude-view-relay`)
- Mac running `bun run dev` with `RELAY_URL=wss://claude-view-relay.fly.dev/ws` in `.env`

**Step 1: Verify Mac relay_client connects**

Check Mac server logs for:
```
INFO relay_client: connected to relay
INFO relay_client: auth_ok received
```

This confirms Task 3 (always-connect) works.

**Step 2: Open PairingPanel and scan QR**

1. Open `localhost:5173` in browser
2. Click phone icon → QR code appears
3. Scan QR with phone camera → opens relay mobile page
4. Phone generates keys, claims pairing

**Step 3: Verify pairing completes on Mac**

Check Mac server logs for:
```
INFO relay_client: paired device stored in Keychain, device_id=phone-XXXXXXXX
```

This confirms Tasks 1, 2, 4 all work together.

**Step 4: Verify sessions flow**

1. Start a Claude Code session on Mac
2. Check phone — should show session card with project name, status, cost
3. Session updates should appear in real-time

**Step 5: Commit any fixes needed**

If any issues found, fix and recommit before proceeding to Phase B.

---

## Files Changed

| File | Action | Task |
|------|--------|------|
| `crates/relay/src/pairing.rs` | Modify | 1 |
| `crates/relay/static/mobile.html` | Modify | 2 |
| `src/pages/MobilePairingPage.tsx` | Modify | 2 |
| `crates/server/src/live/relay_client.rs` | Modify | 3, 4 |

## Task Dependencies

```
Task 1 (relay ClaimRequest) ──┐
                               ├── Task 4 (pair_complete) ── Task 5 (deploy) ── Task 6 (E2E)
Task 2 (phone sends pubkey) ──┘                                                     │
                                                                                     │
Task 3 (always-connect) ────────────────────────────────────────────────────────────┘
```

Tasks 1+2+3 can be done simultaneously.
