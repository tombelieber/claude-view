# Production Hardening Design — Share Worker + Relay

**Date:** 2026-02-28
**Status:** Approved
**Surfaces:** Cloudflare Share Worker + Fly.io Relay
**Domains:** claudeview.ai, claudeview.com

## Problem

Two internet-facing surfaces run on the operator's (paid) infrastructure with no user identity layer, no rate limiting, and wide-open CORS. An attacker or botnet can exhaust R2 storage, flood the relay with fake pairing offers, or DDOS either surface with zero friction.

Reference standard: `taipofire-donations` — enterprise-grade hardening on a free-tier stack.

## Decisions Made

| Concern | Decision | Rationale |
|---------|----------|-----------|
| E2E encryption for shares | Required | Trust model: "your data stays yours." Relay is zero-knowledge; shares must be too. |
| Crypto scheme | AES-256-GCM | Web Crypto API native in browsers; `aes-gcm` crate audited in Rust; hardware-accelerated. |
| Key delivery | URL fragment (`#k=...`) | Fragments never sent to server. Bitwarden Send / Excalidraw pattern. Proven at scale. |
| Auth provider | Supabase Auth | 50K MAU free, official Expo SDK, self-hostable, already used in taipofire. One JWT across all surfaces. |
| Title in D1 | Plaintext (not encrypted) | User-chosen metadata, not conversation content. Same as Bitwarden Send showing item name. |
| Rate limiting storage | D1 counter table | Free, self-contained, same pattern as taipofire `check_rate_limit`. |
| Observability | Sentry + PostHog | Sentry for errors (5K/mo free), PostHog for product analytics (1M events/mo free). |
| Hardening approach | Approach A: Hardened from day 1 | No security debt. Open-source credibility requires trust model intact at launch. |
| Monetization lever | Active share count (future) | Not rate limits — those are abuse prevention only, not product features. |

## Architecture

### Share Flow

```
Mac (Rust server + Supabase JWT)
├── Read JSONL → serialize to JSON → gzip → AES-256-GCM encrypt (random 256-bit key)
├── POST /api/share          [JWT + HMAC sig]
│     Worker: verify JWT → rate limit per user_id → create token → D1 row (status: pending)
│     PostHog: share_created event
├── PUT /api/share/:token/blob  [encrypted bytes ≤ 50MB]
│     Worker: verify token pending → store to R2 → mark D1 row ready
└── Returns: https://share.claudeview.ai/s/{token}#k={base64url(aes_key)}
                                                    ↑
                                             never hits server

Recipient browser
├── Loads viewer SPA (Cloudflare Pages, static)
├── Extracts AES key from window.location.hash — never leaves browser
├── GET /api/share/:token   [no auth — token = credential]
│     Worker: fetch from R2 → return encrypted blob
│     PostHog: share_viewed event
├── gunzip → AES-256-GCM decrypt (Web Crypto API)
└── Render conversation (read-only)
```

### Relay Flow (hardened)

```
Phone (Expo + Supabase JWT)
├── POST /pair/claim  [JWT in Authorization header]
│     Relay: verify JWT → rate limit per user_id → consume token
└── GET /ws           [JWT in ?token= query param]
      Relay: verify JWT → rate limit per user_id → enforce connection limits
      Max: 3 connections per device_id, 1000 global
```

### JWT Validation (same standard across all surfaces)

```
Supabase issues RS256 JWT:
  { sub: "user-uuid", email: "...", exp: now+3600, iss: "https://{project}.supabase.co/auth/v1" }

Rust (jsonwebtoken crate):
  Fetch JWKS from Supabase once at startup → cache in AppState
  Validate RS256 signature + exp + iss on every authenticated request

Cloudflare Worker (jose library):
  Fetch JWKS from Supabase (cached via CF Cache API)
  Validate RS256 signature + exp + iss on every mutation endpoint

Expo (React Native):
  @supabase/supabase-js — official SDK, Expo Secure Store integration
  supabase.auth.signInWithOtp({ email }) — magic link
  supabase.auth.getSession() → JWT attached to all requests
```

## Hardening Spec — Share Worker (Cloudflare)

### Auth

| Endpoint | Auth required | Method |
|----------|--------------|--------|
| `POST /api/share` | Yes | Supabase JWT (RS256) |
| `PUT /api/share/:token/blob` | No — token-is-auth | Token must exist in D1 as `pending` |
| `GET /api/share/:token` | No — public | Token is the credential (131-bit entropy) |
| `DELETE /api/share/:token` | Yes | Supabase JWT — user_id must match D1 row |
| `GET /api/shares` | Yes | Supabase JWT — returns only caller's shares |

### Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `POST /api/share` | 10/hour | per user_id |
| `PUT /api/share/:token/blob` | 10/hour | per user_id (shared budget with create) |
| `GET /api/share/:token` | 60/min | per IP |
| `DELETE /api/share/:token` | 20/hour | per user_id |
| `GET /api/shares` | 30/min | per user_id |

**Implementation:** D1 `rate_limits` table with sliding window counter. Same pattern as taipofire `check_rate_limit` Postgres RPC.

```sql
CREATE TABLE rate_limits (
  key       TEXT NOT NULL,   -- "{user_id}:{endpoint}"
  window    INTEGER NOT NULL, -- unix timestamp floored to window size
  count     INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (key, window)
);
```

### Size Limits

- Blob upload: reject if `Content-Length > 52_428_800` (50MB) → 413
- JSON bodies: reject if > 1KB → 413
- Basis: largest real session = 143MB raw → 28.7MB gzipped. 50MB headroom is sufficient.

### CORS

```typescript
const ALLOWED_ORIGINS = [
  'https://share.claudeview.ai',
  'https://claudeview.ai',
  'https://share.claudeview.com',
  'https://claudeview.com',
];
// GET /api/share/:token → allow '*' (public blob reads, works in Slack previews)
// All other endpoints → restrict to ALLOWED_ORIGINS
// Dev: allow http://localhost:* in non-production
```

### Cleanup Cron

Cloudflare Cron Trigger (free, runs in Worker):
- Every hour: `DELETE FROM shares WHERE status = 'pending' AND created_at < now() - 3600`
- For each deleted row: `R2.delete(token)` if blob exists
- Prevents abandoned uploads (user started share, upload failed) from lingering in D1

### D1 Schema

```sql
CREATE TABLE shares (
  token       TEXT PRIMARY KEY,           -- 22-char base62
  user_id     TEXT NOT NULL,              -- Supabase user UUID
  session_id  TEXT NOT NULL,
  title       TEXT,                       -- plaintext (user-chosen metadata)
  size_bytes  INTEGER,                    -- encrypted blob size
  status      TEXT NOT NULL DEFAULT 'pending', -- pending | ready | revoked
  created_at  INTEGER NOT NULL,           -- unix timestamp
  expires_at  INTEGER,                    -- null = no expiry (future paid feature)
  view_count  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_shares_user_id ON shares(user_id);
CREATE INDEX idx_shares_status_created ON shares(status, created_at);
```

### Observability

**Sentry (`@sentry/cloudflare`):**
- Unhandled exceptions, D1 query failures, R2 errors, slow requests (>2s)
- Auth failures logged as warnings with user_id (hashed) + endpoint

**PostHog (server-side, `fetch` to capture API):**
- `share_created` — `{ blob_size_bytes, compressed_size_bytes }`
- `share_viewed` — `{ token_hashed }` (SHA-256 of token)
- `share_revoked` — no PII
- Never include conversation content, titles, or plaintext user data

## Hardening Spec — Relay (Fly.io / Rust)

### Auth Changes

Add Supabase JWT validation to two endpoints:
- `POST /pair/claim` — `Authorization: Bearer <jwt>` header
- `GET /ws` — `?token=<jwt>` query param (browsers can't set WS headers)

JWT validation middleware: fetch Supabase JWKS at startup, cache in `RelayState`. Validate RS256 + exp + iss. Extract `user_id` from `sub` claim.

Backward compat: if no JWT present → reject with 401 (no anonymous access post-launch).

### Rate Limits (Tower middleware)

```
POST /pair       → 5 requests/min per IP
POST /pair/claim → 10 requests/min per IP
POST /push-tokens → 10 requests/min per device_id
WS messages      → 60 messages/min per connection
WS connections   → 3 per device_id, 1000 global
```

**Storage:** In-memory `DashMap<String, TokenBucket>` — relay is single-instance, no external storage needed.

### Additional Hardening

```rust
// Tower layers (added to all routes)
RequestBodyLimitLayer::new(256 * 1024)     // 256KB max body
TimeoutLayer::new(Duration::from_secs(30)) // 30s request timeout
TraceLayer::new_for_http()                 // structured request logging
```

### CORS

```rust
CorsLayer::new()
  .allow_origin([
    "https://claudeview.ai".parse().unwrap(),
    "https://claudeview.com".parse().unwrap(),
    "http://localhost:8081".parse().unwrap(), // Expo dev
  ])
  .allow_methods([GET, POST, DELETE])
  .allow_headers([CONTENT_TYPE, AUTHORIZATION])
```

### Observability

**Sentry (`sentry` + `sentry-tracing` crates):**
- Panics, auth failures (device_id + reason, no content), WS disconnects, push send failures
- Auth failure events: `{ device_id, reason, endpoint }` — no conversation data

**PostHog (via `reqwest` POST to capture API):**
- `relay_paired` — `{ device_type: "phone" | "mac" }`
- `relay_connected` — `{ device_type }`
- `relay_message_forwarded` — `{ size_bytes }` (no content)
- `push_notification_sent` — `{ agent_state }`

## E2E Encryption Spec — AES-256-GCM

### Wire Format

```
Plaintext:   JSON bytes
Step 1:      gzip compress  →  compressed bytes
Step 2:      Generate random 12-byte IV (crypto-secure RNG)
Step 3:      AES-256-GCM encrypt with random 256-bit key
             Output: IV (12 bytes) || ciphertext || GCM tag (16 bytes)
Upload:      encrypted bytes → R2
URL:         https://share.claudeview.ai/s/{token}#k={base64url(key)}
```

### Rust (Encryption)

```rust
// crates: aes-gcm, rand
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit, OsRng, rand_core::RngCore}};

let mut key_bytes = [0u8; 32];
OsRng.fill_bytes(&mut key_bytes);
let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
let cipher = Aes256Gcm::new(key);

let mut iv = [0u8; 12];
OsRng.fill_bytes(&mut iv);
let nonce = Nonce::from_slice(&iv);

let ciphertext = cipher.encrypt(nonce, compressed.as_ref())?;
// Upload: iv || ciphertext (GCM tag appended by library)
let blob = [iv.as_slice(), ciphertext.as_slice()].concat();
let key_b64 = base64url::encode(&key_bytes);
// Return: format!("https://share.claudeview.ai/s/{token}#k={key_b64}")
```

### Browser (Decryption — Web Crypto API, no library needed)

```typescript
const hash = window.location.hash.slice(1); // strip '#'
const key_b64 = new URLSearchParams(hash).get('k')!;
const keyBytes = base64urlDecode(key_b64);

const cryptoKey = await crypto.subtle.importKey(
  'raw', keyBytes, { name: 'AES-GCM' }, false, ['decrypt']
);

const blob = new Uint8Array(await fetchBlob(token));
const iv = blob.slice(0, 12);
const ciphertext = blob.slice(12);

const plaintext = await crypto.subtle.decrypt(
  { name: 'AES-GCM', iv }, cryptoKey, ciphertext
);
// gunzip → JSON.parse → render
```

## Supabase Auth Setup

**Project:** One Supabase project shared across share + relay + mobile.

**Config:**
- Enable Email (magic link) + Google OAuth
- JWT expiry: 3600s (1 hour), refresh token: 30 days
- Redirect URLs: `https://claudeview.ai/**`, `claudeview://auth` (Expo deep link)

**User table:** Use Supabase's built-in `auth.users` — no custom user table needed at this stage.

**Secrets:**
- `SUPABASE_URL` + `SUPABASE_PUBLISHABLE_KEY` → Share Worker (Cloudflare secret)
- `SUPABASE_URL` + `SUPABASE_PUBLISHABLE_KEY` → Relay (Fly.io secret)
- `SUPABASE_URL` + `SUPABASE_PUBLISHABLE_KEY` → Mac Rust server (env var)
- JWKS URL: `{SUPABASE_URL}/auth/v1/.well-known/jwks.json`

## UX — Sign-in Flow

```
First launch after update (Mac app):
  ┌─────────────────────────────────────────┐
  │  Sign in to enable sharing              │
  │  and mobile sync                        │
  │                                         │
  │  [Continue with Google]                 │
  │  [Send magic link →  email@example.com] │
  └─────────────────────────────────────────┘

  One sign-in. Same account for shares AND mobile pairing.
  JWT stored in ~/.claude-view/auth.json. Auto-refreshed.
  User never signs in again on the same machine.
```

## Updated Deliverables

| Phase | What | New vs original share plan |
|-------|------|---------------------------|
| 0 | Supabase project setup + Google OAuth config | New |
| 1 | Share Worker: JWT middleware + rate limiter + CORS + Sentry + PostHog + D1 schema + cron | +hardening throughout |
| 2 | Rust: AES-256-GCM encryption + JWT auth + share endpoints | +crypto, +auth |
| 3 | React SPA: Supabase sign-in UI + share button + hooks | +auth UI |
| 4 | Viewer SPA: Web Crypto decrypt + Sentry + PostHog | +crypto, +observability |
| 5 | Relay: JWT validation + CORS + rate limits + Tower middleware + Sentry + PostHog | New |

## Cost Model

| Scale | Share Worker | Relay | Supabase Auth | Sentry | PostHog | Total |
|-------|-------------|-------|--------------|--------|---------|-------|
| 1–100 users | $0 | $0 | $0 (50K MAU free) | $0 | $0 | **$0** |
| 1,000 users | $0 | ~$5-10 | $0 | $0 | $0 | **$5-10** |
| 10,000 users | ~$3.60 | ~$15 | $0 | $0 | $0 | **~$20** |

## Security Properties (Summary)

| Property | Share Worker | Relay |
|----------|-------------|-------|
| Operator can read conversation content | No (AES-256-GCM, key only in URL fragment) | No (NaCl box E2E, existing) |
| Unauthenticated mutations | No (Supabase JWT required) | No (JWT required post-launch) |
| Rate limited | Yes (per user_id, D1 counter) | Yes (per user_id + IP, in-memory) |
| CORS restricted | Yes | Yes |
| Request size limited | Yes (50MB blob, 1KB JSON) | Yes (256KB) |
| Error monitoring | Sentry | Sentry |
| Product analytics | PostHog (no PII) | PostHog (no PII) |
