# Mobile M1 — Implementation Plan (revised 2026-02-26)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship an Expo native app that pairs with Mac via QR and shows a real-time live dashboard of AI agent sessions.

**Architecture:** Monorepo already restructured (`apps/web`, `apps/mobile`, `packages/shared`). Expo/React Native + Tamagui v2, keypair auth, dumb relay, relay protocol types in shared package.

**Tech Stack:** Expo SDK 55, React Native 0.83, Tamagui v2, Turborepo, Bun workspaces, tweetnacl, Axum relay

**Design doc:** `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-design.md`

**What's already done:**
- Monorepo structure: `apps/web`, `apps/mobile`, `apps/landing`, `packages/shared`, `packages/design-tokens`
- Expo SDK 55 scaffold with Tamagui v2, Expo Router, tab navigation
- `packages/shared` with relay protocol types (`RelaySession`, `RelaySessionSnapshot`, etc.)
- `packages/design-tokens` with colors, spacing, typography
- `crates/relay/` with pairing, WebSocket auth, message forwarding
- `crates/server/src/crypto.rs` with device identity, paired device storage, NaCl box encryption
- `crates/server/src/live/relay_client.rs` with WebSocket relay streaming

---

## Phase 1: Shared Package & Types

### Task 1: Extend `packages/shared/` with crypto, relay hook, and utility functions

**Context:** `packages/shared/` currently only exports relay TypeScript types and a theme placeholder. We need to add the crypto module (NaCl keypair management, encryption, signing), a relay WebSocket hook, and shared utility functions that both the web and mobile apps will use.

**Files:**
- Modify: `packages/shared/package.json` (add dependencies)
- Modify: `packages/shared/tsconfig.json` (if needed)
- Create: `packages/shared/src/crypto/nacl.ts`
- Create: `packages/shared/src/crypto/storage.ts`
- Create: `packages/shared/src/relay/use-relay-connection.ts`
- Create: `packages/shared/src/utils/format-cost.ts`
- Create: `packages/shared/src/utils/format-duration.ts`
- Create: `packages/shared/src/utils/group-sessions.ts`
- Modify: `packages/shared/src/index.ts` (add new exports)

**Step 1: Add dependencies to `packages/shared/package.json`**

```json
{
  "name": "@claude-view/shared",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "dependencies": {
    "tweetnacl": "^1.0.3",
    "tweetnacl-util": "^0.15.1"
  }
}
```

```bash
cd packages/shared && bun install
```

**Step 2: Create abstract key storage interface**

```ts
// packages/shared/src/crypto/storage.ts

/** Abstract key storage — web uses IndexedDB, mobile uses expo-secure-store */
export interface KeyStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
}
```

**Step 3: Create NaCl crypto module**

```ts
// packages/shared/src/crypto/nacl.ts
import nacl from 'tweetnacl';
import { decodeBase64, encodeBase64, decodeUTF8 } from 'tweetnacl-util';
import type { KeyStorage } from './storage';

const SIGNING_KEY = 'device_signing_key';
const ENCRYPTION_KEY = 'device_encryption_key';
const DEVICE_ID_KEY = 'device_id';

export interface PhoneKeys {
  signingKeyPair: nacl.SignKeyPair;
  boxKeyPair: nacl.BoxKeyPair;
  deviceId: string;
}

/** Generate and store phone keypair in secure storage. */
export async function generatePhoneKeys(storage: KeyStorage): Promise<PhoneKeys> {
  const signingKeyPair = nacl.sign.keyPair();
  const boxKeyPair = nacl.box.keyPair();
  const deviceId = `phone-${crypto.randomUUID().slice(0, 8)}`;

  await storage.setItem(SIGNING_KEY, encodeBase64(signingKeyPair.secretKey));
  await storage.setItem(ENCRYPTION_KEY, encodeBase64(boxKeyPair.secretKey));
  await storage.setItem(DEVICE_ID_KEY, deviceId);

  return { signingKeyPair, boxKeyPair, deviceId };
}

/** Load existing keys from storage, or null if not paired. */
export async function loadPhoneKeys(storage: KeyStorage): Promise<PhoneKeys | null> {
  const [signingB64, encryptionB64, deviceId] = await Promise.all([
    storage.getItem(SIGNING_KEY),
    storage.getItem(ENCRYPTION_KEY),
    storage.getItem(DEVICE_ID_KEY),
  ]);
  if (!signingB64 || !encryptionB64 || !deviceId) return null;

  const signingSecret = decodeBase64(signingB64);
  const boxSecret = decodeBase64(encryptionB64);
  return {
    signingKeyPair: nacl.sign.keyPair.fromSecretKey(signingSecret),
    boxKeyPair: nacl.box.keyPair.fromSecretKey(boxSecret),
    deviceId,
  };
}

/** Sign auth challenge: "timestamp:device_id" */
export function signAuthChallenge(
  deviceId: string,
  signingSecretKey: Uint8Array,
): { timestamp: number; signature: string } {
  const timestamp = Math.floor(Date.now() / 1000);
  const payload = `${timestamp}:${deviceId}`;
  const signature = nacl.sign.detached(decodeUTF8(payload), signingSecretKey);
  return { timestamp, signature: encodeBase64(signature) };
}

/** Decrypt a NaCl box message (nonce || ciphertext) from Mac. */
export function decryptFromDevice(
  encryptedB64: string,
  senderPubkey: Uint8Array,
  recipientSecretKey: Uint8Array,
): Uint8Array | null {
  const wire = decodeBase64(encryptedB64);
  const nonce = wire.slice(0, nacl.box.nonceLength);
  const ciphertext = wire.slice(nacl.box.nonceLength);
  return nacl.box.open(ciphertext, nonce, senderPubkey, recipientSecretKey);
}

/** Encrypt phone pubkey for Mac using NaCl box. */
export function encryptForDevice(
  plaintext: Uint8Array,
  recipientPubkey: Uint8Array,
  senderSecretKey: Uint8Array,
): string {
  const nonce = nacl.randomBytes(nacl.box.nonceLength);
  const ciphertext = nacl.box(plaintext, nonce, recipientPubkey, senderSecretKey);
  const wire = new Uint8Array(nonce.length + ciphertext.length);
  wire.set(nonce);
  wire.set(ciphertext, nonce.length);
  return encodeBase64(wire);
}

export interface ClaimPairingParams {
  macPubkeyB64: string;
  token: string;
  relayUrl: string;
  keys: PhoneKeys;
  storage: KeyStorage;
}

/** Claim a pairing offer at the relay. */
export async function claimPairing(params: ClaimPairingParams): Promise<void> {
  const { macPubkeyB64, token, relayUrl, keys, storage } = params;
  const macPubkey = decodeBase64(macPubkeyB64);

  // Encrypt phone's X25519 pubkey for Mac
  const encryptedBlob = encryptForDevice(keys.boxKeyPair.publicKey, macPubkey, keys.boxKeyPair.secretKey);

  // HTTP base URL (relay WS URL -> HTTPS)
  const httpUrl = relayUrl.replace('wss://', 'https://').replace('/ws', '');

  const res = await fetch(`${httpUrl}/pair/claim`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      one_time_token: token,
      device_id: keys.deviceId,
      pubkey: encodeBase64(keys.signingKeyPair.publicKey),
      pubkey_encrypted_blob: encryptedBlob,
      x25519_pubkey: encodeBase64(keys.boxKeyPair.publicKey),
    }),
  });

  if (!res.ok) {
    const status = res.status;
    if (status === 404) throw new Error('Pairing code not found. Try scanning again.');
    if (status === 410) throw new Error('Pairing code expired. Generate a new one on Mac.');
    throw new Error(`Pairing failed (${status})`);
  }

  // Store relay URL and Mac pubkey for future connections
  await storage.setItem('relay_url', relayUrl);
  await storage.setItem('mac_x25519_pubkey', macPubkeyB64);
  await storage.setItem('mac_device_id', ''); // Will be filled by pair_complete
}

/** Clear all pairing data from storage. */
export async function unpair(storage: KeyStorage): Promise<void> {
  await Promise.all([
    storage.removeItem(SIGNING_KEY),
    storage.removeItem(ENCRYPTION_KEY),
    storage.removeItem(DEVICE_ID_KEY),
    storage.removeItem('relay_url'),
    storage.removeItem('mac_x25519_pubkey'),
    storage.removeItem('mac_device_id'),
  ]);
}
```

**Step 4: Create relay WebSocket connection hook**

This is a React hook that connects to the relay via WebSocket, authenticates, and exposes a sessions map + connection state.

```ts
// packages/shared/src/relay/use-relay-connection.ts
import { useCallback, useEffect, useRef, useState } from 'react';
import { decodeBase64 } from 'tweetnacl-util';
import { decryptFromDevice, loadPhoneKeys, signAuthChallenge, type PhoneKeys } from '../crypto/nacl';
import type { KeyStorage } from '../crypto/storage';
import type { RelaySession } from '../types/relay';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected';

export interface UseRelayConnectionOptions {
  storage: KeyStorage;
}

export interface UseRelayConnectionResult {
  sessions: Record<string, RelaySession>;
  connectionState: ConnectionState;
  disconnect: () => void;
}

export function useRelayConnection(opts: UseRelayConnectionOptions): UseRelayConnectionResult {
  const { storage } = opts;
  const [sessions, setSessions] = useState<Record<string, RelaySession>>({});
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const wsRef = useRef<WebSocket | null>(null);
  const keysRef = useRef<PhoneKeys | null>(null);
  const macPubkeyRef = useRef<Uint8Array | null>(null);

  const disconnect = useCallback(() => {
    wsRef.current?.close();
    wsRef.current = null;
    setConnectionState('disconnected');
  }, []);

  useEffect(() => {
    let cancelled = false;
    let reconnectTimer: ReturnType<typeof setTimeout>;

    async function connect() {
      const keys = await loadPhoneKeys(storage);
      if (!keys || cancelled) return;
      keysRef.current = keys;

      const relayUrl = await storage.getItem('relay_url');
      const macPubkeyB64 = await storage.getItem('mac_x25519_pubkey');
      if (!relayUrl || !macPubkeyB64 || cancelled) return;
      macPubkeyRef.current = decodeBase64(macPubkeyB64);

      setConnectionState('connecting');
      const ws = new WebSocket(relayUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        if (cancelled || wsRef.current !== ws) return;
        // Authenticate
        const { timestamp, signature } = signAuthChallenge(
          keys.deviceId,
          keys.signingKeyPair.secretKey,
        );
        ws.send(JSON.stringify({
          type: 'auth',
          device_id: keys.deviceId,
          timestamp,
          signature,
        }));
      };

      ws.onmessage = (event) => {
        if (cancelled || wsRef.current !== ws) return;
        try {
          const data = JSON.parse(event.data as string);

          // Auth response
          if (data.type === 'auth_ok') {
            setConnectionState('connected');
            return;
          }
          if (data.type === 'error') {
            console.error('Relay auth error:', data.message);
            ws.close();
            return;
          }

          // Encrypted message from Mac
          if (data.payload && macPubkeyRef.current && keysRef.current) {
            const decrypted = decryptFromDevice(
              data.payload,
              macPubkeyRef.current,
              keysRef.current.boxKeyPair.secretKey,
            );
            if (!decrypted) return;

            const text = new TextDecoder().decode(decrypted);
            const msg = JSON.parse(text);

            // Session update — merge into sessions map
            if (msg.id) {
              setSessions(prev => ({ ...prev, [msg.id]: msg }));
            }
            // Session completed — remove from map
            if (msg.type === 'session_completed' && msg.session_id) {
              setSessions(prev => {
                const next = { ...prev };
                delete next[msg.session_id];
                return next;
              });
            }
          }
        } catch {
          // Ignore parse errors
        }
      };

      ws.onclose = () => {
        if (cancelled) return;
        setConnectionState('disconnected');
        // Reconnect after backoff
        reconnectTimer = setTimeout(connect, 5000);
      };

      ws.onerror = () => {
        ws.close();
      };
    }

    connect();

    return () => {
      cancelled = true;
      clearTimeout(reconnectTimer);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [storage]);

  return { sessions, connectionState, disconnect };
}
```

**Step 5: Create utility functions**

```ts
// packages/shared/src/utils/format-cost.ts

/** Format a USD amount with 2 decimal places. */
export function formatUsd(usd: number): string {
  if (usd < 0.01) return '$0.00';
  return `$${usd.toFixed(2)}`;
}
```

```ts
// packages/shared/src/utils/format-duration.ts

/** Format seconds into human-readable duration. */
export function formatDuration(seconds: number): string {
  if (seconds < 0) return '0s';
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}
```

```ts
// packages/shared/src/utils/group-sessions.ts
import type { RelaySession } from '../types/relay';

/** Group sessions by whether they need user attention. */
export function groupByStatus(sessions: RelaySession[]): {
  needsYou: RelaySession[];
  autonomous: RelaySession[];
} {
  const needsYou: RelaySession[] = [];
  const autonomous: RelaySession[] = [];

  for (const s of sessions) {
    if (s.status === 'waiting') {
      needsYou.push(s);
    } else {
      autonomous.push(s);
    }
  }

  return { needsYou, autonomous };
}
```

**Step 6: Update barrel export**

```ts
// packages/shared/src/index.ts
export * from './types/relay';
export * from './theme';
export * from './crypto/nacl';
export * from './crypto/storage';
export { useRelayConnection, type ConnectionState, type UseRelayConnectionResult } from './relay/use-relay-connection';
export * from './utils/format-cost';
export * from './utils/format-duration';
export * from './utils/group-sessions';
```

**Step 7: Verify the shared package builds**

```bash
cd packages/shared && bun run tsc --noEmit
```

**Step 8: Commit**

```bash
git add packages/shared/
git commit -m "feat: extend packages/shared with crypto, relay hook, and utility functions"
```

---

### Task 2: Wire up ts-rs type generation

**Files:**
- Modify: `crates/core/Cargo.toml` (verify ts-rs dependency)
- Modify: relevant Rust struct files (add `#[derive(TS)]`)
- Create: `packages/shared/src/types/generated/` (output directory)
- Create: `scripts/generate-types.sh`

**Step 1: Verify ts-rs is in workspace dependencies**

Check root `Cargo.toml` for:
```toml
ts-rs = { version = "11", features = ["serde-compat"] }
```

If not present, add it. Then in `crates/core/Cargo.toml`:
```toml
[dependencies]
ts-rs = { workspace = true }
```

**Step 2: Add `#[derive(TS)]` to key structs**

Find these structs in `crates/core/src/` and `crates/server/src/live/`:

```rust
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../packages/shared/src/types/generated/")]
pub struct LiveSession { /* ... */ }

// Repeat for: SubAgentInfo, SubAgentStatus, ProgressItem
```

**Step 3: Create generation script**

```bash
#!/bin/bash
# scripts/generate-types.sh
set -euo pipefail
echo "Generating TypeScript types from Rust structs..."
cargo test -p claude-view-core export_bindings -- --nocapture 2>/dev/null || true
cargo test -p claude-view-server export_bindings -- --nocapture 2>/dev/null || true
echo "Types written to packages/shared/src/types/generated/"
ls packages/shared/src/types/generated/*.ts 2>/dev/null | head -20
```

**Step 4: Run and verify**

```bash
chmod +x scripts/generate-types.sh
./scripts/generate-types.sh
```

**Step 5: Create types barrel export**

```ts
// packages/shared/src/types/index.ts
export * from './relay';
// Add generated exports as they appear:
// export * from './generated/LiveSession';
```

**Step 6: Commit**

```bash
git add packages/shared/src/types/ scripts/generate-types.sh crates/
git commit -m "feat: wire up ts-rs for Rust→TypeScript type generation"
```

---

## Phase 2: Relay Fixes

### Task 3: Fix relay pairing bugs

Three known bugs in the relay and relay client.

**Bug 1:** `ClaimRequest` missing `x25519_pubkey` field — Mac can't learn Phone's encryption key.
**Bug 2:** Relay client early-returns when no paired devices, preventing it from receiving `pair_complete`.
**Bug 3:** Relay client `pair_complete` handler is a TODO stub.

**Files:**
- Modify: `crates/relay/src/pairing.rs` (add x25519_pubkey to ClaimRequest)
- Modify: `crates/server/src/live/relay_client.rs` (always-connect, pair_complete handler)
- Test: `crates/relay/tests/`

**Step 1: Add x25519_pubkey to ClaimRequest**

In `crates/relay/src/pairing.rs`, add field to `ClaimRequest`:

```rust
#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    #[serde(with = "base64_bytes")]
    pub pubkey: Vec<u8>,
    pub pubkey_encrypted_blob: String,
    pub x25519_pubkey: Option<String>,  // ADD THIS — base64 X25519 pubkey
}
```

In the `claim_pair` handler, include `x25519_pubkey` in the `pair_complete` JSON:

```rust
// Forward encrypted phone pubkey blob to Mac via WS (if connected)
if let Some(mac_conn) = state.connections.get(&offer.device_id) {
    let msg = serde_json::json!({
        "type": "pair_complete",
        "device_id": req.device_id,
        "pubkey_encrypted_blob": req.pubkey_encrypted_blob,
        "x25519_pubkey": req.x25519_pubkey,  // ADD THIS
    });
    let _ = mac_conn.tx.send(msg.to_string());
}
```

**Step 2: Fix relay client always-connect bug**

In `crates/server/src/live/relay_client.rs`, lines 66-71 have an early return when no paired devices exist. This prevents receiving `pair_complete` messages. Fix:

```rust
// BEFORE (lines 66-71):
let paired_devices = load_paired_devices();
if paired_devices.is_empty() {
    tokio::time::sleep(Duration::from_secs(10)).await;
    continue;
}

// AFTER:
let paired_devices = load_paired_devices();
// Always connect even with no paired devices — we need to receive pair_complete.
// Only SEND session data when there are paired devices.
```

Then in `connect_and_stream`, guard the session-sending code:

```rust
// Only send session snapshots if we have paired devices
if !paired_devices.is_empty() {
    let sessions_map = sessions.read().await;
    for session in sessions_map.values() {
        // ... existing send logic
    }
}
```

**Step 3: Implement pair_complete handler**

In `crates/server/src/live/relay_client.rs`, replace the TODO at line 231:

```rust
Some(Ok(Message::Text(text))) => {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
        if val.get("type").and_then(|t| t.as_str()) == Some("pair_complete") {
            info!("received pair_complete from relay");
            let device_id = val["device_id"].as_str().unwrap_or_default().to_string();

            // Decrypt phone's X25519 pubkey from the encrypted blob
            if let Some(blob) = val["pubkey_encrypted_blob"].as_str() {
                // The blob is encrypted with Mac's X25519 pubkey — decrypt it
                if let Ok(decrypted) = decrypt_phone_pubkey(blob, &box_secret) {
                    let x25519_pubkey = STANDARD.encode(&decrypted);
                    let name = format!("Phone ({})", &device_id[..device_id.len().min(8)]);
                    let device = PairedDevice {
                        device_id: device_id.clone(),
                        x25519_pubkey,
                        name,
                        paired_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };
                    if let Err(e) = add_paired_device(device) {
                        error!("Failed to store paired device: {e}");
                    } else {
                        info!("Paired with device: {device_id}");
                    }
                }
            } else if let Some(x25519_b64) = val["x25519_pubkey"].as_str() {
                // Fallback: unencrypted X25519 pubkey (for dev/testing)
                let name = format!("Phone ({})", &device_id[..device_id.len().min(8)]);
                let device = PairedDevice {
                    device_id: device_id.clone(),
                    x25519_pubkey: x25519_b64.to_string(),
                    name,
                    paired_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                if let Err(e) = add_paired_device(device) {
                    error!("Failed to store paired device: {e}");
                } else {
                    info!("Paired with device: {device_id}");
                }
            }
        }
    }
}
```

Helper function at the bottom of the file:

```rust
fn decrypt_phone_pubkey(blob_b64: &str, box_secret: &BoxSecretKey) -> Result<Vec<u8>, String> {
    let wire = STANDARD.decode(blob_b64).map_err(|e| format!("bad base64: {e}"))?;
    if wire.len() < 24 {
        return Err("blob too short".into());
    }
    let nonce = &wire[..24];
    let ciphertext = &wire[24..];
    // The phone encrypted its pubkey using OUR public key and ITS secret key.
    // We don't know the phone's pubkey yet — that's what we're decrypting.
    // This means the encrypted blob should actually be decryptable differently.
    // For M1, use the x25519_pubkey field directly (sent in plaintext as fallback).
    // Full encrypted exchange is M2 work.
    Err("encrypted blob decryption requires phone pubkey — use x25519_pubkey field".into())
}
```

> **Note:** For M1, the `x25519_pubkey` field sent in plaintext is sufficient since the relay uses TLS. Full NaCl-box-encrypted key exchange is M2 hardening work.

**Step 4: Run all relay tests**

```bash
cargo test -p claude-view-relay -- --nocapture
cargo test -p claude-view-server relay -- --nocapture
```

**Step 5: Commit**

```bash
git add crates/relay/ crates/server/src/live/relay_client.rs crates/server/src/crypto.rs
git commit -m "fix: relay pairing bugs — x25519_pubkey forwarding, always-connect, pair_complete handler"
```

---

## Phase 3: Expo App Screens

### Task 4: Install missing Expo dependencies

The scaffold exists but is missing dependencies for QR scanning, push notifications, haptics, and bottom sheet.

**Files:**
- Modify: `apps/mobile/package.json`

**Step 1: Install missing Expo packages**

```bash
cd apps/mobile
npx expo install expo-camera expo-notifications expo-haptics
bun add tweetnacl tweetnacl-util
```

**Step 2: Update `app.config.ts` plugins**

In `apps/mobile/app.config.ts`, add camera permission and notifications to the plugins array:

```ts
plugins: [
  'expo-router',
  'expo-secure-store',
  ['expo-camera', { cameraPermission: 'Allow Claude View to scan QR codes for pairing.' }],
  'expo-notifications',
],
```

**Step 3: Verify Expo starts**

```bash
cd apps/mobile && npx expo start
```

**Step 4: Commit**

```bash
git add apps/mobile/
git commit -m "feat: install expo-camera, expo-notifications, expo-haptics, tweetnacl"
```

---

### Task 5: Pair screen (QR scan)

**Files:**
- Create: `apps/mobile/app/pair.tsx`
- Create: `apps/mobile/hooks/use-pairing-status.ts`
- Create: `apps/mobile/lib/secure-store-adapter.ts`
- Modify: `apps/mobile/app/_layout.tsx` (add pair route)
- Modify: `apps/mobile/app/(tabs)/index.tsx` (redirect to pair if unpaired)

**Step 1: Create SecureStore adapter implementing KeyStorage interface**

```ts
// apps/mobile/lib/secure-store-adapter.ts
import * as SecureStore from 'expo-secure-store';
import type { KeyStorage } from '@claude-view/shared';

export const secureStoreAdapter: KeyStorage = {
  async getItem(key: string) {
    return SecureStore.getItemAsync(key);
  },
  async setItem(key: string, value: string) {
    await SecureStore.setItemAsync(key, value);
  },
  async removeItem(key: string) {
    await SecureStore.deleteItemAsync(key);
  },
};
```

**Step 2: Create pairing status hook**

```ts
// apps/mobile/hooks/use-pairing-status.ts
import { useState, useEffect, useCallback } from 'react';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export function usePairingStatus() {
  const [isPaired, setIsPaired] = useState<boolean | null>(null);

  const check = useCallback(async () => {
    const url = await secureStoreAdapter.getItem('relay_url');
    setIsPaired(url !== null);
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  return { isPaired, refresh: check };
}
```

**Step 3: Build Pair screen with Tamagui**

```tsx
// apps/mobile/app/pair.tsx
import { useState } from 'react';
import { CameraView, useCameraPermissions } from 'expo-camera';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { Button, Text, YStack } from 'tamagui';
import { generatePhoneKeys, claimPairing } from '@claude-view/shared';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export default function PairScreen() {
  const [permission, requestPermission] = useCameraPermissions();
  const [scanned, setScanned] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleBarCodeScanned = async ({ data }: { data: string }) => {
    if (scanned) return;
    setScanned(true);
    setError(null);
    await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);

    try {
      // QR payload is a URL with k (Mac X25519 pubkey), t (token), r (relay URL)
      const url = new URL(data);
      const macPubkeyB64 = url.searchParams.get('k');
      const token = url.searchParams.get('t');
      const relayUrl = url.searchParams.get('r');

      if (!macPubkeyB64 || !token || !relayUrl) throw new Error('Invalid QR code');

      const keys = await generatePhoneKeys(secureStoreAdapter);

      await claimPairing({
        macPubkeyB64,
        token,
        relayUrl,
        keys,
        storage: secureStoreAdapter,
      });

      router.replace('/(tabs)');
    } catch (e) {
      setScanned(false);
      setError(e instanceof Error ? e.message : 'Pairing failed');
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    }
  };

  if (!permission?.granted) {
    return (
      <YStack flex={1} backgroundColor="$gray900" alignItems="center" justifyContent="center" padding="$8">
        <Text color="$gray50" fontSize="$lg" textAlign="center" marginBottom="$6">
          Camera access needed to scan QR code
        </Text>
        <Button onPress={requestPermission} backgroundColor="$primary600" color="$gray50" size="$5">
          Grant Camera Access
        </Button>
      </YStack>
    );
  }

  return (
    <YStack flex={1} backgroundColor="$gray900">
      <CameraView
        style={{ flex: 1 }}
        barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
        onBarcodeScanned={scanned ? undefined : handleBarCodeScanned}
      />
      <YStack
        position="absolute"
        bottom={0}
        left={0}
        right={0}
        padding="$8"
        alignItems="center"
      >
        <Text color="$gray50" fontSize="$lg" textAlign="center">
          Scan the QR code from your Mac's Claude View
        </Text>
        <Text color="$gray400" fontSize="$sm" marginTop="$2" textAlign="center">
          One scan. No account. No password. Ever.
        </Text>
        {error && (
          <Text color="#ef4444" fontSize="$sm" marginTop="$4" textAlign="center">
            {error}
          </Text>
        )}
      </YStack>
    </YStack>
  );
}
```

**Step 4: Update root layout to include pair route**

In `apps/mobile/app/_layout.tsx`, update the Stack to include the pair screen:

```tsx
<Stack>
  <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
  <Stack.Screen name="pair" options={{ headerShown: false, presentation: 'fullScreenModal' }} />
</Stack>
```

**Step 5: Update tab index to redirect if unpaired**

```tsx
// apps/mobile/app/(tabs)/index.tsx
import { Redirect } from 'expo-router';
import { H1, Spinner, Text, YStack } from 'tamagui';
import { usePairingStatus } from '../../hooks/use-pairing-status';

export default function SessionsScreen() {
  const { isPaired } = usePairingStatus();

  // Loading state
  if (isPaired === null) {
    return (
      <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
        <Spinner size="large" />
      </YStack>
    );
  }

  // Not paired — redirect to pair screen
  if (!isPaired) {
    return <Redirect href="/pair" />;
  }

  // Paired — show dashboard (next task)
  return (
    <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
      <H1>Claude View Mobile</H1>
      <Text color="$colorSubtle" marginTop="$2">
        Session monitoring coming soon
      </Text>
    </YStack>
  );
}
```

**Step 6: Test on iOS simulator**

```bash
cd apps/mobile && npx expo run:ios
```

Verify: camera opens, can scan a QR code (use a test QR from Mac).

**Step 7: Commit**

```bash
git add apps/mobile/
git commit -m "feat: pair screen — QR scan with expo-camera, SecureStore keypair storage"
```

---

### Task 6: Dashboard screen

**Files:**
- Modify: `apps/mobile/app/(tabs)/index.tsx` (replace placeholder with dashboard)
- Create: `apps/mobile/components/SessionCard.tsx`
- Create: `apps/mobile/components/SummaryBar.tsx`
- Create: `apps/mobile/components/ConnectionDot.tsx`
- Create: `apps/mobile/hooks/use-relay-sessions.ts`

**Step 1: Create relay sessions hook (thin wrapper)**

```ts
// apps/mobile/hooks/use-relay-sessions.ts
import { useRelayConnection } from '@claude-view/shared';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export function useRelaySessions() {
  return useRelayConnection({ storage: secureStoreAdapter });
}
```

**Step 2: Create ConnectionDot component**

```tsx
// apps/mobile/components/ConnectionDot.tsx
import { XStack, Text, Circle } from 'tamagui';
import type { ConnectionState } from '@claude-view/shared';

const STATE_CONFIG: Record<ConnectionState, { color: string; label: string }> = {
  connected: { color: '#22c55e', label: 'Connected' },
  connecting: { color: '#f59e0b', label: 'Connecting' },
  disconnected: { color: '#ef4444', label: 'Mac offline' },
};

export function ConnectionDot({ state }: { state: ConnectionState }) {
  const { color, label } = STATE_CONFIG[state];
  return (
    <XStack alignItems="center" gap="$2">
      <Circle size={8} backgroundColor={color} />
      <Text color="$gray400" fontSize="$sm">{label}</Text>
    </XStack>
  );
}
```

**Step 3: Create SessionCard component**

```tsx
// apps/mobile/components/SessionCard.tsx
import { Text, XStack, YStack } from 'tamagui';
import { Pressable } from 'react-native';
import { formatUsd, type RelaySession } from '@claude-view/shared';

const STATUS_COLORS: Record<string, string> = {
  active: '#22c55e',
  waiting: '#f59e0b',
  idle: '#3b82f6',
  done: '#6b7280',
};

interface Props {
  session: RelaySession;
  onPress: () => void;
}

export function SessionCard({ session, onPress }: Props) {
  const statusColor = STATUS_COLORS[session.status] ?? '#6b7280';

  return (
    <Pressable onPress={onPress} style={({ pressed }) => ({ opacity: pressed ? 0.8 : 1 })}>
      <YStack
        backgroundColor="$gray800"
        borderRadius="$4"
        padding="$4"
        marginBottom="$2"
      >
        <Text color="$gray50" fontWeight="600" fontSize="$base">
          {session.project}
        </Text>
        <XStack alignItems="center" gap="$2" marginTop="$1">
          <Circle size={6} backgroundColor={statusColor} />
          <Text color="$gray400" fontSize="$sm">{session.status}</Text>
          <Text color="$gray500" fontSize="$sm">·</Text>
          <Text color="$gray400" fontSize="$sm" fontFamily="$mono">
            {session.model}
          </Text>
        </XStack>
        <XStack justifyContent="space-between" alignItems="center" marginTop="$3">
          <Text color="$gray400" fontFamily="$mono" fontSize="$sm">
            {formatUsd(session.cost_usd)}
          </Text>
          <Text color="$gray500" fontSize="$xs">
            {session.tokens.input + session.tokens.output} tokens
          </Text>
        </XStack>
      </YStack>
    </Pressable>
  );
}

// Re-export Circle since Tamagui doesn't have it by default
function Circle({ size, backgroundColor }: { size: number; backgroundColor: string }) {
  return (
    <YStack
      width={size}
      height={size}
      borderRadius={size / 2}
      backgroundColor={backgroundColor}
    />
  );
}
```

**Step 4: Create SummaryBar component**

```tsx
// apps/mobile/components/SummaryBar.tsx
import { Text, XStack } from 'tamagui';
import { formatUsd, groupByStatus, type RelaySession } from '@claude-view/shared';

export function SummaryBar({ sessions }: { sessions: RelaySession[] }) {
  const { needsYou, autonomous } = groupByStatus(sessions);
  const totalCost = sessions.reduce((sum, s) => sum + s.cost_usd, 0);

  return (
    <XStack
      backgroundColor="$gray800"
      borderTopWidth={1}
      borderTopColor="$gray700"
      paddingHorizontal="$4"
      paddingVertical="$3"
      justifyContent="space-between"
    >
      <Text color="#f59e0b" fontSize="$sm">{needsYou.length} needs you</Text>
      <Text color="#22c55e" fontSize="$sm">{autonomous.length} auto</Text>
      <Text color="$gray400" fontFamily="$mono" fontSize="$sm">{formatUsd(totalCost)}</Text>
    </XStack>
  );
}
```

**Step 5: Build Dashboard in tabs/index.tsx**

Replace the placeholder in `apps/mobile/app/(tabs)/index.tsx`:

```tsx
// apps/mobile/app/(tabs)/index.tsx
import { useMemo, useState } from 'react';
import { ScrollView } from 'react-native';
import { Redirect } from 'expo-router';
import { H4, Spinner, Text, XStack, YStack } from 'tamagui';
import { SafeAreaView } from 'react-native-safe-area-context';
import { groupByStatus } from '@claude-view/shared';
import { usePairingStatus } from '../../hooks/use-pairing-status';
import { useRelaySessions } from '../../hooks/use-relay-sessions';
import { SessionCard } from '../../components/SessionCard';
import { SummaryBar } from '../../components/SummaryBar';
import { ConnectionDot } from '../../components/ConnectionDot';

export default function SessionsScreen() {
  const { isPaired } = usePairingStatus();
  const { sessions, connectionState } = useRelaySessions();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  if (isPaired === null) {
    return (
      <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$gray900">
        <Spinner size="large" />
      </YStack>
    );
  }

  if (!isPaired) {
    return <Redirect href="/pair" />;
  }

  const sessionList = useMemo(() => Object.values(sessions), [sessions]);
  const { needsYou, autonomous } = useMemo(() => groupByStatus(sessionList), [sessionList]);

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: '#111827' }} edges={['top']}>
      {/* Header */}
      <XStack justifyContent="space-between" alignItems="center" paddingHorizontal="$4" paddingVertical="$3">
        <Text color="$gray50" fontWeight="bold" fontSize="$xl">Claude View</Text>
        <ConnectionDot state={connectionState} />
      </XStack>

      {/* Session list */}
      <ScrollView style={{ flex: 1, paddingHorizontal: 16 }}>
        {needsYou.length > 0 && (
          <YStack marginBottom="$4">
            <H4 color="#f59e0b" fontSize="$xs" textTransform="uppercase" letterSpacing={1} marginBottom="$2">
              Needs You
            </H4>
            {needsYou.map(s => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </YStack>
        )}

        {autonomous.length > 0 && (
          <YStack marginBottom="$4">
            <H4 color="#22c55e" fontSize="$xs" textTransform="uppercase" letterSpacing={1} marginBottom="$2">
              Autonomous
            </H4>
            {autonomous.map(s => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </YStack>
        )}

        {sessionList.length === 0 && (
          <YStack flex={1} alignItems="center" justifyContent="center" paddingVertical="$16">
            <Text color="$gray400" fontSize="$lg">
              {connectionState === 'disconnected' ? 'Mac offline' : 'No active sessions'}
            </Text>
          </YStack>
        )}
      </ScrollView>

      {/* Summary bar */}
      <SummaryBar sessions={sessionList} />

      {/* Bottom sheet (Task 7) */}
    </SafeAreaView>
  );
}
```

**Step 6: Test with live Mac data**

Start Mac dev server with `RELAY_URL` set, pair via QR from Expo Go, verify sessions appear.

**Step 7: Commit**

```bash
git add apps/mobile/
git commit -m "feat: dashboard screen — session cards grouped by status, summary bar, connection indicator"
```

---

### Task 7: Session detail sheet

**Files:**
- Create: `apps/mobile/components/SessionDetailSheet.tsx`
- Modify: `apps/mobile/app/(tabs)/index.tsx` (add sheet)

**Approach:** Use Tamagui's built-in `Sheet` component instead of adding `@gorhom/bottom-sheet` as an extra dependency. Tamagui's Sheet is already available since we have Tamagui installed.

**Step 1: Create SessionDetailSheet**

```tsx
// apps/mobile/components/SessionDetailSheet.tsx
import { ScrollView } from 'react-native';
import { Sheet, Text, XStack, YStack, Separator } from 'tamagui';
import { formatUsd, formatDuration, type RelaySession } from '@claude-view/shared';

interface Props {
  session: RelaySession | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SessionDetailSheet({ session, open, onOpenChange }: Props) {
  if (!session) return null;

  return (
    <Sheet
      modal
      open={open}
      onOpenChange={onOpenChange}
      snapPoints={[85, 50]}
      dismissOnSnapToBottom
    >
      <Sheet.Overlay backgroundColor="rgba(0,0,0,0.5)" />
      <Sheet.Handle backgroundColor="$gray500" />
      <Sheet.Frame backgroundColor="$gray800" borderTopLeftRadius="$4" borderTopRightRadius="$4">
        <ScrollView style={{ padding: 16 }}>
          {/* Header */}
          <Text color="$gray50" fontWeight="bold" fontSize="$xl">
            {session.project}
          </Text>
          <Text color="$gray400" fontSize="$sm" marginTop="$1">
            {session.model}
          </Text>

          {/* Status */}
          <XStack flexWrap="wrap" marginTop="$4" gap="$4">
            <InfoItem label="Status" value={session.status} />
            <InfoItem label="Model" value={session.model} />
            <InfoItem
              label="Tokens"
              value={`${Math.round((session.tokens.input + session.tokens.output) / 1000)}k`}
            />
          </XStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Cost */}
          <SectionLabel>Cost</SectionLabel>
          <YStack backgroundColor="$gray900" borderRadius="$3" padding="$3">
            <CostRow label="Total" value={session.cost_usd} bold />
          </YStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Last activity */}
          {session.last_message && (
            <>
              <SectionLabel>Last Activity</SectionLabel>
              <Text color="$gray200" fontSize="$sm" numberOfLines={4}>
                {session.last_message}
              </Text>
            </>
          )}

          {/* M2 teaser */}
          <YStack
            marginTop="$6"
            backgroundColor="$gray900"
            borderRadius="$4"
            padding="$4"
            alignItems="center"
            opacity={0.5}
          >
            <Text color="$gray400" fontSize="$sm">Approve / Deny — coming in M2</Text>
          </YStack>
        </ScrollView>
      </Sheet.Frame>
    </Sheet>
  );
}

function SectionLabel({ children }: { children: string }) {
  return (
    <Text color="$gray400" fontSize="$xs" textTransform="uppercase" letterSpacing={1} marginBottom="$2">
      {children}
    </Text>
  );
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <YStack>
      <Text color="$gray400" fontSize="$xs">{label}</Text>
      <Text color="$gray50" fontSize="$sm">{value}</Text>
    </YStack>
  );
}

function CostRow({ label, value, bold }: { label: string; value: number; bold?: boolean }) {
  return (
    <XStack justifyContent="space-between" paddingVertical="$1">
      <Text color={bold ? '$gray50' : '$gray400'} fontSize="$sm" fontWeight={bold ? '600' : '400'}>
        {label}
      </Text>
      <Text
        color={bold ? '$gray50' : '$gray400'}
        fontFamily="$mono"
        fontSize="$sm"
        fontWeight={bold ? '600' : '400'}
      >
        {formatUsd(value)}
      </Text>
    </XStack>
  );
}
```

**Step 2: Wire sheet into dashboard**

Add to `apps/mobile/app/(tabs)/index.tsx`:

```tsx
import { SessionDetailSheet } from '../../components/SessionDetailSheet';

// Inside component:
const selectedSession = selectedId ? sessions[selectedId] ?? null : null;

// At the bottom of the return, after SummaryBar:
<SessionDetailSheet
  session={selectedSession}
  open={selectedId !== null}
  onOpenChange={(open) => { if (!open) setSelectedId(null); }}
/>
```

**Step 3: Test interaction**

Verify: tap card → sheet slides up → shows session details → drag down to dismiss.

**Step 4: Commit**

```bash
git add apps/mobile/
git commit -m "feat: session detail sheet with cost breakdown and activity preview"
```

---

## Phase 4: Push & Ship

### Task 8: Push notifications

**Files:**
- Create: `apps/mobile/hooks/use-push-notifications.ts`
- Modify: `crates/relay/src/state.rs` (add push_tokens field)
- Create: `crates/relay/src/push.rs`
- Modify: `crates/relay/src/lib.rs` (add push token route)
- Modify: `apps/mobile/app/_layout.tsx` (register on startup)

**Step 1: Add push_tokens to RelayState**

In `crates/relay/src/state.rs`, add field:

```rust
#[derive(Clone, Default)]
pub struct RelayState {
    pub connections: Arc<DashMap<String, DeviceConnection>>,
    pub pairing_offers: Arc<DashMap<String, PairingOffer>>,
    pub devices: Arc<DashMap<String, RegisteredDevice>>,
    pub push_tokens: Arc<DashMap<String, String>>,  // ADD: device_id → Expo push token
}

impl RelayState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
            push_tokens: Arc::new(DashMap::new()),
        }
    }
}
```

**Step 2: Create push token endpoint**

```rust
// crates/relay/src/push.rs
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::state::RelayState;

#[derive(Deserialize)]
pub struct RegisterToken {
    pub device_id: String,
    pub token: String,
}

pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(body): Json<RegisterToken>,
) -> StatusCode {
    state.push_tokens.insert(body.device_id, body.token);
    StatusCode::OK
}
```

**Step 3: Add route to relay router**

In `crates/relay/src/lib.rs`:

```rust
mod push;

// In router():
.route("/push-tokens", post(push::register_push_token))
```

**Step 4: Create push notification hook**

```ts
// apps/mobile/hooks/use-push-notifications.ts
import { useEffect, useRef } from 'react';
import * as Notifications from 'expo-notifications';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: true,
  }),
});

export function usePushNotifications() {
  const listenerRef = useRef<Notifications.EventSubscription>();

  useEffect(() => {
    registerPushToken();

    listenerRef.current = Notifications.addNotificationResponseReceivedListener(
      (_response) => {
        // TODO: Navigate to session detail when notification tapped
      },
    );

    return () => {
      listenerRef.current?.remove();
    };
  }, []);
}

async function registerPushToken() {
  const { status } = await Notifications.requestPermissionsAsync();
  if (status !== 'granted') return;

  const tokenData = await Notifications.getExpoPushTokenAsync();
  const deviceId = await secureStoreAdapter.getItem('device_id');
  const relayUrl = await secureStoreAdapter.getItem('relay_url');

  if (!deviceId || !relayUrl) return;

  const httpUrl = relayUrl.replace('wss://', 'https://').replace('/ws', '');
  await fetch(`${httpUrl}/push-tokens`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ device_id: deviceId, token: tokenData.data }),
  }).catch(() => {
    // Silently fail — push is optional, will retry on next app launch
  });
}
```

**Step 5: Register in root layout**

Add to `apps/mobile/app/_layout.tsx`:

```tsx
import { usePushNotifications } from '../hooks/use-push-notifications';

export default function RootLayout() {
  usePushNotifications();
  // ... rest of layout
}
```

**Step 6: Test**

Trigger a session state change on Mac → verify notification appears on phone.

**Step 7: Commit**

```bash
git add apps/mobile/ crates/relay/
git commit -m "feat: push notifications for session state changes via Expo Notifications"
```

---

### Task 9: Landing page polish

**Status:** Mostly done — `apps/landing/` already deployed to Cloudflare Pages.

**Files:**
- Modify: `apps/landing/src/index.html` (verify branding says "Claude View", not "clawmini")
- Verify: `apps/landing/.well-known/apple-app-site-association` exists (or add to PROGRESS.md deferred list)

**Step 1: Check and update branding**

Read `apps/landing/src/index.html` and ensure:
- Title says "Claude View" not "clawmini"
- Description matches current product positioning
- App Store / Play Store links have placeholder IDs

**Step 2: Verify AASA file status**

The `apple-app-site-association` file needs a real Apple Team ID. Confirm this is on the deferred list in `docs/plans/PROGRESS.md` (it is — see "Deferred / Pre-Launch Checklist").

**Step 3: Commit if changes made**

```bash
git add apps/landing/
git commit -m "chore: update landing page branding to Claude View"
```

---

### Task 10: TestFlight build + submission

**Step 1: Configure EAS Build**

```bash
cd apps/mobile && eas build:configure
```

Create `apps/mobile/eas.json`:

```json
{
  "build": {
    "development": {
      "developmentClient": true,
      "distribution": "internal",
      "env": { "APP_VARIANT": "development" }
    },
    "preview": {
      "distribution": "internal",
      "env": { "APP_VARIANT": "preview" }
    },
    "production": {
      "env": { "APP_VARIANT": "production" }
    }
  },
  "submit": {
    "production": {
      "ios": {
        "appleId": "YOUR_APPLE_ID",
        "ascAppId": "YOUR_ASC_APP_ID"
      }
    }
  }
}
```

**Step 2: Build for iOS**

```bash
cd apps/mobile && eas build --platform ios --profile production
```

**Step 3: Submit to TestFlight**

```bash
cd apps/mobile && eas submit --platform ios --profile production
```

**Step 4: E2E verification on device**

Install from TestFlight. Full flow: scan QR → sessions appear → tap card → sheet opens → push notification fires.

**Step 5: Commit**

```bash
git add apps/mobile/eas.json
git commit -m "chore: EAS build configuration for TestFlight submission"
```

---

## Task Dependency Graph

```
Task 1 (shared pkg) → Task 2 (ts-rs) ────────────────┐
                                                       ↓
Task 3 (relay fixes) ──────────────────→ Task 5 (pair screen)
                                                       ↓
                                   Task 4 (deps) → Task 6 (dashboard)
                                                       ↓
                                                  Task 7 (detail sheet)
                                                       ↓
                                                  Task 8 (push)
                                                       ↓
                                   Task 9 (landing) + Task 10 (TestFlight)
```

Tasks 1 and 3 can run in parallel.
Tasks 4 can run in parallel with Tasks 1-3.
Task 9 can run in parallel with Tasks 6-8.

## Success Criteria

1. Scan QR on Mac → phone shows all active sessions within 2 seconds
2. Session state changes on Mac → phone updates within 1 second
3. Push notification fires when agent needs user attention
4. "Mac offline" shows correctly when Mac sleeps
5. App is on TestFlight (iOS)
