# M1 Phase B: Auth + Deployment

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Supabase auth, build Expo native app, configure custom domains, bake default relay URL. After this phase, users get the zero-setup experience.

**Architecture:** Phone (Expo/React Native native app) ↔ Relay (Fly.io + JWT validation) ↔ Mac (Rust server). Supabase provides magic link + Google OAuth.

**Tech Stack:** Supabase Auth, Expo/React Native, Fly.io, jsonwebtoken (Rust), @supabase/supabase-js, expo-secure-store

**Prerequisite:** [Phase A](./m1-phase-a-bug-fixes.md) complete (pairing works locally)

**Parent epic:** [PROGRESS.md](./PROGRESS.md)
**Design doc:** [design.md](./design.md)

---

### Task 7: Set up Supabase project (manual)

**Why:** Need auth provider for user identity, rate limiting, and usage tracking.

**This is a manual task — do it in the Supabase dashboard.**

**Step 1: Create project**

1. Go to `supabase.com/dashboard`
2. Create new project: "claude-view"
3. Choose region closest to relay (Tokyo if available, otherwise Singapore)
4. Note down:
   - Project URL: `https://<ref>.supabase.co`
   - Anon key: `eyJ...` (public, safe to embed in frontend)
   - JWT secret: `your-jwt-secret` (private, for relay JWT validation)

**Step 2: Enable auth providers**

1. Go to Authentication → Providers
2. Enable **Email** (magic link, disable password signup)
3. Enable **Google** OAuth:
   - Create OAuth credentials in Google Cloud Console
   - Set authorized redirect URI: `https://<ref>.supabase.co/auth/v1/callback`
   - Add client ID and secret to Supabase

**Step 3: Configure auth settings**

1. Authentication → URL Configuration
2. Set Site URL: `claude-view://` (deep link scheme for Expo app)
3. Add redirect URLs:
   - `claude-view://auth/callback` (Expo deep link)
   - `https://m.claudeview.ai/*` (universal link fallback)
   - `http://localhost:19006/*` (for Expo local dev)

**Step 4: Create usage tables**

Go to SQL Editor, run:

```sql
create table public.usage_log (
  id         bigint generated always as identity primary key,
  user_id    uuid references auth.users(id),
  event      text not null,
  metadata   jsonb default '{}',
  created_at timestamptz default now()
);

create table public.paired_devices (
  id          bigint generated always as identity primary key,
  user_id     uuid references auth.users(id),
  device_id   text not null,
  device_type text not null,
  paired_at   timestamptz default now(),
  last_seen   timestamptz
);

alter table public.usage_log enable row level security;
alter table public.paired_devices enable row level security;

create policy "Users can insert own usage" on public.usage_log
  for insert with check (auth.uid() = user_id);

create policy "Users can view own usage" on public.usage_log
  for select using (auth.uid() = user_id);

create policy "Users can manage own devices" on public.paired_devices
  for all using (auth.uid() = user_id);
```

**Step 5: Save credentials to `.env.local`**

Create `.env.local` (gitignored):

```
SUPABASE_URL=https://<ref>.supabase.co
SUPABASE_ANON_KEY=eyJ...
SUPABASE_JWT_SECRET=your-jwt-secret
```

---

### Task 8: Add Supabase auth to mobile React pages

**Why:** Phone must sign in before pairing. This gives us user identity for rate limiting and usage tracking.

**Files:** (in Expo project `packages/mobile/`)
- Create: `lib/supabase.ts` (Supabase client with expo-secure-store token persistence)
- Create: `components/AuthGate.tsx` (auth screen)
- Integrate into: `app/_layout.tsx` (root layout wraps with auth gate)
- Dependencies: `@supabase/supabase-js`, `expo-secure-store`

**Step 1: Install Supabase SDK**

```bash
bun add @supabase/supabase-js
```

**Step 2: Create Supabase client**

Create `src/lib/supabase.ts`:

```typescript
import { createClient } from '@supabase/supabase-js'

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY

if (!supabaseUrl || !supabaseAnonKey) {
  console.warn('Supabase not configured — auth disabled')
}

export const supabase = supabaseUrl && supabaseAnonKey
  ? createClient(supabaseUrl, supabaseAnonKey)
  : null
```

**Step 3: Create MobileAuthGate component**

Create `src/components/mobile/MobileAuthGate.tsx` — a full-screen auth gate with Google OAuth and magic link. When `supabase` is null (env vars not set), the gate is bypassed for local dev.

See `docs/plans/mobile-remote/m1-combined.md` Task 8 Step 3 for the full component code.

**Step 4: Wrap MobilePairingPage with auth gate**

In `src/pages/MobilePairingPage.tsx`, rename the existing function to `MobilePairingPageInner` and export a wrapper:

```tsx
import { MobileAuthGate } from '@/components/mobile/MobileAuthGate'

export function MobilePairingPage() {
  return (
    <MobileAuthGate>
      <MobilePairingPageInner />
    </MobileAuthGate>
  )
}

function MobilePairingPageInner() {
  // ... existing component code ...
}
```

**Step 5: Wrap MobileMonitorPage with auth gate**

Same pattern. Also rename `MobileMonitorPageMobile` → `MobileMonitorPage` for consistency. Update any imports/routes.

**Step 6: Add env vars to `.env.local`**

```
VITE_SUPABASE_URL=https://<ref>.supabase.co
VITE_SUPABASE_ANON_KEY=eyJ...
```

**Step 7: Verify it compiles**

Run: `bun run typecheck`
Expected: No errors.

**Step 8: Commit**

```bash
git add src/lib/supabase.ts src/components/mobile/MobileAuthGate.tsx \
    src/pages/MobilePairingPage.tsx src/pages/MobileMonitorPage.tsx \
    package.json bun.lock
git commit -m "feat(mobile): add Supabase auth gate for mobile pages"
```

---

### Task 9: Add JWT validation to relay

**Why:** The relay must verify Supabase JWTs to authenticate users and enforce rate limits.

**Files:**
- Modify: `crates/relay/Cargo.toml` (add jsonwebtoken dependency)
- Create: `crates/relay/src/jwt.rs` (JWT validation module)
- Modify: `crates/relay/src/lib.rs` (add jwt module)
- Modify: `crates/relay/src/state.rs` (add jwt_secret to RelayState)
- Modify: `crates/relay/src/pairing.rs` (validate JWT on claim)
- Modify: `crates/relay/src/ws.rs` (validate JWT on WS connect)
- Modify: `src/hooks/use-mobile-relay.ts` (send JWT as query param)
- Modify: `src/pages/MobilePairingPage.tsx` (send JWT in claim headers)

**Step 1: Add jsonwebtoken dependency**

In `crates/relay/Cargo.toml`, add:

```toml
jsonwebtoken = "9"
```

**Step 2: Create JWT validation module**

Create `crates/relay/src/jwt.rs`:

```rust
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SupabaseClaims {
    pub sub: String,
    pub email: Option<String>,
    pub exp: u64,
}

pub fn validate_jwt(token: &str, jwt_secret: &str) -> Result<SupabaseClaims, String> {
    let key = DecodingKey::from_secret(jwt_secret.as_bytes());
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["sub", "exp"]);

    decode::<SupabaseClaims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| format!("JWT validation failed: {e}"))
}
```

**Step 3: Add jwt_secret to RelayState**

In `crates/relay/src/state.rs`, add `jwt_secret: Option<String>` to `RelayState`. Read from `SUPABASE_JWT_SECRET` env var in `new()`. Log warning if not set.

**Step 4: Validate JWT on pair/claim**

In `crates/relay/src/pairing.rs`, add `headers: axum::http::HeaderMap` parameter to `claim_pair()`. Extract `Authorization: Bearer <jwt>` and validate with `crate::jwt::validate_jwt()`. Skip validation if `state.jwt_secret` is None.

**Step 5: Validate JWT on WebSocket connect**

In `crates/relay/src/ws.rs`, add `Query(query): Query<WsQuery>` parameter where `WsQuery { token: Option<String> }`. Validate JWT from query param. Return 401 if invalid.

**Step 6: Register jwt module**

In `crates/relay/src/lib.rs`, add `pub mod jwt;`

**Step 7: Send JWT from phone**

In `src/hooks/use-mobile-relay.ts`:
```typescript
const session = supabase ? (await supabase.auth.getSession()).data.session : null
const tokenParam = session?.access_token ? `?token=${session.access_token}` : ''
const ws = new WebSocket(`${relayUrl}${tokenParam}`)
```

In `src/pages/MobilePairingPage.tsx`, add `Authorization: Bearer <jwt>` to claim POST headers.

**Step 8: Set Fly.io secret**

```bash
fly secrets set SUPABASE_JWT_SECRET=your-jwt-secret --config crates/relay/fly.toml
```

**Step 9: Verify it compiles**

```bash
cargo check -p claude-view-relay
bun run typecheck
```

**Step 10: Commit**

```bash
git add crates/relay/src/jwt.rs crates/relay/src/lib.rs crates/relay/src/state.rs \
    crates/relay/src/pairing.rs crates/relay/src/ws.rs crates/relay/Cargo.toml \
    src/hooks/use-mobile-relay.ts src/pages/MobilePairingPage.tsx
git commit -m "feat(relay): add Supabase JWT validation on claim and WS connect"
```

---

### Task 10: Configure custom domains (manual)

**Why:** Production needs `relay.claudeview.ai` instead of `*.fly.dev`. `m.claudeview.ai` serves as universal link redirect to App Store.

**This is a manual task — do it in DNS dashboards.**

**Step 1: Add relay domain to Fly.io**

```bash
fly certs add relay.claudeview.ai --config crates/relay/fly.toml
```

**Step 2: Add DNS CNAME in Cloudflare**

| Type | Name | Target | Proxy |
|------|------|--------|-------|
| CNAME | `relay` | `claude-view-relay.fly.dev` | DNS only (gray cloud) |

**Important:** DNS only (no Cloudflare proxy) for the relay — Fly.io needs direct TLS termination for WebSocket.

**Step 3: Verify**

```bash
curl https://relay.claudeview.ai/health
```

Expected: `ok`

---

### Task 11: Set up Expo project and build native app

**Why:** The mobile client is an Expo/React Native app distributed via App Store and Play Store.

**Location:** `packages/mobile/` (monorepo) or standalone repo (TBD)

**Step 1: Create Expo project**

```bash
npx create-expo-app packages/mobile --template blank-typescript
cd packages/mobile
npx expo install expo-secure-store expo-notifications expo-camera expo-linking @supabase/supabase-js
```

**Step 2: Project structure**

```
packages/mobile/
├── app/                    # Expo Router screens
│   ├── _layout.tsx         # Root layout + auth check
│   ├── index.tsx           # Pairing screen (QR scan)
│   └── monitor.tsx         # Session monitor
├── lib/
│   ├── supabase.ts         # Supabase client (uses expo-secure-store for token persistence)
│   ├── relay.ts            # WebSocket relay client + E2E encryption
│   └── crypto.ts           # NaCl box encryption (tweetnacl-js)
├── components/
│   ├── AuthGate.tsx        # Supabase auth screen
│   ├── SessionCard.tsx     # Session status card
│   └── QRScanner.tsx       # Camera-based QR scanner
├── app.json                # Expo config (scheme: "claude-view")
└── eas.json                # EAS Build config
```

**Step 3: Configure deep linking**

In `app.json`:
```json
{
  "expo": {
    "scheme": "claude-view",
    "ios": { "bundleIdentifier": "com.claudeview.mobile" },
    "android": { "package": "com.claudeview.mobile" }
  }
}
```

**Step 4: Set up EAS Build**

```bash
npx eas-cli build:configure
```

Create `eas.json` with development, preview (TestFlight), and production profiles.

**Step 5: Build and test on simulator**

```bash
npx expo start
# Press 'i' for iOS simulator, 'a' for Android emulator
```

**Step 6: Submit to TestFlight (beta)**

```bash
npx eas-cli build --platform ios --profile preview
npx eas-cli submit --platform ios
```

**Step 7: Set up m.claudeview.ai as universal link redirect**

Deploy a simple static HTML page to Cloudflare Pages that:
- Detects platform (iOS/Android)
- Redirects to App Store / Play Store
- Falls back to Expo Web build if on desktop

**Step 8: Commit**

```bash
git add packages/mobile/
git commit -m "feat: add Expo mobile app project with auth, pairing, and session monitoring"
```

---

### Task 12: Update QR URL and bake default RELAY_URL

**Why:** QR code should use deep link for native app, with web fallback. Binary should default to managed relay.

**Files:**
- Modify: `crates/server/src/routes/pairing.rs:85-90`
- Modify: `crates/server/src/live/relay_client.rs:38-50`
- Modify: `.env.example`

**Step 1: QR URL uses deep link with web fallback**

```rust
// Deep link for native app; m.claudeview.ai redirects to App Store if not installed
let mobile_base = std::env::var("MOBILE_URL")
    .unwrap_or_else(|_| "https://m.claudeview.ai".to_string());
let url = format!("{mobile_base}/pair?k={x25519_pubkey_b64}&t={token}");
```

**Step 2: Default RELAY_URL**

```rust
const DEFAULT_RELAY_URL: &str = "wss://relay.claudeview.ai/ws";

pub fn relay_url() -> Option<String> {
    std::env::var("RELAY_URL").ok().or_else(|| Some(DEFAULT_RELAY_URL.to_string()))
}
```

**Step 3: Update .env.example**

Document that both values have sensible defaults. Local dev can override.

**Step 4: Commit**

```bash
git add crates/server/src/routes/pairing.rs crates/server/src/live/relay_client.rs .env.example
git commit -m "feat: QR uses deep link, default RELAY_URL to managed relay"
```

---

### Task 13: Final redeploy and E2E test

**Why:** All code changes done. Deploy and verify full flow.

**Step 1: Redeploy relay**

```bash
fly deploy --config crates/relay/fly.toml
```

**Step 2: Full E2E test**

1. `bun run dev` on Mac (no `.env` needed — defaults to managed relay)
2. Mac logs: `connected to relay`, `auth_ok received`
3. Click phone icon → QR appears
4. Open claude-view app on phone → scan QR
5. Auth screen → sign in with Google or email
6. Pairing auto-completes
7. Mac logs: `paired device stored`
8. Start Claude session → phone shows it live
9. Kill app → reopen → already signed in, sessions appear

---

## Files Changed

| File | Action | Task |
|------|--------|------|
| `crates/relay/Cargo.toml` | Modify | 9 |
| `crates/relay/src/jwt.rs` | Create | 9 |
| `crates/relay/src/lib.rs` | Modify | 9 |
| `crates/relay/src/state.rs` | Modify | 9 |
| `crates/relay/src/pairing.rs` | Modify | 9 |
| `crates/relay/src/ws.rs` | Modify | 9 |
| `src/lib/supabase.ts` | Create | 8 |
| `src/components/mobile/MobileAuthGate.tsx` | Create | 8 |
| `src/pages/MobilePairingPage.tsx` | Modify | 8, 9 |
| `src/pages/MobileMonitorPage.tsx` | Modify | 8 |
| `src/hooks/use-mobile-relay.ts` | Modify | 9 |
| `packages/mobile/` | Create | 11 |
| `crates/server/src/routes/pairing.rs` | Modify | 12 |
| `crates/server/src/live/relay_client.rs` | Modify | 12 |
| `.env.example` | Modify | 12 |
| `packages/mobile/package.json` | Create | 8, 11 |

## Task Dependencies

```
Task 7 (Supabase setup) ─── Task 8 (auth gate) ──┐
                                                    ├── Task 9 (JWT) ──┐
Task 10 (DNS domains) ────────────────────────────┘                    │
                                                                        ├── Task 13 (E2E)
Task 11 (Expo app build) ────────────────────────────────────────────┘
                                                                        │
Task 12 (QR URL + default relay) ─────────────────────────────────────┘
```

Tasks 7+10 (manual infra) can be done in parallel with Phase A code work.
