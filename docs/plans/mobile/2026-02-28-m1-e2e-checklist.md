# M1 Mobile — E2E Testing & Launch Checklist

> **Purpose:** Complete checklist for manual E2E testing and L1 launch readiness.
> Pick this up in any new session — everything needed is here.

**Status as of 2026-02-28:** App compiles clean (`tsc --noEmit` = 0 errors). All Rust crates compile. All wiring verified.

**Related docs:**
- Design: `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-design.md`
- Impl: `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-impl.md`
- Audit: `docs/plans/mobile-remote/2026-02-27-m1-audit-results.md`

---

## Phase 0: Prerequisites (One-time Setup)

- [ ] **OneSignal account** — Create app at onesignal.com, upload APNs .p8 key (Apple Developer → Keys → APNs)
- [ ] **OneSignal env vars** — `flyctl secrets set ONESIGNAL_APP_ID=xxx ONESIGNAL_REST_API_KEY=xxx` + export locally
- [x] **EAS project init** — ✅ (2026-03-04) `@vicky-ai/claude-view`, projectId `f395dbf3-420b-4f67-8892-d466bd185d85`
- [ ] **Apple Developer account** — Needed for TestFlight/push notifications
- [ ] **Deploy relay** — `cd crates/relay && flyctl deploy` (Fly.io config in `fly.toml`)
- [ ] **Set RELAY_URL** — Export `RELAY_URL=wss://claude-view-relay.fly.dev/ws` before starting server
- [ ] **Build dev client** — `cd apps/mobile && npx expo prebuild --clean && npx expo run:ios` (OneSignal requires native build, NOT Expo Go)

---

## Phase 1: Local Dev Testing (No Deployment Needed)

### 1.1 Start Services

```bash
# Terminal 1: Rust server (with relay URL pointing to deployed relay)
cd /path/to/monorepo-expo
RELAY_URL=wss://claude-view-relay.fly.dev/ws cargo run -p claude-view-server

# Terminal 2: Web frontend (for QR generation)
cd apps/web && bun run dev

# Terminal 3: Build and run Expo dev client (NOT Expo Go — OneSignal requires native build)
cd apps/mobile && npx expo prebuild --clean && npx expo run:ios
```

### 1.2 Test: App Launches

- [ ] Dev client opens on phone (built via `npx expo run:ios`, NOT Expo Go)
- [ ] App shows "Camera access needed" screen (first launch, no pairing yet)
- [ ] If previously paired: shows sessions dashboard with "Mac offline" state

### 1.3 Test: QR Pairing Flow

| Step | Action | Expected | Status |
|------|--------|----------|--------|
| 1 | Open web UI → Settings | Settings page loads | [ ] |
| 2 | Click "Generate QR" | QR code appears with scannable pattern | [ ] |
| 3 | Open dev client → scan QR | Camera opens on pair.tsx screen | [ ] |
| 4 | Scan the QR code | Haptic feedback (success vibration) | [ ] |
| 5 | (auto) | Phone calls `POST /pair/claim` to relay | [ ] |
| 6 | (auto) | Relay forwards to Mac, Mac verifies HMAC | [ ] |
| 7 | (auto) | Phone navigates to `/(tabs)` dashboard | [ ] |
| 8 | Check server logs | `pair_complete` received, device stored | [ ] |

### 1.4 Test: Session Dashboard (Live Data)

| Step | Action | Expected | Status |
|------|--------|----------|--------|
| 1 | With server running, start Claude Code session | Session card appears on phone (~1s) | [ ] |
| 2 | Check session card | Shows: project name, status dot, model, cost | [ ] |
| 3 | Check grouping | "Needs You" section for waiting sessions | [ ] |
| 4 | Check grouping | "Autonomous" section for working sessions | [ ] |
| 5 | Check summary bar | Shows counts + total cost at bottom | [ ] |
| 6 | Tap session card | Bottom sheet slides up with detail view | [ ] |
| 7 | Check detail sheet | Shows: status, model, token count, cost, last activity | [ ] |
| 8 | Swipe down on sheet | Sheet dismisses | [ ] |

### 1.5 Test: Connection States

| Step | Action | Expected | Status |
|------|--------|----------|--------|
| 1 | With server running | Green dot, "Connected" label | [ ] |
| 2 | Kill server (Ctrl+C) | Red dot, "Mac offline" label | [ ] |
| 3 | Restart server | Auto-reconnect, green dot returns | [ ] |
| 4 | Kill relay | Red dot, exponential backoff reconnection | [ ] |

### 1.6 Test: Push Notifications

| Step | Action | Expected | Status |
|------|--------|----------|--------|
| 1 | On first launch after pairing | Push permission prompt appears (OneSignal) | [ ] |
| 2 | Accept permissions | Check OneSignal dashboard — device appears with external_user_id = device_id | [ ] |
| 3 | Lock phone, trigger "needs_you" on Mac | Push notification: "[project] needs your input" | [ ] |
| 4 | Tap notification | App opens to dashboard | [ ] |
| 5 | Check OneSignal dashboard | Delivery confirmation, open rate tracked | [ ] |

> **Note:** Push requires native dev build (not Expo Go) + OneSignal configured + physical device.

---

## Phase 2: Smoke Tests Without Phone (curl)

These verify server + relay independently:

```bash
# 1. Server health
curl http://localhost:47892/api/health

# 2. Generate QR payload
curl http://localhost:47892/api/pairing/qr | jq .
# Expected: { url, r, k, t, s, v: 1 }

# 3. Relay health
curl https://claude-view-relay.fly.dev/health

# 4. List paired devices
curl http://localhost:47892/api/pairing/devices | jq .

# 5. Simulate pairing claim (requires valid token from step 2)
curl -X POST https://claude-view-relay.fly.dev/pair/claim \
  -H 'Content-Type: application/json' \
  -d '{"one_time_token":"TOKEN_FROM_QR","device_id":"test","pubkey":"...","x25519_pubkey":"..."}'
```

---

## Phase 3: Build & Distribution

### 3.1 Development Build (Internal Testing)

```bash
cd apps/mobile

# iOS development build (requires Apple Developer account)
eas build --profile development --platform ios

# Install on device via QR from EAS dashboard
```

### 3.2 TestFlight Build (L1 Release)

```bash
cd apps/mobile

# Production iOS build
eas build --profile production --platform ios

# Submit to TestFlight
eas submit --platform ios
```

### 3.3 Pre-Submission Checklist

- [ ] EAS projectId set (not placeholder)
- [ ] Bundle identifier: `ai.claudeview.mobile`
- [ ] App icon + splash screen assets in `apps/mobile/assets/`
- [ ] Privacy policy URL (required for App Store)
- [ ] Camera usage description in `app.config.ts` (already set)
- [ ] Push notification entitlement configured
- [ ] Relay deployed and stable at production URL

---

## Known Issues & Workarounds

| Issue | Severity | Workaround | Fix When |
|-------|----------|------------|----------|
| `as any` type assertion on dynamic `bg` props (ConnectionDot, SessionCard) | Low | Works at runtime, lint warning only | When Tamagui v2 stabilizes |
| AASA file missing for universal links | Low | Not needed for M1 (QR pairing, no URL deep links) | M2 (when adding "open in app" from web) |
| `YOUR_EAS_PROJECT_ID` placeholder | Blocker for builds | Run `eas init` once | Before first device build |
| `YOUR_ONESIGNAL_APP_ID` placeholder | Blocker for push | Create OneSignal app, set env var | Before first device build |
| Push notifications untested | Medium | Requires native dev build + OneSignal + physical device | Phase 1.6 |

---

## Architecture Quick Reference

```
Phone (Dev Client)                 Mac (Rust Server)
     │                                   │
     │ scan QR ──────────────────────────>│ /api/pairing/qr
     │                                   │
     │ POST /pair/claim ──> Relay ──────>│ pair_complete (HMAC verified)
     │                                   │
     │ <── WSS (encrypted) ─── Relay <───│ session updates (NaCl box)
     │                                   │
     │ OneSignal.login(deviceId)          │ push_hint when needs_you
     │                                   │
     └──── OneSignal <──── Relay ─────────┘
```

**Crypto:** Ed25519 identity (signing) + X25519 (encryption). HMAC anti-MITM binding on pairing. NaCl box per-message encryption.

**Key files:**
- Server pairing: `crates/server/src/routes/pairing.rs`
- Server relay client: `crates/server/src/live/relay_client.rs`
- Server crypto: `crates/server/src/crypto.rs`
- Relay: `crates/relay/src/{main,lib,pairing,ws,push,state}.rs`
- Phone pairing: `apps/mobile/app/pair.tsx`
- Phone dashboard: `apps/mobile/app/(tabs)/index.tsx`
- Phone relay hook: `packages/shared/src/relay/use-relay-connection.ts`
- Phone crypto: `packages/shared/src/crypto/nacl.ts`
- Web QR: `apps/web/src/components/PairingQrCode.tsx`

---

## Session Pickup Notes

When resuming this task in a new session:

1. Run `cd apps/mobile && bunx tsc --noEmit` — should pass with 0 errors
2. Run `cargo check -p claude-view-server && cargo check -p claude-view-relay` — both should pass
3. Check this checklist for incomplete items (marked `[ ]`)
4. Tamagui v2 uses `@tamagui/config/v5` shorthands: `bg`, `p`, `m`, `mt`, `mb`, `px`, `py`, `items`, `justify`, `rounded`, `text`, `b`, `l`, `r`, `t` — NOT `ai`, `jc`, `br`, `ta`, `padding`
5. The root `.env` doesn't exist by design — each service manages its own env vars
