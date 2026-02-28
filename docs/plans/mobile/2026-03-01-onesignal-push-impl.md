# OneSignal Push Notification Migration — Implementation Plan

> **Status:** DONE (2026-03-01) — all 12 tasks implemented, shippable audit passed (SHIP IT)

## Completion Summary

All work is in unstaged changes (not yet committed). 12 tasks, 9 files changed.

| Task | Files | Description |
|------|-------|-------------|
| 1 | `state.rs` | Remove push_tokens/push_rate_limiter, add OneSignal env vars |
| 2 | `push.rs` | Rewrite for OneSignal REST API |
| 3 | `lib.rs` | Remove /push-tokens route |
| 4 | `ws.rs` | Pass target device_id to push |
| 5 | `main.rs` | Remove push rate limiter |
| 6 | `.env.example` | Add OneSignal env var docs |
| 7 | `package.json` | Swap expo-notifications for react-native-onesignal |
| 8 | `app.config.ts` | OneSignal plugin + oneSignalAppId in extra |
| 9 | `use-push-notifications.ts` | Rewrite: OneSignal.initialize + login(deviceId) |
| 10 | `_layout.tsx` | Verified — no changes needed |
| 11 | E2E checklist | Updated for OneSignal |
| 12 | Final verification | cargo check + tsc clean, zero stale references |

Shippable audit: 0 blockers, 1 warning (reusable HTTP client in push.rs — minor perf, push is infrequent).

---

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace Expo Push with OneSignal for day-1 product ops (delivery tracking, open rates, failure diagnostics, dashboard).

**Architecture:** Mobile app uses OneSignal SDK for token management + permission handling. Relay calls OneSignal REST API to send notifications. No token storage needed on relay — OneSignal manages the APNs token lifecycle. The relay identifies devices via `external_user_id` (= our `device_id`).

**Tech Stack:** `react-native-onesignal` + `onesignal-expo-plugin` (mobile), OneSignal REST API v1 (relay/Rust), local prebuild via Xcode (no Expo Go).

**Critical:** OneSignal SDK does NOT work with Expo Go. After this change, dev requires `npx expo prebuild --clean && npx expo run:ios`.

---

## Prerequisites (User Does Manually Before Execution)

1. Create OneSignal account at https://onesignal.com
2. Create a new app in OneSignal dashboard
3. Go to Apple Developer portal → Keys → Create a new key with APNs enabled → download .p8 file
4. In OneSignal dashboard → Settings → Platforms → Apple iOS → upload the .p8 key, enter Key ID + Team ID + Bundle ID (`ai.claudeview.mobile`)
5. Note your **App ID** and **REST API Key** from Settings → Keys & IDs
6. Set on Fly: `flyctl secrets set ONESIGNAL_APP_ID=xxx ONESIGNAL_REST_API_KEY=xxx`
7. For local dev, export these in your shell too

---

### Task 1: Relay — Simplify state.rs (remove Expo push token storage)

**Files:**
- Modify: `crates/relay/src/state.rs`

**Step 1: Remove push_tokens and push_rate_limiter, add OneSignal config**

Replace `push_tokens` and `push_rate_limiter` fields with OneSignal env vars:

```rust
// REMOVE these two fields from RelayState:
//   pub push_tokens: Arc<DashMap<String, String>>,
//   pub push_rate_limiter: Arc<RateLimiter>,

// ADD these two fields to RelayState:
    /// OneSignal App ID (None = push disabled).
    pub onesignal_app_id: Option<String>,
    /// OneSignal REST API Key (None = push disabled).
    pub onesignal_api_key: Option<String>,
```

**Step 2: Update `RelayState::new()` signature and body**

Remove `push_rate_limiter` parameter. Add env reads:

```rust
impl RelayState {
    pub fn new(
        supabase_auth: Option<Arc<SupabaseAuth>>,
        pair_rate_limiter: Arc<RateLimiter>,
        claim_rate_limiter: Arc<RateLimiter>,
        // push_rate_limiter parameter REMOVED
    ) -> Self {
        let posthog_key = std::env::var("POSTHOG_API_KEY").unwrap_or_default();
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
            // push_tokens REMOVED
            supabase_auth,
            pair_rate_limiter,
            claim_rate_limiter,
            // push_rate_limiter REMOVED
            onesignal_app_id: std::env::var("ONESIGNAL_APP_ID").ok(),
            onesignal_api_key: std::env::var("ONESIGNAL_REST_API_KEY").ok(),
            posthog_client: if posthog_key.is_empty() {
                None
            } else {
                Some(reqwest::Client::new())
            },
            posthog_api_key: posthog_key,
        }
    }
}
```

**Step 3: Run cargo check**

Run: `cargo check -p claude-view-relay`
Expected: Errors in main.rs, push.rs, lib.rs (they still reference removed fields). That's expected — we fix those next.

---

### Task 2: Relay — Rewrite push.rs for OneSignal REST API

**Files:**
- Modify: `crates/relay/src/push.rs`

**Step 1: Replace entire file contents**

```rust
use tracing::warn;

use crate::state::RelayState;

/// Send a push notification via OneSignal REST API.
///
/// If `target_device_id` is Some, targets that specific device (by external_user_id).
/// If None, sends to all subscribed users.
pub async fn send_push_notification(
    state: &RelayState,
    title: &str,
    body: &str,
    target_device_id: Option<&str>,
) {
    let (app_id, api_key) = match (&state.onesignal_app_id, &state.onesignal_api_key) {
        (Some(a), Some(k)) => (a.as_str(), k.as_str()),
        _ => return, // OneSignal not configured — skip silently
    };

    let mut payload = serde_json::json!({
        "app_id": app_id,
        "headings": { "en": title },
        "contents": { "en": body },
    });

    if let Some(did) = target_device_id {
        payload["include_aliases"] = serde_json::json!({ "external_id": [did] });
        payload["target_channel"] = serde_json::json!("push");
    } else {
        payload["included_segments"] = serde_json::json!(["Subscribed Users"]);
    }

    let client = reqwest::Client::new();
    match client
        .post("https://api.onesignal.com/api/v1/notifications")
        .header("Authorization", format!("Basic {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if !resp.status().is_success() => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("OneSignal push failed ({status}): {body}");
        }
        Err(e) => {
            warn!("OneSignal push request error: {e}");
        }
        _ => {} // success
    }

    // PostHog: track push notification sent
    if let Some(ref ph_client) = state.posthog_client {
        let ph_client = ph_client.clone();
        let api_key = state.posthog_api_key.clone();
        tokio::spawn(async move {
            crate::posthog::track(
                &ph_client,
                &api_key,
                "push_notification_sent",
                "relay_server",
                serde_json::json!({}),
            )
            .await;
        });
    }
}
```

Note: `register_push_token` handler and `RegisterToken` struct are removed entirely. The `auth` and `base64` imports are no longer needed.

**Step 2: Run cargo check**

Run: `cargo check -p claude-view-relay`
Expected: Errors in lib.rs (still references `push::register_push_token`) and ws.rs (wrong call signature). Fixed next.

---

### Task 3: Relay — Update lib.rs (remove /push-tokens route)

**Files:**
- Modify: `crates/relay/src/lib.rs`

**Step 1: Remove the push-tokens route**

Remove this line (currently line 50):
```rust
.route("/push-tokens", post(push::register_push_token))
```

The `push` module import stays — it's still used by ws.rs.

**Step 2: Run cargo check**

Run: `cargo check -p claude-view-relay`
Expected: Error in ws.rs only (call signature changed). Fixed next.

---

### Task 4: Relay — Update ws.rs (pass target device_id to push)

**Files:**
- Modify: `crates/relay/src/ws.rs`

**Step 1: Update the push trigger block (lines 184-191)**

Replace:
```rust
if let Some(ref hint) = envelope.push_hint {
    let title = envelope.push_title.as_deref().unwrap_or("Session update");
    let state = state.clone();
    let title = title.to_string();
    let hint = hint.clone();
    tokio::spawn(async move {
        push::send_push_notification(&state, &title, &hint).await;
    });
}
```

With:
```rust
if let Some(ref hint) = envelope.push_hint {
    let title = envelope.push_title.as_deref().unwrap_or("Session update");
    let state = state.clone();
    let title = title.to_string();
    let hint = hint.clone();
    let target = envelope.to.clone();
    tokio::spawn(async move {
        push::send_push_notification(&state, &title, &hint, Some(&target)).await;
    });
}
```

Only change: capture `envelope.to` and pass as `Some(&target)`.

**Step 2: Run cargo check**

Run: `cargo check -p claude-view-relay`
Expected: Error in main.rs only (constructor signature changed). Fixed next.

---

### Task 5: Relay — Update main.rs (remove push rate limiter)

**Files:**
- Modify: `crates/relay/src/main.rs`

**Step 1: Remove push rate limiter creation and usage**

Remove these lines:
```rust
// Line 58:
let push_rl = Arc::new(RateLimiter::new(10.0 / 60.0, 10.0));

// Line 64 — remove push_rl.clone() from RelayState::new() call:
// Before:
let state = claude_view_relay::state::RelayState::new(supabase_auth, pair_rl.clone(), claim_rl.clone(), push_rl.clone());
// After:
let state = claude_view_relay::state::RelayState::new(supabase_auth, pair_rl.clone(), claim_rl.clone());

// Line 71 — remove push_rl_clone:
let push_rl_clone = push_rl.clone();

// Line 78 — remove push eviction line:
push_rl_clone.evict_stale(Duration::from_secs(600)).await;
```

**Step 2: Run cargo check**

Run: `cargo check -p claude-view-relay`
Expected: Clean compile, 0 errors.

**Step 3: Commit relay changes**

```bash
git add crates/relay/src/{push.rs,state.rs,lib.rs,ws.rs,main.rs}
git commit -m "feat(relay): replace Expo Push with OneSignal REST API

- Remove push token storage (DashMap) and /push-tokens endpoint
- Call OneSignal API with external_user_id targeting
- Remove push rate limiter (OneSignal handles rate limits)
- Add ONESIGNAL_APP_ID and ONESIGNAL_REST_API_KEY env vars"
```

---

### Task 6: Relay — Update .env.example

**Files:**
- Modify: `crates/relay/.env.example`

**Step 1: Add OneSignal env vars**

Append to file:
```
# OneSignal push notifications (optional — push disabled if not set)
# ONESIGNAL_APP_ID=your-onesignal-app-id
# ONESIGNAL_REST_API_KEY=your-rest-api-key
```

**Step 2: Commit**

```bash
git add crates/relay/.env.example
git commit -m "docs(relay): add OneSignal env vars to .env.example"
```

---

### Task 7: Mobile — Swap push notification dependencies

**Files:**
- Modify: `apps/mobile/package.json`

**Step 1: Remove expo-notifications, add OneSignal packages**

```bash
cd apps/mobile
bun remove expo-notifications
bun add react-native-onesignal onesignal-expo-plugin
```

**Step 2: Verify package.json**

Run: `cat apps/mobile/package.json | grep -E "onesignal|expo-notifications"`
Expected: `react-native-onesignal` and `onesignal-expo-plugin` present, `expo-notifications` gone.

**Step 3: Commit**

```bash
git add apps/mobile/package.json bun.lock
git commit -m "feat(mobile): swap expo-notifications for react-native-onesignal"
```

---

### Task 8: Mobile — Update app.config.ts

**Files:**
- Modify: `apps/mobile/app.config.ts`

**Step 1: Replace expo-notifications plugin with OneSignal**

In the plugins array, replace `'expo-notifications'` with:
```ts
['onesignal-expo-plugin', {
  mode: 'development',
}],
```

Also add OneSignal App ID to the extra config:
```ts
extra: {
  eas: {
    projectId: 'YOUR_EAS_PROJECT_ID', // Replace after `eas init`
  },
  oneSignalAppId: process.env.ONESIGNAL_APP_ID || 'YOUR_ONESIGNAL_APP_ID',
},
```

**Step 2: Run typecheck**

Run: `cd apps/mobile && bunx tsc --noEmit`
Expected: Pass (config is JS-valued, no type issues expected).

**Step 3: Commit**

```bash
git add apps/mobile/app.config.ts
git commit -m "feat(mobile): configure onesignal-expo-plugin in app config"
```

---

### Task 9: Mobile — Rewrite use-push-notifications.ts

**Files:**
- Modify: `apps/mobile/hooks/use-push-notifications.ts`

**Step 1: Replace entire file contents**

```ts
import Constants from 'expo-constants'
import { useEffect } from 'react'
import { OneSignal } from 'react-native-onesignal'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

const ONESIGNAL_APP_ID =
  Constants.expoConfig?.extra?.oneSignalAppId || 'YOUR_ONESIGNAL_APP_ID'

export function usePushNotifications() {
  useEffect(() => {
    OneSignal.initialize(ONESIGNAL_APP_ID)
    OneSignal.Notifications.requestPermission(false)

    syncExternalUserId()
  }, [])
}

/**
 * Set the OneSignal external user ID to our device_id so the relay
 * can target push notifications to this specific device.
 */
async function syncExternalUserId() {
  const deviceId = await secureStoreAdapter.getItem('device_id')
  if (!deviceId) return
  OneSignal.login(deviceId)
}
```

Key changes:
- No more `expo-notifications` imports
- No more manual token registration with relay
- No more Ed25519 auth dance for push
- No more AppState foreground listener (OneSignal handles token rotation)
- `OneSignal.login(deviceId)` sets external_user_id for targeting

**Step 2: Run typecheck**

Run: `cd apps/mobile && bunx tsc --noEmit`
Expected: Pass.

**Step 3: Commit**

```bash
git add apps/mobile/hooks/use-push-notifications.ts
git commit -m "feat(mobile): rewrite push hook to use OneSignal SDK

- Initialize OneSignal SDK with app ID
- Set external_user_id via OneSignal.login(deviceId)
- Remove manual token registration, Ed25519 auth, foreground listener
- OneSignal handles token lifecycle automatically"
```

---

### Task 10: Mobile — Clean up unused imports in _layout.tsx

**Files:**
- Verify: `apps/mobile/app/_layout.tsx`

**Step 1: Check if _layout.tsx needs changes**

The import `import { usePushNotifications } from '../hooks/use-push-notifications'` and `usePushNotifications()` call on line 20 should still work since we kept the same export name and signature. No changes needed.

**Step 2: Run full typecheck**

Run: `cd apps/mobile && bunx tsc --noEmit`
Expected: 0 errors.

---

### Task 11: Update E2E checklist + docs

**Files:**
- Modify: `docs/plans/mobile-remote/2026-02-28-m1-e2e-checklist.md`

**Step 1: Update Phase 0 prerequisites**

Replace the EAS project init item and add OneSignal setup:
```markdown
- [ ] **OneSignal account** — Create app at onesignal.com, upload APNs .p8 key
- [ ] **OneSignal env vars** — `flyctl secrets set ONESIGNAL_APP_ID=xxx ONESIGNAL_REST_API_KEY=xxx`
- [ ] **EAS project init** — `cd apps/mobile && eas init` (still needed for EAS builds)
```

**Step 2: Update Phase 1 start services section**

Replace Expo Go instruction with development build:
```markdown
# Terminal 3: Build and run Expo dev client (NOT Expo Go — OneSignal requires native build)
cd apps/mobile && npx expo prebuild --clean && npx expo run:ios
```

**Step 3: Update Phase 1.6 push notification test steps**

Replace with:
```markdown
| 1 | On first launch after pairing | Push permission prompt appears | [ ] |
| 2 | Accept permissions | Check OneSignal dashboard — device appears with external_user_id = device_id | [ ] |
| 3 | Lock phone, trigger "needs_you" on Mac | Push notification: "[project] needs your input" | [ ] |
| 4 | Tap notification | App opens to dashboard | [ ] |
| 5 | Check OneSignal dashboard | Delivery confirmation, open rate tracked | [ ] |
```

**Step 4: Commit**

```bash
git add docs/plans/mobile-remote/2026-02-28-m1-e2e-checklist.md
git commit -m "docs: update E2E checklist for OneSignal (replaces Expo Push)"
```

---

### Task 12: Final verification

**Step 1: Full relay compile**

Run: `cargo check -p claude-view-relay`
Expected: Clean, 0 errors.

**Step 2: Full mobile typecheck**

Run: `cd apps/mobile && bunx tsc --noEmit`
Expected: Clean, 0 errors.

**Step 3: Verify no stale Expo Push references**

Run: `grep -r "exp.host" crates/ apps/ packages/ --include="*.rs" --include="*.ts" --include="*.tsx"`
Expected: No matches.

Run: `grep -r "expo-notifications" apps/mobile/ --include="*.ts" --include="*.tsx" --include="*.json"`
Expected: No matches.

**Step 4: Commit any remaining changes**

If clean, no commit needed. Otherwise fix and commit.
