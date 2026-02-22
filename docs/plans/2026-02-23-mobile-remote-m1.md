# Mobile Remote M1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the 3 pairing bugs, add Supabase auth, deploy mobile SPA to Cloudflare Pages, and ship a zero-setup "scan QR → sign in → see sessions" experience.

**Architecture:** Phone (React SPA on Cloudflare Pages) ↔ Relay (Fly.io WebSocket broker + JWT validation) ↔ Mac (Rust server with Keychain). Supabase provides auth (magic link + Google OAuth). All session data is E2E encrypted (NaCl box). Relay is zero-knowledge.

**Tech Stack:** Rust/Axum (relay + server), React/TypeScript (mobile SPA), Supabase Auth (magic link + Google), Cloudflare Pages (hosting), Fly.io (relay), tweetnacl (crypto), NaCl box (E2E encryption)

**Design doc:** `docs/plans/2026-02-23-mobile-remote-zero-setup-design.md`
**Bug analysis:** `docs/plans/2026-02-23-mobile-remote-TODO.md`

---

## Phase A: Fix the 3 Pairing Bugs (local testing)

These tasks fix the broken pairing flow so Mac and phone can connect end-to-end. Test locally with `cargo run -p claude-view-relay` + `bun run dev`.

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

Find the block that iterates `paired_devices` to send the initial snapshot and wrap it:

```rust
if !paired_devices.is_empty() {
    // Send initial snapshot of all current sessions
    // ... existing snapshot code ...
}
```

This way, when connected with 0 devices, the client just listens for incoming messages (like `pair_complete`) without trying to encrypt/send anything.

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

This confirms Task 3 (always-connect) works. Previously the logs would show nothing because the client never connected.

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

## Phase B: Auth + Deployment (production zero-setup)

These tasks add Supabase auth, deploy the mobile SPA to Cloudflare Pages, and configure custom domains. After this phase, users get the zero-setup experience.

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
2. Set Site URL: `https://m.claudeview.ai`
3. Add redirect URLs:
   - `https://m.claudeview.ai/*`
   - `http://localhost:5173/*` (for local dev)

**Step 4: Create usage tables**

Go to SQL Editor, run:

```sql
-- Usage tracking
create table public.usage_log (
  id         bigint generated always as identity primary key,
  user_id    uuid references auth.users(id),
  event      text not null,
  metadata   jsonb default '{}',
  created_at timestamptz default now()
);

-- Server-side pairing record
create table public.paired_devices (
  id          bigint generated always as identity primary key,
  user_id     uuid references auth.users(id),
  device_id   text not null,
  device_type text not null,
  paired_at   timestamptz default now(),
  last_seen   timestamptz
);

-- RLS policies
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

**Files:**
- Create: `src/lib/supabase.ts` (Supabase client singleton)
- Create: `src/components/mobile/MobileAuthGate.tsx` (auth screen)
- Modify: `src/pages/MobilePairingPage.tsx` (wrap with auth gate)
- Modify: `src/pages/MobileMonitorPage.tsx` (wrap with auth gate)
- Modify: `package.json` (add @supabase/supabase-js dependency)

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

Create `src/components/mobile/MobileAuthGate.tsx`:

```tsx
import { useEffect, useState, type ReactNode } from 'react'
import type { User } from '@supabase/supabase-js'
import { supabase } from '@/lib/supabase'

export function MobileAuthGate({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [loading, setLoading] = useState(true)
  const [email, setEmail] = useState('')
  const [sent, setSent] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!supabase) {
      // Auth not configured — skip gate (local dev)
      setLoading(false)
      setUser({} as User)
      return
    }

    supabase.auth.getSession().then(({ data: { session } }) => {
      setUser(session?.user ?? null)
      setLoading(false)
    })

    const { data: { subscription } } = supabase.auth.onAuthStateChange((_event, session) => {
      setUser(session?.user ?? null)
    })

    return () => subscription.unsubscribe()
  }, [])

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gray-950 text-white">
        <div className="animate-spin w-6 h-6 border-2 border-white/30 border-t-white rounded-full" />
      </div>
    )
  }

  if (user) {
    return <>{children}</>
  }

  const handleMagicLink = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!supabase || !email.trim()) return
    setError(null)

    const { error: err } = await supabase.auth.signInWithOtp({
      email: email.trim(),
      options: { emailRedirectTo: window.location.href },
    })

    if (err) {
      setError(err.message)
    } else {
      setSent(true)
    }
  }

  const handleGoogle = async () => {
    if (!supabase) return
    setError(null)

    const { error: err } = await supabase.auth.signInWithOAuth({
      provider: 'google',
      options: { redirectTo: window.location.href },
    })

    if (err) setError(err.message)
  }

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-950 text-white px-6">
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center space-y-2">
          <h1 className="text-2xl font-bold">Claude View</h1>
          <p className="text-gray-400 text-sm">Sign in to connect your phone</p>
        </div>

        <button
          onClick={handleGoogle}
          className="w-full flex items-center justify-center gap-2 bg-white text-gray-900 rounded-lg px-4 py-3 font-medium hover:bg-gray-100 transition-colors"
        >
          <svg className="w-5 h-5" viewBox="0 0 24 24">
            <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"/>
            <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
            <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
            <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
          </svg>
          Continue with Google
        </button>

        <div className="flex items-center gap-3">
          <div className="flex-1 h-px bg-gray-800" />
          <span className="text-gray-500 text-xs">or</span>
          <div className="flex-1 h-px bg-gray-800" />
        </div>

        {sent ? (
          <div className="text-center space-y-2">
            <p className="text-green-400 text-sm">Check your email for a sign-in link</p>
            <button
              onClick={() => setSent(false)}
              className="text-gray-500 text-xs underline"
            >
              Try a different email
            </button>
          </div>
        ) : (
          <form onSubmit={handleMagicLink} className="space-y-3">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              className="w-full bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
              required
            />
            <button
              type="submit"
              className="w-full bg-blue-600 text-white rounded-lg px-4 py-3 font-medium hover:bg-blue-500 transition-colors"
            >
              Sign in with email
            </button>
          </form>
        )}

        {error && (
          <p className="text-red-400 text-sm text-center">{error}</p>
        )}
      </div>
    </div>
  )
}
```

**Step 4: Wrap MobilePairingPage with auth gate**

In `src/pages/MobilePairingPage.tsx`, wrap the component:

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
  // ... existing component code (rename from MobilePairingPage) ...
}
```

**Step 5: Wrap MobileMonitorPage with auth gate**

Same pattern in `src/pages/MobileMonitorPage.tsx`:

```tsx
import { MobileAuthGate } from '@/components/mobile/MobileAuthGate'

export function MobileMonitorPage() {
  return (
    <MobileAuthGate>
      <MobileMonitorPageInner />
    </MobileAuthGate>
  )
}

function MobileMonitorPageInner() {
  // ... existing component code ...
}
```

Note: The current export is named `MobileMonitorPageMobile` — rename it to `MobileMonitorPage` for consistency while doing this refactor. Update any imports/routes accordingly.

**Step 6: Add env vars to `.env.local`**

```
VITE_SUPABASE_URL=https://<ref>.supabase.co
VITE_SUPABASE_ANON_KEY=eyJ...
```

**Step 7: Verify it compiles**

Run: `bun run typecheck`
Expected: No errors.

**Step 8: Test locally**

1. Run `bun run dev`
2. Open `/mobile` route in browser
3. Should see auth screen with Google button and email input
4. Skip auth (if `VITE_SUPABASE_URL` not set, gate is bypassed)

**Step 9: Commit**

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
- Modify: `crates/relay/src/lib.rs` (add jwt module, pass JWT secret to state)
- Modify: `crates/relay/src/state.rs` (add jwt_secret to RelayState)
- Modify: `crates/relay/src/pairing.rs` (validate JWT on claim)
- Modify: `crates/relay/src/ws.rs` (validate JWT on WS connect)

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
    pub sub: String,       // user UUID
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

Note: Supabase uses HS256 with the JWT secret by default. If the project is configured for RS256, switch to fetching the JWKS. HS256 with the `jwt_secret` from Supabase dashboard settings is simplest for M1.

**Step 3: Add jwt_secret to RelayState**

In `crates/relay/src/state.rs`, add to RelayState:

```rust
pub struct RelayState {
    pub connections: Arc<DashMap<String, DeviceConnection>>,
    pub pairing_offers: Arc<DashMap<String, PairingOffer>>,
    pub devices: Arc<DashMap<String, RegisteredDevice>>,
    pub jwt_secret: Option<String>,
}
```

Update `RelayState::new()`:

```rust
impl RelayState {
    pub fn new() -> Self {
        let jwt_secret = std::env::var("SUPABASE_JWT_SECRET").ok();
        if jwt_secret.is_none() {
            tracing::warn!("SUPABASE_JWT_SECRET not set — JWT validation disabled");
        }
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
            jwt_secret,
        }
    }
}
```

**Step 4: Validate JWT on pair/claim**

In `crates/relay/src/pairing.rs`, add JWT validation to `claim_pair()`. Extract Bearer token from `Authorization` header:

Add to the function parameters:

```rust
pub async fn claim_pair(
    State(state): State<Arc<RelayState>>,
    headers: axum::http::HeaderMap,
    Json(claim): Json<ClaimRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
```

At the top of the function body, before the token lookup:

```rust
// Validate JWT if configured
if let Some(ref secret) = state.jwt_secret {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    crate::jwt::validate_jwt(token, secret)
        .map_err(|e| {
            tracing::warn!(error = %e, "JWT validation failed on claim");
            StatusCode::UNAUTHORIZED
        })?;
}
```

**Step 5: Validate JWT on WebSocket connect**

In `crates/relay/src/ws.rs`, extract JWT from query parameter (WebSocket can't use Authorization header easily). Modify `ws_handler` to accept query params:

```rust
#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_handler(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Validate JWT if configured
    if let Some(ref secret) = state.jwt_secret {
        if let Some(ref token) = query.token {
            if let Err(e) = crate::jwt::validate_jwt(token, secret) {
                tracing::warn!(error = %e, "JWT validation failed on WS");
                return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
            }
        } else {
            return (StatusCode::UNAUTHORIZED, "Missing token").into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
        .into_response()
}
```

**Step 6: Register jwt module**

In `crates/relay/src/lib.rs`, add:

```rust
pub mod jwt;
```

**Step 7: Update phone WebSocket URL to include JWT token**

In `src/hooks/use-mobile-relay.ts`, when connecting to relay WebSocket, append the Supabase session token as query parameter:

```typescript
import { supabase } from '@/lib/supabase'

// Inside connect():
const session = supabase ? (await supabase.auth.getSession()).data.session : null
const tokenParam = session?.access_token ? `?token=${session.access_token}` : ''
const ws = new WebSocket(`${relayUrl}${tokenParam}`)
```

Similarly in `src/pages/MobilePairingPage.tsx`, add JWT to the claim POST headers:

```typescript
import { supabase } from '@/lib/supabase'

// Inside handleQRPayload, before fetch:
const session = supabase ? (await supabase.auth.getSession()).data.session : null
const headers: Record<string, string> = { 'Content-Type': 'application/json' }
if (session?.access_token) {
  headers['Authorization'] = `Bearer ${session.access_token}`
}
```

And update the static `mobile.html` similarly — store session token and include in WS URL + claim headers. Since mobile.html doesn't use Supabase SDK, this page will work without auth (for development/fallback). The React SPA is the production path.

**Step 8: Set SUPABASE_JWT_SECRET on Fly.io**

```bash
fly secrets set SUPABASE_JWT_SECRET=your-jwt-secret --config crates/relay/fly.toml
```

**Step 9: Verify it compiles**

```bash
cargo check -p claude-view-relay
bun run typecheck
```

Expected: Both pass with no errors.

**Step 10: Commit**

```bash
git add crates/relay/src/jwt.rs crates/relay/src/lib.rs crates/relay/src/state.rs \
    crates/relay/src/pairing.rs crates/relay/src/ws.rs crates/relay/Cargo.toml \
    src/hooks/use-mobile-relay.ts src/pages/MobilePairingPage.tsx
git commit -m "feat(relay): add Supabase JWT validation on claim and WS connect"
```

---

### Task 10: Configure custom domains (manual)

**Why:** Production needs `relay.claudeview.ai` and `m.claudeview.ai` instead of `*.fly.dev`.

**This is a manual task — do it in DNS dashboards.**

**Step 1: Add relay domain to Fly.io**

```bash
fly certs add relay.claudeview.ai --config crates/relay/fly.toml
```

This outputs CNAME target(s). Note them down.

**Step 2: Add DNS records in Cloudflare**

In Cloudflare dashboard for `claudeview.ai`:

| Type | Name | Target | Proxy |
|------|------|--------|-------|
| CNAME | `relay` | `claude-view-relay.fly.dev` | DNS only (no proxy — Fly handles TLS) |

**Important:** Set Cloudflare proxy to "DNS only" (gray cloud) for the relay. Fly.io needs direct TLS termination for WebSocket. If proxied through Cloudflare, WebSocket upgrade may fail or add latency.

**Step 3: Verify relay domain**

Wait for DNS propagation (usually <5 min with Cloudflare), then:

```bash
curl https://relay.claudeview.ai/health
```

Expected: `ok`

**Step 4: Verify WebSocket**

```bash
wscat -c wss://relay.claudeview.ai/ws
```

Expected: Connection opens (then fails auth, which is fine — it connected).

---

### Task 11: Deploy mobile SPA to Cloudflare Pages

**Why:** The React mobile pages need a home at `m.claudeview.ai`.

**This task involves creating a separate Vite build for the mobile pages and deploying to Cloudflare Pages.**

**Files:**
- Create: `vite.config.mobile.ts` (mobile-specific Vite config)
- Create: `mobile-index.html` (entry point for mobile SPA)
- Create: `src/mobile-main.tsx` (mobile React entry)
- Create: `src/mobile-routes.tsx` (mobile router — only mobile pages)

**Step 1: Create mobile entry point HTML**

Create `mobile-index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="theme-color" content="#030712" />
    <meta name="apple-mobile-web-app-capable" content="yes" />
    <title>Claude View</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/mobile-main.tsx"></script>
  </body>
</html>
```

**Step 2: Create mobile React entry**

Create `src/mobile-main.tsx`:

```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MobilePairingPage } from './pages/MobilePairingPage'
import { MobileMonitorPage } from './pages/MobileMonitorPage'
import './index.css'

const queryClient = new QueryClient()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<MobilePairingPage />} />
          <Route path="/monitor" element={<MobileMonitorPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>,
)
```

**Step 3: Create mobile Vite config**

Create `vite.config.mobile.ts`:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  root: '.',
  build: {
    outDir: 'dist-mobile',
    rollupOptions: {
      input: path.resolve(__dirname, 'mobile-index.html'),
    },
  },
})
```

**Step 4: Add build script to package.json**

In `package.json` scripts, add:

```json
"build:mobile": "vite build --config vite.config.mobile.ts"
```

**Step 5: Update mobile page routes**

The React mobile pages currently use `/mobile` and `/mobile/monitor` routes (when served from the desktop app). For the standalone Cloudflare deploy, they'll be at `/` and `/monitor`.

In `src/pages/MobilePairingPage.tsx`, update the navigation after successful pairing:

```typescript
// Change from:
navigate('/mobile/monitor')
// To:
navigate('/monitor')
```

In `src/pages/MobileMonitorPage.tsx`, update the redirect when unpaired:

```typescript
// Change from:
navigate('/mobile')
// To:
navigate('/')
```

**Important:** These routes are ONLY for the Cloudflare-hosted mobile SPA. The desktop app's route configuration (`/mobile`, `/mobile/monitor`) stays as-is. The mobile-main.tsx router handles the Cloudflare routes independently.

Actually, to keep both working without code duplication, use an environment variable or detect the hostname:

```typescript
const basePath = window.location.hostname.includes('claudeview') ? '' : '/mobile'
navigate(`${basePath}/monitor`)
```

Or simpler: use relative navigation:

```typescript
navigate('../monitor', { relative: 'path' })
```

The exact routing approach should be decided during implementation based on what React Router supports cleanly.

**Step 6: Add Cloudflare Pages `_redirects`**

Create `public-mobile/_redirects` (for SPA fallback):

```
/*    /mobile-index.html   200
```

Or configure in `wrangler.toml` — Cloudflare Pages auto-handles SPA routing if index.html exists at root.

**Step 7: Build and test locally**

```bash
bun run build:mobile
bunx serve dist-mobile
```

Open `http://localhost:3000` — should show auth screen (or pairing if no Supabase configured).

**Step 8: Deploy to Cloudflare Pages**

Option A — Connect to Git:
1. Go to Cloudflare Dashboard → Pages → Create project
2. Connect GitHub repo
3. Set build command: `bun run build:mobile`
4. Set build output: `dist-mobile`
5. Set environment variables:
   - `VITE_SUPABASE_URL`
   - `VITE_SUPABASE_ANON_KEY`
6. Set custom domain: `m.claudeview.ai`

Option B — Direct upload via Wrangler:
```bash
bunx wrangler pages deploy dist-mobile --project-name claude-view-mobile
```

Then add custom domain `m.claudeview.ai` in Pages dashboard.

**Step 9: Verify deployment**

```bash
curl https://m.claudeview.ai
```

Expected: HTML with React app shell.

**Step 10: Commit**

```bash
git add vite.config.mobile.ts mobile-index.html src/mobile-main.tsx package.json
git commit -m "feat: add mobile SPA build for Cloudflare Pages deployment"
```

---

### Task 12: Update QR URL and bake default RELAY_URL

**Why:** QR code should point to `m.claudeview.ai` instead of relay's `/mobile` page. And `RELAY_URL` should default to the managed relay.

**Files:**
- Modify: `crates/server/src/routes/pairing.rs:85-90` (QR URL)
- Modify: `crates/server/src/live/relay_client.rs:38-50` (default RELAY_URL)
- Modify: `.env.example` (update docs)

**Step 1: Update QR URL to point to mobile SPA**

In `crates/server/src/routes/pairing.rs`, the QR URL construction (around line 85-90) currently builds:

```rust
let url = format!("{relay_http}/mobile?k={x25519_pubkey_b64}&t={token}");
```

Change to point to the Cloudflare-hosted SPA:

```rust
let mobile_base = std::env::var("MOBILE_URL")
    .unwrap_or_else(|_| "https://m.claudeview.ai".to_string());
let url = format!("{mobile_base}?k={x25519_pubkey_b64}&t={token}");
```

The `r` field in QrPayload also needs to be included so the mobile SPA knows which relay to connect to:

```rust
let relay_ws = relay_ws_url().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
```

This is already the case — `r` is set to the relay WS URL. The mobile SPA reads `r` from the QR payload to know where to connect.

**Step 2: Bake default RELAY_URL**

In `crates/server/src/live/relay_client.rs`, where `relay_url` is read from config (around line 38-50):

```rust
pub fn relay_url() -> Option<String> {
    std::env::var("RELAY_URL").ok()
}
```

Add a default:

```rust
const DEFAULT_RELAY_URL: &str = "wss://relay.claudeview.ai/ws";

pub fn relay_url() -> Option<String> {
    std::env::var("RELAY_URL").ok().or_else(|| Some(DEFAULT_RELAY_URL.to_string()))
}
```

This means `npx claude-view` works without any `.env` configuration — it connects to the managed relay by default.

**Step 3: Update .env.example**

```
# Mobile relay server URL (WebSocket endpoint)
# Defaults to managed relay: wss://relay.claudeview.ai/ws
# Override for local dev: ws://localhost:47893/ws
# RELAY_URL=ws://localhost:47893/ws

# Mobile SPA URL (where QR code points to)
# Defaults to: https://m.claudeview.ai
# Override for local dev: http://localhost:5173/mobile
# MOBILE_URL=http://localhost:5173/mobile
```

**Step 4: Verify it compiles**

```bash
cargo check -p claude-view-server
```

**Step 5: Commit**

```bash
git add crates/server/src/routes/pairing.rs crates/server/src/live/relay_client.rs .env.example
git commit -m "feat: QR points to m.claudeview.ai, default RELAY_URL to managed relay"
```

---

### Task 13: Final redeploy and E2E test

**Why:** All code changes are done. Deploy relay, verify full flow end-to-end.

**Step 1: Redeploy relay**

```bash
fly deploy --config crates/relay/fly.toml
```

**Step 2: Set Fly.io secrets**

```bash
fly secrets set SUPABASE_JWT_SECRET=your-jwt-secret --config crates/relay/fly.toml
```

**Step 3: Verify relay on custom domain**

```bash
curl https://relay.claudeview.ai/health
```

Expected: `ok`

**Step 4: Full E2E test**

1. `bun run dev` on Mac (unset `RELAY_URL` to use default managed relay, or keep `.env` pointing to `relay.claudeview.ai`)
2. Check Mac logs: `relay_client: connected to relay` and `auth_ok received`
3. Open `localhost:5173` → click phone icon → QR code appears
4. **QR encodes `https://m.claudeview.ai?k=...&t=...`** (not relay URL)
5. Scan QR with phone
6. Phone opens `m.claudeview.ai`
7. **Auth screen appears** → sign in with Google or email
8. After auth: pairing auto-starts
9. Mac logs: `paired device stored in Keychain, device_id=phone-XXXXXXXX`
10. Mac reconnects, starts forwarding sessions
11. Start a Claude Code session on Mac
12. **Phone shows session card in real-time**
13. Stop Claude session → phone session disappears or shows "completed"

**Step 5: Test re-open flow**

1. Close phone browser completely
2. Re-open `m.claudeview.ai`
3. Should be already signed in (Supabase session persisted)
4. Should auto-reconnect to relay
5. Sessions appear immediately

**Step 6: Test "Add to Home Screen" (PWA)**

1. On phone, tap Share → Add to Home Screen
2. Open from home screen
3. Should work like an app (no browser chrome)
4. Sessions visible

---

## Task Dependency Graph

```
Task 1 (relay ClaimRequest) ──┐
                               ├── Task 4 (pair_complete handler) ── Task 5 (deploy relay)
Task 2 (phone sends pubkey) ──┘                                          │
                                                                          │
Task 3 (always-connect) ─────────────────────────────────────────────── Task 6 (local E2E)
                                                                          │
Task 7 (Supabase setup) ─── Task 8 (auth gate) ──┐                       │
                                                    ├── Task 9 (JWT)      │
Task 10 (DNS domains) ────────────────────────────┘      │               │
                                                          │               │
Task 11 (Cloudflare Pages) ──────────────────────────────┘               │
                                                          │               │
Task 12 (QR URL + default relay) ────────────────────────┘               │
                                                                          │
Task 13 (final deploy + E2E) ◄───────────────────────────────────────────┘
```

**Parallelizable:** Tasks 1+2+3 can be done simultaneously. Tasks 7+10 (manual infra setup) can be done in parallel with Phase A code work.

---

## Files Changed Summary

| File | Action | Task |
|------|--------|------|
| `crates/relay/src/pairing.rs` | Modify | 1, 9 |
| `crates/relay/static/mobile.html` | Modify | 2 |
| `src/pages/MobilePairingPage.tsx` | Modify | 2, 8 |
| `crates/server/src/live/relay_client.rs` | Modify | 3, 4, 12 |
| `crates/relay/Cargo.toml` | Modify | 9 |
| `crates/relay/src/jwt.rs` | Create | 9 |
| `crates/relay/src/lib.rs` | Modify | 9 |
| `crates/relay/src/state.rs` | Modify | 9 |
| `crates/relay/src/ws.rs` | Modify | 9 |
| `src/lib/supabase.ts` | Create | 8 |
| `src/components/mobile/MobileAuthGate.tsx` | Create | 8 |
| `src/pages/MobileMonitorPage.tsx` | Modify | 8 |
| `src/hooks/use-mobile-relay.ts` | Modify | 9 |
| `vite.config.mobile.ts` | Create | 11 |
| `mobile-index.html` | Create | 11 |
| `src/mobile-main.tsx` | Create | 11 |
| `crates/server/src/routes/pairing.rs` | Modify | 12 |
| `.env.example` | Modify | 12 |
| `package.json` | Modify | 8, 11 |
