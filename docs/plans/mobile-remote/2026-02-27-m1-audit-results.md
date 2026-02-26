# M1 Mobile Plan — Full Audit Results (2026-02-27)

> **For Claude:** Read this file completely before executing the implementation plan at
> `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md`.
> Every issue listed here MUST be fixed in the plan (or in the codebase) before executing the corresponding task.

**Audit method:** 4 parallel agents read every file referenced in the design doc and impl plan.
**Verdict: FAIL — Do not execute without rework.**

**Plans audited:**
- `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-design.md` (design)
- `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md` (implementation)

---

## Summary: 7 Blockers, 12 Warnings, 5 Minor

| Category | Blockers | Warnings | Minor |
|----------|----------|----------|-------|
| Wire protocol / types | 3 | 0 | 0 |
| Tamagui tokens / fonts | 3 | 0 | 1 |
| Monorepo / Metro | 1 | 4 | 1 |
| Security / crypto | 1 | 7 | 2 |
| Relay bugs (staleness) | 0 | 1 | 2 |

---

## BLOCKERS — Must Fix Before Executing

### B1. Wire Protocol Type Mismatch: `RelaySession` TS ≠ `LiveSession` Rust

**Files:** `packages/shared/src/types/relay.ts` vs `crates/server/src/live/state.rs:59-129`
**Affects:** Tasks 1, 5, 6, 7 (everything that renders session data)

The Mac's `relay_client.rs` sends `serde_json::to_vec(session)` where `session: &LiveSession`.
`LiveSession` has `#[serde(rename_all = "camelCase")]`. The TS `RelaySession` type does NOT match:

| Field | TypeScript `RelaySession` expects | Rust `LiveSession` actually sends |
|-------|-----------------------------------|-----------------------------------|
| status | `'active' \| 'waiting' \| 'idle' \| 'done'` | `'working' \| 'paused' \| 'done'` (different enum values) |
| cost | `cost_usd: number` (flat) | `cost: { totalUsd, inputUsd, outputUsd, cacheReadUsd, cacheWriteUsd }` (nested object) |
| tokens | `tokens: { input, output }` (snake_case keys) | `tokens: { inputTokens, outputTokens, cacheReadTokens, cacheCreationTokens }` (camelCase) |
| last_message | `last_message: string` | `lastUserMessage: string` (different name + camelCase) |
| updated_at | `updated_at: number` | `lastActivityAt: number` (different name + camelCase) |
| model | `model: string` (required) | `model: string \| null` (Option) |
| (extra) | — | `agentState`, `currentActivity`, `turnCount`, `gitBranch`, etc. (25+ extra fields) |

**Additionally:** The phone's `useRelayConnection` hook checks `msg.type === 'sessions'` to detect a full snapshot. The Mac **never sends** that message format — it sends individual `LiveSession` objects one-by-one (relay_client.rs:170-178), not a batched `{ type: 'sessions', sessions: [...] }` envelope. The `type === 'sessions'` branch will never trigger.

**Fix required:** Rewrite `RelaySession` in `packages/shared/src/types/relay.ts` to match `LiveSession`'s actual serialized shape. Or add a transform/adapter on the phone side. Also add a `type: 'snapshot'` envelope format to the Mac's relay_client if batch delivery is desired.

### B2. `session_completed` Key Inconsistency

**File:** `crates/server/src/live/relay_client.rs:203` vs `state.rs:181`

The relay client constructs a raw JSON literal with `"session_id"` (snake_case):
```rust
serde_json::json!({"type": "session_completed", "session_id": session_id})
```

But the `SessionEvent::SessionCompleted` struct uses `#[serde(rename_all = "camelCase")]`, which would serialize to `"sessionId"`. The TypeScript hook looks for `msg.session_id`. The raw literal wins (it bypasses the struct), so the TS hook's `msg.session_id` check matches — but this is fragile and inconsistent. If anyone refactors to use the struct, the phone breaks silently.

**Fix required:** Standardize on one casing convention. Either use the struct serialization or update the TS hook.

### B3. Relay Client Always-Connect Bug Still Present

**File:** `crates/server/src/live/relay_client.rs:69-74`

```rust
let paired_devices = load_paired_devices();
if paired_devices.is_empty() {
    tokio::time::sleep(Duration::from_secs(10)).await;
    continue;
}
```

This early-return means the Mac **never connects to the relay** when no devices are paired yet. Since `pair_complete` is received over the relay WebSocket, the first-ever pairing can never complete — the Mac isn't connected to receive it. This is a bootstrap paradox.

**Fix required (same as plan's Task 3 Bug 2):** Remove the early-return. Always connect. Guard only the session-sending code inside `connect_and_stream` with `if !paired_devices.is_empty()`.

### B4. Tamagui Token Syntax Wrong Everywhere

**Files:** Every component in Tasks 5, 6, 7
**Root cause:** `apps/mobile/tamagui.config.ts` + `packages/design-tokens/src/colors.ts`

The `colors` export from design-tokens is a nested object:
```ts
{ primary: { 50: '...', 600: '...', 900: '...' }, gray: { 50: '...', 900: '...' }, status: {...} }
```

This is spread into Tamagui's color tokens. Tamagui v2 nested tokens use **dot notation**: `$gray.900`, `$gray.50`, `$primary.600`.

The plan uses `$gray900`, `$gray800`, `$gray50`, `$primary600` — **all wrong**. This affects:
- Task 5 `pair.tsx`: `backgroundColor="$gray900"`, `color="$gray50"`, `backgroundColor="$primary600"`, `color="$gray400"`
- Task 6 `ConnectionDot.tsx`, `SessionCard.tsx`, `SummaryBar.tsx`, `index.tsx`: dozens of token references
- Task 7 `SessionDetailSheet.tsx`: dozens more

**Fix options:**
- **(A) Flatten colors in tamagui.config.ts** (less plan changes): Convert nested `{ gray: { 900: '#...' } }` to flat `{ gray900: '#...' }` before passing to Tamagui config. Then `$gray900` works.
- **(B) Rewrite all component code** to use `$gray.900` dot-notation (more plan changes, but uses Tamagui idiomatically).

### B5. `$mono` Font Token Does Not Exist

**File:** `apps/mobile/tamagui.config.ts`

The `defaultConfig` from `@tamagui/config/v5` defines only two fonts: `body` and `heading`. There is no `mono` font registered. The `tamagui.config.ts` does not add any additional fonts.

The plan uses `fontFamily="$mono"` in SessionCard, SummaryBar, SessionDetailSheet (all cost/data displays).

**Fix required:** Register a monospace font in tamagui.config.ts:
```ts
import { createFont } from 'tamagui'
const monoFont = createFont({
  family: 'monospace', // or a specific font like 'FiraCode'
  size: { ... },
  // ...
})
// In config: fonts: { ...defaultConfig.fonts, mono: monoFont }
```

### B6. `$sm`/`$lg`/`$xs`/`$xl`/`$base` fontSize Tokens Not Registered

**File:** `apps/mobile/tamagui.config.ts`

The `design-tokens` package exports `fontSize: { xs: 12, sm: 14, base: 16, lg: 18, xl: 20, 2xl: 24 }`, but these are NOT passed into Tamagui's font configuration. Tamagui v5 default font sizes use numeric keys (1-16).

`fontSize="$lg"` etc. will fail as unresolved tokens at runtime.

**Fix required:** Either (a) map design-token font sizes into the Tamagui font config, or (b) use Tamagui's numeric scale (`fontSize="$6"` for 18px).

### B7. `"type": "module"` in `packages/shared` Breaks Metro

**File:** `packages/shared/package.json:5`, `packages/design-tokens/package.json`

Metro (Expo's bundler) uses CJS-style resolution. `"type": "module"` causes Metro to reject or mishandle the package. Both `packages/shared` and `packages/design-tokens` have this.

**Fix required:** Remove `"type": "module"` from both `packages/shared/package.json` and `packages/design-tokens/package.json`.

---

## WARNINGS — Should Fix Before Shipping

### W1. "Zero-Knowledge Relay" Claim Is False for M1

**Files:** Design doc line 90, `crates/relay/src/pairing.rs:119-127`

The design doc states: "Relay sees: Only encrypted blobs. Zero-knowledge."

This is false. During pairing, the phone's `x25519_pubkey` is sent in plaintext. A compromised relay operator can substitute their own X25519 key in the `pair_complete` message → classic MITM → can decrypt all subsequent session data. TLS protects the transport, not the relay operator.

The plan acknowledges this ("M2 will use `crypto_box_seal`") but the design doc still claims zero-knowledge.

**Fix:** Either (a) ship `crypto_box_seal` key exchange for M1, or (b) update the design doc to say "M1 relay operator can read session data; zero-knowledge deferred to M2."

### W2. `/push-tokens` Endpoint Has Zero Authentication

**File:** Plan Task 8 `push.rs`

```rust
pub async fn register_push_token(
    State(state): State<RelayState>,
    Json(body): Json<RegisterToken>,
) -> StatusCode {
    state.push_tokens.insert(body.device_id, body.token);
    StatusCode::OK
}
```

Any unauthenticated client can register any push token for any device_id. If an attacker knows a target's device_id (visible in QR payloads), they can hijack that device's push notifications.

**Fix:** Require Ed25519 auth signature (same as WS auth) in the request body.

### W3. No Auth Replay Prevention (No Nonce Store)

**File:** `crates/relay/src/auth.rs:16-31`

The relay checks 60s timestamp freshness and verifies Ed25519 signature, but stores no nonces. The same valid auth message can be replayed unlimited times within the 60s window.

**Fix:** Add `seen_nonces: Arc<DashMap<(u64, String), Instant>>` to RelayState. Reject any `(timestamp, device_id)` pair seen before. Expire entries after 90s.

### W4. Push Metadata Leaks Project Names

**Files:** `crates/relay/src/ws.rs:22-26`, plan lines 1455

The `push_hint` field sends project names and agent states in plaintext through: relay memory → Expo Push API → APNS → phone lock screen. Project names (e.g., `internal-billing-v2`) are sensitive for corporate users.

**Fix for M1 (solo devs):** Document this as an accepted tradeoff. **Fix for teams:** Use silent push (`{"type": "ping"}`) + client-side WS fetch.

### W5. Mac Offline During Pairing — Silent `pair_complete` Drop

**File:** `crates/relay/src/pairing.rs:119-127`

If Mac sleeps between QR generation and phone scan, `pair_complete` is silently dropped (no queuing). Phone receives 200 OK, navigates to dashboard, shows "Mac offline" indefinitely. User cannot distinguish "Mac is offline" from "pairing silently failed."

**Fix:** Return `mac_online: bool` in `claim_pair` response. Phone shows "Pairing accepted — open Claude View on Mac to complete" when Mac is offline.

### W6. Silent Decryption Failures Mask Key Mismatches

**File:** Plan `use-relay-connection.ts:311-317`

```ts
const decrypted = decryptFromDevice(data.payload, macPubkeyRef.current, keysRef.current.boxKeyPair.secretKey);
if (!decrypted) return;  // silent
```

After Mac identity reset or phone SecureStore wipe, every message fails to decrypt. Phone shows "connected" with empty session list — worst possible failure mode: silent data loss with no error.

**Fix:** Track consecutive failures. After 3, set `connectionState: 'crypto_error'` and render "Re-pair needed" banner.

### W7. Push Token Registration Never Retries

**File:** Plan `use-push-notifications.ts:1513-1520`

Comment says "will retry on next app launch" but `registerPushToken()` only runs in `useEffect` on mount. A user who never force-quits will never retry.

**Fix:** Add `AppState.addEventListener('change')` listener to re-register token when app comes to foreground.

### W8. `setSessions({})` on Disconnect Flashes Empty State

**File:** Plan `use-relay-connection.ts:349-356`

On `ws.onclose`, sessions are cleared immediately. During transient reconnects (Wi-Fi/LTE handoffs), this flashes "No active sessions" for several seconds.

**Fix:** Store last known sessions in a ref. Show them with a "reconnecting" overlay while `connectionState === 'connecting'`. Replace only when fresh snapshot arrives. (Slack desktop uses this pattern.)

### W9. `allow_origin(Any)` CORS on Production Relay

**File:** `crates/relay/src/lib.rs:29-32`

Any web page can POST to `/pair/claim` or `/push-tokens` from a user's browser. Development permissiveness inherited by production.

**Fix:** Lock CORS to `["https://claudeview.ai", "http://localhost:5173"]` in production.

### W10. `packages/shared` Missing `react` Peer Dependency

**File:** `packages/shared/package.json`

The shared package exports a React hook (`useRelayConnection`) but doesn't declare `react` as a `peerDependency`. Works with Bun hoisting but fragile if hoisting behavior changes.

**Fix:** Add `"peerDependencies": { "react": ">=18" }`.

### W11. Duplicate `react-native-worklets/plugin` in babel.config.js

**File:** `apps/mobile/babel.config.js:14-16`

`babel-preset-expo` in SDK 55 already includes `react-native-worklets/plugin` via Reanimated v4. Manually adding it again causes the worklet transform to run twice — potential runtime errors or slower builds.

**Fix:** Remove the `'react-native-worklets/plugin'` line.

### W12. `packages/shared/tsconfig.json` Has `declaration: true` + Base Has `noEmit: true`

**File:** `packages/shared/tsconfig.json` vs `tsconfig.base.json`

These conflict. Also `verbatimModuleSyntax: true` from base requires `import type` syntax which some plan code doesn't use.

**Fix:** Remove `declaration` and `declarationMap` from shared tsconfig (it has no build step, consumed as source).

---

## MINOR

### M1. Plan's Task 3 Bug 1 Fix Would REGRESS Existing Code

**File:** `crates/relay/src/pairing.rs:18-29`

Plan says to add `x25519_pubkey: Option<String>` to `ClaimRequest`. The code **already has** `x25519_pubkey: String` (required, non-optional). The plan's `Option<String>` is weaker. **Mark Bug 1 as ALREADY FIXED. Do not apply.**

### M2. Plan's Task 3 Bug 3 Fix Would Replace Stronger Implementation

**File:** `crates/server/src/live/relay_client.rs:241-279`

Plan says `pair_complete` handler is a TODO stub. It is **fully implemented** with NaCl blob verification (more secure than plan's plaintext-only proposal). **Mark Bug 3 as ALREADY FIXED. Do not apply.**

### M3. No Rate Limiting on `/pair` or `/pair/claim`

**File:** `crates/relay/src/pairing.rs`

No per-IP rate limiting on pairing endpoints. Token entropy is not enforced at the relay level. QR TTL is up to 360s in worst case (cleanup runs every 60s, offers expire after 300s).

**Fix (low priority):** Add `tower-http` `RateLimitLayer`. Enforce minimum 16-byte token length.

### M4. Mac Identity Stored as Plaintext JSON, Not Keychain

**File:** `crates/server/src/crypto.rs:45-73`

Design doc claims "Mac key storage: macOS Keychain" but the implementation writes plaintext JSON to `~/.claude-view/identity.json` containing Ed25519 and X25519 secret keys.

**Fix:** Either use `keyring` crate for actual Keychain storage, or correct the claim in the design doc.

### M5. Mobile tsconfig Doesn't Extend Repo Base

**File:** `apps/mobile/tsconfig.json`

Extends `expo/tsconfig.base` instead of the repo's `tsconfig.base.json`. CLAUDE.md says "shared TypeScript base config, apps extend it." Minor DX divergence.

---

## Plan Staleness Summary

These items in the plan describe work that is **already done** in the codebase:

| Plan Reference | Status in Code |
|---------------|----------------|
| Task 3 Bug 1: Add `x25519_pubkey` to `ClaimRequest` | Already present as required `String` (non-optional) |
| Task 3 Bug 3: Implement `pair_complete` handler | Fully implemented with NaCl verification |
| Task 2: Verify ts-rs in workspace | ts-rs v11 already in workspace Cargo.toml, used in 29 files across 4 crates |
| Task 2: Add ts-rs to core crate | Already in `crates/core/Cargo.toml` |

These items are **NOT yet done** (confirmed absent):

| Plan Reference | Status in Code |
|---------------|----------------|
| Task 1: `packages/shared/src/crypto/` | Directory does not exist |
| Task 1: `packages/shared/src/relay/` | Directory does not exist |
| Task 1: `packages/shared/src/utils/` | Directory does not exist |
| Task 1: tweetnacl dependency in shared | Not in package.json |
| Task 2: `#[derive(TS)]` on `LiveSession` | Not present on `LiveSession` or related structs in `state.rs` |
| Task 3 Bug 2: Remove always-connect early-return | Still present at relay_client.rs:69-74 |
| Task 4: expo-camera, expo-notifications, expo-haptics | Not installed |
| Task 5: `apps/mobile/app/pair.tsx` | Does not exist |
| Task 5: `apps/mobile/hooks/` | Directory does not exist |
| Task 5: `apps/mobile/lib/` | Directory does not exist |
| Task 6: `apps/mobile/components/` | Directory does not exist |
| Task 7: SessionDetailSheet | Does not exist |
| Task 8: Push notification infrastructure in relay | No push_tokens in RelayState, no push.rs, no /push-tokens route |
| Task 8: `apps/mobile/hooks/use-push-notifications.ts` | Does not exist |

---

## Recommended Fix Order

### Before executing ANY task:

1. **Remove `"type": "module"`** from `packages/shared/package.json` and `packages/design-tokens/package.json` (B7)
2. **Flatten colors in `tamagui.config.ts`** so `$gray900` token syntax works (B4), OR rewrite all component code to dot-notation
3. **Register `$mono` font** in tamagui.config.ts (B5)
4. **Register fontSize tokens** (`$sm`, `$lg`, etc.) in tamagui font config (B6)
5. **Remove duplicate worklets plugin** from `babel.config.js` (W11)

### Before executing Task 1:

6. **Rewrite `RelaySession` TS type** to match `LiveSession`'s actual camelCase serialized shape (B1)
7. **Add `react` peer dependency** to `packages/shared/package.json` (W10)
8. **Remove `declaration: true`** from `packages/shared/tsconfig.json` (W12)

### Before executing Task 3:

9. **Mark Bug 1 and Bug 3 as ALREADY FIXED** — only Bug 2 (always-connect) needs code changes (M1, M2)
10. **Fix always-connect early-return** at relay_client.rs:69-74 (B3)

### Before executing Task 6 (use-relay-connection hook):

11. **Add snapshot message handling** — Mac currently sends individual sessions, not batched `{ type: 'sessions' }` (B1 continued)
12. **Add decrypt failure counter** → surface re-pair prompt (W6)
13. **Store stale sessions during reconnect** instead of clearing to empty (W8)

### Before executing Task 8:

14. **Add Ed25519 auth to `/push-tokens`** endpoint (W2)
15. **Add `AppState` foreground listener** for push token retry (W7)

### Before shipping to users:

16. **Correct "zero-knowledge" claim** in design doc, or ship `crypto_box_seal` (W1)
17. **Lock CORS** to known origins in production relay (W9)
18. **Return `mac_online` in `claim_pair` response** (W5)

---

## Key Files Reference

Anyone implementing or debugging this feature must read:

| File | What it contains |
|------|-----------------|
| `crates/server/src/live/state.rs:59-129` | `LiveSession` struct — the actual wire format Mac sends |
| `crates/server/src/live/relay_client.rs` | Mac-side relay client: auth, snapshot, events, pair_complete |
| `crates/server/src/crypto.rs` | `DeviceIdentity`, `PairedDevice`, encryption/decryption |
| `crates/relay/src/pairing.rs` | Pairing offer creation, claim handler, WS forwarding |
| `crates/relay/src/ws.rs` | WebSocket auth, connection registration, message routing |
| `crates/relay/src/state.rs` | `RelayState` in-memory model |
| `crates/relay/src/lib.rs` | Route registration, CORS config |
| `crates/relay/src/auth.rs` | Ed25519 auth verification, 60s freshness |
| `packages/shared/src/types/relay.ts` | TypeScript types phone uses (CURRENTLY MISMATCHED) |
| `packages/design-tokens/src/colors.ts` | Nested color structure causing token name issues |
| `apps/mobile/tamagui.config.ts` | Token registration (missing $mono, fontSize tokens) |
| `apps/mobile/metro.config.js` | Monorepo resolution (correct but missing Tamagui transform) |
| `apps/mobile/babel.config.js` | Has duplicate worklets plugin |
| `apps/mobile/package.json` | Missing expo-camera, expo-notifications, expo-haptics |
| `apps/mobile/app.config.ts` | Missing camera + notifications plugins |

---

## Audit Metadata

- **Date:** 2026-02-27
- **Agents used:** 4 parallel `feature-dev:code-explorer` agents
- **Dimensions covered:** Backend types & relay, Frontend types & hooks, Error handling & security, Monorepo wiring & deps
- **Total files read:** ~50 across all agents
- **Skill used:** `prove-it` + `auditing-plans`
