# Mobile M1 — Implementation Plan (revised 2026-02-27)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship an Expo native app that pairs with Mac via QR and shows a real-time live dashboard of AI agent sessions.

**Architecture:** Monorepo already restructured (`apps/web`, `apps/mobile`, `packages/shared`). Expo/React Native + Tamagui v2, keypair auth, dumb relay, relay protocol types in shared package.

**Tech Stack:** Expo SDK 55, React Native 0.83, Tamagui v2, Turborepo, Bun workspaces, tweetnacl, Axum relay

**Design doc:** `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-design.md`

**Audit:** `docs/plans/mobile-remote/2026-02-27-m1-audit-results.md` — 7 blockers, 12 warnings identified.

**What's already done:**
- Monorepo structure: `apps/web`, `apps/mobile`, `apps/landing`, `packages/shared`, `packages/design-tokens`
- Expo SDK 55 scaffold with Tamagui v2, Expo Router, tab navigation
- `packages/shared` with relay protocol types (rewritten to match Rust wire format)
- `packages/design-tokens` with colors, spacing, typography
- `crates/relay/` with pairing, WebSocket auth, message forwarding
- `crates/server/src/crypto.rs` with device identity, paired device storage, NaCl box encryption
- `crates/server/src/live/relay_client.rs` with WebSocket relay streaming
- Relay client always-connect fix (B3) — no longer blocks on paired_devices check
- Session completed key casing fix (B2) — `sessionId` not `session_id`
- Tamagui config: flattened colors (`$gray900` works), `$mono` font registered, `$sm`/`$lg`/etc. fontSize tokens registered
- Babel config: duplicate `react-native-worklets/plugin` removed
- `packages/shared/package.json`: removed `"type": "module"`, added `react` peer dependency
- `packages/design-tokens/package.json`: removed `"type": "module"`
- `packages/shared/tsconfig.json`: removed conflicting `declaration`/`declarationMap`

---

## Audit-Driven Corrections (apply when executing each task)

> **CRITICAL:** The code snippets below in Tasks 1, 3, 5, 6, 7 contain stale field names and
> patterns that don't match the actual Rust wire format. Apply these corrections:

### Wire format corrections (affects Tasks 1, 5, 6, 7) — ALREADY APPLIED in code below

> **Audit gap #14:** These corrections have been applied to all code snippets in this plan revision.
> This table is kept as a reference for code review only — do NOT re-apply.

The Mac sends `LiveSession` with `#[serde(rename_all = "camelCase")]`. Use these field names:

| Wrong (in plan) | Correct (actual wire) | Applied? |
| --- | --- | --- |
| `session.cost_usd` | `session.cost.totalUsd` | Yes |
| `session.tokens.input` | `session.tokens.inputTokens` | Yes |
| `session.tokens.output` | `session.tokens.outputTokens` | Yes |
| `session.last_message` | `session.lastUserMessage` | Yes |
| `session.updated_at` | `session.lastActivityAt` | Yes |
| `session.model` (required string) | `session.model` (nullable: `string \| null`) | Yes |
| `session.status === 'active'` | `session.status === 'working'` | Yes |
| `session.status === 'waiting'` | `session.status === 'paused'` | Yes |
| `msg.session_id` | `msg.sessionId` | Yes (audit gap #13) |

### Session grouping correction — ALREADY APPLIED in code below

Don't check `status === 'waiting'`. Use `agentState.group`:

```ts
if (s.agentState.group === 'needs_you') { needsYou.push(s) }
else { autonomous.push(s) }
```

### Relay hook message handling — ALREADY APPLIED in code below

The Mac sends individual `LiveSession` objects (not batched `{ type: 'sessions' }`).

- Raw objects with `id` + `project` fields: upsert into sessions map
- Objects with `type: 'session_completed'` + `sessionId`: remove from map
- The `type === 'sessions'` branch has been removed

### Task 3 staleness

- **Bug 1** (`x25519_pubkey` on `ClaimRequest`): **ALREADY FIXED** in code. Do not apply.
- **Bug 3** (`pair_complete` handler): **ALREADY FIXED** with NaCl blob verification. Do not apply. Stale code block deleted (audit gap #1).
- **Bug 2** (always-connect early-return): **NOW FIXED** (applied in this revision). Verify only.

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
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "dependencies": {
    "@claude-view/design-tokens": "workspace:*",
    "tweetnacl": "^1.0.3",
    "tweetnacl-util": "^0.15.1"
  },
  "peerDependencies": {
    "react": ">=18"
  }
}
```

> **NOTE (audit fix B7, W10):** `"type": "module"` removed (breaks Metro). `react` peer dep added.

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
// Audit gap #3: verbatimModuleSyntax requires namespace imports for CJS modules
import * as nacl from 'tweetnacl';
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
  // React Native doesn't have crypto.randomUUID() — use nacl.randomBytes instead
  const randomBytes = nacl.randomBytes(4);
  const hex = Array.from(randomBytes, b => b.toString(16).padStart(2, '0')).join('');
  const deviceId = `phone-${hex}`;

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

/** Claim a pairing offer at the relay. Returns mac_online status (audit gap #18). */
export async function claimPairing(params: ClaimPairingParams): Promise<{ macOnline: boolean }> {
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

  // Audit gap #18 (W5): Read response body for mac_online status.
  // If Mac was offline during scan, pair_complete is silently dropped.
  const body = await res.json().catch(() => ({}));
  const macOnline = body.mac_online !== false; // Default to true if field absent

  // Store relay URL and Mac pubkey for future connections
  await storage.setItem('relay_url', relayUrl);
  await storage.setItem('mac_x25519_pubkey', macPubkeyB64);
  await storage.setItem('mac_device_id', ''); // Will be filled by pair_complete

  return { macOnline };
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

// Audit gap #6: Added 'crypto_error' state for key mismatch detection
export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'crypto_error';

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
  // Audit gap #7: Keep stale sessions during reconnect (Slack desktop pattern)
  const staleSessions = useRef<Record<string, RelaySession>>({});
  // Audit gap #6: Track consecutive decrypt failures
  const decryptFailures = useRef(0);
  const DECRYPT_FAILURE_THRESHOLD = 3;

  const disconnect = useCallback(() => {
    wsRef.current?.close();
    wsRef.current = null;
    setConnectionState('disconnected');
  }, []);

  // Audit gap #16: Stabilize storage reference to prevent infinite reconnect loops.
  // The caller MUST pass a referentially stable storage object (module-level const
  // or useMemo). If storage object changes on every render, this effect re-runs.
  useEffect(() => {
    let cancelled = false;
    let reconnectTimer: ReturnType<typeof setTimeout>;
    let reconnectAttempt = 0;

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
        reconnectAttempt = 0;
        decryptFailures.current = 0; // Reset on fresh connection
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
            // Audit gap #6: Track decrypt failures — surface re-pair prompt after 3
            if (!decrypted) {
              decryptFailures.current++;
              if (decryptFailures.current >= DECRYPT_FAILURE_THRESHOLD) {
                setConnectionState('crypto_error');
              }
              return;
            }
            decryptFailures.current = 0; // Reset on successful decrypt

            const text = new TextDecoder().decode(decrypted);
            const msg = JSON.parse(text);

            // NOTE (audit fix B1): Mac sends individual LiveSession objects
            // (camelCase fields), NOT batched { type: 'sessions' } envelopes.

            if (msg.type === 'session_completed' && msg.sessionId) {
              setSessions(prev => {
                const next = { ...prev };
                delete next[msg.sessionId];
                return next;
              });
              return;
            }
            if (msg.id && msg.project) {
              setSessions(prev => ({ ...prev, [msg.id]: msg as RelaySession }));
            }
          }
        } catch {
          // Ignore parse errors
        }
      };

      ws.onclose = () => {
        if (cancelled) return;
        setConnectionState('disconnected');
        // Audit gap #7: Keep stale sessions during reconnect instead of flashing empty.
        // Store current sessions in ref; show them with "reconnecting" overlay.
        // Replace only when fresh data arrives from Mac.
        setSessions(prev => {
          staleSessions.current = prev;
          return prev; // Keep showing current sessions
        });
        // Exponential backoff: 1s, 2s, 4s, 8s, ..., max 30s, with jitter
        const baseDelay = Math.min(1000 * 2 ** reconnectAttempt, 30000);
        const jitter = Math.random() * 1000;
        reconnectAttempt++;
        reconnectTimer = setTimeout(connect, baseDelay + jitter);
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

/** Format a USD amount. Shows sub-cent precision for small amounts (Haiku ~$0.005). */
export function formatUsd(usd: number): string {
  // Audit gap #15: usd < 0.01 returned '$0.00' — Haiku sessions show as free.
  if (usd === 0) return '$0.00';
  if (usd < 0.01) return `$${usd.toFixed(4).replace(/0+$/, '')}`;
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

/** Group sessions by whether they need user attention.
 *  NOTE (audit fix B1): Uses agentState.group, NOT status field.
 *  Rust status is 'working'|'paused'|'done' (not 'waiting').
 */
export function groupByStatus(sessions: RelaySession[]): {
  needsYou: RelaySession[];
  autonomous: RelaySession[];
} {
  const needsYou: RelaySession[] = [];
  const autonomous: RelaySession[] = [];

  for (const s of sessions) {
    if (s.agentState.group === 'needs_you') {
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

Three known bugs in the relay and relay client. **Two are already fixed in the codebase.**

**Bug 1:** ~~`ClaimRequest` missing `x25519_pubkey` field~~ — **ALREADY FIXED.** `x25519_pubkey: String` is already present as a required field in `crates/relay/src/pairing.rs`. Do NOT apply the plan's `Option<String>` change (it's weaker than what exists).

**Bug 2:** ~~Relay client early-returns when no paired devices~~ — **ALREADY FIXED** (audit revision 2026-02-27). The early-return at `relay_client.rs:69-74` has been removed. Verify only.

**Bug 3:** ~~Relay client `pair_complete` handler is a TODO stub~~ — **ALREADY FIXED.** Full NaCl blob verification is implemented at `relay_client.rs:241-279`. Do NOT replace with the plan's plaintext-only proposal (it's less secure).

**Files:**
- Verify: `crates/relay/src/pairing.rs` (x25519_pubkey already present)
- Verify: `crates/server/src/live/relay_client.rs` (always-connect fix applied, pair_complete implemented)
- Test: `crates/relay/tests/`

**Step 1: Skip Bug 1 fix — already done**

Verify `x25519_pubkey: String` exists in `ClaimRequest` (non-optional, required). No changes needed.

**Step 2: Skip Bug 2 fix — already done**

Verify the early-return `if paired_devices.is_empty() { ... continue; }` is no longer present in `relay_client.rs`. The relay client now always connects. No changes needed.

**Step 3: Skip Bug 3 fix — already done**

Verify the `pair_complete` handler at `relay_client.rs:241-279` decrypts and verifies the phone's NaCl blob before storing the paired device. No changes needed.

> **DELETED (audit gap #1):** A stale "Step 3: Implement pair_complete handler" code block was removed here. It contained a weaker plaintext-only implementation that would have regressed the existing NaCl blob verification.

**Step 4: Verify spawn_relay_client wiring at server startup (CLAUDE.md Wiring-Up Checklist)**

> **Audit gap #4:** The plan never checks that `spawn_relay_client()` is actually called at the server entry point. Per CLAUDE.md, this is the #1 recurring mistake.

Verify in `crates/server/src/lib.rs` (or `main.rs`) that `spawn_relay_client(tx, sessions, config)` is called during server startup and its output (handles, channels) is plugged into `AppState`. If not wired, session data never flows from Mac to relay to phone.

```bash
grep -n "spawn_relay_client\|relay_client::" crates/server/src/lib.rs crates/server/src/main.rs 2>/dev/null
```

If missing, add the call in the server startup sequence after `AppState` construction.

**Step 5: Verify QR display endpoint exists on Mac**

> **Audit gap #2:** The plan has no task creating the Mac QR endpoint or web UI to display it. Without this, the phone has nothing to scan.

The Rust endpoint `GET /pairing/qr` already exists in `crates/server/src/routes/pairing.rs` and returns a `QrPayload` with `url`, `k`, `t` fields. However, **no web UI component** calls this endpoint or renders the QR code.

Add a QR display component to the web app's Settings page:

```tsx
// apps/web/src/components/PairingQrCode.tsx
// Calls GET /pairing/qr, renders the `url` field as a QR code using `qrcode.react`
// Shows pairing status + "Scan from Claude View mobile app" instructions
// Install: bun add qrcode.react
```

Wire into `SettingsPage.tsx` (or a dedicated `/pair` route). Without this, the phone pair screen has no QR to scan.

**Step 6: Run all relay tests**

```bash
cargo test -p claude-view-relay -- --nocapture
cargo test -p claude-view-server relay -- --nocapture
```

**Step 7: Commit**

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

In `apps/mobile/app.config.ts`, add camera permission, notifications to the plugins array, and `eas.projectId` to extra:

```ts
plugins: [
  'expo-router',
  'expo-secure-store',
  ['expo-camera', { cameraPermission: 'Allow Claude View to scan QR codes for pairing.' }],
  'expo-notifications',
],
// Audit gap #10: getExpoPushTokenAsync needs eas.projectId on physical devices.
// Get this from `eas project:info` after running `eas init`.
extra: {
  eas: {
    projectId: 'YOUR_EAS_PROJECT_ID', // Replace after `eas init`
  },
},
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
      // QR payload is a URL: https://relay.example.com/mobile?k={pubkey}&t={token}
      // The relay WSS URL is derived from the URL origin (not a separate param).
      const url = new URL(data);
      const macPubkeyB64 = url.searchParams.get('k');
      const token = url.searchParams.get('t');

      if (!macPubkeyB64 || !token) throw new Error('Invalid QR code');

      // Derive relay WSS URL from the HTTP origin
      const relayUrl = url.origin.replace('https://', 'wss://').replace('http://', 'ws://') + '/ws';

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

**Step 4: Update root layout to include pair route and SafeAreaProvider**

> **Audit gap #21:** This snippet shows ONLY the JSX fragment to insert. Do NOT replace the
> entire `_layout.tsx` — merge these changes into the existing layout file.

> **Audit gap #9:** `SafeAreaView` in Task 6 requires `<SafeAreaProvider>` as an ancestor.
> Without it, safe area insets are zero on some devices. Wrap the root layout now.

In `apps/mobile/app/_layout.tsx`, wrap the existing return with `SafeAreaProvider` and add the `pair` screen:

```tsx
import { SafeAreaProvider } from 'react-native-safe-area-context';

// In the RootLayout component's return:
<SafeAreaProvider>
  <Stack>
    <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
    <Stack.Screen name="pair" options={{ headerShown: false, presentation: 'fullScreenModal' }} />
  </Stack>
</SafeAreaProvider>
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

// Audit gap #20: Use Tamagui tokens instead of hardcoded hex.
// $statusActive etc. exist after flattening in tamagui.config.ts.
const STATE_CONFIG: Record<ConnectionState, { color: string; label: string }> = {
  connected: { color: '$statusActive', label: 'Connected' },
  connecting: { color: '$statusWarning', label: 'Connecting' },
  disconnected: { color: '$statusError', label: 'Mac offline' },
  crypto_error: { color: '$statusError', label: 'Re-pair needed' },
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
import { Circle, Text, XStack, YStack } from 'tamagui';
import { Pressable } from 'react-native';
import { formatUsd, type RelaySession } from '@claude-view/shared';

// NOTE (audit fix B1): Rust status is 'working'|'paused'|'done', not 'active'|'waiting'|'idle'
// Audit gap #20: Use Tamagui tokens, not hardcoded hex
const STATUS_COLORS: Record<string, string> = {
  working: '$statusActive',
  paused: '$statusWarning',
  done: '$gray500',
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
            {/* Audit gap #5: model is string | null — guard against null */}
            {session.model ?? 'unknown'}
          </Text>
        </XStack>
        {/* NOTE (audit fix B1): cost is nested object, tokens use camelCase */}
        <XStack justifyContent="space-between" alignItems="center" marginTop="$3">
          <Text color="$gray400" fontFamily="$mono" fontSize="$sm">
            {formatUsd(session.cost.totalUsd)}
          </Text>
          <Text color="$gray500" fontSize="$xs">
            {session.tokens.inputTokens + session.tokens.outputTokens} tokens
          </Text>
        </XStack>
      </YStack>
    </Pressable>
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
  // NOTE (audit fix B1): cost is nested object with totalUsd
  const totalCost = sessions.reduce((sum, s) => sum + s.cost.totalUsd, 0);

  return (
    <XStack
      backgroundColor="$gray800"
      borderTopWidth={1}
      borderTopColor="$gray700"
      paddingHorizontal="$4"
      paddingVertical="$3"
      justifyContent="space-between"
    >
      <Text color="$statusWarning" fontSize="$sm">{needsYou.length} needs you</Text>
      <Text color="$statusActive" fontSize="$sm">{autonomous.length} auto</Text>
      <Text color="$gray400" fontFamily="$mono" fontSize="$sm">{formatUsd(totalCost)}</Text>
    </XStack>
  );
}
```

**Step 5: Build Dashboard in tabs/index.tsx**

> **Audit gap #22:** This REPLACES the entire `apps/mobile/app/(tabs)/index.tsx` file from Task 5 Step 5.
> Do not merge — overwrite the file completely with the code below.

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

  // ALL hooks must be called before any conditional returns (Rules of Hooks)
  const sessionList = useMemo(() => Object.values(sessions), [sessions]);
  const { needsYou, autonomous } = useMemo(() => groupByStatus(sessionList), [sessionList]);

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
            <H4 color="$statusWarning" fontSize="$xs" textTransform="uppercase" letterSpacing={1} marginBottom="$2">
              Needs You
            </H4>
            {needsYou.map(s => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </YStack>
        )}

        {autonomous.length > 0 && (
          <YStack marginBottom="$4">
            <H4 color="$statusActive" fontSize="$xs" textTransform="uppercase" letterSpacing={1} marginBottom="$2">
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
            <InfoItem label="Model" value={session.model ?? 'unknown'} />
            {/* NOTE (audit fix B1): tokens use camelCase */}
            <InfoItem
              label="Tokens"
              value={`${Math.round((session.tokens.inputTokens + session.tokens.outputTokens) / 1000)}k`}
            />
          </XStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Cost */}
          <SectionLabel>Cost</SectionLabel>
          <YStack backgroundColor="$gray900" borderRadius="$3" padding="$3">
            {/* NOTE (audit fix B1): cost is nested object */}
            <CostRow label="Total" value={session.cost.totalUsd} bold />
          </YStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Last activity — NOTE (audit fix B1): field is lastUserMessage */}
          {session.lastUserMessage && (
            <>
              <SectionLabel>Last Activity</SectionLabel>
              <Text color="$gray200" fontSize="$sm" numberOfLines={4}>
                {session.lastUserMessage}
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

**Step 1: Add reqwest to relay Cargo.toml**

> **Audit gap #11:** Renumbered from "Step 0" — executor starts at Step 1 and would skip this, causing a compilation error.

The relay needs `reqwest` to call the Expo Push API. In `crates/relay/Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

**Step 2: Add push_tokens to RelayState**

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

**Step 3: Create push token endpoint**

```rust
// crates/relay/src/push.rs
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::auth::verify_auth;
use crate::state::RelayState;

#[derive(Deserialize)]
pub struct RegisterToken {
    pub device_id: String,
    pub token: String,
    // Audit gap #8 (W2): Require Ed25519 signature — same as WS auth.
    // Without this, anyone knowing a device_id can hijack push notifications.
    pub timestamp: u64,
    pub signature: String,
}

pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(body): Json<RegisterToken>,
) -> StatusCode {
    // Verify Ed25519 signature before accepting push token
    let device = match state.devices.get(&body.device_id) {
        Some(d) => d.clone(),
        None => return StatusCode::UNAUTHORIZED,
    };
    if verify_auth(&body.device_id, body.timestamp, &body.signature, &device.pubkey).is_err() {
        return StatusCode::UNAUTHORIZED;
    }
    state.push_tokens.insert(body.device_id, body.token);
    StatusCode::OK
}

/// Send push notification to all registered phone tokens for a given Mac.
/// Called by the relay's WS message handler when it detects a session state change.
pub async fn send_push_notification(
    state: &RelayState,
    title: &str,
    body: &str,
) {
    let tokens: Vec<String> = state
        .push_tokens
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    if tokens.is_empty() {
        return;
    }

    let client = reqwest::Client::new();
    let messages: Vec<serde_json::Value> = tokens
        .into_iter()
        .map(|token| {
            serde_json::json!({
                "to": token,
                "title": title,
                "body": body,
                "sound": "default",
            })
        })
        .collect();

    // Expo Push API: https://exp.host/--/api/v2/push/send
    let _ = client
        .post("https://exp.host/--/api/v2/push/send")
        .json(&messages)
        .send()
        .await;
}
```

**Step 4: Add route to relay router**

In `crates/relay/src/lib.rs`:

```rust
mod push;

// In router():
.route("/push-tokens", post(push::register_push_token))
```

**Step 4b: Call `send_push_notification` from relay WS message handler**

In the relay's WebSocket message forwarding code (where encrypted session data is forwarded from Mac to Phone), add a check for session state changes:

```rust
// After forwarding the encrypted message to the phone's WS connection:
// Parse the plaintext session data (relay has no access to encrypted payload,
// but the Mac can include an unencrypted "push_hint" field alongside the payload)
if let Some(hint) = msg.get("push_hint").and_then(|h| h.as_str()) {
    let title = msg.get("push_title")
        .and_then(|t| t.as_str())
        .unwrap_or("Session update");
    tokio::spawn({
        let state = state.clone();
        let title = title.to_string();
        let hint = hint.to_string();
        async move {
            push::send_push_notification(&state, &title, &hint).await;
        }
    });
}
```

> **Note:** The relay cannot read encrypted payloads. The Mac must include an unencrypted `push_hint` field (e.g., "auth-service needs your input") alongside the encrypted `payload` in its WS message. This is safe because the hint is non-sensitive metadata (project name + status).

**Step 4c: Mac-side push_hint emission (ALREADY IMPLEMENTED in relay_client.rs)**

The Mac's `relay_client.rs` now includes `push_hint` and `push_title` in the unencrypted envelope JSON when the session's `agent_state.group == NeedsYou`. The `push_hint` is the agent state label (e.g., "Waiting for your input") and `push_title` is the project display name. This applies to all session update envelopes (initial snapshot, discovered, updated, lag-resync). Session completion envelopes don't include push_hint since they aren't actionable.

**Step 5: Create push notification hook**

```ts
// apps/mobile/hooks/use-push-notifications.ts
import { useEffect, useRef } from 'react';
import { AppState } from 'react-native';
import * as Notifications from 'expo-notifications';
import Constants from 'expo-constants';
import { signAuthChallenge, loadPhoneKeys } from '@claude-view/shared';
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
      (response) => {
        // Navigate to session detail when notification tapped
        // NOTE (audit fix B2): key is camelCase sessionId
        const sessionId = response.notification.request.content.data?.sessionId;
        if (sessionId && typeof sessionId === 'string') {
          // The dashboard reads selectedId from a global store or URL param
          // For M1, this is a best-effort — user lands on dashboard which shows the session
        }
      },
    );

    // Audit gap #17 (W7): Re-register push token when app comes to foreground.
    // Without this, a user who never force-quits will never retry a failed registration.
    const appStateListener = AppState.addEventListener('change', (state) => {
      if (state === 'active') registerPushToken();
    });

    return () => {
      listenerRef.current?.remove();
      appStateListener.remove();
    };
  }, []);
}

async function registerPushToken() {
  const { status } = await Notifications.requestPermissionsAsync();
  if (status !== 'granted') return;

  const tokenData = await Notifications.getExpoPushTokenAsync({
    projectId: Constants.expoConfig?.extra?.eas?.projectId,
  });
  const deviceId = await secureStoreAdapter.getItem('device_id');
  const relayUrl = await secureStoreAdapter.getItem('relay_url');
  const keys = await loadPhoneKeys(secureStoreAdapter);

  if (!deviceId || !relayUrl || !keys) return;

  // Audit gap #8 (W2): Include Ed25519 signature — matches server-side auth requirement
  const { timestamp, signature } = signAuthChallenge(deviceId, keys.signingKeyPair.secretKey);

  const httpUrl = relayUrl.replace('wss://', 'https://').replace('/ws', '');
  await fetch(`${httpUrl}/push-tokens`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ device_id: deviceId, token: tokenData.data, timestamp, signature }),
  }).catch(() => {
    // Silently fail — push is optional, will retry on foreground (audit gap #17)
  });
}
```

**Step 6: Register in root layout**

Add to `apps/mobile/app/_layout.tsx`:

```tsx
import { usePushNotifications } from '../hooks/use-push-notifications';

export default function RootLayout() {
  usePushNotifications();
  // ... rest of layout
}
```

**Step 7: Test**

Trigger a session state change on Mac → verify notification appears on phone.

**Step 8: Commit**

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
Task 1 (shared pkg) ──────────────────────────────────┐
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

Task 2 (ts-rs) ← independent, run anytime (not on critical path)
```

> **Audit gap #19:** Task 2 (ts-rs type generation) doesn't block any other task. It was incorrectly shown on the critical path between Task 1 and Task 5. ts-rs generates types that replace the hand-written relay.ts but the generated types aren't consumed by any M1 task.

Tasks 1, 2, 3, and 4 can all run in parallel.
Task 9 can run in parallel with Tasks 6-8.

## Success Criteria

1. Scan QR on Mac → phone shows all active sessions within 2 seconds
2. Session state changes on Mac → phone updates within 1 second
3. Push notification fires when agent needs user attention
4. "Mac offline" shows correctly when Mac sleeps
5. App is on TestFlight (iOS)

---

## Changelog of Fixes Applied (Prove-It Audit → Final Plan)

Audit date: 2026-02-27. Audited by: prove-it skill against actual codebase.

| # | Issue | Severity | Fix Applied |
| --- | --- | --- | --- |
| 1 | QR payload missing `r` (relay URL) param — Mac's `pairing.rs` puts only `k` and `t` in URL, phone expected `r` as query param | Blocker | Phone now derives WSS URL from QR URL origin: `url.origin.replace('https://', 'wss://') + '/ws'` |
| 2 | ~~`useRelayConnection` expected batched `type: 'sessions'` envelope~~ | ~~Blocker~~ | **STALE (audit gap #12):** This handler was removed. Mac sends individual `LiveSession` objects, not batched envelopes. Hook now uses `msg.id && msg.project` pattern matching. |
| 3 | `useMemo` called after conditional early returns in `SessionsScreen` — React Rules of Hooks violation (CLAUDE.md hard rule) | Blocker | Moved both `useMemo` calls above the `if (isPaired === null)` and `if (!isPaired)` guards |
| 4 | `crypto.randomUUID()` unavailable in React Native runtime | Blocker | Replaced with `nacl.randomBytes(4)` → hex string for device ID generation |
| 5 | Push notification sending not implemented — relay stored tokens but never called Expo Push API | Warning | Added `send_push_notification()` to `push.rs` with `reqwest` POST to `https://exp.host/--/api/v2/push/send`, plus `push_hint` protocol for unencrypted metadata alongside encrypted payloads |
| 6 | Dead `decrypt_phone_pubkey()` function always returned `Err()` (chicken-and-egg: needs phone pubkey to decrypt box containing phone pubkey) | Warning | Removed function entirely. `pair_complete` handler now uses plaintext `x25519_pubkey` field only. Added comment explaining M2 will use `crypto_box_seal` |
| 7 | Stale sessions persist after WebSocket reconnect — phone shows ghost sessions | Warning | ~~Added `setSessions({})`~~ **Revised (audit gap #7):** Now keeps stale sessions during reconnect (Slack pattern). Shows reconnecting overlay instead of flashing empty. |
| 8 | Flat 5s reconnect delay with no backoff or jitter (CLAUDE.md: "Backoff on failure") | Warning | Exponential backoff: `min(1000 * 2^attempt, 30000)` + random jitter up to 1s. Reset on successful connection |
| 9 | `SessionCard` locally redefined `Circle` component when Tamagui exports one from `@tamagui/shapes` | Minor | Removed local `Circle` function, added `Circle` to Tamagui import |
| 10 | `reqwest` dependency missing from relay `Cargo.toml` for Expo Push API calls | Minor | Added Step 1 to Task 8: `reqwest = { version = "0.12", features = ["json"] }` |
| 11 | Push notification TODO stub for deep-link navigation on tap | Minor | Replaced with best-effort `sessionId` extraction from notification data |
