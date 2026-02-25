# Mobile Remote M1 — TODO (pick up from here)

**Date:** 2026-02-23
**Branch:** `worktree-mobile-remote`
**Last commit:** `c00a5538` — feat: QR encodes URL, self-contained mobile page on relay, fix claim API

## What Works

- Relay deployed to Fly.io: `https://claude-view-relay.fly.dev`
  - `/health` → 200 ok
  - `/mobile` → 200 (self-contained pairing + monitor page)
  - `/ws` → WebSocket endpoint
  - `/pair` / `/pair/claim` → pairing API
- QR code in PairingPanel encodes a URL: `https://claude-view-relay.fly.dev/mobile?k=<pubkey>&t=<token>`
- Phone scans QR → opens mobile page → generates keypairs → claims pairing → **pairing succeeds**
- Phone connects to relay WSS and authenticates (gets `auth_ok`)
- .env + dotenvy config for RELAY_URL (no hardcoded URLs)
- WSS TLS works (rustls-tls-webpki-roots)

## What's Broken: 0 Sessions Shown

Phone pairs and connects but sees 0 sessions. Three root causes:

### 1. Mac relay_client never connects (chicken-and-egg)

**File:** `crates/server/src/live/relay_client.rs:65-71`

```rust
loop {
    let paired_devices = load_paired_devices();
    if paired_devices.is_empty() {
        // ← STUCK HERE: sleeps forever because no paired device stored
        tokio::time::sleep(Duration::from_secs(10)).await;
        continue;
    }
    // Never reaches connect_and_stream()
}
```

The relay_client only connects to the relay when paired devices exist in Keychain. But paired devices only get stored when `pair_complete` is received via WS. The Mac is never connected to WS, so it never receives `pair_complete`.

**Fix:** Always connect to relay when `RELAY_URL` is set. Even with 0 paired devices, the Mac must be online to receive `pair_complete`. When no devices are paired, skip sending session data but still listen for incoming messages.

### 2. `pair_complete` handler is a TODO

**File:** `crates/server/src/live/relay_client.rs:218-221`

```rust
if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
    info!("received pair_complete from relay");
    // TODO: decrypt phone pubkey and store in Keychain ← NEVER IMPLEMENTED
}
```

When the phone claims pairing, the relay sends `pair_complete` to the Mac via WS with `device_id` and `pubkey_encrypted_blob`. The Mac needs to:
1. Extract the phone's X25519 public key
2. Store it as a `PairedDevice` in macOS Keychain via `add_paired_device()`
3. Reconnect to reload paired devices list and start forwarding session data

**But there's a sub-problem (see #3).**

### 3. Circular crypto dependency in `pubkey_encrypted_blob`

The phone encrypts its X25519 pubkey using NaCl box:
```
encrypt(phone_x25519_pubkey, nonce, mac_x25519_pubkey, phone_x25519_secret)
```

To decrypt, the Mac needs:
```
decrypt(ciphertext, nonce, phone_x25519_pubkey, mac_x25519_secret)
```

**But the Mac doesn't have `phone_x25519_pubkey` yet** — that's the thing being encrypted! NaCl box requires the sender's public key to compute the shared secret for decryption. Circular dependency.

**Fix options (pick one):**

**A) Send phone X25519 pubkey in the clear (recommended for M1)**
- Add `x25519_pubkey` field to relay's ClaimRequest
- Forward it in `pair_complete` message to Mac
- Mac stores it directly as PairedDevice.x25519_pubkey
- The encrypted blob becomes optional proof-of-possession
- Security is fine: X25519 pubkeys are public by definition, relay already knows device IDs

**B) Use NaCl sealedbox (anonymous sender)**
- `crypto_box_seal(plaintext, recipient_public)` — sender is anonymous
- `crypto_box_seal_open(ciphertext, recipient_public, recipient_secret)` — no sender pubkey needed
- Requires adding `crypto_secretbox` or sealed box support to tweetnacl
- More complex, no real security benefit for M1

**C) Pre-exchange via relay HTTP (two-step claim)**
- Phone POSTs X25519 pubkey first, gets Mac's X25519 pubkey back
- Then encrypts with shared secret
- Over-engineered for M1

**Recommendation: Option A.** Public keys are public. Save the complexity for later.

## Implementation Plan

### Task 1: Add `x25519_pubkey` to relay claim API

**Files:**
- `crates/relay/src/pairing.rs` — Add `x25519_pubkey: String` to ClaimRequest, forward in pair_complete
- `crates/relay/static/mobile.html` — Send `x25519_pubkey: b64Enc(encKp.publicKey)` in claim POST
- `src/pages/MobilePairingPage.tsx` — Send `x25519_pubkey: naclUtil.encodeBase64(encryptionPublicKey)` in claim POST

### Task 2: Always connect relay_client to relay

**File:** `crates/server/src/live/relay_client.rs`

Change the main loop:
```rust
loop {
    // Always connect — must receive pair_complete even with 0 devices
    let paired_devices = load_paired_devices();
    match connect_and_stream(&identity, &paired_devices, ...).await {
        Ok(()) => { backoff = 1s; }
        Err(e) => { warn!(...); }
    }
    sleep(backoff);
    backoff = min(backoff * 2, max);
}
```

In `connect_and_stream`, skip initial snapshot and event forwarding when `paired_devices.is_empty()`.

### Task 3: Implement `pair_complete` handler

**File:** `crates/server/src/live/relay_client.rs`

Replace the TODO:
```rust
if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
    let device_id = val["device_id"].as_str().unwrap_or_default();
    let x25519_pubkey = val["x25519_pubkey"].as_str().unwrap_or_default();
    if !device_id.is_empty() && !x25519_pubkey.is_empty() {
        let device = PairedDevice {
            device_id: device_id.to_string(),
            x25519_pubkey: x25519_pubkey.to_string(),
            name: device_id.to_string(),
            paired_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        };
        if let Err(e) = add_paired_device(device) {
            error!("failed to store paired device: {e}");
        } else {
            info!(device_id, "paired device stored in Keychain");
            // Return to reconnect with updated device list
            return Ok(());
        }
    }
}
```

Need to add `add_paired_device` to the import list from `crate::crypto`.

### Task 4: Redeploy relay to Fly.io

```bash
fly deploy --config crates/relay/fly.toml
```

**Important:** Deploy from workspace root, not from `crates/relay/`.

### Task 5: End-to-end test

1. `bun run dev` on Mac
2. Open PairingPanel in browser → QR code appears
3. Scan QR with phone camera → opens relay mobile page
4. Phone auto-pairs → Mac logs "paired device stored in Keychain"
5. Start a Claude Code session on Mac
6. Phone shows the session in real-time

### Task 6 (if needed): Verify relay WS message routing

The relay only forwards messages between paired devices (ws.rs:99-103). After pairing, verify the relay's in-memory `devices` map correctly tracks the Mac-phone pairing so encrypted session envelopes are forwarded.

## Deploy Notes

- Always deploy relay from **workspace root**: `fly deploy --config crates/relay/fly.toml`
- The Dockerfile needs workspace-level Cargo.toml + all crate manifests for workspace resolution
- Relay is at `https://claude-view-relay.fly.dev` (app: claude-view-relay, region: nrt)
- Machines auto-stop when idle, auto-start on request

## Key File Map

| File | What it does |
|------|-------------|
| `crates/relay/src/lib.rs` | Relay router: /health, /mobile, /ws, /pair, /pair/claim |
| `crates/relay/src/pairing.rs` | ClaimRequest struct + pair_complete forwarding |
| `crates/relay/src/ws.rs` | WS auth, message routing between paired devices |
| `crates/relay/static/mobile.html` | Self-contained phone page (pairing + monitor) |
| `crates/server/src/live/relay_client.rs` | Mac's WSS client — connects to relay, forwards encrypted sessions |
| `crates/server/src/crypto.rs` | NaCl box encrypt, Ed25519 sign, Keychain storage |
| `crates/server/src/routes/pairing.rs` | Desktop API: QR generation, device list, unpair |
| `src/components/PairingPanel.tsx` | Desktop UI: QR popover |
| `src/pages/MobilePairingPage.tsx` | React mobile pairing (camera QR scanner) |
| `src/hooks/use-pairing.ts` | React Query hooks for pairing API |
| `.env` | `RELAY_URL=wss://claude-view-relay.fly.dev/ws` |
